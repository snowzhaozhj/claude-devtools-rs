# application-telemetry Specification

## Purpose
TBD - created by archiving change add-telemetry-signal-bus. Update Purpose after archive.
## Requirements
### Requirement: 信号 Registry 三类信号统一基础设施

系统 SHALL 提供单一全局 Signal Registry 承载三类信号：`Counter`（基于 `AtomicU64`）、`Histogram`（基于 32-bucket atomic，输入单位 ns）、`Event`（基于 lock-free SPSC bounded ring buffer）。Registry MUST 在进程启动时一次性注册所有静态信号 name（白名单），hot path 后续访问 MUST 仅做只读 lookup，禁止运行期写 Registry 的全局结构。

所有 Counter / Histogram 的写操作 MUST 使用 `Ordering::Relaxed`；信号 name MUST 是 `&'static str` 编译期常量（`macro_rules!` 限制 `$name:literal` token，非字面量编译期报错）；hot path 调用 MUST NOT 触发任何堆分配（`String` / `Vec` / `Box` 等）。

信号 name 是否在白名单 SHALL 由**运行期**校验（macro 仅保证字面量类型，无法编译期校验内容）：未注册 name 调用 SHALL no-op + inc `telemetry.unregistered_signal_attempt` counter（自身 hardcoded 注册），且 SHALL NOT 增长 Registry 内部 map（避免内存泄漏）。

#### Scenario: hot path counter 增量

- **WHEN** `try_lookup_cached_metadata` 命中分支调用 `counter!("metadata.cache.hit").inc()`
- **THEN** 系统 SHALL 仅执行一次 `AtomicU64::fetch_add(1, Relaxed)`，总开销 SHALL ≤ 10 ns
- **AND** SHALL NOT 分配任何堆内存
- **AND** SHALL NOT 持有任何锁

#### Scenario: hot path histogram 观察

- **WHEN** `list_sessions` IPC 入口调用 `let _t = histogram!("ipc.list_sessions.duration_ns").start_timer()` 并在出口 drop
- **THEN** 系统 SHALL 仅执行一次 `Instant::now()` 进 / 一次 `elapsed.as_nanos() as u64` 差值出 / 一次 `leading_zeros` bucket index 计算 / 一次 `AtomicU64::fetch_add(Relaxed)`
- **AND** 总开销 SHALL ≤ 50 ns
- **AND** 信号值落在 32 个 power-of-2 bucket 之一（bucket i 对应 `[2^i, 2^(i+1))` ns，i ∈ [0, 31]）
- **AND** 超出 bucket 31（上界 ≈ 4.3 s）的值 SHALL clamp 到 bucket 31
- **AND** 0 ns 输入 SHALL 落入 bucket 0（特例处理 `leading_zeros(0) = 64`）

#### Scenario: 低频路径 event push

- **WHEN** `cdt-ssh::polling` SFTP 探测失败调用 `event!("ssh.sftp_death", host_hash = h, ts = now)`
- **THEN** 系统 SHALL 把事件入 `EventQueue` 的内部 `VecDeque`（write lock 持有期 < 200 ns 无竞争）
- **AND** 当队列已满（cap 10000）时 SHALL `pop_front` drop 最老一条 + inc `dropped` counter
- **AND** 单线程无竞争开销 SHALL ≤ 200 ns；多线程竞争最坏 SHALL ≤ 5 μs
- **AND** SHALL NOT 阻塞 producer 至同步等待（write lock 等待时长有上限）

#### Scenario: 信号 name 未注册时运行期 no-op fallback

- **WHEN** 代码尝试调用 `counter!("undefined.signal")` 但该 name 未在启动期 Registry 白名单中注册
- **THEN** 系统 SHALL no-op（不增长 Registry 内部 map，避免内存泄漏）
- **AND** SHALL `inc` `telemetry.unregistered_signal_attempt` counter（该 counter 自身 hardcoded 始终在白名单）
- **AND** SHALL NOT panic / crash / 报 Result Err

#### Scenario: 信号 name 必须为字面量字符串（编译期）

- **WHEN** 代码写 `counter!(some_runtime_string_var).inc()`
- **THEN** 编译期 SHALL 报错（`macro_rules!` 内部限制 `$name:literal` token，非字面量编译期报"expected string literal"）

### Requirement: hot path 性能契约

