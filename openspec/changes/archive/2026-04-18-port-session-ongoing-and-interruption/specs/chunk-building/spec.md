## MODIFIED Requirements

### Requirement: Filter sidechain and hard-noise messages

The system SHALL exclude messages where `is_sidechain == true` and messages whose `MessageCategory` is `HardNoise(_)` before building chunks. 被过滤掉的消息 SHALL NOT 影响 chunk 顺序、指标或语义步骤。`MessageCategory::Interruption` 类别的消息 MUST NOT 被此过滤器排除——它们
在 chunk-building 主循环中以语义步骤形式处理（详见
`Emit interruption semantic step for interrupt-marker messages`
Requirement）。

#### Scenario: Sidechain subagent messages in main stream
- **WHEN** the input contains messages marked `is_sidechain = true`
- **THEN** those messages SHALL NOT appear in any main-thread chunk and SHALL NOT contribute to any `ChunkMetrics`

#### Scenario: Hard-noise messages dropped before chunk construction
- **WHEN** the input contains messages classified as `MessageCategory::HardNoise(_)` (synthetic assistant placeholder, empty command output, 等)
- **THEN** the system SHALL drop them before chunk construction and SHALL NOT emit a chunk for them

#### Scenario: Interruption category is not filtered as noise
- **WHEN** the input contains a message classified as `MessageCategory::Interruption`
- **THEN** the message SHALL NOT be dropped by the sidechain / hard-noise filter, and chunk-building SHALL process it per the interruption semantic step rule

### Requirement: Extract semantic steps for AIChunks

The system SHALL extract a list of `SemanticStep` (thinking、text output、tool execution、subagent spawn、interruption) from each `AIChunk` in chronological order for UI visualization. `Thinking` 与 `Text` 步骤从 `ParsedMessage.content` 中按 block 顺序抽取；`ToolExecution` 步骤以 `tool_use_id` + `tool_name` + `timestamp` 的形式生成，与 `AIChunk.tool_executions` 里的条目一一对应（可通过 `tool_use_id` 交叉查找真实 `ToolExecution`）；`SubagentSpawn` 变体先保留但不产出，留给 `team-coordination-metadata` 填充；`Interruption` 变体由 `Emit interruption semantic step for interrupt-marker messages` Requirement 负责产出。

#### Scenario: AIChunk with thinking + text + tool
- **WHEN** an assistant response contains a thinking block, a text block, then a `tool_use`
- **THEN** the semantic steps SHALL be emitted in that exact order: `Thinking` → `Text` → `ToolExecution`

#### Scenario: SubagentSpawn step is reserved but not yet emitted
- **WHEN** chunk-building runs without the downstream subagent capability
- **THEN** no `SemanticStep::SubagentSpawn` SHALL be emitted, and the enum variant SHALL remain available for later ports

## ADDED Requirements

### Requirement: Emit interruption semantic step for interrupt-marker messages

The system SHALL append a `SemanticStep::Interruption { text, timestamp }`
to the immediately-preceding (or currently-buffering) `AIChunk` whenever
chunk-building encounters a `MessageCategory::Interruption` message. If
no `AIChunk` is active or buffered when the interruption arrives, the
interruption SHALL be silently discarded (对齐原版：独立中断不产出新
chunk 类型)。 MUST NOT emit a dedicated `Chunk` variant for the
interruption——保持 chunk 列表的四种类型不变。

#### Scenario: Interruption appended to the current assistant buffer
- **WHEN** the message stream is `assistant(text) → user("[Request interrupted by user for tool use]")`
- **THEN** the single resulting `AIChunk.semantic_steps` SHALL end with `SemanticStep::Interruption { text: <raw interrupt text>, timestamp: <user msg ts> }` and the `AIChunk` SHALL be flushed after it

#### Scenario: Interruption appended to the last AIChunk when buffer is empty
- **WHEN** the message stream is `assistant(text)` flushed, then a later `user("[Request interrupted by user]")` arrives with no new assistant messages in between
- **THEN** the interruption SHALL be appended to the `semantic_steps` of the most recent `AIChunk` already emitted

#### Scenario: Interruption without any prior assistant
- **WHEN** the message stream begins with a `MessageCategory::Interruption` message and there is no prior `AIChunk`
- **THEN** no chunk SHALL be emitted for it and the chunk list SHALL remain unchanged from the non-interruption case

#### Scenario: Multiple interruptions in a row
- **WHEN** two consecutive interrupt messages follow an assistant response
- **THEN** each interruption SHALL produce one `SemanticStep::Interruption` in the same `AIChunk.semantic_steps`, preserving original order
