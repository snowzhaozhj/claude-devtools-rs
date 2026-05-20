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

`GET /api/projects/{projectId}/sessions` 响应行为 SHALL 按 `cursor` query 参数分叉：

- **首页路径（无 `cursor` query 参数 / `cursor=null`）**：响应**前** `min(page_size, EAGER_FIRST_PAGE_LIMIT = 20)` 条 `SessionSummary` SHALL 含真实的 `sessionId` / `projectId` / `timestamp` / `title` / `messageCount` / `isOngoing` / `gitBranch`（`title=null` 仅当 jsonl 真无 user message——为真值不是占位；个别条目 metadata 解析超时 / 失败时降级为占位）。`page_size > EAGER_FIRST_PAGE_LIMIT` 时**剩余** `page_size - 20` 条 SHALL 为骨架占位（`title=null` / `messageCount=0` / `isOngoing=false` / `gitBranch=null`），SHALL 通过 `/api/events` SSE 的 `session_metadata_update` 事件异步推送真实值。axum handler SHALL 调 `DataApi::list_sessions(project_id, pagination)`（**不**得调 `list_sessions_sync`），由 trait 实现内部按 `pagination.cursor.is_none()` 进入 eager 路径。eager 同步等到的前 20 条 SHALL NOT 触发 `/api/events` SSE 上的 `session_metadata_update` event；超时 / 失败条 deferred retry + remainder scan emit 的 update SHALL 通过 SSE 推送（与翻页路径同语义）。
- **翻页路径（`cursor=<value>` query 参数）**：响应每条 `SessionSummary` 的 `sessionId` / `projectId` / `timestamp` SHALL 为真实值，`title` / `messageCount` / `isOngoing` / `gitBranch` SHALL 允许为占位值。`try_lookup_cached_metadata` lookup-fast-path 命中条 SHALL 在响应中直接 inline 填回真值（zero 后续 SSE emit）；未命中条的真实 metadata SHALL 通过 `/api/events` SSE 的 `session_metadata_update` 事件异步推送（详见本 spec §"Push events via Server-Sent Events"）。

**禁止** axum handler 调 `DataApi::list_sessions_sync(...)`——该方法保留作为 trait fallback，但 HTTP 路由不再使用。

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

#### Scenario: GET paginated sessions first page eager-await first 20 entries

- **WHEN** 客户端发起 `GET /api/projects/{projectId}/sessions?pageSize=20`（或省略 `pageSize` 走默认；无 `cursor` query 参数）
- **THEN** axum handler `cdt-api::http::routes::list_sessions` SHALL 调 `DataApi::list_sessions(project_id, pagination)` 且 `pagination.cursor` 为 `None`
- **AND** 响应 body SHALL 是 `PaginatedResponse<SessionSummary>`：前 20 条 `SessionSummary` 的所有字段 SHALL 为真值（`title` 可为 `null` 当 jsonl 真无 user message——这是真值不是占位；个别条目 metadata 解析超时 / 失败时降级为占位，整页响应 SHALL 仍为 `200`）
- **AND** 该次响应 SHALL NOT 对前 20 条 eager 同步等到的 sessionId 触发 `/api/events` SSE `session_metadata_update` event；超时 / 失败条的 deferred retry 仍可能 emit（兜底）

#### Scenario: GET paginated sessions first page pageSize > 20 splits eager and skeleton

- **WHEN** 客户端发起 `GET /api/projects/{projectId}/sessions?pageSize=50`（无 `cursor` query 参数）
- **THEN** 响应 body 中前 20 条 SHALL 含真值 metadata；剩余 30 条 SHALL 为骨架占位
- **AND** 后台 `scan_metadata_for_page` 任务对剩余 30 条的扫描产物 SHALL 通过 `/api/events` SSE `session_metadata_update` 事件推送真实值（与翻页路径同语义）
- **AND** 浏览器 client `transport.ts::BrowserTransport` SHALL 按既有归一化路径转交 `session-metadata-update` 事件给 listener

#### Scenario: GET paginated sessions subsequent pages return skeleton with cache-hit inline real values
- **WHEN** 客户端发起 `GET /api/projects/{projectId}/sessions?pageSize=N&cursor=C`（`cursor` query 参数非空）
- **AND** 项目下某些 session 的 `MetadataCache` 已命中（`FileSignature` 等价），其余 cache miss
- **THEN** axum handler SHALL 调 `DataApi::list_sessions(project_id, pagination)` 且 `pagination.cursor` 为 `Some(C)`（**不**得调 `list_sessions_sync`）
- **AND** 响应 body SHALL 是 `PaginatedResponse<SessionSummary>`：cache 命中条 SHALL inline 携带真实 `title` / `messageCount` / `isOngoing` / `gitBranch`；cache miss 条 SHALL 为占位值（`title=null` / `messageCount=0` / `isOngoing=false` / `gitBranch=null`）
- **AND** cache miss 条 SHALL 在后续 backend 后台扫描完成后通过 `/api/events` SSE `session_metadata_update` 事件推送真实值

