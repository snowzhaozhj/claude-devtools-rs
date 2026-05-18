# http-data-api Specification

## Purpose

把 `ipc-data-api` 暴露的全部数据操作（项目 / 会话 / 搜索 / 配置 / 通知 / 通用工具 / SSH）通过 `/api` 前缀的 HTTP endpoint 镜像出去，并以 Server-Sent Events 推送同一套实时事件流。本 capability 让远端浏览器或第三方客户端不依赖 Tauri runtime 即可消费会话数据。
## Requirements
### Requirement: Serve projects and sessions over HTTP under /api prefix

系统 SHALL 在 `/api` 前缀下暴露与 IPC data API 同形数据返回的 HTTP endpoint，覆盖：列项目、取项目详情、取项目仓库信息、列会话（含分页与按 id 批量两种 variant）、取会话详情、取会话 chunk、取会话 metrics、取 waterfall 数据、取 subagent 详情。

项目 / 会话域的实际路由清单（Method + URL）SHALL 至少包含：

- `GET /api/projects` — 列所有项目
- `GET /api/projects/{projectId}/sessions` — 列指定项目下的会话（query `pageSize` / `cursor` 支持分页）
- `POST /api/projects/{projectId}/session-summaries/batch` — 按 id 列表批量取会话 summary（body 为 `string[]`）
- `GET /api/sessions/{sessionId}` — 取会话详情
- `POST /api/sessions/batch` — 按 id 列表批量取会话详情（body 为 `string[]`）

`GET /api/sessions/{sessionId}` URL 仅携带 `session_id` 而**不**携带 `project_id`。系统 SHALL 在该 handler 内调 `DataApi::find_session_project(session_id)` 反查所属 `project_id`，再委托 `DataApi::get_session_detail(project_id, session_id)` 返回详情。`find_session_project` 返回 `Ok(None)` 时 SHALL 走既有 `Return safe defaults on lookup failures` 路径返 `404` + `code=not_found`，**不**得返回 `200` 配空 body 或 `500`。

`POST /api/sessions/batch`（对应 `DataApi::get_sessions_by_ids`）入参仅含 session id 列表；系统 SHALL 在 trait 实现层为每个 id 内部走 `find_session_project` + `get_session_detail` 复合路径，**不**得直接调 `get_session_detail("", session_id)`。某条 id 反查失败时 SHALL 在该位置返回 `metadata.status = "not_found"` 占位条目，整体响应 SHALL 仍为 `200`。

#### Scenario: GET list of projects
- **WHEN** 客户端发起 `GET /api/projects`
- **THEN** 响应 SHALL 是与 IPC list-projects 操作返回同形的 JSON 项目列表

#### Scenario: GET session detail
- **WHEN** 客户端发起 `GET /api/sessions/{sessionId}`
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
- **WHEN** 客户端发起 `GET /api/projects/{projectId}/sessions?pageSize=N&cursor=C`
- **THEN** 响应 SHALL 与 IPC 分页 sessions 返回同形

#### Scenario: POST batch session summaries
- **WHEN** 客户端发起 `POST /api/projects/{projectId}/session-summaries/batch`，body 为该项目下若干 session id 列表
- **THEN** 响应 SHALL 是与 IPC `get_session_summaries_by_ids` 同形的 summary 数组，顺序与请求 id 顺序一致

### Requirement: Serve search endpoints

系统 SHALL 在 `/api` 下暴露与 `session-search` capability 对应的搜索 endpoint，接受 POST body 形式的 query 参数，返回有序结果。

实际路由 SHALL 为 `POST /api/search`（**不是** `POST /api/search/sessions`）。body schema 与 IPC `DataApi::search` 入参一致，至少包含 `query` 字符串与可选的 `projectId` / `sessionId` 限定字段；响应与 IPC 搜索操作返回同形。

#### Scenario: POST session search
- **WHEN** 客户端发起 `POST /api/search`，body 含 query、可选 project id、可选 session id
- **THEN** 响应 SHALL 与等价 IPC 搜索操作返回同形

### Requirement: Serve auxiliary, subagent, utility, and validation endpoints

系统 SHALL 暴露与 `ipc-data-api` 中所有辅助操作一一对应的 HTTP endpoint，包括 subagent 详情 / trace、仓库分组、worktree sessions、CLAUDE.md 读取、agent configs、路径 / mention 校验、通用 shell 操作、SSH、updater。

辅助域的实际路由清单（Method + URL）SHALL 至少包含：

- `GET /api/repository-groups` — 列仓库分组（聚合多 worktree 的 project）
- `GET /api/worktrees/{groupId}/sessions` — 列指定 repository group 下所有 worktree 的会话（query `pageSize` / `cursor`）
- `POST /api/validate/path` — 校验文件系统路径是否存在 + 是否在允许根之内（body 含 `path`、可选 `project_root`）
- `GET /api/claude-md?project_root=...` — 读取 global / project / project-local CLAUDE.md 文件
- `POST /api/mentioned-file` — 读取被 `@<path>` 提及的文件内容（body 含 `path`、`project_root`）
- `GET /api/agent-configs?project_root=...` — 读取 agent config 文件清单
- `GET /api/contexts` — 列所有已注册 service context（local + 各 SSH）
- `POST /api/contexts/switch` — 切换当前活跃 context（body 含 `context_id`）
- `POST /api/ssh/connect` — 建立 SSH 连接（body 与 `SshConnectRequest` 一致）
- `POST /api/ssh/disconnect` — 断开 SSH 连接（body 含 `context_id`）
- `GET /api/ssh/resolve-host?alias=...` — 通过 ssh_config alias 解析远端 host

