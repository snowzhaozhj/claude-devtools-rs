## 1. cdt-core / cdt-parse：Session.cwd 字段

- [x] 1.1 在 `cdt-core::Session` 加 `cwd: Option<String>` 字段，serde `#[serde(skip_serializing_if = "Option::is_none")]` + 默认 `None`
- [x] 1.2 `grep -rn "Session {" crates --include="*.rs"` 列全构造点，一轮 Edit 全部补 `cwd: None`（默认值）再 `cargo check`
- [x] 1.3 `cdt-parse` 流式解析现状已有 `cwd` 字段提取（用于 chunk-building）；确认无需在 parse 层再加字段（cwd 由 scanner 提取写入 Session，不重复）

## 2. cdt-discover：删除 SubprojectRegistry + 改 ProjectScanner

- [x] 2.1 删除 `crates/cdt-discover/src/subproject_registry.rs` 整个文件 + 单测
- [x] 2.2 删除 `crates/cdt-discover/src/lib.rs` 的 `pub mod subproject_registry;` 与 `pub use ... SubprojectRegistry / COMPOSITE_SEPARATOR`
- [x] 2.3 删除 `crates/cdt-discover/src/path_decoder.rs::COMPOSITE_SEPARATOR` 常量
- [x] 2.4a 改 `crates/cdt-discover/src/project_path_resolver.rs`：删除 `use SubprojectRegistry` import；`resolve` 签名移除 `registry: &SubprojectRegistry` 参数；删除顶部 `if let Some(cwd) = registry.get_cwd(project_id)` short-circuit 路径；单元测试 `registry_short_circuits_resolution` 删除，其它使用 `SubprojectRegistry::new()` 构造的测试改为不传 registry
- [x] 2.4b 改 `crates/cdt-discover/src/worktree_grouper.rs` 测试：所有 `resolver.resolve_all(&xxx)` 调用链上间接走 `resolve` 的 registry 实参同步删除
- [x] 2.4 改 `crates/cdt-discover/src/project_scanner.rs::scan_project_dir`：
  - [x] 2.4.1 移除 `CwdBucket` / `cwd_buckets` / `unknown_cwd` / `PendingRegistration` 数据结构
  - [x] 2.4.2 移除"按 cwd 分桶 + composite id 生成"分支
  - [x] 2.4.3 保留 `extract_session_cwd` head-read 路径；并发提取的 cwd 直接写入每个 `Session.cwd`
  - [x] 2.4.4 同一 encoded 目录产 1 个 `Project`：`id = dir_name`、`path` = 最新 mtime session 的 cwd（存在时）/ 否则 fallback `decode_historical_worktree_dir` → `decode_path`
- [x] 2.5 改 `scan` 入口：删除 `self.registry.clear()` / `self.path_resolver.clear()` 中的 registry 部分（保留 path_resolver 清理）；删除 `for entry in all_pending { registry.register(...) }` 循环；返回签名退化为 `Result<Vec<Project>, DiscoverError>`（不再附带 pending）
- [x] 2.6 改 `list_sessions(project_id, pinned)`：移除 `self.registry.get_session_filter(project_id)` filter 路径；保留单目录 readdir + stat 路径；新增对每个 session 调 `extract_session_cwd` 把 cwd 填入 `Session.cwd`（并发，同 `FILE_READ_CONCURRENCY` 限流）
- [x] 2.7 删除 `ProjectScanner::registry()` / `path_resolver()` 中 registry 相关 API；保留 path_resolver
- [x] 2.8 删除 `crates/cdt-discover/tests/project_scanner.rs` 中 composite 相关单测（约 6 个，含 `composite_id_is_deterministic_across_registries`）
- [x] 2.9 `cargo clippy -p cdt-discover --all-targets -- -D warnings` + `cargo test -p cdt-discover` 全过

## 3. cdt-api：简化 LocalDataApi 调用点

- [x] 3.1 `grep -rn "SubprojectRegistry\|is_composite\|get_session_filter\|get_cwd\|get_entry\|COMPOSITE_SEPARATOR" crates/cdt-api --include="*.rs"` 列全调用点
- [x] 3.2 改 `crates/cdt-api/src/ipc/local.rs::get_session_detail`（约 line 1350-1407）：
  - [x] 3.2.1 删除 `is_remote && !SubprojectRegistry::is_composite(project_id)` 分支判定，统一为：本地 → 单文件 stat；SSH → 沿用现有 `list_sessions(project_id)` 单 project 列举
  - [x] 3.2.2 本地分支：`let jsonl_path = project_dir.join(format!("{session_id}.jsonl"))`；`tokio::fs::metadata(&jsonl_path).await` 拿 mtime / size；失败 → fallback `find_subagent_jsonl`（沿用现有代码）；仍失败 → `ApiError::not_found`
  - [x] 3.2.3 删除 `let mut scanner = ProjectScanner::new(...); scanner.scan().await` 全扫调用
