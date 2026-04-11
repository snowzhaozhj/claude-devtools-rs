# notification-triggers Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Detect errors from tool executions

The system SHALL detect tool execution errors by inspecting tool_result blocks for `is_error=true` and by matching configured error patterns against tool output content.

#### Scenario: Tool result flagged is_error
- **WHEN** a tool_result block has `is_error=true`
- **THEN** a DetectedError record SHALL be produced with the tool name, message uuid, and output preview

#### Scenario: Tool output matches configured error pattern
- **WHEN** a tool output contains a substring matching a configured regex error pattern
- **THEN** a DetectedError record SHALL be produced

### Requirement: Evaluate notification triggers against new messages

The system SHALL evaluate all user-configured notification triggers against each newly ingested message and emit a NotificationEvent when a trigger matches.

#### Scenario: Trigger with literal keyword
- **WHEN** a trigger is configured with literal keyword "ERROR" and a new assistant message contains "ERROR"
- **THEN** a NotificationEvent SHALL be emitted carrying the trigger id, session id, and matched preview

#### Scenario: Trigger with regex pattern
- **WHEN** a trigger is configured with a regex pattern
- **THEN** the system SHALL apply the regex to the message content and emit a NotificationEvent on match

#### Scenario: Trigger scoped to specific tool names
- **WHEN** a trigger is scoped to tool name `Bash` and a matching Bash tool_result appears
- **THEN** the NotificationEvent SHALL fire; matches in other tools SHALL not fire this trigger

### Requirement: Validate regex patterns safely

The system SHALL validate user-provided regex patterns before use, rejecting patterns known to cause catastrophic backtracking within a fixed time budget.

#### Scenario: Pathological regex submitted
- **WHEN** a user submits a regex that exceeds the validation time budget on a test string
- **THEN** the system SHALL reject the regex and return a validation error, not apply it

### Requirement: Test triggers against historical sessions

The system SHALL let a caller test a trigger configuration against existing session data and return the list of historical messages that would have matched, without persisting any notification.

#### Scenario: Preview a new trigger
- **WHEN** a user previews a new trigger against the last 30 days of sessions
- **THEN** the system SHALL return all would-have-matched messages with session id, timestamp, and preview

### Requirement: Persist and expose notifications

The system SHALL persist emitted notifications with read/unread state and expose them to consumers with paging and mark-as-read operations.

#### Scenario: Mark notification as read
- **WHEN** a caller marks a notification id as read
- **THEN** the notification state SHALL update, the unread count SHALL decrement, and the new state SHALL survive process restarts

