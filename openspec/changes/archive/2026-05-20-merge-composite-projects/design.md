## Context

`project-discovery` 在端口初期沿用 TS 原版"同一 encoded 目录下不同 cwd 即拆分为多个虚拟 project"的设计。随后引入 `WorktreeGrouper` / `RepositoryGroup`（spec：`project-discovery::Group projects by git repository identity`），在 git identity 层把 `/repo` 与 `/repo/.claude/worktrees/feat-x` 这类**不同 encoded 目录**的 worktree 归并为同一 `RepositoryGroup`。两层归并在产品层产生冗余：

- **RepositoryGroup**：跨多个 encoded 目录按 git repo 聚合 —— 解决 worktree 不同 path 的归并问题。
- **composite split**：单个 encoded 目录内按 cwd 拆分 —— 解决"同一编码目录下进程曾在不同 cwd 启动"的拆分问题。

后者实际触发场景非常窄：
1. dev 用 `EnterWorktree` 切 cwd 后写入了 jsonl（同一 encoded 目录内出现多个 cwd）。
2. 极少数 Claude Code 自身行为改变 process cwd 的情况。

但成本由所有用户承担：`LocalDataApi::get_session_detail`、`list_sessions`、其它 `is_composite` 调用点都为支持反解 composite id 走 `ProjectScanner::scan()` 全扫，开销 ~14K file reads / IPC（27 project × 534 sessions × 多次读 + cwd 提取）。

`get_session_detail` 仅为拿到目标 session 的 `last_modified` + `size` 两个字段付出全扫，**与 SSH 分支不对称**（SSH 走轻量 `list_sessions(project_id)`）。

## Goals / Non-Goals

**Goals:**

- 同一 encoded 目录下所有 session 始终聚合在一个 `Project`，sidebar 视图统一。
- 移除 `SubprojectRegistry` / composite id / cwd 分桶相关全部代码与 spec 契约。
- `get_session_detail` 本地分支 locate 阶段从几百 ms 降到 < 5 ms（单文件 stat）。
- `Session` 暴露 `cwd: Option<String>` 字段，让前端在 sidebar 项目项 + session 列表行 + session 详情头展示 cwd badge / 路径，区分同一 project 内不同 cwd 的 session。
- 启动时一次性迁移配置（`pinned_sessions` / `notification_triggers`）里残留的 composite id，无需用户感知。
- `RepositoryGroup` / `WorktreeGrouper` 行为不变；`Project.path` 退化为 encoded 目录解码值（与 historical worktree path 解析路径共用）。

**Non-Goals:**

- 不引入"按 cwd 折叠分组"的 UI 模式（折叠/展开切换、二级目录树）—— 直接平铺 + badge 已够用，未来如需再走单独 change。
- 不在本 change 内重做 `RepositoryGroup` 的 worktree 排序 / 分组算法。
- 不为 composite id 提供"opt-in 拆分"开关（CLAUDE 用户已明确："彻底合并、不留 opt-in"）。
- 不动远程 SSH 路径的逻辑（SSH 一直走 `list_sessions(project_id)` 路径，与本 change 后行为天然一致）。
- 不优化 `list_repository_groups` 的 scanner cache 跨 IPC 复用（属于另一性能维度，留待 follow-up）。

## Decisions

### D1：`SubprojectRegistry` 整体删除而非保留为 no-op

**选择**：直接删除 `crates/cdt-discover/src/subproject_registry.rs` 文件 + `pub use` 导出 + 所有调用点。

**替代方案**：保留类型 + 让 `is_composite` 永远返回 `false`、`get_session_filter` 永远返回 `None`，作为代码兼容层。

**理由**：
- 删除符合 CLAUDE.md "Don't add features, refactor, or introduce abstractions beyond what the task requires" 与 "No half-finished implementations"。
- 留兼容层会让未来读者疑惑 "registry 是不是该用"，反而增加心智负担。
- 配置迁移是**数据层**问题（一次性 fold），不需要类型层做兼容。

