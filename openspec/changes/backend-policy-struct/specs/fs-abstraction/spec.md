## MODIFIED Requirements

### Requirement: `BackendPolicy` enum 雏形定义

系统 SHALL 在 `cdt-fs` 内定义 `BackendPolicy` struct + `InitialLoadPolicy` enum + `PrefetchPolicy` enum + `StaleCheckStrategy` enum，作为 LocalDataApi 业务路径**选择后端相关行为**的真相源。`BackendPolicy` 字段 SHALL 是 primitive（`Copy + PartialEq + Eq + Clone + Debug` derive 安全）类型，**禁止**承担 `Arc<dyn Trait>` / 非 Copy 字段——业务侧的 trait object 与 Clone 类型策略（如 `GitIdentityResolver` / `SearchConfig`）SHALL 放在更高层（如 `cdt-api::ipc::backend_resolvers`），与 `BackendPolicy` 配套使用。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendPolicy {
    pub initial_load_policy: InitialLoadPolicy,
    pub max_round_trips_for_initial_page: u8,
    pub supports_incremental_updates: bool,
    pub prefetch_policy: PrefetchPolicy,
    /// 是否支持 memory 文件读取（Local true / SSH false）。
    pub supports_memory: bool,
    /// 是否支持 subagent JSONL 扫描（Local true / SSH false）。
    pub supports_subagent_scan: bool,
    /// 5min stale 判定策略——`LocalClock5min` 用本机 mtime 比对，
    /// `SkipUntilClockSync` 跳过（远端 mtime 跨 clock domain 不可比对）。
    pub stale_check_strategy: StaleCheckStrategy,
}

pub enum InitialLoadPolicy {
    FullEager,
    SkeletonThenStream,
}

pub enum PrefetchPolicy {
    None,
    PrefetchNext,
}

