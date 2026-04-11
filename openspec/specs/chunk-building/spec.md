# chunk-building Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Build independent chunks from classified messages

The system SHALL convert a sequence of ParsedMessages into a sequence of independent chunks of four types: UserChunk, AIChunk, SystemChunk, CompactChunk. Chunks SHALL NOT be paired — a UserChunk does not "own" the following AIChunks.

#### Scenario: User question followed by AI response
- **WHEN** the input is a real user message followed by one assistant message
- **THEN** the output SHALL be one UserChunk and one AIChunk as independent entries, in input order

#### Scenario: Multiple assistant turns before next user input
- **WHEN** several assistant messages appear consecutively without intervening real user input
- **THEN** they SHALL be coalesced into a single AIChunk whose responses field holds all assistant messages and their tool executions

#### Scenario: Command output appears inline
- **WHEN** a message wrapped in `<local-command-stdout>` appears in the stream
- **THEN** a SystemChunk SHALL be emitted for it, not absorbed into a surrounding AIChunk

### Requirement: Filter sidechain and hard-noise messages

The system SHALL exclude messages where `isSidechain=true` and messages classified as hard noise before building chunks.

#### Scenario: Sidechain subagent messages in main stream
- **WHEN** the input contains messages marked `isSidechain=true`
- **THEN** those messages SHALL NOT appear in any main-thread chunk

### Requirement: Compute per-chunk metrics

Each chunk SHALL expose timestamp, duration, and metrics containing: input tokens, output tokens, cache creation tokens, cache read tokens, cost (USD), and tool invocation count.

#### Scenario: AIChunk with multiple tool uses
- **WHEN** an AIChunk contains 3 tool_use blocks across its assistant responses
- **THEN** its metrics.toolCount SHALL equal 3

#### Scenario: UserChunk without token usage
- **WHEN** a UserChunk has no usage data
- **THEN** its metrics token fields SHALL all be zero

### Requirement: Link tool uses to tool results

The system SHALL pair each `tool_use` block with the corresponding `tool_result` block by tool_use_id across messages, attaching the paired pair to the AIChunk that originated the tool_use.

#### Scenario: Tool result appears in a later user message
- **WHEN** an assistant tool_use is followed by a user message carrying its matching tool_result
- **THEN** the AIChunk containing the tool_use SHALL expose a tool execution record with both sides linked

#### Scenario: Tool use with no matching result (orphan)
- **WHEN** an assistant tool_use has no matching tool_result in the session
- **THEN** the AIChunk SHALL still expose the tool execution record with result marked as missing, without throwing

### Requirement: Filter Task tool uses when subagent data is available

The system SHALL omit `Task` tool_use blocks from the AIChunk's tool execution list when a corresponding subagent has been resolved for that task; orphaned Task calls (no matching subagent) SHALL be retained.

#### Scenario: Task call with resolved subagent
- **WHEN** a Task tool_use has a matching subagent entry
- **THEN** that Task tool_use SHALL be removed from the AIChunk's tool list and represented via the attached subagent process instead

#### Scenario: Task call with no matching subagent
- **WHEN** a Task tool_use has no matching subagent
- **THEN** it SHALL remain visible in the AIChunk's tool list

### Requirement: Attach subagents to AIChunks

The system SHALL attach subagent Process records to the AIChunk whose assistant messages spawned them, enabling drill-down into each subagent's own chunks.

#### Scenario: Single subagent spawn
- **WHEN** an AIChunk's assistant messages spawned one subagent
- **THEN** the AIChunk.subagents SHALL contain one Process record with its own session id, timestamps, metrics, and optional team metadata

### Requirement: Extract semantic steps for AIChunks

The system SHALL extract a list of semantic steps (thinking, text output, tool executions, subagent spawns) from each AIChunk in chronological order for UI visualization.

#### Scenario: AIChunk with thinking + text + tool
- **WHEN** an assistant response contains a thinking block, a text block, then a tool_use
- **THEN** the semantic steps SHALL be emitted in that exact order

### Requirement: Emit CompactChunks at compaction boundaries

The system SHALL emit a CompactChunk whenever a compact summary boundary message is encountered, preserving the summary text and boundary timestamp.

#### Scenario: Session with one compaction
- **WHEN** the session contains exactly one compact summary boundary
- **THEN** exactly one CompactChunk SHALL be emitted at that position