**全部调用点清单**（grep 校验后）：
- `crates/cdt-api/src/ipc/local.rs:766`（`list_sessions` `is_composite` 分支）
- `crates/cdt-api/src/ipc/local.rs:1366`（`get_session_detail` `is_composite` 分支）
- `crates/cdt-discover/src/project_path_resolver.rs:25`（`use SubprojectRegistry`）
- `crates/cdt-discover/src/project_path_resolver.rs:52-63`（`resolve(... registry: &SubprojectRegistry ...)` 签名 + `registry.get_cwd(project_id)` short-circuit + 测试中所有 `SubprojectRegistry::new()` 构造、`registry.register(...)` 注册）
- `crates/cdt-discover/src/project_scanner.rs`（`self.registry` 成员、`scan_with_name` 返回 `PendingRegistration`、`scan_project_dir` 内分桶逻辑、`registry()` getter、tests）
- `crates/cdt-discover/src/worktree_grouper.rs`（测试中 `resolver.resolve_all(&xxx)` 间接调用 `resolve`，签名修改后同步去掉 registry 实参）
- `crates/cdt-discover/src/path_decoder.rs`（`COMPOSITE_SEPARATOR` 常量 + `find(COMPOSITE_SEPARATOR)` 分支）
- `crates/cdt-discover/tests/project_scanner.rs::composite_id_is_deterministic_across_registries` + 其它 composite 测试
- `crates/cdt-discover/src/lib.rs`（`pub use ... SubprojectRegistry, COMPOSITE_SEPARATOR`）

**影响**：跨 crate 接口的 `pub use` 导出需要同步删除；`ProjectPathResolver::resolve` 签名变为 `resolve(&self, project_id, hint, session_paths) -> Result<PathBuf, _>`（少一个参数），所有调用点需同步修改；下游 `cdt-api` 需 grep 全部使用点删除。

### D2：`Session.cwd` 在 `cdt-discover::ProjectScanner` 提取，不挪到 `cdt-parse`

**选择**：保留 `extract_session_cwd(path) -> Option<String>` 在 `ProjectScanner` 内（已存在），但**调用产出的 cwd 直接写入 `Session` struct**，不再喂给 `CwdBucket` 分桶逻辑。

**替代方案 A**：把 cwd 提取挪到 `cdt-parse`，让 `parse_file` 返回的 messages 头部带 cwd，scanner 只读 stat。

**替代方案 B**：让 IPC 层在 `get_session_detail` 完整 parse 后再回填 `Session.cwd`，list_sessions 不带 cwd。

**理由**：
- 当前 `extract_session_cwd` 只读前 20 行（已优化的 head-read 路径），成本固定且与一次 stat 同量级。挪到 `cdt-parse` 会让 `list_sessions` 必须做完整 parse，反而退化成本。
- `Session` 是 `cdt-core` 类型，cdt-discover 已经 own 了 session 列表的构造，挪动 cwd 提取层会破坏单向依赖。
- 替代方案 B 会让 sidebar 在"列表渲染时还没 cwd"，UI 必须二次 IPC 拿 cwd，体验不连贯。
- 复用现有 `FILE_READ_CONCURRENCY` semaphore 限流即可，不需新基础设施。

**影响**：`scan_project_dir` 在产生 `Project` 前对每个 session 调一次 `extract_session_cwd`（已经在做了）+ 把结果直接放进 `Session.cwd`；`Session` 序列化加 `cwd: Option<String>`（camelCase `cwd`）。

### D3：配置迁移策略 —— 启动时一次性 fold + 写回

**选择**：`ConfigManager` 在 load 配置时只迁移 `pinned_sessions: HashMap<String, Vec<PinnedSession>>`（key 为 project_id），按 `{baseDir}::{hash}` → `{baseDir}` fold；同 baseDir 内合并多条 entry 时按 `(session_id, pinned_at)` 去重，**保留 `pinned_at` 最早**的那条（用户最早 pin 的语义）；fold 后写回配置文件一次。

**`NotificationTrigger.repository_ids` 不迁移**：检查 `crates/cdt-config/src/types.rs:113` 与 `notification-triggers` capability 后确认，`repository_ids` 存的是 `RepositoryGroup.id`（即 git-common-dir 绝对路径，详见 `project-discovery` spec line 117 / `ipc-data-api` spec line 816+），与 composite project id 形态完全不同；composite 拆分前后 RepositoryGroup id 不变，无需迁移。spec delta 增加一个 scenario "trigger.repository_ids 不受 composite 移除影响"作为防回归保护。

