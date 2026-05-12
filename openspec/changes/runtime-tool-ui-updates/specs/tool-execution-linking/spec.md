## ADDED Requirements

### Requirement: Preserve tool timing and error data for UI

工具执行记录 SHALL 保留 UI 展示耗时、等待状态与失败原因所需的原始数据。已配对工具 MUST 暴露 `start_ts`、`end_ts`、`is_error` 与 `output`；当 `tool_result.is_error=true` 且 JSONL 顶层 `toolUseResult` 含 `message` / `error` / `stderr` 时，记录 MUST 暴露 `error_message`；未配对工具 MUST 保留 `start_ts` 且 `end_ts=None`。

#### Scenario: Completed failed tool retains output

- **WHEN** `tool_result` 的 `is_error=true` 且 content 含错误文本
- **THEN** `ToolExecution` SHALL 设置 `is_error=true`
- **AND** `output` SHALL 保留该错误文本，供 UI 展示失败原因

#### Scenario: Top-level toolUseResult message becomes errorMessage

- **WHEN** `tool_result` 的 `is_error=true` 且 JSONL 顶层 `toolUseResult.message = "command not found"`
- **THEN** `ToolExecution.error_message` SHALL 为 `"command not found"`
- **AND** IPC 序列化字段名 SHALL 为 `errorMessage`

#### Scenario: Structured failed tool retains raw value

- **WHEN** `tool_result` 的 `is_error=true` 且 content 是结构化 JSON
- **THEN** `ToolExecution.output` SHALL 保留结构化值，不丢弃 `error`、`message`、`stderr` 等字段

#### Scenario: Orphan tool keeps pending timing source

- **WHEN** 一个 `tool_use` 没有匹配到 `tool_result`
- **THEN** `ToolExecution` SHALL 保留 `start_ts` 且 `end_ts=None`
- **AND** UI 能基于该记录展示等待或未完成状态
