# chunk-building Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Build independent chunks from classified messages

The system SHALL convert a sequence of `ParsedMessage` into a sequence of independent chunks of four types: `UserChunk`, `AIChunk`, `SystemChunk`, `CompactChunk`. Chunks SHALL NOT be paired — a `UserChunk` does not "own" the following `AIChunk`. 连续的 assistant 消息 SHALL 被合并到同一个 `AIChunk.responses` 中，直到遇到真实用户消息、`SystemChunk` 对应的 `<local-command-stdout>` 消息、`CompactChunk` 对应的 compact summary 消息或输入末尾时 flush。

`AIChunk` SHALL 暴露 `slash_commands: Vec<SlashCommand>` 字段，包含由前述 isMeta 消息中提取的 slash 命令。默认为空数组。

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

#### Scenario: AIChunk includes slash commands from preceding isMeta message
- **WHEN** an isMeta user message with a slash command precedes an assistant response
- **THEN** the resulting `AIChunk` SHALL have the extracted slash command in its `slash_commands` field

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

### Requirement: Compute per-chunk metrics

Each chunk SHALL expose `timestamp`、可选的 `duration` 和 `metrics`，其中 `ChunkMetrics` 包含：`input_tokens`、`output_tokens`、`cache_creation_tokens`、`cache_read_tokens`、`tool_count` 与可选 `cost_usd`。

在 `team-coordination-metadata` 能力把端到端 subagent 候选装载接入 `build_chunks` 默认路径之前，`tool_count` SHALL 统计 `AIChunk.responses` 中所有 `tool_use` 块（包含 `Task` 调用）；`cost_usd` SHALL 取 `None`。`team-coordination-metadata` 移植完成后，`tool_count` 将按 Task 过滤语义修正，`cost_usd` 在价格表引入后改为 `Some(_)`。

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

`AIChunk` SHALL 在默认的 `build_chunks` 路径中暴露已填充的 `tool_executions: Vec<ToolExecution>` 字段：每个 `tool_use` 块都对应一条 `ToolExecution` 记录（由 `tool-execution-linking` capability 产出），未配对的 `tool_use` 以 orphan 形式保留（`output = Missing`、`end_ts = None`）。

#### Scenario: Tool executions populated by build_chunks
- **WHEN** `build_chunks` runs over a session containing assistant `tool_use` blocks
- **THEN** each owning `AIChunk.tool_executions` SHALL contain one `ToolExecution` per `tool_use`, distributed according to the originating assistant message `uuid`

#### Scenario: Tool result appears in a later user message
- **WHEN** an assistant `tool_use` is followed by a user message carrying its matching `tool_result`
- **THEN** the corresponding `AIChunk.tool_executions` entry SHALL expose both `start_ts` and `end_ts` and preserve the result content as `output`

#### Scenario: Tool use with no matching result (orphan)
- **WHEN** an assistant `tool_use` has no matching `tool_result` in the session
- **THEN** the `AIChunk.tool_executions` entry SHALL have `output = Missing`, `end_ts = None`, and `is_error = false`, without panic

### Requirement: Filter Task tool uses when subagent data is available

`AIChunk.tool_executions` SHALL 在 subagent 候选可用时省略已解析的 `Task` `tool_use`；orphaned Task 调用（没有匹配 subagent）SHALL 被保留。chunk-building capability 自身不调用 filter——纯函数 `tool_linking::filter_resolved_tasks` 在 `port-tool-execution-linking` 内实现并受独立测试覆盖，而把 filter 接入 `build_chunks` 默认路径、装载 subagent 候选、同步更新 `ChunkMetrics::tool_count` 的工作由 `team-coordination-metadata` 完成。

#### Scenario: Task call with resolved subagent
- **WHEN** a `Task` `tool_use` has a matching subagent entry in the caller-supplied candidate pool
- **THEN** `tool_linking::filter_resolved_tasks` SHALL remove that entry from a mutable `Vec<ToolExecution>`, leaving only non-Task executions and orphan Task calls

#### Scenario: Task call with no matching subagent
- **WHEN** a `Task` `tool_use` resolves to `Resolution::Orphan`
- **THEN** it SHALL remain in the `AIChunk.tool_executions` list

#### Scenario: Default build_chunks does not filter Tasks in this port
- **WHEN** `build_chunks` is invoked without passing any subagent candidate pool
- **THEN** Task tool executions SHALL remain in `AIChunk.tool_executions`, and downstream consumers MAY still invoke `filter_resolved_tasks` explicitly; end-to-end default-path filtering is deferred to `team-coordination-metadata`

### Requirement: Attach subagents to AIChunks

`AIChunk` SHALL 暴露一个稳定字段用于挂载由该 chunk 生成的 subagent Process 记录。chunk-building 只负责结构占位：字段默认空列表；真实的 Process 归集由 `team-coordination-metadata` capability 履行。

#### Scenario: Structure slot exists
- **WHEN** an `AIChunk` is built under the chunk-building capability only
- **THEN** its subagents field SHALL exist and be empty

#### Scenario: Single subagent spawn
- **WHEN** an `AIChunk` assistant messages spawned one subagent
- **THEN** after `team-coordination-metadata` runs, `AIChunk.subagents` SHALL contain one Process record with its own session id, timestamps, metrics, and optional team metadata (verified under that capability)

### Requirement: Extract semantic steps for AIChunks

The system SHALL extract a list of `SemanticStep` (thinking、text output、tool execution、subagent spawn、interruption) from each `AIChunk` in chronological order for UI visualization. `Thinking` 与 `Text` 步骤从 `ParsedMessage.content` 中按 block 顺序抽取；`ToolExecution` 步骤以 `tool_use_id` + `tool_name` + `timestamp` 的形式生成，与 `AIChunk.tool_executions` 里的条目一一对应（可通过 `tool_use_id` 交叉查找真实 `ToolExecution`）；`SubagentSpawn` 变体先保留但不产出，留给 `team-coordination-metadata` 填充；`Interruption` 变体由 `Emit interruption semantic step for interrupt-marker messages` Requirement 负责产出。

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

### Requirement: Extract slash commands from isMeta messages

The system SHALL 在构建 chunks 时从 `isMeta=true` 的 user 消息中提取 slash 命令信息。Slash 命令通过 `<command-name>/xxx</command-name>` XML 标签识别，提取 name、message（`<command-message>`）和 args（`<command-args>`）。

提取的 slash 命令 SHALL 附加到紧随其后的 `AIChunk` 的 `slash_commands` 字段中。若 isMeta 消息不含 slash 命令格式，SHALL 静默跳过。

#### Scenario: isMeta message with slash command
- **WHEN** an isMeta user message contains `<command-name>/commit</command-name>`
- **THEN** the system SHALL extract a `SlashCommand` with `name="commit"` and attach it to the next `AIChunk.slash_commands`

#### Scenario: isMeta message with slash command including message and args
- **WHEN** an isMeta user message contains `<command-name>/review-pr</command-name><command-message>review-pr</command-message><command-args>123</command-args>`
- **THEN** the extracted `SlashCommand` SHALL have `name="review-pr"`, `message=Some("review-pr")`, `args=Some("123")`

#### Scenario: isMeta message without slash format
- **WHEN** an isMeta user message contains a system-reminder injection without `<command-name>` tags
- **THEN** no `SlashCommand` SHALL be extracted and the message SHALL be handled as before (tool_result merge or skip)

#### Scenario: Slash command with no following AIChunk
- **WHEN** a slash command is extracted from an isMeta message but no subsequent AIChunk exists
- **THEN** the slash command SHALL be discarded without error

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