启用 telemetry 后的 wall time 增量 SHALL < 0.2%；user time 增量 SHALL < 0.5%；max RSS 增量 SHALL < 1 MB（Registry 静态分配）。系统 SHALL 通过 perf 测试同步守护：以 feature flag 控制 telemetry 启用 / 关闭，运行同一负载（10000 次 `try_lookup_cached_metadata` + 1000 次 `list_sessions(50)`），对比两个分支的 wall time / user time / max RSS 满足上述阈值。

`event!` 宏 SHALL NOT 出现在 hot path（`cdt-api` IPC 入口、`cdt-discover` / `cdt-parse` / `cdt-analyze` 主循环 / 主算法）。CI 检查 SHALL 拦截 hot-path 文件中出现 `event!(` 字面量；命中即 fail PR。

#### Scenario: telemetry 启用后 hot path 无回归

- **WHEN** perf 测试以 `telemetry-enabled` feature 启用 telemetry，运行同一负载
- **THEN** wall time 增量 SHALL < 0.2%（按 baseline `try_lookup_cached_metadata` ~10-50 μs / list_sessions(50) ~95 ms 计）
- **AND** max RSS 增量 SHALL < 1 MB

#### Scenario: 在 hot path 调用低频 event API 被拦截

- **WHEN** PR 在 hot-path 文件（如 `cdt-api` IPC 入口）内加一行 `event!("perf.skeleton.start", ...)` 并 push
- **THEN** CI hot-path event 检查 SHALL fail
- **AND** PR SHALL NOT 通过（pipeline 红）

### Requirement: tracing bridge 方向 1 自动归类

系统 SHALL 注册 `tracing-subscriber::Layer` 实现 `TelemetryLayer`，钩 `Level::ERROR` 与 `Level::WARN` 事件，按事件 `target` 顶级 crate 名（取 `target.split("::").next()`）匹配启动期注册的白名单 `OnceLock<HashMap<&'static str, &'static str>>`，命中 SHALL 自动 `Counter::inc` 对应 counter（如 `cdt_ssh.error` / `cdt_watch.warn`）；未命中 SHALL 不归类、不分配、不阻塞。

白名单 MUST 至少覆盖：`cdt_core` / `cdt_parse` / `cdt_analyze` / `cdt_discover` / `cdt_watch` / `cdt_config` / `cdt_ssh` / `cdt_api`。其他 crate（包括 `tokio` / `reqwest` / `tauri` 等外部依赖）SHALL NOT 进入白名单——避免外部依赖的 ERROR 噪音污染应用 telemetry。

`TelemetryLayer::on_event` 单次开销 SHALL ≤ 200 ns（含 target 字符串 split + HashMap lookup + counter inc）；常态调用频率 < 100 / 秒时累积开销 SHALL < 0.001% CPU。

#### Scenario: 既有 tracing::error 自动归 counter

- **WHEN** `cdt-ssh::polling::watcher` 模块调用 `tracing::error!(target: "cdt_ssh::polling", "SFTP died: {:?}", e)`
- **THEN** `TelemetryLayer` SHALL 取顶级 crate 名 `cdt_ssh`，命中白名单
- **AND** SHALL 调 `counter!("cdt_ssh.error").inc()`
- **AND** 既有 `tracing::error!` 调用代码 SHALL NOT 被改动

#### Scenario: 外部依赖 ERROR 不归 counter

- **WHEN** 依赖 crate `tokio` 内部 `tracing::error!(target: "tokio::io", ...)` 触发
- **THEN** `TelemetryLayer` SHALL 取 `tokio`，未命中白名单
- **AND** SHALL no-op（不增长任何 counter）
- **AND** SHALL 不分配字符串 / map entry

### Requirement: panic critical event always-keep 通道

系统 SHALL 在 `src-tauri/src/lib.rs::run` 启动时**先调用 `std::panic::take_hook()` 取出既有 hook 并保存为 `Box<dyn Fn>`，再用 `std::panic::set_hook` 注册包装后的新 hook**——包装 hook 内 SHALL 先调用既有 hook 引用、再执行 telemetry 逻辑。MUST NOT 直接 `set_hook` 覆盖既有 hook（会丢失 Tauri / Tokio runtime 注册的默认 panic 行为）。

每次 panic SHALL（按顺序）：

