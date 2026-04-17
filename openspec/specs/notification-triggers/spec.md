# notification-triggers Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Detect errors from tool executions

The system SHALL detect tool execution errors by inspecting `tool_result` blocks for `is_error=true` flag and by matching configured error patterns against tool output content. The `is_error` flag check MUST take precedence over content pattern matching. Each produced `DetectedError` SHALL carry a deterministic id derived from the underlying `(session_id, file_path, line_number, tool_use_id, trigger_id, message)` tuple so that re-detection of the same occurrence yields the same id.

#### Scenario: Tool result flagged `is_error`
- **WHEN** a `tool_result` block has `is_error=true` and the trigger mode is `error_status` with `require_error=true`
- **THEN** a `DetectedError` record SHALL be produced with the tool name, message uuid, output preview, trigger id, and trigger color
- **AND** if the error message matches any ignore pattern, the error SHALL be suppressed

#### Scenario: Tool output matches configured error pattern
- **WHEN** a tool output contains a substring matching a configured regex error pattern and the trigger mode is `content_match`
- **THEN** a `DetectedError` record SHALL be produced

#### Scenario: Token threshold exceeded
- **WHEN** a trigger mode is `token_threshold` and a tool execution's estimated token count exceeds the configured threshold
- **THEN** a `DetectedError` record SHALL be produced for each exceeding tool_use block, with token count details

#### Scenario: Deterministic id across rescans
- **WHEN** `create_detected_error` is invoked twice with identical parameters (same session, line, tool_use_id, trigger_id, message)
- **THEN** both invocations SHALL return records whose `id` field is byte-for-byte equal

### Requirement: Evaluate notification triggers against new messages

The system SHALL evaluate all user-configured enabled triggers against each newly ingested message and produce `DetectedError` records when triggers match.

#### Scenario: Trigger with literal keyword
- **WHEN** a trigger is configured with a `content_match` pattern "ERROR" and a new assistant message contains "ERROR"
- **THEN** a `DetectedError` SHALL be produced carrying the trigger id, session id, and matched preview

#### Scenario: Trigger with regex pattern
- **WHEN** a trigger is configured with a regex pattern
- **THEN** the system SHALL apply the regex (case-insensitive) to the message content and produce a `DetectedError` on match

#### Scenario: Trigger scoped to specific tool names
- **WHEN** a trigger specifies `tool_name = "Bash"` and a matching Bash `tool_result` appears
- **THEN** the `DetectedError` SHALL fire; matches in other tools SHALL NOT fire this trigger

#### Scenario: Ignore patterns suppress matches
- **WHEN** a trigger matches but the matched content also matches one of the trigger's `ignore_patterns`
- **THEN** the match SHALL be suppressed and no `DetectedError` produced

### Requirement: Validate regex patterns safely

The system SHALL validate user-provided regex patterns before use, rejecting patterns known to cause catastrophic backtracking within a fixed time budget.

#### Scenario: Pathological regex submitted
- **WHEN** a user submits a regex that exceeds the validation time budget on a test string
- **THEN** the system SHALL reject the regex and return a validation error, not apply it

#### Scenario: Regex cache bounded
- **WHEN** more than 500 unique regex patterns are compiled
- **THEN** the oldest cached entry SHALL be evicted (LRU policy)

### Requirement: Test triggers against historical sessions

The system SHALL let a caller test a trigger configuration against existing session data and return the list of historical messages that would have matched, without persisting any notification.

#### Scenario: Preview a new trigger
- **WHEN** a user previews a new trigger against the last 30 days of sessions
- **THEN** the system SHALL return all would-have-matched messages with session id, timestamp, and preview

### Requirement: Persist and expose notifications

The system SHALL persist emitted notifications to `~/.claude/claude-devtools-notifications.json` with read/unread state and expose them with paging and mark-as-read operations. The persistence layer SHALL deduplicate incoming notifications by their `id`, treating re-submission of an existing id as a no-op that does not change stored state or counts.

#### Scenario: Mark notification as read
- **WHEN** a caller marks a notification id as read
- **THEN** the notification state SHALL update, the unread count SHALL decrement, and the new state SHALL survive process restarts

#### Scenario: Auto-prune on startup
- **WHEN** the stored notification count exceeds 100
- **THEN** the system SHALL remove the oldest notifications to bring the count to 100

#### Scenario: Paged retrieval
- **WHEN** a caller requests notifications with limit and offset
- **THEN** the system SHALL return the requested page, total count, unread count, and `has_more` flag

#### Scenario: Same-id submission is idempotent
- **WHEN** `add_notification` is called twice with `DetectedError` records sharing the same deterministic `id`
- **THEN** the store SHALL retain exactly one entry, the unread count SHALL increase by at most one, and the second call SHALL return a signal (e.g. `Ok(false)`) indicating the write was a duplicate

### Requirement: Automatic background notification pipeline

The system SHALL run a background pipeline that subscribes to `file-watching` change events, re-parses the affected session file, evaluates all enabled triggers against the parsed messages, and persists newly detected errors through `NotificationManager` without requiring any UI action.

#### Scenario: New JSONL line with tool error triggers detection
- **WHEN** a `.jsonl` session file is appended with a new assistant message containing a `tool_result` with `is_error=true`
- **AND** the user has an enabled `error_status` trigger with `require_error=true`
- **THEN** the pipeline SHALL produce a `DetectedError`, persist it via `NotificationManager::add_notification`, and publish it on the pipeline's `DetectedError` broadcast channel

#### Scenario: Duplicate detection across rescans is suppressed
- **WHEN** the same session file change triggers detection more than once (e.g. because of another unrelated append causing a re-scan)
- **AND** the `DetectedError` computed for the same `(session_id, line_number, tool_use_id, trigger_id, message)` tuple is produced again
- **THEN** `NotificationManager` SHALL recognize the existing id and skip persistence, and the pipeline SHALL NOT re-broadcast the duplicate on the `DetectedError` channel

#### Scenario: Deleted file events are ignored
- **WHEN** the `FileChangeEvent` carries `deleted: true`
- **THEN** the pipeline SHALL NOT attempt to parse the missing file and SHALL NOT produce any `DetectedError`

#### Scenario: Empty trigger set is a no-op
- **WHEN** the user has no enabled triggers configured
- **THEN** the pipeline SHALL receive file change events but SHALL NOT call `detect_errors` nor write any notification