subagent 详情 / chunk / waterfall 等若干路由当前由 `GET /api/sessions/{sessionId}` 复用 `SessionDetail` payload 内联返回；若未来拆出独立 endpoint，SHALL 同步追加路由清单。

#### Scenario: GET subagent detail via session detail payload
- **WHEN** 客户端发起 `GET /api/sessions/{rootSessionId}`
- **THEN** 响应 `SessionDetail` SHALL 含该 session 下所有 subagent 的 chunks 占位、metrics、spawning context；`subagent.messages` 默认按 IPC 同样的 omit 策略懒加载

#### Scenario: POST path validation
- **WHEN** 客户端发起 `POST /api/validate/path`，body `{ "path": "/etc/passwd", "project_root": "/Users/me/proj" }`
- **THEN** 响应 SHALL 标明路径是否存在以及是否在允许根之内（与 IPC `validate_path` 同形）

#### Scenario: GET CLAUDE.md files
- **WHEN** 客户端发起 `GET /api/claude-md?project_root=/Users/me/proj`
- **THEN** 响应 SHALL 与 IPC `read_claude_md_files` 同形，含 global / project / project-local 三层来源

#### Scenario: SSH host alias resolution
- **WHEN** 客户端发起 `GET /api/ssh/resolve-host?alias=prod-box`
- **THEN** 响应 SHALL 是与 IPC `resolve_ssh_host` 同形的解析结果（含 hostname、port、user 等）

### Requirement: Serve config and notification endpoints

系统 SHALL 暴露读取 / 更新配置以及列出 / 标记通知为已读的 HTTP endpoint，语义与 IPC data API 一致。

实际路由清单 SHALL 至少包含：

- `GET /api/config` — 读取当前配置
- `PATCH /api/config` — 更新配置字段（body 与 `ConfigUpdateRequest` 一致）
- `GET /api/notifications?limit=N&offset=M` — 列通知（默认 `limit=50`、`offset=0`）
- `POST /api/notifications/{notificationId}/read` — 标记单条通知为已读
- `DELETE /api/notifications/{notificationId}` — 删除单条通知
- `POST /api/notifications/mark-all-read` — 标记全部为已读
- `POST /api/notifications/clear` — 按可选 `trigger_id` 清理通知（body 可空，空时清全部）

#### Scenario: PATCH config field
- **WHEN** 客户端发起 `PATCH /api/config`，body 含合法字段更新
- **THEN** 响应 SHALL 反映新配置，且变更 SHALL 已被持久化

#### Scenario: GET notifications with pagination
- **WHEN** 客户端发起 `GET /api/notifications?limit=20&offset=40`
- **THEN** 响应 SHALL 是与 IPC `get_notifications(20, 40)` 同形的通知数组（按时间倒序）

#### Scenario: Mark all notifications read
- **WHEN** 客户端发起 `POST /api/notifications/mark-all-read`
- **THEN** 响应 SHALL 含 `{"success": true}`，所有通知 SHALL 已被持久化为已读状态

### Requirement: Push events via Server-Sent Events

系统 SHALL 暴露一个 Server-Sent Events endpoint，路径 SHALL 为 `GET /api/events`，传递与 IPC push channel 相同的事件流：`file-change`、`todo-change`、`new-notification`、`ssh-status`、updater 事件。

启动 HTTP server 的进程 SHALL 同时启动事件 producer：

1. **`file-change`**：订阅 `cdt_watch::FileWatcher::subscribe_files()` 的 `broadcast::Receiver<FileChangeEvent>`，每条事件转换为 `PushEvent::FileChange { project_id, session_id }` 后通过 `AppState.events_tx.send(...)` 推送（`deleted=true` 的事件 SHALL 同样推送，让客户端能感知删除）。
2. **`todo-change`**：订阅 `FileWatcher::subscribe_todos()`，每条 `TodoChangeEvent` 转换为 `PushEvent::TodoChange { project_id, session_id }` 推送（todo 文件名仅含 session_id；project_id 字段 SHALL 填空字符串占位以保留 schema 一致）。
3. **`new-notification`**：订阅 `LocalDataApi::subscribe_detected_errors()` 的 `broadcast::Receiver<DetectedError>`，每条 `DetectedError` 序列化成 `serde_json::Value` 后包成 `PushEvent::NewNotification { notification: <value> }` 推送。
4. **`ssh-status`** / **updater 事件**：当前 `cdt-ssh` / updater 模块未提供 broadcast 源；本 capability **不**强制系统在该实现阶段已经为这两类事件接入 producer——`PushEvent` enum 仍保留对应 variant，未来 producer 接通后 SSE 客户端 SHALL 按本 Requirement 描述的同一桥接模式收到。