pub enum StaleCheckStrategy {
    LocalClock5min,
    SkipUntilClockSync,
}
```

`PrefetchPolicy` 与 `InitialLoadPolicy` SHALL 是**正交字段**——前者表达"翻页预取策略"（不预取 vs 预取下一页），后者表达"首屏加载策略"（一次性 vs 增量补全）。**不得**把 `PrefetchNext` 塞进 `InitialLoadPolicy` 当第三 variant。

`StaleCheckStrategy` SHALL 至少含 `LocalClock5min` 与 `SkipUntilClockSync` 两个 variant；未来可扩展 `ClockSkewCompensated { offset_secs: i64 }` 等 variant。enum exhaustive match 保证调用方在加 variant 时编译期发现未处理路径。

本 capability SHALL 提供 `BackendPolicy::for_local()` / `BackendPolicy::for_ssh()` / `BackendPolicy::for_http()` 三个 const 构造器；每个 SHALL 完整覆盖 7 个字段值。

业务代码（典型如 `cdt-api::ipc::local::LocalDataApi` 的 IPC handler）SHALL 通过 `BackendPolicy` 字段读取选择 backend-specific 行为，**禁止**直接 `match fs.kind()` / `if fs.kind() == FsKind::Ssh / Local` 表达策略——`fs.kind()` 仅允许在策略**派生**点（如顶层 `active_fs_and_policy()` helper 内部、`BackendResolvers::from_fs(&fs)` 内部）使用，业务 callsite SHALL 读 policy 字段。

#### Scenario: for_local 返回 SkeletonThenStream + supports_memory true + LocalClock5min stale

- **WHEN** 调 `BackendPolicy::for_local()`
- **THEN** SHALL 返回 `initial_load_policy = SkeletonThenStream`，`max_round_trips_for_initial_page >= 2`，`supports_incremental_updates = true`，`prefetch_policy = None`
- **AND** SHALL 返回 `supports_memory = true`，`supports_subagent_scan = true`，`stale_check_strategy = StaleCheckStrategy::LocalClock5min`

#### Scenario: for_ssh 返回 FullEager + supports_memory false + SkipUntilClockSync stale

- **WHEN** 调 `BackendPolicy::for_ssh()`
- **THEN** SHALL 返回 `initial_load_policy = FullEager`，`max_round_trips_for_initial_page = 1`，`supports_incremental_updates = false`，`prefetch_policy = None`
- **AND** SHALL 返回 `supports_memory = false`，`supports_subagent_scan = false`，`stale_check_strategy = StaleCheckStrategy::SkipUntilClockSync`

#### Scenario: for_http 新增 PR-E 字段按 Local 数据源语义；initial-load 字段保持 HTTP 现状

- **WHEN** 调 `BackendPolicy::for_http()`
- **THEN** initial-load 相关字段 SHALL 保持现状（`initial_load_policy = FullEager`，`max_round_trips_for_initial_page = 1`，`supports_incremental_updates = false`，`prefetch_policy = None`）—— HTTP backend 当前 round trip 模型不变
- **AND** PR-E 新增的三字段 SHALL 按 Local 数据源语义填（`supports_memory = true`，`supports_subagent_scan = true`，`stale_check_strategy = StaleCheckStrategy::LocalClock5min`）—— HTTP server 当前把 LocalDataApi 作为数据源访问 Local `~/.claude/`，与 SSH 不共行为；未来若 HTTP server 接 SSH backend 再加分支

#### Scenario: PrefetchPolicy 与 InitialLoadPolicy 正交

- **WHEN** 显式构造 `BackendPolicy { initial_load_policy: SkeletonThenStream, prefetch_policy: PrefetchNext, .. }`
- **THEN** SHALL 编译通过且 `==` 自身（两字段独立可组合）
- **AND** SHALL NOT 出现 `InitialLoadPolicy` 含 `PrefetchNext` variant 的设计

#### Scenario: BackendPolicy 是 Copy + Eq 类型

- **WHEN** 编译 `cdt-fs`
- **THEN** `BackendPolicy` SHALL derive `Copy + Clone + PartialEq + Eq + Debug`
- **AND** 所有字段 SHALL 是 primitive 或 Copy 类型（bool / u8 / Copy enum），**禁止**含 `Arc<dyn Trait>` / `Box<dyn Trait>` / `Vec<T>` / `String` 等非 Copy 字段

#### Scenario: 业务代码通过 BackendPolicy 字段选择行为

- **WHEN** `cdt-api::ipc::local::LocalDataApi` 的 IPC handler 需要根据后端类型选择行为
- **THEN** handler SHALL 读 `BackendPolicy` 字段（如 `policy.supports_memory` / `policy.stale_check_strategy`），**不得**直接 `if fs.kind() == FsKind::Ssh` 或等价 `let is_remote = fs.kind() == Ssh` 后做策略分支
- **AND** `fs.kind() ==` 比较仅允许出现在 `active_fs_and_policy()` 顶层派生 helper 内 + `cdt-api::ipc::backend_resolvers::BackendResolvers::from_fs()` 内 + cdt-fs / cdt-discover provider 实现内部
- **AND** `crates/cdt-api/tests/no_kind_compare_outside_resolvers.rs` 集成测试 SHALL 扫 `crates/cdt-api/src/ipc/local.rs` 统计 `fs.kind() ==` / `let is_remote =` 出现次数并断言 ≤ 阈值（本 change 后期望 ≤ 1，仅 `active_fs_and_policy` 内部）

#### Scenario: StaleCheckStrategy enum 至少包含 LocalClock5min 与 SkipUntilClockSync

- **WHEN** 编译 `cdt-fs`
- **THEN** `StaleCheckStrategy` SHALL 至少含 `LocalClock5min` 与 `SkipUntilClockSync` 两个 variant
- **AND** SHALL derive `Copy + Clone + PartialEq + Eq + Debug`
- **AND** 调用方对 `policy.stale_check_strategy` 的 `match` SHALL 通过 exhaustive 检查（未来加 variant 时编译期暴露未处理路径）