#### Scenario: POST batch session summaries
- **WHEN** 客户端发起 `POST /api/projects/{projectId}/session-summaries/batch`，body 为该项目下若干 session id 列表
- **THEN** 响应 SHALL 是与 IPC `get_session_summaries_by_ids` 同形的 summary 数组，顺序与请求 id 顺序一致

### Requirement: 浏览器 client SHALL 在首次 list_sessions 前订阅 SSE

由于 `GET /api/projects/{projectId}/sessions` **翻页路径**（cursor=Some）改为骨架 + SSE 异步 patch 语义，浏览器 client SHALL 在发起翻页 `GET /api/projects/{projectId}/sessions?cursor=...` 之前确保 `EventSource('/api/events')` 已进入 `OPEN` 状态——否则 backend 在 GET response 返回后立即 emit 的 `session_metadata_update` 事件会在 EventSource 订阅前发生，导致 metadata patch 永久丢失。

**首页路径**（`cursor=null` 或省略）由后端 eager 同步等真值 + 前 20 条不 emit broadcast，浏览器 client SHALL NOT **await** `ensureSseReady()` —— 该 gate 对首页前 20 条无意义且会拖慢首屏（最多 1000ms 闸门）。但因为首页路径仍可能产生 deferred retry / `pageSize > 20` remainder scan 的 broadcast emit，浏览器 client SHALL **fire-and-forget** 调 `void this.ensureSseReady()` —— 异步触发 EventSource 建立 / scheduleReconnect 不阻塞 fetch；后台 SSE 进入 OPEN 后通过既有 `sseRecoveryPending` → onopen → emit `sse-recovered` 链路触发 UI silent refresh 重拉，兜底任何在 SSE OPEN 前发生的 broadcast emit。

实现 SHALL 在 `ui/src/lib/transport.ts::BrowserTransport.invokeHttp(cmd, args)` 内按 `cmd` + `args.cursor` 双判断：

- `cmd === 'list_sessions' && (!args.cursor || args.cursor === null)`：**首页路径**，**不 await** `ensureSseReady()`，但 SHALL `void this.ensureSseReady()` 异步触发 SSE 订阅（fire-and-forget 后台跑）
- `cmd === 'list_sessions' && args.cursor`：**翻页路径**，await `ensureSseReady()` 1000ms gate
- `cmd === 'list_repository_groups' / 'list_worktree_sessions'`：保留 `ensureSseReady()` await（这些命令内部可能触发翻页扫描走 broadcast）
- 其它命令：不变

`ensureSseReady()` 行为不变：
- 若 `source.readyState === EventSource.OPEN` → 立返 `Promise.resolve()`
- 若 `source.readyState === EventSource.CONNECTING` → await 单次 `onopen` 或最大 1000 ms 超时
- 若 `source.readyState === EventSource.CLOSED` → 触发 `scheduleReconnect(source)` 后 await 新 source `onopen` 或最大 1000 ms 超时
- 超时后 SHALL 放行（不抛错）让首次 fetch 继续；丢失的 metadata 由后续 file-change silent refresh 兜底

Tauri runtime（`TauriTransport`）SHALL NOT 受此 Requirement 影响——Tauri IPC event listener 由 `@tauri-apps/api` 同步注册，不存在 EventSource 异步 OPEN 等待问题。

#### Scenario: BrowserTransport 翻页路径等待 SSE OPEN 后才发 fetch

- **WHEN** 浏览器 runtime 调用 `invoke('list_sessions', { projectId: "p", pageSize: 20, cursor: "20" })`
- **AND** `EventSource('/api/events')` 处于 `CONNECTING`
- **THEN** `BrowserTransport.invokeHttp` SHALL await `ensureSseReady()` 直至 source 进入 `OPEN` 或 1000 ms 超时
- **AND** 仅在 await 返回后 SHALL 发起 `fetch('/api/projects/p/sessions?pageSize=20&cursor=20')`

#### Scenario: BrowserTransport 首页路径不 await 但 fire-and-forget 触发 SSE 订阅

- **WHEN** 浏览器 runtime 调用 `invoke('list_sessions', { projectId: "p", pageSize: 20 })`（不带 cursor）或 `invoke('list_sessions', { projectId: "p", pageSize: 20, cursor: null })`
- **THEN** `BrowserTransport.invokeHttp` SHALL NOT **await** `ensureSseReady()`——直接发 `fetch('/api/projects/p/sessions?pageSize=20')`
- **AND** SHALL 同时 fire-and-forget 调 `void this.ensureSseReady()`（异步触发 EventSource 建立 / scheduleReconnect 在后台跑），不阻塞 fetch
- **AND** 即使 `EventSource('/api/events')` 处于 `CONNECTING` / `CLOSED`，首页 fetch 等待时间 SHALL NOT 受 SSE-ready 1000 ms gate 影响（首屏延迟仅由后端 eager 同步等真值的实际成本决定）