**替代方案 A**：保留 composite id 在配置里，运行时按 `id.split("::")` 做 read-side 兼容。

**替代方案 B**：要求用户手动清理。

**理由**：
- A 让"composite 概念"在配置层永久残留，违反"彻底合并"的产品目标；新加的 trigger 还要决定写哪种 id 形式，反复决策。
- B 不可接受 —— 用户感知 + 数据丢失风险。
- 一次性 fold 是简单数据迁移，无 schema 变更，向前向后兼容（读老配置 fold 后等价于读新配置）。

**写盘失败 + 重启行为**：fold 后若 atomic-write 失败（磁盘满 / 权限错），warn 不阻塞启动；磁盘上配置文件仍含 composite key，下次启动 `ConfigManager::load` 命中同样的 composite key → 再次 fold → 再次尝试写盘。**`migrate_composite_ids` 实现 SHALL 是幂等的**：纯粹基于 input 的 composite 命中重写，不依赖任何"已迁移"标志位（避免 flag 与配置文件 desync）。

**备份命名**：写回前备份到 `<config-path>.pre-merge-composite.bak`；如该 `.bak` 已存在则覆盖（避免反复迁移堆积旧 backup）。与 `cdt-config` 现有的"损坏配置自动备份到 `.bak.<timestamp_ms>`"（参 `configuration-management` spec）命名上区分，避免误判已损坏。

**影响**：`crates/cdt-config/src/manager.rs` load 路径加 `migrate_composite_ids(&mut Config) -> bool`（返回是否触发了 fold）函数；触发时备份 + 写回；migration 失败不阻塞启动，warn 日志即可。新增 `configuration-management` spec delta，正式登记该 fold 行为。

### D4：`list_repository_groups` 行为不变 + scanner 内部简化

**选择**：保留 `list_repository_groups → ProjectScanner::scan() → WorktreeGrouper::group_by_repository` 调用链；scan 内部不再做 cwd 分桶，直接每个 encoded 目录产 1 个 `Project`。

**替代方案**：把 scanner 改成不再扫 cwd —— 完全跳过 `extract_session_cwd`。

**理由**：
- `Session.cwd` 需要扫到每个 session 的 cwd（D2），所以 head-read 不能砍。
- `WorktreeGrouper` 用 `Project.path` 解析 git identity，path 必须是真实磁盘路径（不是 encoded name）。原 path 来源是 `CwdBucket.cwd`（即单 cwd 时是该 cwd / 多 cwd 时取 bucket 代表）。删除分桶后，单 cwd 场景退化为 `decode_path(encoded)` / `decode_historical_worktree_dir(encoded)`（已有路径）；多 cwd 场景下"代表 cwd"的概念消失 —— 取**第一条 session（按 mtime 最新）的 cwd**作为 `Project.path`，否则退化为 decoded。这是一致性折中，保证 git identity 解析仍有真实 path 喂入。
- `WorktreeGrouper` 现有的 "historical worktree 归入父 repo" Scenario（spec line 249）继续生效。

**影响**：`Project.path` 在"多 cwd 编码目录"场景下取最新 session 的 cwd 而非 decoded path；这与现状（取 bucket 代表）等价 —— 历史 cwd 不同 session 总是少数。`list_repository_groups` Scenario 与 Group 排序契约不变。

### D5：Windows 大小写归一化 —— 删 `SubprojectRegistry` 部分，保留 `ProjectPathResolver` 部分

**选择**：现有 `project-discovery::Compare paths case-insensitively on Windows` Requirement 同时约束 `ProjectPathResolver`（cache key）和 `SubprojectRegistry`（compose_id 输入归一）。本 change 删除 `SubprojectRegistry` 部分；`ProjectPathResolver` 部分保留（它是 base_dir → 真实 path 的解析层，与 composite 无关）。

**替代方案**：把 Windows 大小写归一整个 Requirement 一并删除。

