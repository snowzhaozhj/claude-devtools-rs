## MODIFIED Requirements

### Requirement: Expose project and session queries

The system SHALL expose data queries for projects and sessions over a request/response IPC channel set, including at minimum: list projects, list sessions for a project (with pagination), get session detail, get session metrics, get waterfall data, and get subagent detail.

`get_session_detail` SHALL 在返回 session 详情时集成 subagent 解析：从同 project 的其他 session 中扫描候选 subagent，调用 `resolve_subagents` 填充 `AIChunk.subagents` 字段。若扫描失败或无候选，`subagents` SHALL 为空数组（不报错）。

`list_sessions` 返回的每个 `SessionSummary` 与 `get_session_detail` 返回
的 `SessionDetail` MUST 携带 `isOngoing: boolean` 字段。字段值来自
`cdt_analyze::check_messages_ongoing(messages)`（session-parsing 全文件
扫描后调用），true 表示会话中最后一个 ending event 之后仍有 AI 活动，
尚未显式结束。序列化 SHALL 使用 camelCase（`isOngoing`）。

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

#### Scenario: SessionSummary carries isOngoing
- **WHEN** a caller invokes `list_sessions` for a project containing a session whose last activity is an assistant tool_use with no following ending event
- **THEN** the resulting `SessionSummary.isOngoing` SHALL be `true`; a session ending with a user `text_output` or interrupt marker SHALL have `isOngoing = false`

#### Scenario: SessionDetail carries isOngoing
- **WHEN** a caller invokes `get_session_detail` on the same session id
- **THEN** the resulting `SessionDetail.isOngoing` SHALL agree with the matching `SessionSummary.isOngoing`
