# fs-abstraction Spec Delta

## MODIFIED Requirements

### Requirement: `BackendPolicy` enum 雏形定义

fs 抽象 crate SHALL 定义 backend 行为策略 struct + 三个独立 enum 字段，作为业务路径**选择后端相关行为**的真相源：

- **InitialLoadPolicy**：表达"首屏加载策略"（一次性 vs 增量补全），至少含 `FullEager` 与 `SkeletonThenStream` 两个 variant
- **PrefetchPolicy**：表达"翻页预取策略"（不预取 vs 预取下一页），至少含 `None` 与 `PrefetchNext` 两个 variant
- **StaleCheckStrategy**：表达"5min stale 判定策略"，至少含 `LocalClock5min`（用本机 mtime 比对）与 `SkipUntilClockSync`（远端 mtime 跨 clock domain 不可比对）

`BackendPolicy` struct SHALL 是 Copy + Eq + Clone + Debug；字段 SHALL 是 primitive 或 Copy 类型（bool / u8 / Copy enum），**禁止**含 `Arc<dyn Trait>` / `Box<dyn Trait>` / `Vec<T>` / `String` 等非 Copy 字段——业务侧的 trait object 与 Clone 类型策略 SHALL 放在更高层（业务 IPC 层 backend resolvers）与 `BackendPolicy` 配套。

`PrefetchPolicy` 与 `InitialLoadPolicy` SHALL 是**正交字段**——**不得**把 `PrefetchNext` 塞进 `InitialLoadPolicy` 当 variant。

本 capability SHALL 提供三个 const 构造器：Local / SSH / HTTP 各一个，每个完整覆盖所有字段值。业务 callsite SHALL 通过 `BackendPolicy` 字段读取 backend-specific 行为，**禁止**在业务算法层直接 `match fs.kind()` 表达策略——`fs.kind()` 仅允许在策略**派生**点（顶层 helper 或 backend resolver 内部）使用。

#### Scenario: Local policy 含 SkeletonThenStream + 本机 stale

- **WHEN** 取 Local backend 的 policy 构造器结果
- **THEN** SHALL 返回 `initial_load_policy = SkeletonThenStream`、`max_round_trips_for_initial_page >= 2`、`supports_incremental_updates = true`、`prefetch_policy = None`
- **AND** SHALL 返回 `supports_memory = true`、`supports_subagent_scan = true`、`stale_check_strategy = LocalClock5min`

#### Scenario: SSH policy 含 FullEager + 跳过 stale

- **WHEN** 取 SSH backend 的 policy 构造器结果
- **THEN** SHALL 返回 `initial_load_policy = FullEager`、`max_round_trips_for_initial_page = 1`、`supports_incremental_updates = false`、`prefetch_policy = None`
- **AND** SHALL 返回 `supports_memory = true`、`supports_subagent_scan = false`、`stale_check_strategy = SkipUntilClockSync`

#### Scenario: HTTP policy 按 Local 数据源语义填

- **WHEN** 取 HTTP backend 的 policy 构造器结果
- **THEN** initial-load 相关字段 SHALL 保持现状（`FullEager` / `max_round_trips_for_initial_page = 1`）
- **AND** 其余字段按 Local 数据源语义填（`supports_memory = true` / `supports_subagent_scan = true` / `LocalClock5min`）

#### Scenario: PrefetchPolicy 与 InitialLoadPolicy 正交

- **WHEN** 显式构造 `BackendPolicy` 含 `SkeletonThenStream` + `PrefetchNext`
- **THEN** SHALL 编译通过且 `==` 自身（两字段独立可组合）
- **AND** `InitialLoadPolicy` 内 SHALL NOT 出现 `PrefetchNext` variant

#### Scenario: BackendPolicy 可按值复制并相等比较

- **WHEN** 编译 fs 抽象 crate
- **THEN** `BackendPolicy` SHALL derive Copy + Clone + PartialEq + Eq + Debug
- **AND** 所有字段 SHALL 是 primitive 或 Copy 类型，**禁止**含 `Arc<dyn Trait>` / `Box<dyn Trait>` / 非 Copy 容器

#### Scenario: 业务代码通过 BackendPolicy 字段选择行为

- **WHEN** 业务 IPC handler 需要根据后端类型选择行为
- **THEN** handler SHALL 读 `BackendPolicy` 字段，**不得**新增 `if fs.kind() == Ssh` / `let is_remote = ...` / `matches!(fs.kind(), ...)` 等等价直接派生
- **AND** `fs.kind()` 比对仅允许出现在策略**派生**点(顶层 helper / backend resolver 内部 / fs 抽象 crate 自身实现)

#### Scenario: StaleCheckStrategy enum 至少包含 LocalClock5min 与 SkipUntilClockSync