**理由**：`ProjectPathResolver` 在 Windows 平台仍需归一（避免 `C:\users\foo` 与 `c:\Users\Foo` 解析为不同 project），是独立于 composite 的正交需求。Scenario 中提到 `SubprojectRegistry` 的部分改为只覆盖 `ProjectPathResolver`。

### D6：`Project.path` 序列化与前端展示

**选择**：`Project.path` JSON 字段保持 `path: String`（camelCase `path`），值在"该 encoded 目录最新 session cwd 存在"时为该 cwd，否则为 decoded 编码目录。

**替代方案**：删除 `Project.path`，让前端只看 session 列表的 cwd badge。

**理由**：前端 sidebar / Topbar 当前依赖 `Project.path` 显示 project 路径；删除会破坏多处 UI。保留 path 字段、用最新 cwd 当代表是与现状对齐的最小改动。多 cwd 编码目录下 path 不能完全代表所有 session 的 cwd —— 通过 session 列表的 cwd badge 暴露差异。

### D7：性能验证基线断言 + 行为级 contract test

**选择**：双层验证。

**Layer A（CI 不跑，本地手动）**：在 `crates/cdt-api/tests/perf_get_session_detail.rs` 加 `#[ignore]` 标记的 `assert_locate_under_threshold` 断言（locate 阶段 < 5ms，threshold 可调）；保留段位测量探针。

**Layer B（CI 跑，硬保护）**：在 `crates/cdt-api/tests/ipc_contract.rs` 加 contract test：用临时 projects 目录（`tempdir` 下手工铺 3 个 project × 2 session 的 fixture）+ spy `FileSystemProvider`（在 trait wrapper 里记录 `read_dir` / `read_lines_head` / `read_to_string` / `stat` 各方法被调次数 + 路径列表）。调 `get_session_detail(project_A, session_1)` → 断言：
- `read_dir` 调用次数 == 0（不列任何目录）
- `read_lines_head` 调用路径集合 ⊆ {target jsonl}（解析 jsonl 内容会调）
- `stat` 调用路径集合 ⊆ {target jsonl}
- spy 记录的 `read_dir` 路径列表 SHALL NOT 含其它 project 目录

**替代方案**：仅靠 Layer A 的 perf bench 验收。

**理由**：bench 在 CI runner 无真实 `~/.claude/projects/` corpus 时被早 return 跳过（参 `.claude/rules/perf.md`），无 CI 保护。Layer B 用 spy fs + 行为断言取代 wall time 断言 —— **不全扫**是行为契约，能被 mock 验证；时间数字依赖硬件易抖。两层互补：Layer A 抓回归后的真实数字，Layer B 抓代码层的全扫调用复发。

### D8：`agent-configs` scan 入口按 Session.cwd 去重列表展开

**选择**：`LocalDataApi::read_agent_configs` 构造 pairs 时，对每个 `Project` 收集其 sessions 列表里所有非空 `cwd` 的去重集合（`BTreeSet<PathBuf>`），笛卡尔展开为 `Vec<(project_id, cwd_string)>` 喂给 `cdt_discover::agent_configs::read_agent_configs`。原"每个 project 一条 pair"退化为"每个 (project, distinct cwd) 一条 pair"。

**替代方案 A**：保留每 project 一条 pair，cwd 取 `Project.path`（即"最新 mtime session cwd"代表）。

**替代方案 B**：把 agent-configs 扫入口移到 `cdt-discover` 内部，让 scanner 在产 Project 时就构造好。

**理由**：
- A 在多 cwd 场景下漏扫非代表 cwd 的 `.claude/agents/` —— dev 在多 worktree 配了不同 agent 颜色会丢失。
- B 改动大且 scanner 不应 own agent-configs 概念（cdt-discover 的职责边界）。
- 选定方案改动量小、行为契约清晰、可观测（IPC 返回 AgentConfig 列表会显式包含每个 cwd 下的条目）。
- 同 cwd 在不同 session 间重复时 `BTreeSet` 自动去重；不同 cwd 下同名 agent 文件 `cdt_discover::agent_configs::read_agent_configs` 已按 `(scope_global_first, name)` 排序与去重（沿用既有去重逻辑），无新冲突。

