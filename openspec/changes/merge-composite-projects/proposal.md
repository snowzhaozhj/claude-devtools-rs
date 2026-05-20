## Why

`project-discovery` 的 composite 拆分（同一 encoded 目录下不同 `cwd` 的 session 被拆为多个虚拟 project，id 形如 `{baseDir}::{hash8}`）对用户而言**反直觉且碎片化**：同一仓库的多个 git worktree 或同一 monorepo 内的子目录会在 sidebar 上呈现为多个看似独立的 project，用户必须来回切换才能看全活动。

该机制还在 `cdt-api::LocalDataApi::get_session_detail` 上引发了**全量扫描性能问题**：本地分支为了让 `SubprojectRegistry` 能反解 composite id，每次打开会话详情都触发 `ProjectScanner::scan()` 全扫 `~/.claude/projects/` 下所有 project 的所有 jsonl（含读前 20 行 header 提取 cwd），仅为了从结果里拿到目标 session 的 `last_modified` 与 `size` 两个字段（`crates/cdt-api/src/ipc/local.rs:1373`）。同一编码目录下 cwd 不同的情况几乎只在 dev 场景（`EnterWorktree`、worktree 工作流）触发，但产生的扫描开销由所有用户承担。

更早的 `RepositoryGroup` / `WorktreeGrouper` 已经在 git identity 层完成了"多 worktree 归并为一个 repo"的聚合，composite 拆分在产品层已被覆盖且与之冲突。

## What Changes

- **BREAKING**：`project-discovery` 不再拆分 composite project；同一 encoded 目录下所有 session 始终归属同一 `Project`（`id = encoded base_dir`，无 `::` 后缀）。
- `SubprojectRegistry` 整体废弃：`crates/cdt-discover/src/subproject_registry.rs` 删除；`ProjectScanner` 不再按 cwd 分桶、不再返回 `PendingRegistration`、不再持有 registry 状态。
- `Project.path` 退化为 encoded 目录解码出的 base path（沿用 `decode_path` / `decode_historical_worktree_dir` 既有路径）；不再代表"该 cwd bucket 的展示 cwd"。
- `Session`（`cdt-core::Session`）新增 `cwd: Option<String>` 字段：jsonl 内首条带 `cwd` 字段的消息提取；为空时 `None`（向下兼容 historical 数据）。前端通过该字段在 session 列表行 + 详情头展示 cwd badge。
- `LocalDataApi::get_session_detail` 本地分支**移除 `scanner.scan()` 调用**：直接 `tokio::fs::metadata(jsonl_path)` 取 `last_modified` / `size`，与现有 SSH 分支语义对称。
- `LocalDataApi::list_sessions` 不再调 `scanner.scan()` 仅为填充 registry：composite filter 删除后，只读目标 project 目录即可（与 SSH 分支统一）。
- `list_repository_groups` 行为契约不变（仍按 git identity 聚合多 worktree project），但其调用的 `ProjectScanner::scan()` 内部不再做 cwd 分桶。
- **配置迁移**：`configuration-management` 的 `pinned_sessions: HashMap<String, Vec<PinnedSession>>`（key 为 project_id，可能为 composite 形式）启动时按 `{baseDir}::{hash}` → `{baseDir}` 一次性 fold 迁移；同 base_dir 合并多个 entry 时去重保留 `pinned_at` 最早的条目。`notification-triggers` 的 `NotificationTrigger.repository_ids` 存的是 `RepositoryGroup.id`（git-common-dir 绝对路径），与 composite id 无关，**不需要迁移**；本 change 加 spec scenario 防回归。
- UI：`ui/src/lib/components/sidebar/` 项目列表不再出现 `::` id；`ui/src/lib/components/session-list/` 行尾加 cwd `Badge`（已有 Badge 组件）；session 详情头部加 cwd 显示。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `project-discovery`：删除 `Represent split subprojects with a stable composite identifier` Requirement 及其 4 个 Scenario；删除"Windows 大小写归一" Requirement 里对 `SubprojectRegistry` 的引用（保留 `ProjectPathResolver` 部分）；新增 Requirement"Session 暴露 cwd 字段供消费方按需展示"；明确 `ProjectPathResolver::resolve` 签名移除 `registry` 参数（不再走 composite short-circuit，直接走 cache / hint / session-jsonl-cwd 路径）。
- `ipc-data-api`：`Session` 序列化新增 `cwd?: string` 字段（camelCase）；`get_session_detail` 行为 Requirement 增补"本地分支以单文件 stat 取 `lastModified` / `size`，禁止触发跨 project 全量扫描"约束（修当前 implicit 行为）；新增"contract test 层用 spy FileSystemProvider 断言不触发跨 project 读取"测试要求，避免 perf bench 被 `#[ignore]` 跳过后无 CI 保护。
- `configuration-management`：新增 Requirement"Migrate composite project IDs in pinned sessions on load" —— `ConfigManager::load` 时检测 `pinned_sessions` key 含 `::` 的条目，按 `{baseDir}::{hash}` → `{baseDir}` fold 合并；fold 触发时写回前备份配置文件到 `<path>.pre-merge-composite.bak`；写回失败 warn 不阻塞、下次启动重试。
- `agent-configs`：`read_agent_configs` 入口由"按每个 `Project.path` 单 cwd 扫 `.claude/agents/`"改为"按该 project 下所有 session 的 `cwd` 去重集合扫所有 cwd"，避免合并后丢失非代表 cwd 的 agent 配置。

