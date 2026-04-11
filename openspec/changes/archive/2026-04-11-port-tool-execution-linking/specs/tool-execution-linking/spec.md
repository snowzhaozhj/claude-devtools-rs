## MODIFIED Requirements

### Requirement: Pair tool_use with tool_result by id

The system SHALL pair every `tool_use` block with its corresponding `tool_result` block by matching `tool_use_id`, regardless of how many messages separate them. 配对算法 SHALL 为纯同步函数，单次遍历输入消息即可完成；无匹配的 `tool_use` SHALL 作为 orphan 保留，标记 `output = Missing` 且 `end_ts = None`，不抛错。

#### Scenario: Immediate result
- **WHEN** a `tool_use` is followed in the next user message by a `tool_result` with matching id
- **THEN** the pair SHALL be linked and expose a `ToolExecution` record with both start and end timestamps

#### Scenario: Delayed result
- **WHEN** a `tool_use` is followed by several other messages before its `tool_result` appears
- **THEN** the pair SHALL still be linked once the matching result is seen

#### Scenario: Duplicate result ids
- **WHEN** two `tool_result` blocks share the same `tool_use_id`
- **THEN** the system SHALL link the first encountered result, increment a `duplicates_dropped` counter in the returned `ToolLinkingResult`, and emit `tracing::warn!` with the offending id

#### Scenario: Orphan tool_use has no matching result
- **WHEN** an assistant `tool_use` has no matching `tool_result` anywhere in the session
- **THEN** the system SHALL emit a `ToolExecution` record with `output = Missing`, `end_ts = None`, and `is_error = false`, and SHALL NOT panic

### Requirement: Build tool execution records with error state

Each linked pair SHALL produce a `ToolExecution` record exposing: `tool_use_id`、`tool_name`、`input`、`output`（`Text` / `Structured` / `Missing`）、`is_error`、`start_ts`（assistant 消息时间）、`end_ts`（tool_result 消息时间，orphan 为 `None`）、`source_assistant_uuid`（用于回填所属 `AIChunk`）。

#### Scenario: Tool returned an error
- **WHEN** the `tool_result` has `is_error = true`
- **THEN** the `ToolExecution` record SHALL set `is_error = true` and preserve the error content verbatim as `output`

#### Scenario: Bash tool with stdout and stderr
- **WHEN** the `tool_result` content is a structured JSON object carrying both stdout and stderr streams
- **THEN** the record SHALL store the original JSON as `ToolOutput::Structured` without flattening or discarding streams

#### Scenario: Text tool_result
- **WHEN** the `tool_result` content is a plain string (legacy shape)
- **THEN** the record SHALL store it as `ToolOutput::Text`

### Requirement: Resolve Task subagents with three-phase fallback matching

The system SHALL resolve `Task` tool calls to their corresponding subagent sessions using a three-phase fallback strategy, in order, implemented as a pure synchronous function over externally supplied candidates:

1. **Result-based**: 若 Task 对应的 `ToolExecution.output` 是结构化 JSON 且包含 `teammate_spawned` 或 `session_id` 字段，直接从 `candidates` 中按 session id 取出 `Process`。
2. **Description-based**: 用 Task 的 `description` 与 `candidate.description_hint` 做匹配，要求 `|task_ts − candidate.spawn_ts|` 落在 60 秒窗口内；若某 Task 唯一匹配到一个 candidate 则 link。
3. **Positional**: 若 phase 2 结束仍有未分配 Task 且"未分配 Task 数 == 未分配 candidate 数"，则按 spawn order 一一配对。

Unresolved Task calls SHALL remain as `Resolution::Orphan`。候选集合的装载不属本 capability——它由下游能力（例如 `project-discovery` 与 `team-coordination-metadata`）负责预过滤后传入。

#### Scenario: teammate_spawned result links directly
- **WHEN** a `Task` call has a matching `ToolExecution` whose structured output carries a `teammate_spawned` hint with subagent session id, and that session id is present in `candidates`
- **THEN** the function SHALL return `Resolution::ResultBased(Process)` for that task without evaluating later phases

#### Scenario: No result-based link, description matches one subagent
- **WHEN** the `Task` call has no usable `teammate_spawned` hint, and its description uniquely matches exactly one candidate within the 60s spawn-time window
- **THEN** the function SHALL return `Resolution::DescriptionBased(Process)`

#### Scenario: Description ambiguous, positional fallback applies
- **WHEN** description-based matching yields zero unique matches but the count of unresolved Task calls equals the count of unresolved candidates
- **THEN** the function SHALL return `Resolution::Positional(Process)` for each in spawn order

#### Scenario: Task call matches no subagent
- **WHEN** all three phases fail to produce a match for a given Task call
- **THEN** the function SHALL return `Resolution::Orphan` for that task, and the corresponding `ToolExecution` SHALL be retained as-is

#### Scenario: Unrelated candidate does not trigger positional match
- **WHEN** a Task call has no description match, and the candidate pool contains subagent sessions belonging to unrelated parents such that the equality check fails
- **THEN** the function SHALL NOT positionally link and SHALL return `Resolution::Orphan`
