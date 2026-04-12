## MODIFIED Requirements

### Requirement: Detect errors from tool executions

The system SHALL detect tool execution errors by inspecting `tool_result` blocks for `is_error=true` flag and by matching configured error patterns against tool output content. The `is_error` flag check MUST take precedence over content pattern matching.

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

### Requirement: Persist and expose notifications

The system SHALL persist emitted notifications to `~/.claude/claude-devtools-notifications.json` with read/unread state and expose them with paging and mark-as-read operations.

#### Scenario: Mark notification as read
- **WHEN** a caller marks a notification id as read
- **THEN** the notification state SHALL update, the unread count SHALL decrement, and the new state SHALL survive process restarts

#### Scenario: Auto-prune on startup
- **WHEN** the stored notification count exceeds 100
- **THEN** the system SHALL remove the oldest notifications to bring the count to 100

#### Scenario: Paged retrieval
- **WHEN** a caller requests notifications with limit and offset
- **THEN** the system SHALL return the requested page, total count, unread count, and `has_more` flag
