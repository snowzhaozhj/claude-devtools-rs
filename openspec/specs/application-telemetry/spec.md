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

系统 SHALL 在 `cdt-api/tests/perf_telemetry_overhead.rs` 中以 feature flag 控制 telemetry 启用 / 关闭，运行同一负载（10000 次 `try_lookup_cached_metadata` + 1000 次 `list_sessions(50)`），对比两个分支的 wall time。

启用 telemetry 后的 wall time 增量 SHALL < 0.2%；user time 增量 SHALL < 0.5%；max RSS 增量 SHALL < 1 MB（Registry 静态分配）。

`event!` 宏 SHALL NOT 出现在 hot path 文件下：CI 脚本 `scripts/check-no-hot-event.sh` SHALL grep `event!\(` 在 `crates/cdt-api/src/ipc/local.rs`、`crates/cdt-discover/src/`、`crates/cdt-parse/src/`、`crates/cdt-analyze/src/` 内应为 0 命中，否则 PR fail。

#### Scenario: telemetry 启用后 hot path 无回归

- **WHEN** 跑 `perf_telemetry_overhead` 测试以 feature `telemetry-enabled` 启用 telemetry
- **THEN** wall time 增量 SHALL < 0.2%（按 baseline `try_lookup_cached_metadata` ~10-50 μs / list_sessions(50) ~95 ms 计）
- **AND** max RSS 增量 SHALL < 1 MB

#### Scenario: hot path 误用 event 宏被 CI 拦截

- **WHEN** 提交 PR 在 `crates/cdt-api/src/ipc/local.rs::list_sessions_skeleton` 内加一行 `event!("perf.skeleton.start", ...)` 并 push
- **THEN** CI `scripts/check-no-hot-event.sh` SHALL fail
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

#### Scenario: panic 触发 always-keep 通道写入

- **WHEN** 任意线程触发 panic
- **THEN** `counter!("panic.recovered")` SHALL 增 1
- **AND** panic 通道 SHALL 新增一条 event
- **AND** event 字段 SHALL 含 thread_name / panic_message / location，SHALL NOT 含完整 backtrace

#### Scenario: panic 通道满时半压缩保留

- **WHEN** panic 通道已满（1000 条）且第 1001 次 panic 触发
- **THEN** 通道 SHALL 移除最老的 500 条
- **AND** `counter!("panic.dropped_count")` SHALL 增 500
- **AND** 新 panic event SHALL 入队