- **WHEN** 编译 fs 抽象 crate
- **THEN** `StaleCheckStrategy` SHALL 至少含 `LocalClock5min` 与 `SkipUntilClockSync` 两个 variant
- **AND** SHALL derive Copy + Clone + PartialEq + Eq + Debug
- **AND** 调用方对该字段 match SHALL 通过 exhaustive 检查

### Requirement: Provider instrumentation 入口可观测 fs op 次数

fs 抽象 crate SHALL 提供 `InstrumentedFs<P>` wrapper（`P` 是 fs provider）+ counter 类型 + counter 入口函数，让业务调用方可在每个 IPC command 边界统计 fs 操作次数（stat / read / read_dir / read_dir_with_metadata / read_to_string / read_lines_head / open_read / stat_many / write_atomic / create_dir_all / remove_file 各计数）。

**注入机制契约**：counter 通过 `InstrumentedFs` wrapper 在 trait 调用边界自动计数，**不**要求每个 provider 实现内嵌 record hook。具体语义：

1. wrapper 实现 fs provider trait，每个 trait 方法内部先 record 当前 counter，再 delegate 到 inner provider
2. 调用方注入 fs handle 时包一层 wrapper；测试 fake provider 同样包 wrapper 即可，不需要修改 fake 内部代码
3. 未包 wrapper 的 fs handle 调 trait 方法不计数（向后兼容）

counter 入口 SHALL 满足：

1. 基于 task-local 实现，避免全局 atomic 让并发 IPC command 互相干扰
2. async wrapper 函数让调用方包住代码块，结束后拿计数
3. wrapper 在 trait 调用边界自动 record，无需 provider 实现配合
4. 与日志 facade 集成——counter Drop 时自动 emit 一条结构化 event，含每种操作的次数

#### Scenario: wrapper 在 trait 边界自动计数

- **WHEN** 调用方包 wrapper 后用 counter 入口跑一段含若干 fs op 的代码
- **THEN** 返回的 counter snapshot SHALL 含每种 op 的实际计数
- **AND** provider 实现 SHALL NOT 含任何 counter 调用（计数发生在 wrapper 层）

#### Scenario: 未包 wrapper 不计数

- **WHEN** 调用方直接用 provider（未包 wrapper）+ 调 counter 入口
- **THEN** counter snapshot SHALL 全 0
- **AND** SHALL NOT panic（向后兼容）

#### Scenario: counter 不跨 task 污染

- **WHEN** 两个并发 task 各自调 counter 入口 + 各自的 wrapper
- **THEN** 两 task 的计数 SHALL 互不影响（依赖 task-local 隔离）

#### Scenario: wrapper 释放时输出诊断

- **WHEN** counter 入口闭包正常结束
- **THEN** SHALL emit 一条结构化日志 event 含全部计数字段

### Requirement: 本 change 零业务变化下性能基线不退化

本 change 是基建 PR-A，原则上**零业务代码变化**——但 trait 加 `Box<dyn AsyncRead>` 动态分发改了底层 LocalFileSystemProvider 内部路径（之前调用方拿到 inherent typed File，现在拿 Box dyn）。系统 SHALL 通过两套性能 gate 验证零退化：

1. **端到端 baseline 校验**：`cargo test --release -p cdt-api --test perf_cold_scan -- --ignored --nocapture` 与 `perf_get_session_detail` 在本 change apply 前后各跑 **5 次**取 min / median / stddev。回归判据：
   - median 退化 > 5% → 拒
   - stddev > 8ms（baseline 95ms 的 ~8%）→ 拒（说明引入了不稳定性）
   - min 退化 > 8% → 拒
2. **Local micro benchmark**（D4 量化要求）：新增 `crates/cdt-fs/benches/open_read_overhead.rs`，对比同 jsonl 文件（~500KB 与 ~5MB 两个 size）走 `tokio::fs::File::open + BufReader::lines` 直读路径 vs 走 `FileSystemProvider::open_read` dyn 路径，跑 10 次取 min / median / stddev。dyn 路径 SHALL 在 median 上 ≤ 直读路径 × 1.3（vtable overhead 上限），超过则拒

性能 gate SHALL 在本 change apply commit 上有 reproducible 数据（PR 描述贴 `/usr/bin/time -lp` 四维输出 + micro bench 结果），不只口头声称"零变化"。

#### Scenario: 端到端 baseline 不退化

- **WHEN** apply 本 change 后跑 `perf_cold_scan` 5 次
- **THEN** median SHALL ≤ 主线 baseline × 1.05
- **AND** stddev SHALL ≤ 8ms

#### Scenario: open_read 动态分发路径开销不超单态化的 1.3x

- **WHEN** 跑 `cargo bench -p cdt-fs --bench open_read_overhead` 10 次
- **THEN** `fs.open_read` dyn 路径的 median 耗时 SHALL ≤ `tokio::fs::File::open` 直读路径 × 1.3
- **AND** 若超过 1.3x，本 change PR review 拒，需重新评估 D4 决策（关联类型 vs dyn dispatch trade-off）
