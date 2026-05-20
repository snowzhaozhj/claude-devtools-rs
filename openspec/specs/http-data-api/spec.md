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

`GET /api/projects/{projectId}/sessions` SHALL 返回**骨架** `PaginatedResponse<SessionSummary>` —— 每条 `SessionSummary` 的 `sessionId` / `projectId` / `timestamp` SHALL 为真实值，`title` / `messageCount` / `isOngoing` / `gitBranch` SHALL 允许为占位值（与 IPC `list_sessions` 行为对齐，详见 `ipc-data-api` spec §"Expose project and session queries"）。后端 `try_lookup_cached_metadata` lookup-fast-path 命中条 SHALL 在响应中直接 inline 填回真值（zero 后续 SSE emit）；未命中条的真实 metadata SHALL 通过 `/api/events` SSE 的 `session_metadata_update` 事件异步推送（详见本 spec §"Push events via Server-Sent Events"）。**禁止** axum handler 调 `DataApi::list_sessions_sync(...)`——该方法保留作为 trait fallback，但 HTTP 路由不再使用。

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

#### Scenario: GET paginated sessions returns skeleton with cache-hit inline real values
- **WHEN** 客户端发起 `GET /api/projects/{projectId}/sessions?pageSize=N&cursor=C`
- **AND** 项目下某些 session 的 `MetadataCache` 已命中（`FileSignature` 等价），其余 cache miss
- **THEN** axum handler `cdt-api::http::routes::list_sessions` SHALL 调 `DataApi::list_sessions(project_id, pagination)`（**不**得调 `list_sessions_sync`）
- **AND** 响应 body SHALL 是 `PaginatedResponse<SessionSummary>`：cache 命中条 SHALL inline 携带真实 `title` / `messageCount` / `isOngoing` / `gitBranch`；cache miss 条 SHALL 为占位值（`title=null` / `messageCount=0` / `isOngoing=false` / `gitBranch=null`）
- **AND** cache miss 条 SHALL 在后续 backend 后台扫描完成后通过 `/api/events` SSE `session_metadata_update` 事件推送真实值

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

系统 SHALL 暴露一个 Server-Sent Events endpoint，路径 SHALL 为 `GET /api/events`，传递与 IPC push channel 相同的事件流：`file-change`、`todo-change`、`new-notification`、`session-metadata-update`、`ssh-status`、updater 事件。

启动 HTTP server 的进程 SHALL 同时启动事件 producer：

1. **`file-change`**：订阅 `cdt_watch::FileWatcher::subscribe_files()` 的 `broadcast::Receiver<FileChangeEvent>`，每条事件转换为 `PushEvent::FileChange { project_id, session_id }` 后通过 `AppState.events_tx.send(...)` 推送（`deleted=true` 的事件 SHALL 同样推送，让客户端能感知删除）。
2. **`todo-change`**：订阅 `FileWatcher::subscribe_todos()`，每条 `TodoChangeEvent` 转换为 `PushEvent::TodoChange { project_id, session_id }` 推送（todo 文件名仅含 session_id；project_id 字段 SHALL 填空字符串占位以保留 schema 一致）。
3. **`new-notification`**：订阅 `LocalDataApi::subscribe_detected_errors()` 的 `broadcast::Receiver<DetectedError>`，每条 `DetectedError` 序列化成 `serde_json::Value` 后包成 `PushEvent::NewNotification { notification: <value> }` 推送。
4. **`session-metadata-update`**：订阅 `LocalDataApi::subscribe_session_metadata()` 的 `broadcast::Receiver<SessionMetadataUpdate>`，每条事件转换为 `PushEvent::SessionMetadataUpdate { project_id, session_id, title, message_count, is_ongoing, git_branch }` 推送，让浏览器 runtime 的 Sidebar 能复用 IPC 路径的骨架列表 + metadata patch 语义。
5. **`ssh-status`** / **updater 事件**：当前 `cdt-ssh` / updater 模块未提供 broadcast 源；本 capability **不**强制系统在该实现阶段已经为这两类事件接入 producer——`PushEvent` enum 仍保留对应 variant，未来 producer 接通后 SSE 客户端 SHALL 按本 Requirement 描述的同一桥接模式收到。

