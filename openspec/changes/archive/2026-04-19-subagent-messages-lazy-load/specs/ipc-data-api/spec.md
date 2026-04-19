# ipc-data-api Spec Delta

## MODIFIED Requirements

### Requirement: Expose project and session queries

The system SHALL expose data queries for projects and sessions over a request/response IPC channel set, including at minimum: list projects, list sessions for a project (with pagination), get session detail, get session metrics, get waterfall data, and get subagent detail.

`get_session_detail` SHALL 在返回 session 详情时集成 subagent 解析：从同 project 的其他 session 中扫描候选 subagent，调用 `resolve_subagents` 填充 `AIChunk.subagents` 字段。若扫描失败或无候选，`subagents` SHALL 为空数组（不报错）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.subagents[i].messages` 数组 MUST 默认被裁剪为空 Vec，且 `messagesOmitted=true`** —— 用于把首屏 IPC payload 控制在原大小的 ~40%（subagent 嵌套 chunks 全文是大头，详见 `openspec/changes/subagent-messages-lazy-load/design.md` payload breakdown）。`Process.headerModel` / `Process.lastIsolatedTokens` / `Process.isShutdownOnly` 三个 derived 字段 MUST 在 `candidate_to_process` 阶段由 messages 预算后填充，让 SubagentCard header 不依赖完整 `messages` 也能正常渲染。回滚开关 `OMIT_SUBAGENT_MESSAGES: bool` 设 false 时 SHALL 退回完整 payload（messages 不裁剪、`messagesOmitted=false`）。

`list_sessions` 返回的每个 `SessionSummary` MUST 携带 `sessionId` / `projectId` / `timestamp` 的真实值（可直接从目录扫描得出），但 `title` / `messageCount` / `isOngoing` SHALL 允许为占位值（`null` / `0` / `false`）——这些元数据字段的真实值由后端异步扫描后通过 `session-metadata-update` push event 逐条推送（见本 spec "Emit session metadata updates" requirement）。`get_session_detail` 返回的 `SessionDetail.isOngoing` 仍 MUST 为同步计算后的真实值（因为 detail 已在调用链内完成全文件解析）。

序列化 SHALL 使用 camelCase（`isOngoing`、`messagesOmitted`、`headerModel`、`lastIsolatedTokens`、`isShutdownOnly`）。

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

#### Scenario: Subagent messages omitted by default

- **WHEN** `get_session_detail` 返回的 `SessionDetail` 含至少一个 `AIChunk.subagents[i]`
- **THEN** 该 subagent 的 `messages` 数组 SHALL 为空 `[]`，`messagesOmitted` SHALL 为 `true`
- **AND** `headerModel` / `lastIsolatedTokens` / `isShutdownOnly` SHALL 为后端预算后的真实值

#### Scenario: 回滚开关恢复完整 payload

- **WHEN** `OMIT_SUBAGENT_MESSAGES: bool = false`
- **THEN** `get_session_detail` 返回的 subagent `messages` SHALL 携带完整 chunks 流，`messagesOmitted` SHALL 为 `false`

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

### Requirement: Lazy load subagent trace

新 IPC `get_subagent_trace(parentSessionId, subagentSessionId)` MUST 返回该 subagent 的完整 chunks 流，用于 SubagentCard 展开时按需拉取被 `messagesOmitted` 裁剪的 trace 数据。后端 SHALL 在父 session 同 project 下查找 `<parentSessionId>/subagents/agent-<subagentSessionId>.jsonl`（新结构）或旧结构兼容路径，`parse_file` + `build_chunks` 后返回 `Vec<Chunk>`。

#### Scenario: 拉取存在的 subagent trace

- **WHEN** caller 调用 `get_subagent_trace("parent-uuid", "sub-uuid")` 且对应 subagent jsonl 存在
- **THEN** 响应 SHALL 含完整的 `Vec<Chunk>`（与未裁剪时 `Process.messages` 内容一致）

#### Scenario: subagent jsonl 不存在

- **WHEN** caller 调用 `get_subagent_trace` 但目标 jsonl 不存在
- **THEN** 响应 SHALL 为空 `[]`，不报错（与"不存在"等价于"无 trace"——caller UI 显示空 trace 即可）

#### Scenario: 嵌套 subagent 各自独立拉取

- **WHEN** SubagentCard A 展开后含嵌套 SubagentCard B；用户展开 B
- **THEN** 前端 SHALL 用 B 的 sessionId 单独调 `get_subagent_trace(rootSessionId, B.sessionId)`，不复用 A 的结果