**影响**：`agent-configs` capability 加 spec delta，明确"扫描覆盖一个 project 下所有 session cwd"。`crates/cdt-api/src/ipc/local.rs:2491-2512` 的 `read_agent_configs` 入口构造 pairs 部分改写；`cdt-discover/src/agent_configs.rs` 公开签名不变（仍接 `(project_id, cwd)` pairs）。

## Risks / Trade-offs

- **[Risk] 同 encoded 目录、不同 cwd 的 session 现在混在同一 project，用户区分靠 cwd badge** → Mitigation: session 列表每行展示 cwd 末段（如 `feat-x` / `packages/a`），详情头展示完整 cwd；UI 排序仍按 mtime 倒序，最近活动在前，用户主要消费 mtime 不依赖 cwd 分桶。
- **[Risk] `Project.path` 在多 cwd 场景代表性不足** → Mitigation: D6 决议中用最新 session cwd 作为代表，与拆分前的"bucket 代表"等价；前端不依赖 path 做唯一性，唯一性已在 `Project.id`（encoded 目录名）。
- **[Risk] 配置迁移失败导致 trigger / pin 丢失** → Mitigation: fold 前备份配置到 `<file>.pre-merge-composite.bak`；fold 后写回失败 warn 不阻塞；下一次启动重试。
- **[Risk] 同 encoded 目录下大量不同 cwd 的 session 让 sidebar 列表变长** → Mitigation: 现状下这种情况会拆成 N 个 sidebar 项，总条数不变；新设计下 sidebar 项变少（一个 base_dir 一项），session 列表行数同；用户感知反而更聚合。
- **[Trade-off] 移除 composite 后 monorepo 用户失去"按子目录分类"能力** → Mitigation: 用户可在 sidebar 项目内通过 cwd badge 区分；后续如有反馈再加"按 cwd 折叠分组"UI（不需改后端）。
- **[Risk] 历史用户 launch 时第一次 fold 需要瞬时 I/O** → Mitigation: fold 一次性、配置文件量级 KB，耗时 < 10ms 不感知。

## Migration Plan

1. **代码变更**：先做 D1（删 `SubprojectRegistry`）+ D2（`Session.cwd`）+ D4（`scan_project_dir` 不分桶）+ `LocalDataApi` 调用点简化。
2. **配置迁移**：`ConfigManager::load` 路径加 `migrate_composite_ids()`：扫 `pinned_sessions` + 各 trigger 的 `project_id`，含 `::` 的按 split fold；fold 前备份；写回。
3. **UI 改动**：sidebar 不感知（id 形式变化是非破坏性）；session 列表行加 cwd badge；详情头加 cwd 展示。
4. **测试**：删 composite 相关测试；加 `Session.cwd` 字段 ipc_contract test；加 `Project.id` 不含 `::` 的 round-trip test；加配置迁移 fold 测试；vitest / Playwright 覆盖 cwd badge 可见。
5. **bench**：跑 `cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture` 记录前后 locate 阶段数字，PR 描述 Perf impact 段落填四维数据。
6. **回滚策略**：本 change 引入的字段（`Session.cwd`）非破坏；删除的代码（`SubprojectRegistry`）若需回滚可从 git history 还原；配置 fold 通过 `.bak` 文件可手动还原。

## Open Questions

- **Q1**：是否需要保留 `sha2` 依赖？需在 apply 前 grep `crates/cdt-discover` 下其它 `sha2` 用法，若仅 `compose_id` 用则降级。
- **Q2**：composite id 的 backward-compat URL 处理？现状下没有 deep-link 形式用 composite id（前端只在内存里持有），所以 fold 即可。**预设结论**：不需要 redirect 层；apply 前 grep 前端是否有 hard-code composite id 的地方再确认。
- **Q3**：`list_repository_groups` 内 `ProjectScanner::scan()` 仍是全扫吗？是 —— 但只读目录元数据 + 每 session head 20 行，**不**做 cwd 分桶，性能与现状 list path 等量级；本 change 不优化它。后续若要进一步降本，走单独 perf change。
