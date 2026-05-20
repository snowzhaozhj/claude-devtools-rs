## MODIFIED Requirements

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

## ADDED Requirements

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