producer 任务对 `RecvError::Lagged(_)` SHALL `continue` 跳过该条；对 `RecvError::Closed` SHALL 退出 loop。所有 producer task 共用同一 `AppState.events_tx`，但每个 SSE 客户端连接 SHALL 通过 `events_tx.subscribe()` 各自获得独立 receiver——`broadcast` 语义保证多客户端各自**恰好**收到一次事件。

producer 与 SSE handler SHALL 通过 `cdt_api::http::spawn_event_bridge` lib-level 公开函数粘合（签名见 `ipc-data-api` 不变；本 Requirement 仅规约行为，不规约具体函数名以外的实现细节）。

#### Scenario: SSE client subscribes via /api/events and receives file change
- **WHEN** SSE 客户端连接 `GET /api/events`，某 session 文件被修改
- **THEN** 客户端 SHALL 在 debounce 窗口内收到一条 `file-change` 事件，携带 project id 与 session id

#### Scenario: SSE client receives todo change
- **WHEN** SSE 客户端连接 `GET /api/events`，某 todo 文件 `<sessionId>.json` 被修改
- **THEN** 客户端 SHALL 在 debounce 窗口内收到一条 `todo-change` 事件，携带 session id

#### Scenario: SSE client receives new-notification when DetectedError fires
- **WHEN** SSE 客户端连接 `GET /api/events`，notification pipeline 产出一条新的 `DetectedError`
- **THEN** 客户端 SHALL 收到一条 `new-notification` 事件，`notification` 字段含序列化后的 `DetectedError`

#### Scenario: Multiple concurrent SSE clients
- **WHEN** 三个 SSE 客户端均连接 `GET /api/events`，发出一次通知
- **THEN** 每个客户端 SHALL **恰好**收到一次该事件

#### Scenario: Producer skips lagged events without crashing
- **WHEN** 某个 producer task 遇到 `RecvError::Lagged(n)`（订阅者落后导致丢条）
- **THEN** 该 task SHALL `continue` 至下一次 `recv`，**不**得 panic 或退出 loop；后续事件 SHALL 正常推送

### Requirement: Return safe defaults on lookup failures (current baseline)

系统 SHALL 对查询失败返回结构化错误响应，response body 形如 `{"code": "<code>", "message": "<...>"}`。`code` 字符串与 HTTP status 的映射 SHALL 与 `crates/cdt-api/src/ipc/error.rs::ApiErrorCode` 对应实现一致：

- `code: "validation_error"` → `400 Bad Request` — 输入校验失败（缺字段 / 字段类型错 / 值非法）
- `code: "config_error"` → `400 Bad Request` — 配置 JSON 解析失败 / 配置字段值非法
- `code: "not_found"` → `404 Not Found` — 请求资源不存在（项目 / 会话 / 通知 id 等）
- `code: "ssh_error"` → `502 Bad Gateway` — SSH 远端连接 / 命令执行失败（超时、握手失败、远端命令非零退出等）
- `code: "internal"` → `500 Internal Server Error` — 处理请求时未捕获异常 / 不变量违反

这是相对 TS 基线的有意改进——TS 基线返回 `200` 配 `null` / 空数组，本 capability 显式区分状态码并附带机器可读 `code`。

#### Scenario: GET nonexistent session
- **WHEN** 客户端请求一个不存在的 session id
- **THEN** 响应 SHALL 为 `404`，body 含 `code: "not_found"`

#### Scenario: GET sessions for unknown project
- **WHEN** 客户端请求一个无法解析的 project id 的 sessions
- **THEN** 响应 SHALL 为 `404`，body 含 `code: "not_found"`

#### Scenario: PATCH config with invalid field value
- **WHEN** 客户端发起 `PATCH /api/config`，body 字段值不符合 enum / 范围约束
- **THEN** 响应 SHALL 为 `400`，body 含 `code: "config_error"` 或 `code: "validation_error"`（前者用于配置层语义错、后者用于通用入参校验错）

#### Scenario: SSH command fails on remote
- **WHEN** 客户端发起 `POST /api/ssh/connect` 但远端不可达 / 握手失败
- **THEN** 响应 SHALL 为 `502`，body 含 `code: "ssh_error"` 与 `message` 描述底层失败

#### Scenario: Unhandled server exception
- **WHEN** 处理请求时抛出未捕获异常
- **THEN** 响应 SHALL 为 `500`，body 含 `code: "internal"`

### Requirement: Bind to configured port with graceful fallback

系统 SHALL 把 HTTP server 绑定到应用配置中的端口，若该端口已被占用 SHALL 在启动时记录明确的错误，SHALL NOT 静默改用其它端口。

#### Scenario: Configured port is busy
- **WHEN** 配置端口已被其它进程占用
- **THEN** 启动 SHALL 记录明确错误，SHALL NOT 静默切换端口

