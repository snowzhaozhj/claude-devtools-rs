## ADDED Requirements

### Requirement: Session list pagination avoids duplicate full scans

IPC clients that need all sessions for a project SHALL consume `list_sessions` through cursor pagination without re-requesting an already returned page as part of a larger full-list request. The `list_sessions` response MUST preserve the existing skeleton-first contract: returned `SessionSummary` entries may omit expensive metadata fields while background metadata updates fill them later.

#### Scenario: Client accumulates pages without restarting from the first page

- **WHEN** a project has more sessions than the initial client page size and `list_sessions` returns a non-null `nextCursor`
- **THEN** the client requests the next page using that cursor and appends the new sessions to the already returned sessions
- **AND** the client does NOT issue a second request from the beginning with `pageSize = total`

#### Scenario: Skeleton response remains available before metadata completes

- **WHEN** `list_sessions` returns session entries before background metadata parsing has completed
- **THEN** each returned entry remains a valid skeleton `SessionSummary`
- **AND** later `session-metadata-update` events may patch `title`, `messageCount`, `isOngoing`, and related metadata in place