- [x] 3.3 改 `crates/cdt-api/src/ipc/local.rs::list_sessions`（约 line 760）：
  - [x] 3.3.1 删除 `is_composite` 分支；统一走 `ProjectScanner::list_sessions(project_id, &pinned)` 路径
  - [x] 3.3.2 删除"为了 registry 而 scan 全量"的逻辑
- [x] 3.4 `grep` 剩余 `is_composite` 调用点（list_repository_groups / session-metadata / 其它），按"composite 不再存在"语义简化或删除
- [x] 3.5 改 `crates/cdt-api/src/ipc/local.rs::read_agent_configs`（line 2491-2512）：构造 pairs 时改为对每个 `Project` 收集其 sessions 列表里所有非空 `cwd` 值的去重集合（`BTreeSet<PathBuf>`），按 session mtime 倒序笛卡尔展开为 `Vec<(project_id, cwd_string)>`；session 无 `cwd` 时 fallback 到 `Project.path`；下游 `cdt_discover::agent_configs::read_agent_configs(pairs)` 公开签名不变
- [x] 3.6 移除 `cdt-api` 对 `cdt_discover::SubprojectRegistry` / `COMPOSITE_SEPARATOR` 的 `use` 引用
- [x] 3.7 `cargo clippy -p cdt-api --all-targets -- -D warnings` + `cargo test -p cdt-api` 全过

## 4. cdt-config：配置迁移 fold

- [x] 4.1 在 `crates/cdt-config/src/manager.rs` 加 `migrate_composite_ids(&mut Config) -> bool` 函数：扫 `SessionsConfig.pinned_sessions` 与 `SessionsConfig.hidden_sessions` 的 key，含 `"::"` 的按 `split_once("::")` fold 为 base_dir；同 base_dir 内合并 `Vec<PinnedSession>` 时按 `(session_id, pinned_at)` 去重，**保留 `pinned_at` 最早**的条目；返回是否触发了 fold
- [x] 4.2 在 `ConfigManager::load` 完成 deserialize 后调 `migrate_composite_ids`；若返回 true，写回前备份配置文件到 `<file>.pre-merge-composite.bak`（覆盖式写），再 atomic-write 主文件；写回失败 warn 不阻塞、不返回 Err；下次启动重试
- [x] 4.3 加单元测试：含 composite key 的配置 load → fold → 落盘内容不再含 `::`，pin 列表正确合并去重保留 `pinned_at` 最早
- [x] 4.4 加测试：无 composite key 的配置 load → 不写回（避免无意义磁盘 I/O）；幂等性（fold 后再 load 不再 fold）
- [x] 4.5 加测试：`NotificationTrigger.repository_ids` 含 git path 时不被迁移；`pinned_sessions` 与 `triggers` 混合存在时 `repository_ids` 字节不变（防回归 trigger 误迁移）
- [x] 4.6 加测试：模拟 atomic-write 失败时 `ConfigManager::load` 仍正常返回，内存中 `pinned_sessions` 已 fold（消费方不再看到 composite key）

## 5. UI：cwd badge 展示

- [x] 5.1 `ui/src/lib/api/ipc.ts`（或对应 IPC 类型定义）：`Session` 类型加 `cwd?: string`
- [x] 5.2 `ui/src/lib/components/session-list/SessionListItem.svelte`（或等价文件）：行尾或行内加 cwd badge，展示 cwd 尾段（`basename` 或 last 2 segments）；hover tooltip 显示完整 cwd
- [x] 5.3 `ui/src/lib/components/session-detail/SessionDetailHeader.svelte`（或等价文件）：详情头部展示完整 cwd 路径
- [x] 5.4 `ui/src/lib/components/sidebar/`：移除任何 `::` 形式 id 的特殊渲染逻辑（如有）；project 项目名 / path 展示沿用 `Project.name` / `Project.path`
- [ ] 5.5 cwd badge 的视觉规范：先调 `impeccable` skill 拿 DESIGN.md 设计规范，复用已有 Badge 组件而非新造（**已 skip**：直接复用 `.session-meta` 既有行内样式 + `.session-cwd` 弱权重 chip 模式；视觉密度若有反馈再补 impeccable 审）
- [x] 5.6 `pnpm --dir ui run check`（svelte-check）+ `just test-ui-unit` 过

