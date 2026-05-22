## Why

PR-D（`unify-fs-direct-calls`）落地后 `crates/cdt-api/src/ipc/local.rs` 仍残留 **6 处** `if fs.kind() == Ssh / let is_remote = ...` 业务策略分叉，每处都用 `// policy fork: PR-E lift to BackendPolicy::xxx` 注释钉了"上移欠款"。这些分叉不是算法重复（已被 PR-D 消除），而是 LocalDataApi 直接做 "Local vs SSH 行为选择"——违反 fs-abstraction H3 钉死的"业务**算法**代码 `fs.kind() == Ssh` 默认拒；业务**策略**层允许但 SHALL 只选 `BackendPolicy` 字段值"准则中的"选字段值"半。当前是"选行为"而非"选字段值"。

本 change 把这 6 处分叉上移到 `BackendPolicy` struct 字段——业务代码读 `policy.supports_memory` / `policy.stale_check_strategy` 等字段而非直接 `match fs.kind()`，让后端类型成为 fs 边界的实现细节。这是 fs-abstraction capability 设计闭环的最后一公里（PR-A 定义 BackendPolicy 雏形 → PR-D 沉淀 policy fork 注释 → 本 PR wire）。

## What Changes

### `cdt-fs::BackendPolicy` struct 扩展（primitive 字段）

在现有 4 字段（`initial_load_policy` / `max_round_trips_for_initial_page` / `supports_incremental_updates` / `prefetch_policy`）之外新增：

- `supports_memory: bool` —— Local true / SSH false（覆盖 `get_project_memory` line 2207 + `read_memory_file` line 2240）
- `supports_subagent_scan: bool` —— Local true / SSH false（覆盖 `get_session_detail` subagent 扫描 line 2316）
- `stale_check_strategy: StaleCheckStrategy` enum —— `LocalClock5min`（Local）vs `SkipUntilClockSync`（SSH，远端 mtime 跨 clock domain）（覆盖 `get_session_detail` is_ongoing stale 判定 line 2334）
- 新增 `StaleCheckStrategy` enum 定义

`BackendPolicy` 保持 `Copy + PartialEq + Eq + Debug + Clone` —— 所有新字段都是 primitive，不破现有 trait bound。

### `cdt-api` 新增 `BackendResolvers` 结构（持 trait object 与 Clone 字段）

`Arc<dyn GitIdentityResolver>` 与 `SearchConfig` 不能放进 cdt-fs（fs-abstraction spec 钉死 cdt-fs SHALL NOT 依赖 cdt-discover / cdt-core）——拆到 cdt-api 侧：

- `BackendResolvers { search_config: SearchConfig, git_identity_resolver: Arc<dyn GitIdentityResolver> }`
- `for_local()` / `for_ssh()` / `from_fs(&dyn FileSystemProvider)` 工厂方法，`Arc<Self>` 静态实例缓存（`LazyLock`）避免每次重建
- `LocalDataApi` 加 `active_fs_and_policy()` helper 返回 `(fs, projects_dir, ctx, BackendPolicy, Arc<BackendResolvers>)` 五元组（`BackendPolicy` by-value 因为是 Copy 类型；`BackendResolvers` 包 Arc 因为持 `Arc<dyn>`）

### 6 处 callsite 改造

逐行：
- line 2207 / 2240 `get_project_memory` / `read_memory_file`：`if !policy.supports_memory { early-return }`
- line 2316 `get_session_detail` subagent scan：`if policy.supports_subagent_scan { scan... } else { Vec::new() }`
- line 2334 is_ongoing stale check：`match policy.stale_check_strategy`
- line 2739 search：`SearchConfig` 直接从 `resolvers.search_config.clone()` 取
- line 3151-3168 list_repository_groups：`WorktreeGrouper::new_dyn(resolvers.git_identity_resolver.clone())`

### 不变性保护（grep + xtask）

- 加 `crates/cdt-api/tests/no_kind_compare_outside_resolvers.rs` 集成测试：grep `crates/cdt-api/src/ipc/local.rs` 内 `is_remote =` / `fs.kind() == .*::Ssh` / `fs.kind() == .*::Local` 出现次数 SHALL ≤ 阈值（仅允许 BackendPolicy / BackendResolvers 内部构造 + 顶层 `active_fs_and_policy` 派生处）
- 阈值 enum 在测试源码里硬编码 + 注释指明每处剩余出处的合理性，确保新加 fork 会破测

### NOT BREAKING

- 前端 IPC 字段无变动
- `BackendPolicy::for_local()` / `for_ssh()` / `for_http()` 现有签名保留
- `SearchConfig::from_fs_kind` / `WorktreeGrouper::new` 旧 API 保留，新增 `WorktreeGrouper::new_dyn` 并存
- LocalDataApi `new()` / `new_with_xxx()` 构造器签名不变（active_fs_and_policy 内部按需懒构造）

## Capabilities

### Modified Capabilities

- `fs-abstraction`: `BackendPolicy` 字段从 4 个扩到 7 个（+3 个 primitive 字段 + `StaleCheckStrategy` enum）；"业务代码尚未消费 BackendPolicy" Scenario 反转为"业务代码 SHALL 通过 BackendPolicy 字段选择 backend-specific 行为，禁止直接判 `fs.kind()`"。`BackendResolvers` 在 cdt-api 内（非 fs-abstraction capability 范围）不写进 spec。

### New Capabilities

无（本 change 复用现有 fs-abstraction capability）。

## Impact

- **代码**：
  - `crates/cdt-fs/src/backend_policy.rs`：加 3 字段 + 1 enum + 更新 3 个 const 构造器 + 加单测
  - `crates/cdt-api/src/ipc/`：新增 `backend_resolvers.rs` module；`local.rs` 改 6 处 callsite + 加 `active_fs_and_policy()` helper；删 inline `NoopGitIdentityResolver` 改为 `BackendResolvers` 内部静态实例（或保留位置但通过 BackendResolvers 暴露）
  - `crates/cdt-discover/src/worktree_grouper.rs`：加 `WorktreeGrouper::new_dyn(Arc<dyn GitIdentityResolver>)` 入口（thin wrapper），不动现有 generic `new`
- **测试**：`crates/cdt-fs/src/backend_policy.rs` 单测加 7 字段全覆盖；`crates/cdt-api/tests/no_kind_compare_outside_resolvers.rs` 新增 grep 不变性测试；`crates/cdt-api/tests/backend_resolvers.rs` 新增 from_fs 行为单测；现有 IPC contract / integration test 应零回归
- **依赖**：`cdt-fs` 依赖面不变（仍只有 tokio / async-trait / thiserror / tracing）；`cdt-api` 已依赖 cdt-fs / cdt-discover，新 module 无新外部 crate
- **Perf**：架构清理，预期零回归；仍跑 `bash scripts/run-perf-bench.sh --runs 5` 四维 verify
- **Spec**：fs-abstraction `BackendPolicy enum 雏形定义` Requirement MODIFIED
- **Followups**：`stale_check_strategy` 留 "SSH-aware clock skew compensation"（PR-G 决策）；`supports_memory` false 下前端 i18n 错误提示（PR-G）
