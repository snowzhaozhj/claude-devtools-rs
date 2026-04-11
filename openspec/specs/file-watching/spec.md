# file-watching Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Watch Claude projects directory for session changes

The system SHALL watch `~/.claude/projects/` recursively and emit change events when `.jsonl` session files are created, modified, or deleted.

#### Scenario: New session file created
- **WHEN** a new `.jsonl` file appears in a watched project directory
- **THEN** subscribers SHALL receive a `file-change` event carrying the project id and session id within the debounce window

#### Scenario: Existing session file appended
- **WHEN** an existing `.jsonl` file is appended
- **THEN** subscribers SHALL receive a `file-change` event for that session

#### Scenario: Session file deleted
- **WHEN** a `.jsonl` file is deleted
- **THEN** subscribers SHALL receive a `file-change` event with a delete indicator

### Requirement: Watch Claude todos directory

The system SHALL watch `~/.claude/todos/` for `.json` file changes and emit `todo-change` events with the affected session id.

#### Scenario: Todo file updated
- **WHEN** `~/.claude/todos/<sessionId>.json` is updated
- **THEN** subscribers SHALL receive a `todo-change` event carrying the session id

### Requirement: Debounce rapid file events

The system SHALL debounce rapid successive change events on the same file within a 100ms window, emitting a single coalesced event.

#### Scenario: Burst of writes
- **WHEN** a file receives 5 write events within 30ms
- **THEN** subscribers SHALL receive exactly one `file-change` event for that file after the debounce window

### Requirement: Survive transient filesystem errors

The system SHALL log and ignore transient errors (permission denied, temporary lock) without terminating the watcher.

#### Scenario: Temporary permission error on one file
- **WHEN** the watcher encounters a permission error while stat-ing one file
- **THEN** the watcher SHALL log the error and continue watching other files

### Requirement: Broadcast events to multiple subscribers

The system SHALL deliver each emitted event to all active subscribers (Electron renderer via IPC and HTTP clients via SSE) without duplication.

#### Scenario: Two subscribers present
- **WHEN** one file change triggers an event and two subscribers are active
- **THEN** both subscribers SHALL receive the event exactly once each

