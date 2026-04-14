# ipc-data-api Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
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

### Requirement: Expose config and notification operations

The system SHALL expose config read/update operations and notification list/mark-read operations over IPC, matching the behavior described in `configuration-management` and `notification-triggers`.

#### Scenario: Update config field via IPC
- **WHEN** a caller invokes the config update operation
- **THEN** the change SHALL be persisted and the response SHALL contain the new configuration

### Requirement: Expose SSH and context operations

The system SHALL expose operations to list contexts, switch active context, connect/disconnect/test SSH, get SSH status, and resolve SSH host aliases.

#### Scenario: Resolve ssh host alias via IPC
- **WHEN** a caller requests to resolve an alias
- **THEN** the response SHALL contain the resolved hostname, port, user, and identity file path (or a clear error if not found)

### Requirement: Emit push events for file changes and notifications

The system SHALL push events from main to renderer for: session file changes, todo file changes, new notifications, SSH status changes, and updater progress.

#### Scenario: New notification while renderer is subscribed
- **WHEN** a new notification is emitted while the renderer has subscribed to notification events
- **THEN** the renderer SHALL receive a push event carrying the notification payload within the debounce window

### Requirement: Validate inputs and return structured errors

The system SHALL validate IPC input parameters and return structured errors with an error code enum and a human-readable message, rather than propagating raw exceptions.

#### Scenario: Missing required field
- **WHEN** a caller invokes an operation missing a required field
- **THEN** the response SHALL contain a validation error with code `validation_error` and a description of the missing field

#### Scenario: Resource not found
- **WHEN** a caller requests a session or project that does not exist
- **THEN** the response SHALL contain an error with code `not_found` and the resource identifier

### Requirement: Expose file and path validation operations

The system SHALL expose operations to validate filesystem paths and to validate `@mention` references against a session's cwd.

#### Scenario: Validate a path that doesn't exist
- **WHEN** a caller validates a nonexistent path
- **THEN** the response SHALL indicate not-found without throwing

### Requirement: Expose auxiliary read operations

The system SHALL expose auxiliary data operations used by the renderer beyond the core session and project queries: read agent configs (subagent definitions), batch get sessions by ids, get session chat groups, get repository groups, get worktree sessions, read CLAUDE.md files (global/project/directory scopes), read a specific directory's CLAUDE.md, and read a single `@mention`-resolved file.

#### Scenario: Batch get sessions by ids
- **WHEN** a caller invokes the batch get-sessions-by-ids operation with an array of session ids
- **THEN** the response SHALL contain one session entry per requested id, with missing ids returned as not-found placeholders

#### Scenario: Read three-scope CLAUDE.md
- **WHEN** a caller invokes the read-claude-md-files operation for a given project
- **THEN** the response SHALL include entries for the global, project, and (when applicable) directory scopes

#### Scenario: Get worktree sessions
- **WHEN** a caller invokes the get-worktree-sessions operation for a repository group
- **THEN** the response SHALL list sessions belonging to every worktree in that group

#### Scenario: Read agent configs
- **WHEN** a caller invokes the read-agent-configs operation
- **THEN** the response SHALL contain the parsed subagent definitions from `~/.claude/agents/` and project-scoped agent directories

### Requirement: Expose search via Tauri IPC command

The system SHALL expose a `search_sessions` Tauri command that accepts project_id and query parameters, delegates to `LocalDataApi.search()`, and returns the search results as JSON.

#### Scenario: Tauri search command invocation
- **WHEN** the frontend invokes `search_sessions` with a project_id and query
- **THEN** the command SHALL return search results matching the `session-search` capability format

#### Scenario: Tauri search command with nonexistent project
- **WHEN** the frontend invokes `search_sessions` with an invalid project_id
- **THEN** the command SHALL return an error string describing the issue

