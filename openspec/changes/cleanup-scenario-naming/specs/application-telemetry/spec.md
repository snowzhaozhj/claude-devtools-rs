# application-telemetry Spec Delta

## MODIFIED Requirements

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
