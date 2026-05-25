# http-data-api Specification Changes

## MODIFIED Requirements

### Requirement: Push events via Server-Sent Events

系统 SHALL 暴露一个 Server-Sent Events endpoint（`GET /api/events`），传递与 IPC push channel 相同的事件流。启动 HTTP server 的进程 SHALL 同时启动事件 producer，覆盖以下信号源：

1. **file-change**：订阅文件 watcher broadcast，每条事件转换为 SSE PushEvent 推送（删除事件 SHALL 同样推送，让客户端能感知删除）。PushEvent payload 形态见 `[[push-events::file-change]]`。
2. **todo-change**：订阅 todo watcher broadcast，每条事件按统一 schema 推送。PushEvent payload 形态见 `[[push-events::todo-change]]`。
3. **new-notification**：订阅 detected-error broadcast，每条事件序列化后推送。PushEvent payload 形态见 `[[push-events::detected-error]]`。
4. **session-metadata-update**：订阅 session metadata broadcast，每条事件按统一 schema 推送，让浏览器 runtime 可复用 IPC 路径的骨架列表 + metadata patch 语义。PushEvent payload 形态见 `[[push-events::session-metadata-update]]`。
5. **ssh-status / updater 事件**：当前未提供 broadcast 源；PushEvent 仍保留对应 variant，未来 producer 接通后 SSE 客户端 SHALL 按本 Requirement 描述的同一桥接模式收到。PushEvent payload 形态见 `[[push-events::ssh-status-change]]`。

producer 任务对 broadcast Lagged 错误 SHALL 跳过该条；对 Closed 错误 SHALL 退出 loop。所有 producer task 共用同一 events 发送端，但每个 SSE 客户端连接 SHALL 各自获得独立 receiver——broadcast 语义保证多客户端各自**恰好**收到一次事件。

#### Scenario: Browser transport receives session metadata update

- **WHEN** 浏览器 runtime 通过 SSE endpoint 订阅，列会话后台元数据扫描产出一条 metadata 更新
- **THEN** SSE event data SHALL 携带 metadata 更新 type 与 snake_case 原始字段（形态见 `[[push-events::session-metadata-update]]`）
- **AND** 浏览器 transport SHALL 将其归一化为 camelCase payload
- **AND** 浏览器 runtime 的 sidebar SHALL 能用该事件 in-place patch 对应 session summary

### Requirement: /api/events SSE 在 broadcast 容量打满时 SHALL 推送 sse_lagged sentinel

SSE handler 在 broadcast 容量打满 + 当前 receiver 跟不上速度时返 Lagged 错误。系统 SHALL 把 Lagged 转为一条 SSE event，data 字段为 sse-lagged payload（形态见 `[[push-events::sse-lagged]]`）；stream SHALL 继续从最新 PushEvent 接收，**不**退出 stream。

UI 层 browser transport 收到 sse_lagged event SHALL 映射到 sse-lagged event name 派发给所有 handler；订阅方 SHALL 与 sse-recovered 共享同一 silent refresh handler 重拉一轮 metadata。

#### Scenario: 容量打满时 SSE handler 推送 sse_lagged sentinel

- **WHEN** broadcast capacity 被打满 + 当前 SSE receiver 落后导致 Lagged
- **THEN** SSE handler SHALL 推送一条 SSE event（sse-lagged 形态见 `[[push-events::sse-lagged]]`）
- **AND** SSE stream SHALL 继续从最新 PushEvent 接收（**不**退出 stream / **不**静默丢弃）
- **AND** 后续真正的 metadata 更新等事件 SHALL 正常推送到 client

#### Scenario: 浏览器 client 收到 sse_lagged 时触发 silent refresh

- **WHEN** 浏览器 client 收到 sse_lagged event
- **THEN** browser transport SHALL 把它映射到 sse-lagged event name 派发给所有 handler
- **AND** 订阅方 SHALL 对当前选中 project 触发 silent refresh 兜底重拉

#### Scenario: HTTP file-change producer Lagged 时推送 PushEvent sse-lagged

- **WHEN** HTTP server 内 file-change producer 的 broadcast receiver 返回 Lagged(n)
- **THEN** producer SHALL 向 SSE 事件流发送 sse-lagged event（payload 形态见 `[[push-events::sse-lagged]]`）
- **AND** SHALL NOT 静默吞掉该 lag 信号
- **AND** producer SHALL 继续从最新 PushEvent 接收后续事件
