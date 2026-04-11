## MODIFIED Requirements

### Requirement: Build independent chunks from classified messages

The system SHALL convert a sequence of `ParsedMessage` into a sequence of independent chunks of four types: `UserChunk`, `AIChunk`, `SystemChunk`, `CompactChunk`. Chunks SHALL NOT be paired — a `UserChunk` does not "own" the following `AIChunk`. 连续的 assistant 消息 SHALL 被合并到同一个 `AIChunk.responses` 中，直到遇到真实用户消息、`SystemChunk` 对应的 `<local-command-stdout>` 消息、`CompactChunk` 对应的 compact summary 消息或输入末尾时 flush。

#### Scenario: User question followed by AI response
- **WHEN** the input is a real user message followed by one assistant message
- **THEN** the output SHALL be one `UserChunk` and one `AIChunk` as independent entries, in input order

#### Scenario: Multiple assistant turns before next user input
- **WHEN** several assistant messages appear consecutively without intervening real user input
- **THEN** they SHALL be coalesced into a single `AIChunk` whose `responses` field holds all assistant messages in chronological order

#### Scenario: Assistant buffer flushed by following user message
- **WHEN** an assistant buffer of N responses is followed by a real user message
- **THEN** the system SHALL emit the accumulated `AIChunk` before the new `UserChunk`

#### Scenario: Command output appears inline
- **WHEN** a user message whose content is exactly wrapped by `<local-command-stdout>...</local-command-stdout>` appears in the stream
- **THEN** a `SystemChunk` SHALL be emitted for it, not absorbed into a surrounding `AIChunk`, and any in-progress assistant buffer SHALL be flushed first

### Requirement: Filter sidechain and hard-noise messages

The system SHALL exclude messages where `is_sidechain == true` and messages whose `MessageCategory` is `HardNoise(_)` before building chunks. 被过滤掉的消息 SHALL NOT 影响 chunk 顺序、指标或语义步骤。

#### Scenario: Sidechain subagent messages in main stream
- **WHEN** the input contains messages marked `is_sidechain = true`
- **THEN** those messages SHALL NOT appear in any main-thread chunk and SHALL NOT contribute to any `ChunkMetrics`

#### Scenario: Hard-noise messages dropped before chunk construction
- **WHEN** the input contains messages classified as `MessageCategory::HardNoise(_)` (synthetic assistant placeholder, empty command output, interrupt marker, 等)
- **THEN** the system SHALL drop them before chunk construction and SHALL NOT emit a chunk for them

### Requirement: Compute per-chunk metrics

Each chunk SHALL expose `timestamp`、可选的 `duration` 和 `metrics`，其中 `ChunkMetrics` 包含：`input_tokens`、`output_tokens`、`cache_creation_tokens`、`cache_read_tokens`、`tool_count` 与可选 `cost_usd`。

在 `tool-execution-linking` 能力完成移植之前，`tool_count` SHALL 统计 `AIChunk.responses` 中所有 `tool_use` 块（包含 `Task` 调用）；`cost_usd` SHALL 取 `None`。后续移植完成后，`tool_count` 将按 Task 过滤语义修正，而 `cost_usd` 在价格表引入后改为 `Some(_)`。

#### Scenario: AIChunk with multiple tool uses
- **WHEN** an `AIChunk` contains 3 `tool_use` blocks across its assistant responses
- **THEN** its `metrics.tool_count` SHALL equal 3

#### Scenario: UserChunk without token usage
- **WHEN** a `UserChunk` has no usage data
- **THEN** its metrics token fields SHALL all be zero and `cost_usd` SHALL be `None`

#### Scenario: UserChunk duration is unset
- **WHEN** a `UserChunk` is emitted from exactly one `ParsedMessage`
- **THEN** its `duration` SHALL be `None`

### Requirement: Link tool uses to tool results

`AIChunk` SHALL 暴露一个稳定字段用于挂载每个 `tool_use` 对应的 tool execution 记录（uuid、name、input、可选 result、可选 error flag）。chunk-building 本身只负责结构占位：在 `port-chunk-building` 范围内，字段初始化为空列表。按 spec，配对逻辑与 orphan 处理由 `tool-execution-linking` capability 履行。

#### Scenario: Structure slot exists even with no linking implemented
- **WHEN** an `AIChunk` is built under the chunk-building capability only
- **THEN** its tool execution list field SHALL exist and be empty, ready to be filled by `tool-execution-linking`

