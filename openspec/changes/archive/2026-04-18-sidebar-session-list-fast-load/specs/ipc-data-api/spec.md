## MODIFIED Requirements

### Requirement: Expose project and session queries

The system SHALL expose data queries for projects and sessions over a request/response IPC channel set, including at minimum: list projects, list sessions for a project (with pagination), get session detail, get session metrics, get waterfall data, and get subagent detail.

`get_session_detail` SHALL 在返回 session 详情时集成 subagent 解析：从同 project 的其他 session 中扫描候选 subagent，调用 `resolve_subagents` 填充 `AIChunk.subagents` 字段。若扫描失败或无候选，`subagents` SHALL 为空数组（不报错）。

`list_sessions` 返回的每个 `SessionSummary` MUST 携带 `sessionId` / `projectId` / `timestamp` 的真实值（可直接从目录扫描得出），但 `title` / `messageCount` / `isOngoing` SHALL 允许为占位值（`null` / `0` / `false`）——这些元数据字段的真实值由后端异步扫描后通过 `session-metadata-update` push event 逐条推送（见本 spec "Emit session metadata updates" requirement）。`get_session_detail` 返回的 `SessionDetail.isOngoing` 仍 MUST 为同步计算后的真实值（因为 detail 已在调用链内完成全文件解析）。

序列化 SHALL 使用 camelCase（`isOngoing`）。

HTTP API 路径（`GET /projects/:id/sessions`）SHALL 保留同步完整返回语义（不适用骨架化）——因 HTTP 无 push 通道；IPC 路径适用骨架化。

#### Scenario: List projects

- **WHEN** a caller invokes the list-projects operation
- **THEN** the response SHALL contain all discovered projects with their id, decoded path, display name, and session count

#### Scenario: Paginated session list

- **WHEN** a caller invokes the paginated sessions operation with a page size and cursor
- **THEN** the response SHALL contain at most page-size entries and a next-cursor token if more exist

#### Scenario: Get session detail

- **WHEN** a caller requests detail for a session id
- **THEN** the response SHALL contain the built chunks, metrics, and metadata for that session

#### Scenario: Get session detail with subagent resolution

- **WHEN** a caller requests detail for a session that contains Task tool calls
- **THEN** the response SHALL include resolved subagent processes in the corresponding `AIChunk.subagents` fields, matched via the three-phase resolution algorithm (result-based → description-based → positional)

#### Scenario: Get session detail when no subagent candidates exist

- **WHEN** a caller requests detail for a session whose project has no other sessions or no matching candidates
- **THEN** `AIChunk.subagents` SHALL be empty arrays and no error SHALL be returned

#### Scenario: list_sessions IPC 返回骨架元数据

- **WHEN** a caller invokes IPC `list_sessions(projectId)` for a project with N sessions
- **THEN** the response SHALL return within ~200ms carrying N `SessionSummary` entries, each with real `sessionId` / `projectId` / `timestamp` but `title = null` / `messageCount = 0` / `isOngoing = false`
- **AND** 后端 SHALL 在返回后 spawn 并发元数据扫描任务，每扫完一个 session 向订阅者广播一条 `SessionMetadataUpdate`

#### Scenario: 骨架返回后元数据通过 event 推送

- **WHEN** IPC `list_sessions(projectId)` 返回骨架后
- **AND** 后端扫描某个 session 文件完成（得出 title / messageCount / isOngoing）
- **THEN** 订阅者 SHALL 收到一条 `SessionMetadataUpdate { projectId, sessionId, title, messageCount, isOngoing }`；扫描全部完成前允许收到任意顺序、任意数量（0 到 N）的 updates

#### Scenario: SessionDetail carries isOngoing

- **WHEN** a caller invokes `get_session_detail` on a session id
- **THEN** the resulting `SessionDetail.isOngoing` SHALL be the true value computed from the full-file scan (not a placeholder)

#### Scenario: HTTP list_sessions 保留同步完整返回

- **WHEN** a caller invokes HTTP `GET /projects/:id/sessions`
- **THEN** the response SHALL contain `SessionSummary` entries with real `title` / `messageCount` / `isOngoing` values (同步扫描后返回)，不走骨架化路径

## ADDED Requirements

### Requirement: Emit session metadata updates

The system SHALL expose an in-process subscription mechanism on `LocalDataApi` named `subscribe_session_metadata()` that yields a `broadcast::Receiver<SessionMetadataUpdate>`. `SessionMetadataUpdate` SHALL carry `projectId` / `sessionId` / `title` / `messageCount` / `isOngoing` (camelCase when serialized). Tauri host SHALL bridge this subscription to the webview by emitting `session-metadata-update` frontend events.

并发度 SHALL 被限流（固定上限 8），避免 50+ 文件同时打开；每次 `list_sessions(projectId)` 触发新扫描前 SHALL 取消上一轮未完成的扫描（同一 `projectId` 维度），避免事件串扰。

#### Scenario: 订阅接收元数据更新

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 随后调用 `list_sessions("projectA")`，项目下有 3 个 session
- **THEN** receiver SHALL 最多在扫描完成后收到 3 条 `SessionMetadataUpdate`，每条携带对应 sessionId 的真实 title/messageCount/isOngoing

#### Scenario: Tauri host emit session-metadata-update

- **WHEN** Tauri host 在 `setup` 调用 `subscribe_session_metadata()` 并在后台 task 内订阅
- **AND** 后端产出 `SessionMetadataUpdate { project_id: "p", session_id: "s", title: Some("T"), message_count: 12, is_ongoing: false }`
- **THEN** webview SHALL 通过 `listen("session-metadata-update", ...)` 收到 payload `{ projectId: "p", sessionId: "s", title: "T", messageCount: 12, isOngoing: false }`（camelCase）

#### Scenario: 同 projectId 新扫描取消旧扫描

- **WHEN** `list_sessions("projectA")` 正在扫描中（后台有未完成任务）
- **AND** 调用方再次调用 `list_sessions("projectA")`（file-change silent refresh 场景）
- **THEN** 旧扫描任务 SHALL 被 abort，未完成的 session 元数据 SHALL NOT 再推送；新扫描 SHALL 从头开始

#### Scenario: 并发度限制

- **WHEN** 扫描任务在并发处理某页 50 个 session 文件
- **THEN** 同一时刻打开的 JSONL 文件句柄数 SHALL 不超过 8（通过 `tokio::sync::Semaphore` 或等价机制限流）

#### Scenario: 无 watcher 构造器下 subscribe 安全

- **WHEN** `LocalDataApi` 通过不带 watcher 的构造器实例化（集成测试路径）
- **AND** 调用方 `subscribe_session_metadata()`
- **THEN** 返回有效 `broadcast::Receiver`；`list_sessions` 仍能正常推送（broadcast 不依赖 watcher）
