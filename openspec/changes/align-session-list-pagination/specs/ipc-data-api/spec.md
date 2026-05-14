## ADDED Requirements

### Requirement: List sessions uses project-scoped light pagination

`list_sessions(projectId, pagination)` SHALL act as a project-scoped cursor pagination API for session list UI. The synchronous response MUST only require lightweight fields that can be obtained without parsing session contents: `sessionId`, `projectId`, `timestamp`, and pagination metadata. Deep metadata fields (`title`, `messageCount`, `isOngoing`, `gitBranch`) SHALL be allowed to remain placeholders in the synchronous response and be filled later through `session-metadata-update`.

`list_sessions` SHALL NOT require callers to compute or consume an exact total count for the session list first page. Pagination continuation MUST be driven by `nextCursor` / equivalent `hasMore` semantics. If the response type keeps a `total` field for compatibility, callers SHALL treat it as informational and MUST NOT rely on it being a complete project count unless a future dedicated count API states so.

#### Scenario: first page returns light summaries

- **WHEN** caller invokes `list_sessions("projectA", { pageSize: 20, cursor: null })`
- **THEN** response SHALL contain at most 20 `SessionSummary` items for `projectA`
- **AND** each item SHALL contain real `sessionId`, `projectId`, and `timestamp`
- **AND** each item MAY contain placeholder `title = null`, `messageCount = 0`, `isOngoing = false`, and `gitBranch = null`

#### Scenario: continuation uses cursor not total

- **WHEN** first `list_sessions("projectA", { pageSize: 20, cursor: null })` response contains `nextCursor`
- **THEN** caller SHALL request the next page with that cursor
- **AND** caller SHALL NOT need an exact total count to continue pagination

#### Scenario: pageSize zero is rejected

- **WHEN** caller invokes `list_sessions("projectA", { pageSize: 0, cursor: null })`
- **THEN** API SHALL return a validation error instead of silently clamping the page size

### Requirement: Fetch session summaries by id

The API SHALL expose a narrow capability to fetch light `SessionSummary` records for a bounded set of `sessionId` values within a project. This capability exists for pinned/hidden session reconciliation and MUST NOT be used as a replacement for full-history listing.

The response SHALL include summaries for ids that exist in the requested project and SHALL omit ids that do not exist or belong to another project. Returned summaries SHALL follow the same light metadata rules as `list_sessions`: deep metadata MAY be placeholder and MAY be filled through `session-metadata-update` when implementation chooses to scan those ids.

#### Scenario: pinned session outside first page can be fetched

- **WHEN** caller has pinned id `sid-old` that is not present in the first `list_sessions("projectA")` page
- **AND** caller invokes the by-id summary fetch for `projectA` with `["sid-old"]`
- **THEN** response SHALL include `sid-old` if the session exists under `projectA`

#### Scenario: foreign project id is omitted

- **WHEN** caller invokes the by-id summary fetch for `projectA` with an id that exists only under `projectB`
- **THEN** response SHALL NOT include that session summary

## MODIFIED Requirements

### Requirement: Emit session metadata updates

系统 SHALL 在 `LocalDataApi` 上暴露 in-process 订阅机制 `subscribe_session_metadata()`，返回一个 `broadcast::Receiver<SessionMetadataUpdate>`。`SessionMetadataUpdate` SHALL 携带 `projectId` / `sessionId` / `title` / `messageCount` / `isOngoing` / `gitBranch`（序列化时为 camelCase）。Tauri host SHALL 把该订阅桥接到 webview，向前端 emit `session-metadata-update` 事件。

并发度 SHALL 被限流（固定上限 8），避免 50+ 文件同时打开；每次 `list_sessions(projectId, pagination)` 触发新扫描前 SHALL 取消同一 `projectId` 维度上一轮未完成的扫描，避免事件串扰。扫描范围 SHALL 限定为本次 `list_sessions` 返回页中的 sessions；实现 MUST NOT 因为请求第一页而后台扫描完整项目历史。

#### Scenario: 订阅接收当前页元数据更新

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 随后调用 `list_sessions("projectA", { pageSize: 3, cursor: null })`，响应页包含 3 个 session
- **THEN** receiver SHALL 在扫描完成后**最多**收到 3 条 `SessionMetadataUpdate`，每条携带对应 sessionId 的真实 `title` / `messageCount` / `isOngoing` / `gitBranch`

#### Scenario: Tauri host emit session-metadata-update

- **WHEN** Tauri host 在 `setup` 调用 `subscribe_session_metadata()` 并在后台 task 内订阅
- **AND** 后端产出 `SessionMetadataUpdate { project_id: "p", session_id: "s", title: Some("T"), message_count: 12, is_ongoing: false, git_branch: Some("main") }`
- **THEN** webview SHALL 通过 `listen("session-metadata-update", ...)` 收到 payload `{ projectId: "p", sessionId: "s", title: "T", messageCount: 12, isOngoing: false, gitBranch: "main" }`（camelCase）

#### Scenario: 同 projectId 新扫描取消旧扫描

- **WHEN** `list_sessions("projectA", { pageSize: 20, cursor: null })` 正在扫描中（后台有未完成任务）
- **AND** 调用方再次调用 `list_sessions("projectA", { pageSize: 20, cursor: "next" })`
- **THEN** 旧扫描任务 SHALL 被 abort，未完成的 session 元数据 SHALL NOT 再被推送；新扫描 SHALL 只扫描新响应页中的 sessions

#### Scenario: 并发度限制

- **WHEN** 扫描任务在并发处理某页 50 个 session 文件
- **THEN** 同一时刻打开的 JSONL 文件句柄数 SHALL 不超过 8（通过 `tokio::sync::Semaphore` 或等价机制限流）

#### Scenario: 无 watcher 构造器下 subscribe 安全

- **WHEN** `LocalDataApi` 通过不带 watcher 的构造器实例化（集成测试路径）
- **AND** 调用方 `subscribe_session_metadata()`
- **THEN** 返回有效 `broadcast::Receiver`；`list_sessions` 仍能正常推送（broadcast 不依赖 watcher）