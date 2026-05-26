# application-telemetry Specification (delta)

## ADDED Requirements

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

