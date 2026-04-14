## MODIFIED Requirements

### Requirement: Expose project and session queries

The system SHALL expose data queries for projects and sessions over a request/response IPC channel set, including at minimum: list projects, list sessions for a project (with pagination), get session detail, get session metrics, get waterfall data, and get subagent detail.

`get_session_detail` SHALL 在返回 session 详情时集成 subagent 解析：从同 project 的其他 session 中扫描候选 subagent，调用 `resolve_subagents` 填充 `AIChunk.subagents` 字段。若扫描失败或无候选，`subagents` SHALL 为空数组（不报错）。

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

### Requirement: Expose search queries

The system SHALL expose search operations: search within one session, search across one project, and search across all projects. `search` SHALL 委托给 `SessionSearcher`（来自 `session-search` capability）执行真实的全文搜索，而非返回空结果。

#### Scenario: Search all projects via IPC
- **WHEN** a caller invokes the global search operation with a query
- **THEN** the response SHALL contain per-project match groups consistent with the `session-search` capability

#### Scenario: Search returns real results from SessionSearcher
- **WHEN** a caller invokes the search operation with a query that matches session content
- **THEN** the response SHALL contain search hits with `message_uuid`、`offset`、`preview` 和 `message_type` 字段

#### Scenario: Search with empty query
- **WHEN** a caller invokes the search operation with an empty query string
- **THEN** the response SHALL return an empty results array without error

## ADDED Requirements

### Requirement: Expose search via Tauri IPC command

The system SHALL expose a `search_sessions` Tauri command that accepts project_id and query parameters, delegates to `LocalDataApi.search()`, and returns the search results as JSON.

#### Scenario: Tauri search command invocation
- **WHEN** the frontend invokes `search_sessions` with a project_id and query
- **THEN** the command SHALL return search results matching the `session-search` capability format

#### Scenario: Tauri search command with nonexistent project
- **WHEN** the frontend invokes `search_sessions` with an invalid project_id
- **THEN** the command SHALL return an error string describing the issue