## Impact

- **代码**：
  - 删除 `crates/cdt-discover/src/subproject_registry.rs`（约 165 行）
  - 改 `crates/cdt-discover/src/project_scanner.rs`：移除 `CwdBucket` 分桶、`PendingRegistration`、composite id 分支（约 80 行净减）
  - 改 `crates/cdt-discover/src/project_path_resolver.rs`：`resolve` 签名移除 `registry: &SubprojectRegistry` 参数 + 顶部 `registry.get_cwd` short-circuit 路径；单元测试同步删除/重写（约 5 处）
  - 改 `crates/cdt-discover/src/lib.rs`：移除 `SubprojectRegistry` / `COMPOSITE_SEPARATOR` 导出
  - 改 `crates/cdt-discover/src/worktree_grouper.rs`：测试里 `resolver.resolve_all(...)` 调用 + `resolve` 调用点的 registry 参数全部删除
  - 改 `crates/cdt-discover/src/agent_configs.rs` + `crates/cdt-api/src/ipc/local.rs::read_agent_configs`：入口构造 pairs 时改为按 `(project_id, distinct_cwds_from_sessions)` 笛卡尔展开（同 project 多 cwd 时产多条 pair）
  - 改 `crates/cdt-api/src/ipc/local.rs`：`get_session_detail`（local.rs:1364-1407）+ `list_sessions`（local.rs:760+）+ `read_agent_configs`（local.rs:2491）+ 其它 `SubprojectRegistry::is_composite` 调用点（~5 处）简化
  - 改 `crates/cdt-core/src/session.rs` 或对应 `Session` struct：加 `cwd: Option<String>`
  - 改 `crates/cdt-parse/`：扫描 jsonl 时提取 cwd 字段塞进 `Session`（与现有 `extract_session_cwd` 路径合并）
  - 改 `crates/cdt-config/`：启动时迁移 `pinned_sessions` key 中含 `::` 的条目；trigger `repository_ids` 不动
  - 改 UI：`Sidebar.svelte` / `SessionList.svelte` / `SessionDetail.svelte` 加 cwd badge
- **APIs**：
  - Tauri command 返回的 `Session` JSON 新增 `cwd` 字段（**非 breaking**：旧前端忽略新字段）
  - Tauri command 返回的 `Project.id` 不再含 `::`（**breaking for stored client state**：迁移由后端启动时一次性 fold 完成）
- **性能**：
  - `get_session_detail` locate 阶段：基线几百 ms → < 5 ms（依用户 project 数量）
  - `list_sessions` cache miss 路径：scan() 全扫开销消失，缩到目标 project 目录读
- **测试**：
  - 删除 `crates/cdt-discover/src/subproject_registry.rs::tests` + `crates/cdt-discover/tests/project_scanner.rs` 中 composite 相关用例（约 6 个）
  - 新增 `cdt-api/tests/ipc_contract.rs` 中 `Session.cwd` 字段 round-trip 测试
  - 新增 `cdt-api/tests/perf_get_session_detail.rs` baseline 断言（locate 阶段 < 5ms）
  - 新增 vitest / Playwright 验证 sidebar 不再出现 `::` 项 + cwd badge 可见
- **依赖**：移除 `sha2` 仅在 `SubprojectRegistry::compose_id` 使用的情况下可降级（需 grep 确认；其它路径若用到则保留）
- **followups.md**：清理或归档 composite 相关 followups（若有）