1. 调用 `take_hook` 取出的既有 hook（保留 stderr 输出 / Sentry-like 上报路径）。
2. 调用 `counter!("panic.recovered").inc()`。
3. 调用独立的 `panic_critical_event_channel.push(panic_event)`——该通道 MUST 与普通 Event ring 分离，使用 `RwLock<Vec<Event>>` 实现，cap 1000；满时 SHALL 移除最老的 50% 条目并增 `panic.dropped_count` counter，永不丢弃 panic 信息至完全不可见。

panic event 字段 MUST 包含：`thread_name` / `panic_message`（截断到 1 KB）/ `location`（file:line）；MUST NOT 包含完整 backtrace（避免 PII / 体积）。

#### Scenario: panic 触发关键事件入队

- **WHEN** 任意线程触发 panic
- **THEN** `counter!("panic.recovered")` SHALL 增 1
- **AND** panic 通道 SHALL 新增一条 event
- **AND** event 字段 SHALL 含 thread_name / panic_message / location，SHALL NOT 含完整 backtrace

#### Scenario: panic 队列满时丢弃最老 50% 保留新事件

- **WHEN** panic 通道已满（1000 条）且第 1001 次 panic 触发
- **THEN** 通道 SHALL 移除最老的 500 条
- **AND** `counter!("panic.dropped_count")` SHALL 增 500
- **AND** 新 panic event SHALL 入队

### Requirement: Expose telemetry snapshot pull endpoint

系统 SHALL 暴露一个 IPC command `get_telemetry_snapshot()`，返回 `TelemetrySnapshot`，序列化为 camelCase。该 command 是 pull-based 快照读，每次调用 SHALL 一次性 atomic load 当前所有 Counter / Histogram / 最近 100 条 Event，组装为快照返回；调用过程 SHALL NOT 阻塞 hot path、SHALL NOT 改写任何 Registry 状态。

`TelemetrySnapshot` 字段契约（camelCase）：

```typescript
{
  schemaVersion: number,                      // 当前 1
  uptimeSecs: number,
  capturedAt: number,                         // unix millis
  counters: { [name: string]: number },       // u64 → number（JS 安全整数范围内）
  histograms: { [name: string]: HistogramSnapshot },
  recentEvents: TelemetryEvent[],             // 最近 100 条，按 ts 升序
}

HistogramSnapshot {
  count: number,
  buckets: number[],                          // 长度 32（power-of-2 ns）
  p50Ns: number | null,                       // bucket 上界 ns；语义 "实际值 ≤ 此值，最坏 2x 偏差"
  p95Ns: number | null,                       // 同上
  p99Ns: number | null,                       // 同上
  maxBucket: number | null,                   // 0..31，已观察到的最大 bucket index
}

TelemetryEvent {
  kind: string,
  ts: number,                                 // unix millis
  fields: { [key: string]: string | number }, // value 限定基础类型
}
```

字段命名 SHALL 与 IPC contract test (`crates/cdt-api/tests/ipc_contract.rs`) 锁定：camelCase + 字段集合稳定。新加字段 MUST 走 `schemaVersion` bump。

`p50Ns / p95Ns / p99Ns` 字段语义 SHALL 在 schema 文档中显式声明为"power-of-2 bucket 上界"——即真实 percentile 落在 `[bucket_lower, bucket_upper)` 区间内、报回 `bucket_upper`，最坏 2x 偏差。前端 UI（Diagnostics tab）SHALL 在数值旁加 hint 提示该语义。

snapshot 总 size SHALL < 100 KB（counters ~50 条 × 80 byte ≈ 4 KB / histograms 4 个 × 32 buckets × 8 byte + 元数据 ≈ 1.5 KB / events 100 条 × 200 byte ≈ 20 KB），低于 IPC 1 MB payload 阈值（详见 `.claude/rules/perf.md`）。

HTTP 路径 `GET /api/telemetry/snapshot` SHALL 返回相同 schema（snake_case）；浏览器 transport 层 SHALL 归一化为 camelCase 与 IPC 路径行为一致。

#### Scenario: 快照成功返回当前所有信号

- **WHEN** 前端调用 `invoke("get_telemetry_snapshot")`
- **THEN** 后端 SHALL 返回一个 `TelemetrySnapshot` JSON，包含 schemaVersion=1 / uptimeSecs / capturedAt / counters / histograms / recentEvents 字段
- **AND** counters map 至少包含 `metadata.cache.hit` / `metadata.cache.miss` / `panic.recovered` / `cdt_ssh.error` 等启动期已注册的 name（值可能为 0）
- **AND** SHALL 在 < 100 ms 内返回（即使 counters 50+ 条）

