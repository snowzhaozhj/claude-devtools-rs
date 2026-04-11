# tool-execution-linking Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Pair tool_use with tool_result by id

The system SHALL pair every `tool_use` block with its corresponding `tool_result` block by matching `tool_use_id`, regardless of how many messages separate them.

#### Scenario: Immediate result
- **WHEN** a tool_use is followed in the next user message by a tool_result with matching id
- **THEN** the pair SHALL be linked

#### Scenario: Delayed result
- **WHEN** a tool_use is followed by several other messages before its tool_result appears
- **THEN** the pair SHALL still be linked once the matching result is seen

#### Scenario: Duplicate result ids
- **WHEN** two tool_result blocks share the same tool_use_id
- **THEN** the system SHALL link the first encountered result and log a warning for duplicates

### Requirement: Build tool execution records with error state

Each linked pair SHALL produce a tool execution record exposing: tool name, input, output (text or structured), isError flag, start timestamp (from the assistant message), and end timestamp (from the result message).

#### Scenario: Tool returned an error
- **WHEN** the tool_result has `is_error=true`
- **THEN** the tool execution record SHALL set isError=true and preserve the error content as output

#### Scenario: Bash tool with stdout and stderr
- **WHEN** the tool_result contains structured stdout/stderr
- **THEN** the record SHALL preserve both streams

### Requirement: Resolve Task subagents with three-phase fallback matching

The system SHALL resolve `Task` tool calls to their corresponding subagent sessions using a three-phase fallback strategy, in order:

1. **Result-based**: match via the `teammate_spawned` tool_result that carries an explicit subagent session id
2. **Description-based**: match by task description text plus spawn timestamp proximity against subagent session files on disk
3. **Positional**: as a last resort, match by spawn order when description is ambiguous but the number of Task calls equals the number of candidate subagents

Unresolved Task calls remain as orphans.

#### Scenario: teammate_spawned result links directly
- **WHEN** a Task call has a matching `teammate_spawned` tool_result with a subagent session id
- **THEN** the system SHALL emit a Process record keyed on that session id without falling through to later phases

#### Scenario: No result-based link, description matches one subagent
- **WHEN** the Task call has no `teammate_spawned` result but its description uniquely matches one subagent session within the spawn time window
- **THEN** the system SHALL link via description-based matching

#### Scenario: Description ambiguous, positional fallback applies
- **WHEN** description-based matching yields multiple candidates and the number of Task calls in the parent equals the number of candidate subagents
- **THEN** the system SHALL link Task calls to subagents in spawn order

#### Scenario: Task call matches no subagent
- **WHEN** all three phases fail to produce a match
- **THEN** the Task call SHALL remain unresolved and its tool execution record SHALL be retained as an orphan

### Requirement: Enrich subagent processes with team metadata

The system SHALL enrich Process records with `team` metadata (teamName, memberName, memberColor) when the spawning Task input or the `teammate_spawned` tool result carries team information.

#### Scenario: Team member spawned via TaskCreate
- **WHEN** a subagent was spawned via a TaskCreate call carrying team metadata
- **THEN** the Process.team SHALL be populated with the team name, member name, and color

### Requirement: Format readable summaries for team coordination tools

The system SHALL produce a short human-readable summary string for every team coordination tool (TeamCreate, TaskCreate, TaskUpdate, TaskList, TaskGet, SendMessage, TeamDelete), capturing the most salient parameters.

#### Scenario: SendMessage with recipient and body
- **WHEN** a SendMessage tool_use has a `to` and a `message` parameter
- **THEN** the summary SHALL include both the recipient and a truncated message preview