#### Scenario: 首页 fast-open 路径调用时 SSE 非 OPEN 即标记 sseRecoveryPending（无论后续 OPEN 是 timeout 内还是 timeout 后）

- **WHEN** 浏览器调用 `invoke('list_sessions', { ..., cursor: null })` 时 `EventSource` 处于 `CONNECTING` 或 `CLOSED`（即非 `OPEN`）
- **THEN** `BrowserTransport.invokeHttp` SHALL **无条件**设 `sseRecoveryPending = true`（不仅是 timeout 路径才设；codex v3 复审 issue 1 修复 fast-open 竞态）
- **AND** fire-and-forget 调 `void this.ensureSseReady()` 异步触发 SSE 订阅
- **AND** 后续 EventSource 真正进入 `OPEN`（无论是 1000ms 内成功还是 timeout 后重连成功），既有 onopen handler 检查 `sseRecoveryPending` 为 true，SHALL emit 一次 `sse-recovered` pseudo-event 给所有 handler，并清空 `sseRecoveryPending`

#### Scenario: 首页 deferred retry / remainder broadcast 通过 sse-recovered 兜底（限定首页范围）

- **WHEN** 浏览器调用 `invoke('list_sessions', { ..., cursor: null, pageSize: N })` 时 `EventSource` 仍处于 `CONNECTING` / `CLOSED`，fetch 立即发出（不阻塞）；fire-and-forget 的 `ensureSseReady()` 后台跑；`sseRecoveryPending=true` 已设
- **AND** 后端在 IPC return 后 spawn 的 deferred retry（首页超时 / 失败条）通过 broadcast emit `SessionMetadataUpdate`，但 EventSource 在 OPEN 之前发生
- **THEN** EventSource 真正进入 `OPEN` 时（fast-open 路径或 timeout 重连路径），`BrowserTransport` SHALL emit 一次 `sse-recovered` 给所有 handler
- **AND** UI 层（`Sidebar.svelte`）订阅 `sse-recovered` 事件后 SHALL 对当前选中 project 调一次 silent `loadSessions(projectId, true)` —— 走 eager 首页路径重新拉取 `SESSION_PAGE_SIZE = 20` 条，自然兜底任何错过的 deferred retry broadcast emit（response 真值通过 `mergeRecoveryResponse` 覆盖 prev stale）
- **AND** **恢复范围限定为首页前 SESSION_PAGE_SIZE 条 + deferred retry 失败条**（codex v3 复审 issue 6）：当 `pageSize > SESSION_PAGE_SIZE` 时（典型 HTTP CLI 客户端用大 pageSize），21+ 条 remainder 的 broadcast emit 失败 SHALL NOT 通过 sse-recovered silent refresh 兜底——客户端应自负确保 SSE 已订阅再发大 pageSize 的 list_sessions（Sidebar 默认 SESSION_PAGE_SIZE=20，不存在 remainder，全列表都被恢复）

#### Scenario: SSE 已 OPEN 时翻页路径不阻塞 list_sessions

- **WHEN** 浏览器调用 `invoke('list_sessions', { ..., cursor: "20" })` 且 `source.readyState === EventSource.OPEN`
- **THEN** `ensureSseReady()` SHALL 立返（不引入额外 latency）

#### Scenario: list_repository_groups / list_worktree_sessions 仍 await SSE-ready

- **WHEN** 浏览器调用 `invoke('list_repository_groups', ...)` 或 `invoke('list_worktree_sessions', ...)`
- **THEN** `BrowserTransport.invokeHttp` SHALL await `ensureSseReady()`（因为这些命令内部可能触发翻页扫描走 broadcast，仍需要 SSE 已订阅）

#### Scenario: SSE 1000 ms 超时后仍放行（翻页路径）

- **WHEN** 浏览器调用 `invoke('list_sessions', { ..., cursor: "20" })` 且 `EventSource` 由于网络问题 1000 ms 内未进入 `OPEN`
- **THEN** `ensureSseReady()` SHALL resolve（不抛错）
- **AND** 后续 fetch SHALL 正常发起
- **AND** `BrowserTransport` SHALL 记录 timeout 状态，使得后续 EventSource 真正进入 `OPEN` 时（自然成功 / scheduleReconnect 重连成功）SHALL 给所有已注册 handler emit 一次 `sse-recovered` pseudo-event，让 UI 触发 silent refresh 重拉一轮 metadata patch；**不**得仅依赖偶发的 file-change 兜底
