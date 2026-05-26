# application-telemetry Spec Delta

## MODIFIED Requirements

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