#### Scenario: Tool result appears in a later user message
- **WHEN** an assistant `tool_use` is followed by a user message carrying its matching `tool_result`
- **THEN** after `tool-execution-linking` runs, the `AIChunk` containing the `tool_use` SHALL expose a tool execution record with both sides linked (verified under the `tool-execution-linking` capability)

#### Scenario: Tool use with no matching result (orphan)
- **WHEN** an assistant `tool_use` has no matching `tool_result` in the session
- **THEN** after `tool-execution-linking` runs, the `AIChunk` SHALL still expose the tool execution record with result marked as missing, without panic or error (verified under the `tool-execution-linking` capability)

### Requirement: Filter Task tool uses when subagent data is available

`AIChunk` 的 tool execution 列表 SHALL 在 subagent 数据可用时省略已解析的 `Task` `tool_use`；orphaned Task 调用（没有匹配 subagent）SHALL 被保留。chunk-building capability 自身不持有 subagent 信息，因此过滤在 `tool-execution-linking` + `team-coordination-metadata` 协同完成后生效；在 `port-chunk-building` 范围内，tool execution 列表不做 Task 过滤，`metrics.tool_count` 暂时包含 Task 调用（见 "Compute per-chunk metrics"）。

#### Scenario: Task call with resolved subagent
- **WHEN** a `Task` `tool_use` has a matching subagent entry
- **THEN** after the downstream capabilities run, that `Task` `tool_use` SHALL be removed from the `AIChunk` tool list and represented via the attached subagent process instead

#### Scenario: Task call with no matching subagent
- **WHEN** a `Task` `tool_use` has no matching subagent
- **THEN** it SHALL remain visible in the `AIChunk` tool list

### Requirement: Attach subagents to AIChunks

`AIChunk` SHALL 暴露一个稳定字段用于挂载由该 chunk 生成的 subagent Process 记录。chunk-building 只负责结构占位：字段默认空列表；真实的 Process 归集由 `team-coordination-metadata` capability 履行。

#### Scenario: Structure slot exists
- **WHEN** an `AIChunk` is built under the chunk-building capability only
- **THEN** its subagents field SHALL exist and be empty

#### Scenario: Single subagent spawn
- **WHEN** an `AIChunk` assistant messages spawned one subagent
- **THEN** after `team-coordination-metadata` runs, `AIChunk.subagents` SHALL contain one Process record with its own session id, timestamps, metrics, and optional team metadata (verified under that capability)

### Requirement: Extract semantic steps for AIChunks

The system SHALL extract a list of `SemanticStep` (thinking、text output、tool execution、subagent spawn) from each `AIChunk` in chronological order for UI visualization. 在 `port-chunk-building` 范围内：`Thinking` 与 `Text` 步骤从 `ParsedMessage.content` 中按 block 顺序抽取；`ToolExecution` 步骤以 `tool_use_id` + `tool_name` + `timestamp` 的占位形式生成，尚未关联到真实 `ToolExecution`；`SubagentSpawn` 变体先保留但不产出，留给 `team-coordination-metadata` 填充。

#### Scenario: AIChunk with thinking + text + tool
- **WHEN** an assistant response contains a thinking block, a text block, then a `tool_use`
- **THEN** the semantic steps SHALL be emitted in that exact order: `Thinking` → `Text` → `ToolExecution`

#### Scenario: SubagentSpawn step is reserved but not yet emitted
- **WHEN** chunk-building runs without the downstream subagent capability
- **THEN** no `SemanticStep::SubagentSpawn` SHALL be emitted, and the enum variant SHALL remain available for later ports

### Requirement: Emit CompactChunks at compaction boundaries

The system SHALL emit a `CompactChunk` whenever a `ParsedMessage` with `is_compact_summary == true` is encountered, preserving the summary text and boundary timestamp. 在产出 `CompactChunk` 之前，任何正在累积的 `AIChunk` buffer SHALL 先 flush。

#### Scenario: Session with one compaction
- **WHEN** the session contains exactly one `ParsedMessage` with `is_compact_summary == true`
- **THEN** exactly one `CompactChunk` SHALL be emitted at that position with the message's timestamp and textual summary

#### Scenario: Compaction flushes pending assistant buffer
- **WHEN** a compact summary message arrives while an assistant buffer of 2 responses is in progress
- **THEN** the system SHALL first flush the buffered `AIChunk` and THEN emit the `CompactChunk`