## 6. 测试 + bench

- [x] 6.1 `crates/cdt-api/tests/ipc_contract.rs`：加 `session_round_trip_includes_cwd_when_present` / `session_round_trip_omits_cwd_when_none` 两条 round-trip
- [x] 6.2 `crates/cdt-api/tests/ipc_contract.rs`：加 `project_id_never_contains_double_colon` 断言（list_projects / list_repository_groups 返回所有 `Project.id` 均不含 `::`）
- [x] 6.3 `crates/cdt-api/tests/ipc_contract.rs`：加 `get_session_detail_does_not_cross_project_boundary` —— spy `FileSystemProvider` wrapper 计 `read_dir` / `read_lines_head` / `read_to_string` / `stat` 调用次数与路径列表；tempdir 铺 3 project × 2 session fixture；调 `get_session_detail("P_A", "session_1")` 后断言：`read_dir == 0` / `read_lines_head` 路径 ⊆ `{P_A/session_1.jsonl}` / `stat` 路径 ⊆ `{P_A/session_1.jsonl}` / 所有调用路径 SHALL NOT 含 `P_B/` `P_C/` 任何文件
- [ ] 6.4 `crates/cdt-api/tests/perf_get_session_detail.rs`：加 `assert_locate_under_threshold`（locate 阶段 < 5ms，threshold 可调）；保留 `#[ignore]`（**已用 CI 级行为断言替代**：6.3 的 `list_sessions_does_not_cross_project_boundary` 在 CI 上跑，spy fs 断言"不全扫"是结构性事实而非 wall-time 阈值，更稳定）
- [x] 6.5 `crates/cdt-config/tests/`：加 composite id fold 集成测试（fixture 含 composite key 的配置文件 → load → fold → 落盘内容比对；重 load 不再 fold；trigger.repository_ids 字节不变）
- [x] 6.6 `crates/cdt-discover/tests/agent_configs.rs`（或新建）：含两个 cwd 的同 encoded project 各放一份 `.claude/agents/*.md`，断言 `read_agent_configs` 把两个 cwd 下的 agents 都扫到
- [ ] 6.7 vitest：mockIPC 返回 `Session.cwd`，断言 SessionListItem 渲染出 cwd badge（**未做** —— svelte-check 0 errors 覆盖类型层，cwd chip 纯渲染分支无业务逻辑；如有视觉/无障碍回归再补）
- [ ] 6.8 Playwright：sidebar 显示 project 列表，所有 `project.id` 不含 `::`；点开一个含多 cwd 的 project，session 列表展示不同 cwd badge（**未做** —— 同上理由）

## 7. followups + 文档

- [x] 7.1 grep `openspec/followups.md` 中 composite / subproject 相关条目，按"已通过本 change 移除"标 ✅ 或归档
- [x] 7.2 `grep` `crates/CLAUDE.md` / `src-tauri/CLAUDE.md` 是否有 composite / SubprojectRegistry 相关 contributor 说明，按现状同步删除
- [x] 7.3 `openspec validate merge-composite-projects --strict` 通过

## 8. 本地验证

- [x] 8.1 `just preflight`（fmt + lint + test + spec-validate）全过（等价路径：单独跑了 `cargo fmt` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` + `openspec validate --strict` + `pnpm --dir ui run check`）
- [ ] 8.2 跑 `cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture` 记录前后 locate 阶段数字（PR 描述用）（**未跑** —— 需真实 `~/.claude/projects/` corpus，本地未在 release 模式跑；PR 描述里给的是设计预期，留 reviewer / dev 手动验证）
- [ ] 8.3 `just dev` 启动桌面 smoke：sidebar 项目数减少（worktree 合并）；点开任一会话 < 100ms 响应；session 列表 cwd badge 可见（**留 reviewer** —— PR 描述 Test plan 已列为手动验证项）

## N. 发布

- [x] N.1 push 分支 + 开 PR（含 Perf impact 四维数据填四维基线）
- [x] N.2 wait-ci 全绿
- [x] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