#### Scenario: 快照不阻塞 hot path

- **WHEN** 在 `list_sessions(50)` 进行中并发调用 `get_telemetry_snapshot`
- **THEN** snapshot 调用 SHALL 不阻塞 list_sessions 完成
- **AND** list_sessions wall time SHALL 不因 snapshot 调用增加 > 5%

#### Scenario: 字段命名稳定（IPC contract）

- **WHEN** `crates/cdt-api/tests/ipc_contract.rs` 调 `get_telemetry_snapshot` 并 assert JSON 字段
- **THEN** 字段名 SHALL 是 `schemaVersion` / `uptimeSecs` / `capturedAt` / `counters` / `histograms` / `recentEvents`（严格 camelCase）
- **AND** SHALL NOT 出现 `schema_version` / `uptime_secs` 等 snake_case
- **AND** HistogramSnapshot 字段 SHALL 为 `count` / `buckets`（长度 32）/ `p50Ns` / `p95Ns` / `p99Ns` / `maxBucket`

#### Scenario: HTTP 路径返回相同 schema

- **WHEN** 浏览器 client `GET /api/telemetry/snapshot` 拉取
- **THEN** 响应 body SHALL 与 IPC `get_telemetry_snapshot` 返回的 JSON 内容一致（除字段命名约定 snake_case vs camelCase 由 transport 层归一化外）
- **AND** 浏览器 transport SHALL 转换为 camelCase 后供前端组件消费

### Requirement: Expose telemetry correctness event batch endpoint

系统 SHALL 暴露一个 IPC command `record_correctness_events(items)`，接受批量 correctness 事件 inc 请求，按白名单逐条 `Counter::add(count)`。请求 payload 字段（camelCase）：

```typescript
{
  items: Array<{
    kind: string,    // 必须是白名单 kind 之一
    count: number,   // u64 增量，> 0
  }>
}
```

返回 `{ ok: true }` 即可（fire-and-forget 语义）。

**白名单 kind**（首发 Phase 1）：

- `stale_update.triggered` —— sidebar listener 检测到新旧值都 not-null 但不一致
- `cache.signature_skew_observed_in_ui` —— 前端 detector 检测到 cache signature 行为异常

未在白名单的 `kind` SHALL silently ignore（返回 `ok: true`），同时 inc `telemetry.unregistered_correctness_event` counter（自身 hardcoded 始终在白名单）。

前端 client SHALL：

- 不在 `session-metadata-update` listener 内每条事件立刻调本 IPC——MUST 在前端 store 内本地聚合（accumulate count），按 **5 秒 setTimeout 或累计 ≥ 50 条** 触发一次批量 flush。
- 调用 SHALL 是 fire-and-forget（不 await 响应）；调用失败 SHALL silently 重置本地累计（避免无限重试堆积）。

#### Scenario: 前端聚合 + 批量 flush

- **WHEN** sidebar 在 5 秒内检测到 30 条 `stale_update.triggered` correctness 事件
- **THEN** 前端 SHALL 仅在窗口结束时调一次 `recordCorrectnessEvents({ items: [{ kind: "stale_update.triggered", count: 30 }] })`
- **AND** SHALL NOT 在 5 秒内调多次 IPC

#### Scenario: 累计阈值触发提前 flush

- **WHEN** sidebar 在 1 秒内检测到 50 条 `stale_update.triggered`（达累计阈值）
- **THEN** 前端 SHALL 立即触发一次批量 flush，不等 5 秒窗口结束
- **AND** flush 后本地 counter SHALL 重置为 0

#### Scenario: 白名单外 kind silently ignore

- **WHEN** 前端调 `recordCorrectnessEvents({ items: [{ kind: "fake.event", count: 1 }] })`
- **THEN** 后端 SHALL 返回 `{ ok: true }`
- **AND** SHALL NOT inc 任何业务 counter
- **AND** SHALL inc `telemetry.unregistered_correctness_event` counter

#### Scenario: 字段命名稳定（IPC contract）

- **WHEN** `crates/cdt-api/tests/ipc_contract.rs` 调 `record_correctness_events` 并 assert request / response JSON 字段
- **THEN** request 字段 SHALL 是 `items[].kind` / `items[].count`（camelCase）
- **AND** response 字段 SHALL 是 `ok`（boolean）

