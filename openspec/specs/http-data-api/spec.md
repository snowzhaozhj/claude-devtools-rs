# http-data-api Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Serve projects and sessions over HTTP under /api prefix

The system SHALL expose HTTP endpoints under the `/api` prefix that return the same data shapes as the IPC data API for: list projects, get project detail, get project repository, list sessions (including paginated and by-ids variants), get session detail, get session chunks, get session metrics, get waterfall data, and get subagent detail.

#### Scenario: GET list of projects
- **WHEN** a client issues `GET /api/projects`
- **THEN** the response SHALL be JSON containing the same project list as the IPC list-projects operation

#### Scenario: GET session detail
- **WHEN** a client issues `GET /api/sessions/:id`
- **THEN** the response SHALL contain the chunks, metrics, and metadata for that session in the same shape as IPC

#### Scenario: GET paginated sessions for a project
- **WHEN** a client issues `GET /api/projects/:projectId/sessions-paginated?pageSize=N&cursor=C`
- **THEN** the response SHALL match the shape of the IPC paginated sessions response

### Requirement: Serve search endpoints

The system SHALL expose search endpoints under `/api` corresponding to the `session-search` capability, accepting query bodies via POST and returning ranked results.

#### Scenario: POST session search
- **WHEN** a client issues `POST /api/search/sessions` with a JSON body containing query, project id, and optional session id
- **THEN** the response SHALL match the shape of the equivalent IPC search response

### Requirement: Serve auxiliary, subagent, utility, and validation endpoints

The system SHALL expose HTTP endpoints mirroring every IPC auxiliary operation listed in `ipc-data-api`, including subagent detail/trace, repository groups, worktree sessions, CLAUDE.md reads, agent configs, path/mention validation, utility shell operations, SSH, and updater.

#### Scenario: GET subagent detail
- **WHEN** a client issues `GET /api/subagents/:id/detail`
- **THEN** the response SHALL contain the subagent's chunks, metrics, and spawning context

#### Scenario: POST path validation
- **WHEN** a client issues the path validation request with a filesystem path
- **THEN** the response SHALL indicate whether the path exists and whether it is within an allowed root

### Requirement: Serve config and notification endpoints

The system SHALL expose HTTP endpoints to read and update configuration and to list and mark notifications as read, with the same semantics as the IPC data API.

#### Scenario: PATCH config field
- **WHEN** a client issues a config update request
- **THEN** the response SHALL reflect the new configuration and the change SHALL be persisted

### Requirement: Push events via Server-Sent Events

The system SHALL expose a Server-Sent Events endpoint that delivers the same event stream as the IPC push channel: file-change, todo-change, new-notification, ssh-status, and updater events.

#### Scenario: SSE client subscribes and receives file change
- **WHEN** an SSE client is connected and a session file is modified
- **THEN** the client SHALL receive a `file-change` event carrying the project id and session id within the debounce window

#### Scenario: Multiple concurrent SSE clients
- **WHEN** three SSE clients are connected and one notification is emitted
- **THEN** each client SHALL receive the notification event exactly once

### Requirement: Return safe defaults on lookup failures (current baseline)

The system SHALL return structured error responses for lookup failures: `404` with `{"code":"not_found","message":"..."}` for missing resources, `400` with `{"code":"validation_error","message":"..."}` for invalid input. This is an intentional improvement over the TS baseline which returns `200` with `null`/empty arrays.

#### Scenario: GET nonexistent session
- **WHEN** a client requests a session id that does not exist
- **THEN** the response SHALL be `404` with a JSON body containing `code: "not_found"`

#### Scenario: GET sessions for unknown project
- **WHEN** a client requests sessions for a project id that cannot be resolved
- **THEN** the response SHALL be `404` with a JSON body containing `code: "not_found"`

#### Scenario: Unhandled server exception
- **WHEN** an unexpected exception is thrown while serving a request
- **THEN** the response SHALL be `500` with a JSON body containing `code: "internal"`

### Requirement: Bind to configured port with graceful fallback

The system SHALL bind the HTTP server to the port configured in application configuration and SHALL report a clear startup error if the port is already in use, without silently choosing a different port.

#### Scenario: Configured port is busy
- **WHEN** the configured port is already in use by another process
- **THEN** startup SHALL log a clear error and SHALL NOT switch ports silently

