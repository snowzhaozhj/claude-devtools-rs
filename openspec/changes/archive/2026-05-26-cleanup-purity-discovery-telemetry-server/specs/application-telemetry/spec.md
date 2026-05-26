# application-telemetry Specification

## MODIFIED Requirements

### Requirement: hot path 性能契约

启用 telemetry 后的 wall time 增量 SHALL < 0.2%；user time 增量 SHALL < 0.5%；max RSS 增量 SHALL < 1 MB（Registry 静态分配）。系统 SHALL 通过 perf 测试同步守护：以 feature flag 控制 telemetry 启用 / 关闭，运行同一负载，对比两个分支的 wall time / user time / max RSS 满足上述阈值。

`event!` 宏 SHALL NOT 出现在 hot path（IPC 入口、核心扫描 / 解析 / 分析主循环 / 主算法）。CI 检查 SHALL 拦截 hot-path 文件中出现 `event!(` 字面量；命中即 fail PR。

#### Scenario: telemetry 启用后 hot path 无回归

- **WHEN** perf 测试以 `telemetry-enabled` feature 启用 telemetry，运行同一负载
- **THEN** wall time 增量 SHALL < 0.2%
- **AND** max RSS 增量 SHALL < 1 MB

#### Scenario: 在 hot path 调用低频 event API 被拦截

- **WHEN** PR 在 hot-path 文件（如 IPC 入口）内加一行 `event!("perf.skeleton.start", ...)` 并 push
- **THEN** CI hot-path event 检查 SHALL fail
- **AND** PR SHALL NOT 通过（pipeline 红）

### Requirement: tracing bridge 方向 1 自动归类

系统 SHALL 注册日志订阅层实现 telemetry bridge，钩 `Level::ERROR` 与 `Level::WARN` 事件，按事件 `target` 顶级 crate 名（取 `target.split("::").next()`）匹配启动期注册的白名单，命中 SHALL 自动 inc 对应 counter（如 `cdt_ssh.error` / `cdt_watch.warn`）；未命中 SHALL 不归类、不分配、不阻塞。

白名单 MUST 至少覆盖：`cdt_core` / `cdt_parse` / `cdt_analyze` / `cdt_discover` / `cdt_watch` / `cdt_config` / `cdt_ssh` / `cdt_api`。其他 crate（包括外部依赖）SHALL NOT 进入白名单——避免外部依赖的 ERROR 噪音污染应用 telemetry。

telemetry bridge 单次开销 SHALL ≤ 200 ns（含 target 字符串 split + map lookup + counter inc）；常态调用频率 < 100 / 秒时累积开销 SHALL < 0.001% CPU。

#### Scenario: 既有 error 日志自动归 counter

- **WHEN** SSH polling watcher 模块调用 error 级日志记录 SFTP 连接失败
- **THEN** telemetry bridge SHALL 取顶级 crate 名 `cdt_ssh`，命中白名单
- **AND** SHALL 调 `counter!("cdt_ssh.error").inc()`
- **AND** 既有日志调用代码 SHALL NOT 被改动

#### Scenario: 外部依赖 ERROR 不归 counter

- **WHEN** 外部依赖内部触发 error 级日志（target 为非白名单 crate 前缀）
- **THEN** telemetry bridge SHALL 取顶级 crate 名，未命中白名单
- **AND** SHALL no-op（不增长任何 counter）
- **AND** SHALL 不分配字符串 / map entry

### Requirement: panic critical event always-keep 通道

系统 SHALL 在桌面应用入口启动时**先取出既有 panic hook 并保存，再注册包装后的新 hook**——包装 hook 内 SHALL 先调用既有 hook 引用、再执行 telemetry 逻辑。MUST NOT 直接覆盖既有 hook（会丢失运行时注册的默认 panic 行为）。

每次 panic SHALL（按顺序）：

1. 调用取出的既有 hook（保留 stderr 输出 / 上报路径）。
2. 调用 `counter!("panic.recovered").inc()`。
3. 调用独立的 panic 事件通道 push——该通道 MUST 与普通 Event ring 分离，cap 1000；满时 SHALL 移除最老的 50% 条目并增 `panic.dropped_count` counter，永不丢弃 panic 信息至完全不可见。

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

字段命名 SHALL 与 IPC contract test 锁定：camelCase + 字段集合稳定。新加字段 MUST 走 `schemaVersion` bump。

`p50Ns / p95Ns / p99Ns` 字段语义 SHALL 在 schema 文档中显式声明为"power-of-2 bucket 上界"——即真实 percentile 落在 `[bucket_lower, bucket_upper)` 区间内、报回 `bucket_upper`，最坏 2x 偏差。前端 UI（Diagnostics tab）SHALL 在数值旁加 hint 提示该语义。

snapshot 总 size SHALL < 100 KB，低于 IPC 1 MB payload 阈值。

HTTP 路径 `GET /api/telemetry/snapshot` SHALL 返回相同 schema（snake_case）；浏览器 transport 层 SHALL 归一化为 camelCase 与 IPC 路径行为一致。

#### Scenario: 快照成功返回当前所有信号

- **WHEN** 前端调用 `invoke("get_telemetry_snapshot")`
- **THEN** 后端 SHALL 返回一个 `TelemetrySnapshot` JSON，包含 schemaVersion=1 / uptimeSecs / capturedAt / counters / histograms / recentEvents 字段
- **AND** counters map 至少包含 `metadata.cache.hit` / `metadata.cache.miss` / `panic.recovered` / `cdt_ssh.error` 等启动期已注册的 name（值可能为 0）
- **AND** SHALL 在 < 100 ms 内返回（即使 counters 50+ 条）

#### Scenario: 快照不阻塞 hot path

- **WHEN** 在 `list_sessions` 进行中并发调用 `get_telemetry_snapshot`
- **THEN** snapshot 调用 SHALL 不阻塞 list_sessions 完成
- **AND** list_sessions wall time SHALL 不因 snapshot 调用增加 > 5%

#### Scenario: 字段命名稳定（IPC contract）

- **WHEN** IPC contract test 调 `get_telemetry_snapshot` 并 assert JSON 字段
- **THEN** 字段名 SHALL 是 `schemaVersion` / `uptimeSecs` / `capturedAt` / `counters` / `histograms` / `recentEvents`（严格 camelCase）
- **AND** SHALL NOT 出现 `schema_version` / `uptime_secs` 等 snake_case
- **AND** HistogramSnapshot 字段 SHALL 为 `count` / `buckets`（长度 32）/ `p50Ns` / `p95Ns` / `p99Ns` / `maxBucket`

#### Scenario: HTTP 路径返回相同 schema

- **WHEN** 浏览器 client `GET /api/telemetry/snapshot` 拉取
- **THEN** 响应 body SHALL 与 IPC `get_telemetry_snapshot` 返回的 JSON 内容一致（除字段命名约定 snake_case vs camelCase 由 transport 层归一化外）
- **AND** 浏览器 transport SHALL 转换为 camelCase 后供前端组件消费