producer 任务对 `RecvError::Lagged(_)` SHALL `continue` 跳过该条；对 `RecvError::Closed` SHALL 退出 loop。所有 producer task 共用同一 `AppState.events_tx`，但每个 SSE 客户端连接 SHALL 通过 `events_tx.subscribe()` 各自获得独立 receiver——`broadcast` 语义保证多客户端各自**恰好**收到一次事件。

producer 与 SSE handler SHALL 通过 `cdt_api::http::spawn_event_bridge` lib-level 公开函数粘合（本 Requirement 规约行为，不规约具体函数签名）。

#### Scenario: Browser transport receives session metadata update

- **WHEN** 浏览器 runtime 通过 `GET /api/events` 订阅 SSE，`list_sessions` 后台元数据扫描产出一条 `SessionMetadataUpdate`
- **THEN** SSE event data SHALL 携带 `session_metadata_update` type 与 snake_case 原始字段：`project_id` / `session_id` / `title` / `message_count` / `is_ongoing` / `git_branch`
- **AND** 浏览器 transport SHALL 将其归一化为 `session-metadata-update` 事件与 camelCase payload：`projectId` / `sessionId` / `title` / `messageCount` / `isOngoing` / `gitBranch`
- **AND** 浏览器 runtime 的 Sidebar SHALL 能用该事件 in-place patch 对应 session summary

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

### Requirement: HTTP server SHALL layer CORS middleware for localhost origins

`cdt_api::http::start_server` SHALL 在构建 router 时 layer 一道 CORS 中间件，仅放行来自 `localhost` 与 `127.0.0.1` 任意端口（含可选 `https://` 协议）的 origin。CORS 实现 SHALL 用 `tower_http::cors::CorsLayer` 配 `AllowOrigin::predicate` 显式判断，匹配规则 SHALL 等价于正则 `^https?://(localhost|127\.0\.0\.1)(:\d+)?$`。

非 localhost origin（如 `https://localhost.evil.com` / `http://example.com`）SHALL 在 CORS preflight 阶段被拒绝（响应 SHALL 不携带 `Access-Control-Allow-Origin`，浏览器自然拦截）。CORS allow-methods SHALL 至少包含 `GET / POST / PATCH / DELETE / OPTIONS`，allow-headers SHALL 包含 `Content-Type`。同源请求（origin 与 server bind 一致）SHALL 不触发 CORS 检查，行为不变。

CORS layer 不引入鉴权或 origin 配置项——任何放宽（LAN 访问 / 自定义 origin allowlist）SHALL 走单独的后续 change 评估。

#### Scenario: localhost origin 通过 CORS

- **WHEN** 浏览器从 `http://localhost:3456` 发起 `GET /api/projects`，附带 `Origin: http://localhost:3456`
- **THEN** 响应 SHALL 携带 `Access-Control-Allow-Origin: http://localhost:3456`（或等价 echo origin 形态）
- **AND** 请求 SHALL 正常处理返回 200 + 项目列表

#### Scenario: 127.0.0.1 origin 通过 CORS

- **WHEN** 浏览器从 `http://127.0.0.1:3456` 发起 `GET /api/projects`，附带 `Origin: http://127.0.0.1:3456`
- **THEN** 响应 SHALL 携带匹配该 origin 的 `Access-Control-Allow-Origin`
- **AND** 请求 SHALL 正常处理

#### Scenario: 非 localhost origin 被 CORS 拒绝

- **WHEN** 浏览器从 `https://localhost.evil.com` 发起跨域请求 `GET /api/projects`，附带 `Origin: https://localhost.evil.com`
- **THEN** 响应 SHALL **不**携带 `Access-Control-Allow-Origin: https://localhost.evil.com`
- **AND** 浏览器 SHALL 阻止 JavaScript 读取响应

#### Scenario: preflight OPTIONS 请求

- **WHEN** 浏览器对 `PATCH /api/config` 发起 preflight `OPTIONS`，origin 为 `http://localhost:3456`
- **THEN** 响应 SHALL 含 `Access-Control-Allow-Methods` 含 `PATCH`、`Access-Control-Allow-Headers` 含 `Content-Type`、状态 SHALL 为 200/204

### Requirement: HTTP server SHALL serve static frontend assets with SPA fallback

