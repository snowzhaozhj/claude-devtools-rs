# push-events

## ADDED Requirements

### Requirement: jobs-update push event

系统 SHALL 在 jobs 目录下的 state.json 变更时通过 Tauri emit `"jobs-update"` 事件通知前端。

#### Scenario: state.json change triggers push

- **WHEN** FileWatcher 检测到 state.json 变更
- **THEN** 系统 SHALL emit `"jobs-update"` 事件（payload 含 jobId）
- **AND** SSE bridge 同步推送给 HTTP 客户端

#### Scenario: Lagged subscriber silently skips

- **WHEN** broadcast receiver 因消费慢产生 Lagged 错误
- **THEN** 系统 SHALL 静默跳过，不 panic
- **AND** 前端下次收到事件时全量刷新
