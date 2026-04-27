## MODIFIED Requirements

### Requirement: Serve projects and sessions over HTTP under /api prefix

系统 SHALL 在 `/api` 前缀下暴露与 IPC data API 同形数据返回的 HTTP endpoint，覆盖：列项目、取项目详情、取项目仓库信息、列会话（含分页与按 id 批量两种 variant）、取会话详情、取会话 chunk、取会话 metrics、取 waterfall 数据、取 subagent 详情。

`GET /api/sessions/:id` URL 仅携带 `session_id` 而**不**携带 `project_id`。系统 SHALL 在该 handler 内调 `DataApi::find_session_project(session_id)` 反查所属 `project_id`，再委托 `DataApi::get_session_detail(project_id, session_id)` 返回详情。`find_session_project` 返回 `Ok(None)` 时 SHALL 走既有 `Return safe defaults on lookup failures` 路径返 `404` + `code=not_found`，**不**得返回 `200` 配空 body 或 `500`。

`POST /api/sessions/batch`（对应 `DataApi::get_sessions_by_ids`）入参仅含 session id 列表；系统 SHALL 在 trait 实现层为每个 id 内部走 `find_session_project` + `get_session_detail` 复合路径，**不**得直接调 `get_session_detail("", session_id)`。某条 id 反查失败时 SHALL 在该位置返回 `metadata.status = "not_found"` 占位条目，整体响应 SHALL 仍为 `200`。

#### Scenario: GET list of projects
- **WHEN** 客户端发起 `GET /api/projects`
- **THEN** 响应 SHALL 是与 IPC list-projects 操作返回同形的 JSON 项目列表

#### Scenario: GET session detail
- **WHEN** 客户端发起 `GET /api/sessions/:id`
- **THEN** 响应 SHALL 含与 IPC 同形的 chunks、metrics、metadata

#### Scenario: GET session detail resolves project id internally
- **WHEN** 客户端发起 `GET /api/sessions/<sid>`，`<sid>` 对应的 jsonl 位于 project `<encoded>` 目录下
- **THEN** 系统 SHALL 内部反查得到 `project_id="<encoded>"`，再走 `get_session_detail("<encoded>", "<sid>")` 返回详情；`SessionDetail.projectId` SHALL 为 `"<encoded>"`、`SessionDetail.sessionId` SHALL 为 `"<sid>"`

#### Scenario: GET session detail unknown session returns 404
- **WHEN** 客户端发起 `GET /api/sessions/<sid>`，但 `<sid>` 在所有 project 目录下都没有对应 jsonl
- **THEN** 响应 SHALL 为 `404`，body 含 `code: "not_found"` 与 `message` 引用 `<sid>`

#### Scenario: POST sessions batch with mixed-existence ids
- **WHEN** 客户端发起 `POST /api/sessions/batch` body 为 `["sid-existing", "sid-missing"]`
- **THEN** 响应 SHALL 为 `200`，body 是长度 2 的数组：第 1 条为完整 detail（`projectId` 非空），第 2 条 `metadata.status` SHALL 为 `"not_found"`、`projectId` SHALL 为空字符串

#### Scenario: GET paginated sessions for a project
- **WHEN** 客户端发起 `GET /api/projects/:projectId/sessions-paginated?pageSize=N&cursor=C`
- **THEN** 响应 SHALL 与 IPC 分页 sessions 返回同形

### Requirement: Push events via Server-Sent Events

系统 SHALL 暴露一个 Server-Sent Events endpoint，传递与 IPC push channel 相同的事件流：`file-change`、`todo-change`、`new-notification`、`ssh-status`、updater 事件。

启动 HTTP server 的进程 SHALL 同时启动事件 producer：

1. **`file-change`**：订阅 `cdt_watch::FileWatcher::subscribe_files()` 的 `broadcast::Receiver<FileChangeEvent>`，每条事件转换为 `PushEvent::FileChange { project_id, session_id }` 后通过 `AppState.events_tx.send(...)` 推送（`deleted=true` 的事件 SHALL 同样推送，让客户端能感知删除）。
2. **`todo-change`**：订阅 `FileWatcher::subscribe_todos()`，每条 `TodoChangeEvent` 转换为 `PushEvent::TodoChange { project_id, session_id }` 推送（todo 文件名仅含 session_id；project_id 字段 SHALL 填空字符串占位以保留 schema 一致）。
3. **`new-notification`**：订阅 `LocalDataApi::subscribe_detected_errors()` 的 `broadcast::Receiver<DetectedError>`，每条 `DetectedError` 序列化成 `serde_json::Value` 后包成 `PushEvent::NewNotification { notification: <value> }` 推送。
4. **`ssh-status`** / **updater 事件**：当前 `cdt-ssh` / updater 模块未提供 broadcast 源；本 capability **不**强制系统在该实现阶段已经为这两类事件接入 producer——`PushEvent` enum 仍保留对应 variant，未来 producer 接通后 SSE 客户端 SHALL 按本 Requirement 描述的同一桥接模式收到。

producer 任务对 `RecvError::Lagged(_)` SHALL `continue` 跳过该条；对 `RecvError::Closed` SHALL 退出 loop。所有 producer task 共用同一 `AppState.events_tx`，但每个 SSE 客户端连接 SHALL 通过 `events_tx.subscribe()` 各自获得独立 receiver——`broadcast` 语义保证多客户端各自**恰好**收到一次事件。

producer 与 SSE handler SHALL 通过 `cdt_api::http::spawn_event_bridge` lib-level 公开函数粘合（签名见 `ipc-data-api` 不变；本 Requirement 仅规约行为，不规约具体函数名以外的实现细节）。

#### Scenario: SSE client subscribes and receives file change
- **WHEN** SSE 客户端已连接，某 session 文件被修改
- **THEN** 客户端 SHALL 在 debounce 窗口内收到一条 `file-change` 事件，携带 project id 与 session id

#### Scenario: SSE client receives todo change
- **WHEN** SSE 客户端已连接，某 todo 文件 `<sessionId>.json` 被修改
- **THEN** 客户端 SHALL 在 debounce 窗口内收到一条 `todo-change` 事件，携带 session id

#### Scenario: SSE client receives new-notification when DetectedError fires
- **WHEN** SSE 客户端已连接，notification pipeline 产出一条新的 `DetectedError`
- **THEN** 客户端 SHALL 收到一条 `new-notification` 事件，`notification` 字段含序列化后的 `DetectedError`

#### Scenario: Multiple concurrent SSE clients
- **WHEN** 三个 SSE 客户端已连接，发出一次通知
- **THEN** 每个客户端 SHALL **恰好**收到一次该事件

#### Scenario: Producer skips lagged events without crashing
- **WHEN** 某个 producer task 遇到 `RecvError::Lagged(n)`（订阅者落后导致丢条）
- **THEN** 该 task SHALL `continue` 至下一次 `recv`，**不**得 panic 或退出 loop；后续事件 SHALL 正常推送