`cdt_api::http::start_server` SHALL 接受可选参数 `static_dir: Option<PathBuf>`：传入时 SHALL 在 router 上挂静态文件 fallback handler，提供 `static_dir` 下的静态文件 serve；传入路径无效（不存在 / 非目录）时 SHALL 仅 `tracing::warn!` 警告并跳过 fallback，**不**得 panic 或拒绝启动。

静态文件 fallback SHALL 在 `/api` 路由之后注册（顺序保证 API 优先）。fallback handler 按以下三种情况分流：

1. **磁盘上对应文件存在** → serve 文件内容（按扩展名 guess mime；未识别扩展名 fallback 到 `application/octet-stream`）
2. **navigation 请求**（路径无 `.` 扩展名 / 根路径 `/`）→ 返回 `index.html` 让前端 client-side router 接管
3. **带扩展名但磁盘上不存在的资源**（如 `/assets/missing.js`、`/favicon.ico`）→ SHALL 返回 `404`，**不**得 fallback 到 `index.html`——否则浏览器把 HTML 当 JS 解析爆 parse error，且 `200` 状态会被 CDN / 浏览器缓存导致脏数据持久化（SPA 部署经典坑，本 change 显式规约此行为）

path traversal 防御：fallback handler SHALL 拒绝路径中含 `..` 段（包括 URL-encoded 形态 `%2e%2e` / `%2E%2E`）以及 backslash（`\` 或 `%5c`）的请求。具体状态码可为 `403`（fallback handler 主动拒绝）、`400` / `404`（axum 路由层在 normalize / decode 阶段先拦下）任一——**不变量**是：SHALL NOT 返 `200` + 任何静态文件内容（攻击者 SHALL NOT 凭 traversal 拿到磁盘文件）。

`static_dir = None` 时（如 `cdt-cli` 默认行为或 dev mode）SHALL 不挂 fallback，未命中 API 路由的请求 SHALL 返回 `404`（与本 change 之前行为兼容）。Tauri runtime SHALL 在调用 `start_server` 时根据 `tauri::path::resource_dir()` 计算前端 bundle 路径并传入；具体子路径解析 SHALL 在实施期通过 `cargo tauri build` 实测确定（design.md Open Questions 已记）。

#### Scenario: GET / 返回前端 index.html

- **WHEN** 启动 server 时传入 `static_dir = Some(<path/to/ui/dist>)`，浏览器请求 `GET /`
- **THEN** 响应 SHALL 为 `200`，body SHALL 为前端 `index.html` 内容、`Content-Type: text/html`

#### Scenario: GET 已知静态资源命中 ServeDir

- **WHEN** 浏览器请求 `GET /assets/index-abc123.js`，该文件存在于 `static_dir/assets/`
- **THEN** 响应 SHALL 为 `200`、对应 JS 内容、合适 `Content-Type`

#### Scenario: GET 未知前端路由 fallback 到 index.html

- **WHEN** 浏览器请求 `GET /sessions/some-id`（前端 client-side router 路由，磁盘上无此文件）
- **THEN** 响应 SHALL 为 `200` 返回 `index.html` 让前端接管路由

#### Scenario: GET 带扩展名但磁盘上不存在的资源 SHALL 返 404 不 fallback

- **WHEN** 浏览器请求 `GET /assets/missing.js`、`/favicon.ico`、`/some/path/file.png` 等带扩展名路径，但磁盘上无此文件
- **THEN** 响应 SHALL 为 `404`，body **不**得含 `index.html` 内容
- **AND** 浏览器 SHALL NOT 把 HTML 当成 JS 解析（避免 CDN / 浏览器缓存脏 200 状态）

#### Scenario: path traversal 攻击 SHALL NOT 拿到磁盘文件

- **WHEN** 浏览器请求 `GET /../etc/passwd` / `/foo/../../bar` / `/%2e%2e/etc/passwd` / `/%2E%2E/etc/passwd` / `/foo/%2e%2e/bar` / `/foo%5cbar`（含裸 `..`、URL-encoded `..`、URL-encoded `\` 等形态）
- **THEN** 响应 SHALL NOT 为 `200` 且 SHALL NOT 含任何静态文件内容
- **AND** 状态码可为 `403`（fallback handler 主动拒绝）/ `400` / `404`（axum 路由层先拦下），具体由框架决定

#### Scenario: GET /api/* 不被 ServeDir 拦截

- **WHEN** 浏览器请求 `GET /api/projects`
- **THEN** 该请求 SHALL 走 API handler、**不**得被 ServeDir 拦截或 fallback 到 index.html

#### Scenario: static_dir = None 时无 ServeDir

- **WHEN** 启动 server 时不传 `static_dir`（或传 `None`）
- **THEN** 未命中 API 路由的 `GET /` 请求 SHALL 返回 `404`，行为与本 change 之前一致

#### Scenario: static_dir 路径无效仅警告不阻塞启动

- **WHEN** `static_dir = Some("/nonexistent/path")`
- **THEN** server SHALL 启动成功（仅 `tracing::warn!` 提示路径无效），所有 `/api/*` 路由 SHALL 正常 serve
- **AND** `GET /` 请求 SHALL 返回 `404`

### Requirement: Mirror lazy and auxiliary IPC commands as HTTP endpoints

`cdt-api::http::routes` SHALL 在 `/api` 前缀下镜像目前仅在 IPC 路径暴露的 lazy 与辅助 command，让浏览器 runtime（详 [[server-mode]]）能跑通完整 UI 流程。每个 endpoint 的请求 / 响应 schema SHALL 与对应 IPC command 同形（camelCase 字段、相同 enum tag、相同 `xxxOmitted` 语义）。

实际路由清单（Method + URL）SHALL 至少包含：

- `GET /api/projects/{projectId}/memory` — 镜像 `get_project_memory`，返回 `MemoryFile[]`
- `POST /api/projects/{projectId}/memory-files` — 镜像 `read_memory_file`，body 含 `{ "file": "<relative path>" }`，返回文件内容
- `GET /api/sessions/{rootSessionId}/subagents/{subagentSessionId}/trace` — 镜像 `get_subagent_trace`，返回 trace 数据
- `GET /api/sessions/{rootSessionId}/subagents/{sessionId}/blocks/{blockId}/image` — 镜像 `get_image_asset`，返回浏览器可加载的 `data:` URI / base64 字符串；若底层 IPC 返回 Tauri-only `asset://localhost/...`，HTTP handler SHALL 转为 `data:` URI
- `GET /api/sessions/{rootSessionId}/subagents/{sessionId}/tools/{toolUseId}/output` — 镜像 `get_tool_output`，返回 `ToolOutput`（含 `outputBytes` / `outputOmitted` 语义）
- `POST /api/notifications/triggers` — 镜像 `add_trigger`，body 为 `NotificationTrigger`（caller SHALL 在 body 内提供非空 `id`，与 IPC 路径校验语义一致；server 不自动生成 id）
- `DELETE /api/notifications/triggers/{triggerId}` — 镜像 `remove_trigger`
- `POST /api/projects/{projectId}/sessions/{sessionId}/pin` — 镜像 `pin_session`
- `DELETE /api/projects/{projectId}/sessions/{sessionId}/pin` — 镜像 `unpin_session`
- `POST /api/projects/{projectId}/sessions/{sessionId}/hide` — 镜像 `hide_session`
- `DELETE /api/projects/{projectId}/sessions/{sessionId}/hide` — 镜像 `unhide_session`
- `GET /api/projects/{projectId}/session-prefs` — 镜像 `get_project_session_prefs`，返回 `{ pinned: string[], hidden: string[] }`

每个 handler SHALL 直接委托给 `LocalDataApi` 同名方法，错误映射沿用 `Return safe defaults on lookup failures (current baseline)` Requirement 的 `code → status` 表。

**HTTP payload omit 行为**：lazy endpoint（image asset / tool output）SHALL **不**应用 `OMIT_*` 裁剪——它们本就是 IPC 侧懒加载的真实数据来源，HTTP 路径用户主动请求时 SHALL 拿到完整 payload。`get_session_detail` / `get_sessions_by_ids` 路径维持 `ipc-data-api` spec 现有"HTTP 不应用 omit"语义不变（浏览器 runtime 大会话首屏可能因此较慢，作为已知限制 + 后续 follow-up 评估窗口）。

#### Scenario: GET project memory mirrors IPC

- **WHEN** 浏览器请求 `GET /api/projects/<projectId>/memory`
- **THEN** 响应 SHALL 与 IPC `get_project_memory(<projectId>)` 同形（含 CLAUDE.md 各 scope）

#### Scenario: GET image asset returns browser-loadable data

- **WHEN** 浏览器请求 `GET /api/sessions/<root>/subagents/<sid>/blocks/<bid>/image`
- **THEN** 响应 SHALL 是浏览器可加载的 `data:` URI 或 base64 字符串
- **AND** SHALL NOT 返回 Tauri-only `asset://localhost/...` URL
- **AND** SHALL **不**应用任何 `dataOmitted` 裁剪（lazy 端点本就是真实数据源）

#### Scenario: GET tool output preserves outputOmitted semantics

- **WHEN** 浏览器请求 `GET /api/sessions/<root>/subagents/<sid>/tools/<tuid>/output`
- **THEN** 响应 SHALL 与 IPC `get_tool_output` 同形——`outputBytes` 字段保留、`outputOmitted: false`、内层 `text` / `value` 携带完整内容

#### Scenario: POST add trigger persists caller-provided id

- **WHEN** 浏览器 `POST /api/notifications/triggers`，body 为合法 `NotificationTrigger`（含 caller 自行分配的非空 `id`）
- **THEN** 响应 SHALL 返回更新后的完整 `AppConfig` JSON，`notifications.triggers` 含该新 trigger 且 `id` 与 caller 入参一致，与 IPC `add_trigger` 同形
- **AND** 后续 `GET /api/config` 读取的 `notifications.triggers` SHALL 含该新 trigger

#### Scenario: POST pin session 与 DELETE unpin session 互逆

- **WHEN** 浏览器先 `POST /api/projects/<pid>/sessions/<sid>/pin`，再 `GET /api/projects/<pid>/session-prefs`
- **THEN** 响应 `pinned` 数组 SHALL 含 `<sid>`
- **WHEN** 然后 `DELETE /api/projects/<pid>/sessions/<sid>/pin`，再次 GET prefs
- **THEN** `pinned` SHALL 不含 `<sid>`

### Requirement: 浏览器 client SHALL 在首次 list_sessions 前订阅 SSE

由于 `GET /api/projects/{projectId}/sessions` 改为骨架 + SSE 异步 patch 语义，浏览器 client SHALL 在发起首次 `GET /api/projects/{projectId}/sessions` 之前确保 `EventSource('/api/events')` 已进入 `OPEN` 状态——否则 backend 在 GET response 返回后立即 emit 的 `session_metadata_update` 事件会在 EventSource 订阅前发生，导致 metadata patch 永久丢失（直到 file-change 触发 silent refresh 兜底）。

实现 SHALL 在 `ui/src/lib/transport.ts::BrowserTransport.invokeHttp(cmd, args)` 内对 `cmd === 'list_sessions'` 与 `cmd === 'list_repository_groups'` / `cmd === 'list_worktree_sessions'`（任何走 IPC `list_sessions` 链路的 command）添加 `await this.ensureSseReady()` 前置 await。`ensureSseReady()` 行为：

- 若 `source.readyState === EventSource.OPEN` → 立返 `Promise.resolve()`
- 若 `source.readyState === EventSource.CONNECTING` → await 单次 `onopen` 或最大 1000 ms 超时
- 若 `source.readyState === EventSource.CLOSED` → 触发 `scheduleReconnect(source)` 后 await 新 source `onopen` 或最大 1000 ms 超时
- 超时后 SHALL 放行（不抛错）让首次 fetch 继续；丢失的 metadata 由后续 file-change silent refresh 兜底

Tauri runtime（`TauriTransport`）SHALL NOT 受此 Requirement 影响——Tauri IPC event listener 由 `@tauri-apps/api` 同步注册，不存在 EventSource 异步 OPEN 等待问题。

#### Scenario: BrowserTransport 等待 SSE OPEN 后才发 list_sessions

- **WHEN** 浏览器 runtime 首次调用 `invoke('list_sessions', { projectId: "p", pageSize: 20 })`
- **AND** `EventSource('/api/events')` 处于 `CONNECTING`
- **THEN** `BrowserTransport.invokeHttp` SHALL await `ensureSseReady()` 直至 source 进入 `OPEN` 或 1000 ms 超时
- **AND** 仅在 await 返回后 SHALL 发起 `fetch('/api/projects/p/sessions?pageSize=20')`

#### Scenario: SSE 已 OPEN 时不阻塞 list_sessions

- **WHEN** 浏览器调用 `invoke('list_sessions', ...)` 且 `source.readyState === EventSource.OPEN`
- **THEN** `ensureSseReady()` SHALL 立返（不引入额外 latency）

#### Scenario: SSE 1000 ms 超时后仍放行

- **WHEN** `EventSource` 由于网络问题 1000 ms 内未进入 `OPEN`
- **THEN** `ensureSseReady()` SHALL resolve（不抛错）
- **AND** 后续 fetch SHALL 正常发起
- **AND** `BrowserTransport` SHALL 记录 timeout 状态，使得后续 EventSource 真正进入 `OPEN` 时（自然成功 / scheduleReconnect 重连成功）SHALL 给所有已注册 handler emit 一次 `sse-recovered` pseudo-event，让 UI 触发 silent refresh 重拉一轮 metadata patch；**不**得仅依赖偶发的 file-change 兜底（codex 二审 issue 1：纯历史浏览场景无新 file-change 时 metadata 永久卡空）

#### Scenario: SSE 超时后真正 OPEN 时通过 sse-recovered 自愈

- **WHEN** 上一次 `ensureSseReady()` 已超时放行 fetch、`sseRecoveryPending` 标志为 true
- **AND** EventSource 终于进入 `OPEN`（首次成功 / 重连成功）
- **THEN** `BrowserTransport` SHALL 给所有 `EventHandler` 调用 `handler('sse-recovered', {})` 一次
- **AND** 之后立即清空 `sseRecoveryPending` 标志，避免后续 onopen 再次重复 emit
- **AND** UI 层（典型 `Sidebar.svelte`）订阅 `sse-recovered` 事件后 SHALL 对当前选中 project 调一次 silent `loadSessions(projectId, true)` 兜底重拉

### Requirement: /api/events SSE 在 broadcast 容量打满时 SHALL 推送 sse_lagged sentinel

`/api/events` SSE handler 的 `BroadcastStream` 在 `events_tx` 容量打满 + 当前 receiver 跟不上速度时返回 `Err(BroadcastStreamRecvError::Lagged(skipped))`。原实现对 `Lagged` 走 `filter_map None` 静默吞掉，UI 永久看不到落地的 metadata patch（codex 二审 issue 2）。系统 SHALL 把 `Lagged` 转为一条 SSE `Event`，`data` 字段为 `'{"type":"sse_lagged"}'` 字面量字符串；stream SHALL 继续从最新 PushEvent 接收，**不**退出 stream。

UI 层 `BrowserTransport` 收到 `sse_lagged` event SHALL 映射到 `sse-lagged` event name 派发给所有 handler；订阅方（典型 `Sidebar.svelte`）SHALL 与 `sse-recovered` 共享同一 silent refresh handler 重拉一轮 metadata。

实现上 `EVENT_BRIDGE_CAPACITY` SHALL ≥ 1024（`src-tauri/src/server_mode.rs` 与 `crates/cdt-cli/src/main.rs` 同步），给默认 `pageSize=50` × 多 project 切换 × 多 SSE subscriber 留约 20× headroom；这是性能优化、不是行为契约——`sse_lagged` sentinel 是行为兜底，capacity 提升是降低触发频率。

#### Scenario: 容量打满时 SSE handler 推送 sse_lagged sentinel

- **WHEN** `events_tx` 的 broadcast channel capacity 被打满 + 当前 SSE receiver 落后导致 `BroadcastStream` 产出 `Err(Lagged(N))`
- **THEN** SSE handler SHALL 推送一条 SSE event，`data` 字段字符串等于 `'{"type":"sse_lagged"}'`
- **AND** SSE stream SHALL 继续从最新 PushEvent 接收（**不**得退出 stream / **不**得静默丢弃）
- **AND** 后续真正的 `PushEvent::SessionMetadataUpdate` 等事件 SHALL 正常推送到 client

#### Scenario: 浏览器 client 收到 sse_lagged 时触发 silent refresh

- **WHEN** 浏览器 client 收到 SSE event `{"type":"sse_lagged"}`
- **THEN** `BrowserTransport` SHALL 把它映射到 `sse-lagged` event name 派发给所有 handler
- **AND** 订阅方 SHALL 对当前选中 project 调 `loadSessions(projectId, true)` silent refresh 兜底重拉

