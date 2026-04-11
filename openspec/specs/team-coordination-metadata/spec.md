# team-coordination-metadata Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Detect teammate messages

The system SHALL detect user messages that carry a teammate payload wrapped in `<teammate-message teammate_id="..." ...>content</teammate-message>` and classify them as teammate messages rather than real user input.

#### Scenario: Teammate message in string content
- **WHEN** a user message's string content starts with `<teammate-message teammate_id="alice"`
- **THEN** the message SHALL be flagged as a teammate message and SHALL NOT create a UserChunk

#### Scenario: Teammate message in block content
- **WHEN** a user message has a single text block containing the teammate tag
- **THEN** the message SHALL be flagged as a teammate message

### Requirement: Render teammate messages as dedicated items

The system SHALL expose teammate messages as dedicated display records carrying teammate id, color, summary, and body, distinct from normal user and AI items.

#### Scenario: Teammate id and color present
- **WHEN** a teammate message carries `teammate_id="alice"` and `color="blue"`
- **THEN** the display record SHALL expose `teammate_id="alice"` and `color="blue"` so consumers can render a dedicated item

### Requirement: Recognize team coordination tools

The system SHALL recognize the following tool names as team coordination tools and route their formatting through a team-specific summary formatter: `TeamCreate`, `TaskCreate`, `TaskUpdate`, `TaskList`, `TaskGet`, `SendMessage`, `TeamDelete`.

#### Scenario: TaskCreate invocation
- **WHEN** a `TaskCreate` tool use appears
- **THEN** its summary SHALL include the task name and assignee

#### Scenario: SendMessage with shutdown_response
- **WHEN** a `SendMessage` tool use is a shutdown_response with `approve=true`
- **THEN** the system SHALL treat the response as a session-ending signal rather than ongoing activity

### Requirement: Enrich subagent processes with team metadata

The system SHALL enrich a subagent Process's `team` field with `{ teamName, memberName, memberColor }` when the spawning context contains team info from either the Task call input or a matching `teammate_spawned` tool result.

#### Scenario: Task call carries team metadata
- **WHEN** the spawning Task call input contains a team name and member name
- **THEN** Process.team SHALL be populated accordingly

### Requirement: Distinguish teammates from regular subagents via Process metadata

The system SHALL make it possible for callers to distinguish teammate processes from regular subagents by inspecting the `Process.team` field: a subagent with `team` populated is a teammate, one without is a regular subagent. Counting distinct teammates is a caller concern — the data layer SHALL expose the raw field without precomputing the breakdown.

#### Scenario: Inspect Process.team to classify
- **WHEN** an AIChunk spawned three subagents, two of which carry team metadata
- **THEN** callers SHALL be able to derive "1 regular subagent + 2 teammates" by filtering on the presence of `Process.team`

#### Scenario: No team metadata available
- **WHEN** none of the spawned subagents carry team metadata
- **THEN** all entries SHALL appear with `Process.team = undefined` and callers SHALL treat them as regular subagents

