# chunk-building Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Build independent chunks from classified messages

The system SHALL convert a sequence of `ParsedMessage` into a sequence of independent chunks of four types: `UserChunk`, `AIChunk`, `SystemChunk`, `CompactChunk`. Chunks SHALL NOT be paired — a `UserChunk` does not "own" the following `AIChunk`. 连续的 assistant 消息 SHALL 被合并到同一个 `AIChunk.responses` 中，直到遇到真实用户消息、`SystemChunk` 对应的 `<local-command-stdout>` 消息、`CompactChunk` 对应的 compact summary 消息或输入末尾时 flush。

`AIChunk` SHALL 暴露 `slash_commands: Vec<SlashCommand>` 字段，包含紧邻前一条 slash user 消息提取的 slash 命令（详见 `Emit slash commands as both UserChunk and AIChunk.slash_commands` Requirement）。默认为空数组。

`AIChunk` SHALL 同时暴露 `teammate_messages: Vec<TeammateMessage>` 字段，包含被嵌入到该 turn 的 teammate user 消息（详见 `Embed teammate messages into AIChunk` Requirement）。默认为空数组——序列化时通过 `skip_serializing_if = "Vec::is_empty"` 在无 teammate 时省略字段，保持老快照与老 IPC payload 兼容。

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

#### Scenario: AIChunk includes slash commands from preceding slash user message
- **WHEN** a slash user message (content starting with `<command-name>/xxx</command-name>`, `is_meta=false`) immediately precedes an assistant response
- **THEN** the resulting `AIChunk` SHALL have the extracted slash command in its `slash_commands` field

#### Scenario: AIChunk teammate_messages defaults to empty
- **WHEN** an `AIChunk` is built from a turn that contains no teammate reply
- **THEN** its `teammate_messages` field SHALL be an empty `Vec`，且 IPC 序列化结果 SHALL 不包含 `teammateMessages` 键（由 `skip_serializing_if = "Vec::is_empty"` 控制）

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

The system SHALL extract a list of `SemanticStep` (thinking、text output、tool execution、subagent spawn、interruption) from each `AIChunk` in chronological order for UI visualization. `Thinking` 与 `Text` 步骤从 `ParsedMessage.content` 中按 block 顺序抽取；`ToolExecution` 步骤以 `tool_use_id` + `tool_name` + `timestamp` 的形式生成，与 `AIChunk.tool_executions` 里的条目一一对应（可通过 `tool_use_id` 交叉查找真实 `ToolExecution`）；`Interruption` 变体由 `Emit interruption semantic step for interrupt-marker messages` Requirement 负责产出。

当 `build_chunks_with_subagents` 有已解析的 subagent 时，`SubagentSpawn` 步骤 MUST 按 subagent 对应的 Task `tool_use_id` 查找 `semantic_steps` 中的同 id `ToolExecution`，并被 insert 在该 step 之后（相邻位置）；SHALL NOT 统一追加到末尾。若找不到对应 Task `ToolExecution`（异常兜底），MAY 退化为 append 到末尾并记录一条 `tracing::warn!`。

#### Scenario: AIChunk with thinking + text + tool
- **WHEN** an assistant response contains a thinking block, a text block, then a `tool_use`
- **THEN** the semantic steps SHALL be emitted in that exact order: `Thinking` → `Text` → `ToolExecution`

#### Scenario: SubagentSpawn step inserted after the matching Task ToolExecution
- **WHEN** an `AIChunk` 的 `semantic_steps` 依次包含 `ToolExecution(Read)` → `ToolExecution(Task, tool_use_id=t_task)` → `ToolExecution(Grep)`，且对应 `Task` 解析出一个 subagent
- **THEN** 最终 `semantic_steps` 顺序 MUST 为 `ToolExecution(Read)` → `ToolExecution(Task, t_task)` → `SubagentSpawn(placeholder=subagent.session_id)` → `ToolExecution(Grep)`

#### Scenario: 多个 Task 各自插入对应 subagent
- **WHEN** 一个 AIChunk 的 responses 中依次出现 `Task(t1)`、`Task(t2)`，分别匹配到 subagent A / B
- **THEN** `semantic_steps` 中 `SubagentSpawn(A)` MUST 紧随 `ToolExecution(Task, t1)`，`SubagentSpawn(B)` MUST 紧随 `ToolExecution(Task, t2)`

#### Scenario: 未解析的 Task 不产生 SubagentSpawn
- **WHEN** 某个 `Task` `tool_use` 的 `Resolution::Orphan`
- **THEN** 该 Task 的 `ToolExecution` 保留在 `semantic_steps` 中，其后 SHALL NOT 出现与它对应的 `SubagentSpawn`

### Requirement: Emit CompactChunks at compaction boundaries

The system SHALL emit a `CompactChunk` whenever a `ParsedMessage` with `is_compact_summary == true` is encountered, preserving the summary text and boundary timestamp. 在产出 `CompactChunk` 之前，任何正在累积的 `AIChunk` buffer SHALL 先 flush。

#### Scenario: Session with one compaction
- **WHEN** the session contains exactly one `ParsedMessage` with `is_compact_summary == true`
- **THEN** exactly one `CompactChunk` SHALL be emitted at that position with the message's timestamp and textual summary

#### Scenario: Compaction flushes pending assistant buffer
- **WHEN** a compact summary message arrives while an assistant buffer of 2 responses is in progress
- **THEN** the system SHALL first flush the buffered `AIChunk` and THEN emit the `CompactChunk`

### Requirement: Emit slash commands as both UserChunk and AIChunk.slash_commands

Slash 命令消息（content 以 `<command-name>/xxx</command-name>` 起首的 **非 `isMeta`** user 消息，可附加 `<command-message>` 和 `<command-args>`）SHALL 同时产出两处表示，对齐原版 TS 的 UserGroup 气泡 + AIGroup SlashItem 双重渲染：

1. 作为独立 `UserChunk` 发出（content 保留原始 XML，UI 侧由 `cleanDisplayText` 清洗为 `/name args` 气泡展示）；
2. 把提取的 `SlashCommand { name, message, args, message_uuid, timestamp, instructions }` 挂到紧邻下一个 `AIChunk` 的 `slash_commands` 字段。

`SlashCommand.instructions` SHALL 取自 `is_meta=true` 且 `parent_uuid` 指向该 slash user 消息 `uuid` 的 follow-up user 消息的首个非空 text block；若不存在则为 `None`。实现 SHALL 在 chunk-building 入口预扫一次消息序列建立 `parent_uuid → text` 映射，避免依赖消息到达顺序。

**紧邻约束**：pending slash MUST 只能挂到紧随其后的 `AIChunk`。若在 pending slash 与下一个 `AIChunk` 之间出现一条普通 user 消息（非 slash、非 `tool_result`-only、非 `<local-command-stdout>`），实现 SHALL 在产出该普通 `UserChunk` 前清空 pending slash——对齐原版 `extractPrecedingSlashInfo` "只看紧邻前 UserGroup" 的语义。

#### Scenario: Slash message adjacent to assistant response
- **WHEN** a slash user message with content `<command-name>/commit</command-name><command-message>commit</command-message>` is directly followed by an assistant response, with no intervening non-slash user message
- **THEN** the output SHALL contain a `UserChunk` (preserving raw XML content) followed by an `AIChunk` whose `slash_commands` contains one entry with `name="commit"`, `message=Some("commit")`, `args=None`, and `message_uuid` equal to the slash message's uuid

#### Scenario: Slash arguments extracted
- **WHEN** a slash user message contains `<command-name>/review-pr</command-name><command-message>review-pr</command-message><command-args>123</command-args>`
- **THEN** the extracted `SlashCommand` SHALL have `name="review-pr"`, `message=Some("review-pr")`, `args=Some("123")`

#### Scenario: Slash instructions sourced from isMeta follow-up
- **WHEN** a slash user message with uuid `"s1"` is followed by an `is_meta=true` user message whose `parent_uuid == "s1"` carries a text block `"Review this session..."`, then an assistant response
- **THEN** the resulting `AIChunk.slash_commands[0].instructions` SHALL equal `Some("Review this session...")`

#### Scenario: Non-slash isMeta message is not a slash source
- **WHEN** an `is_meta=true` user message contains a system-reminder injection without `<command-name>` tags and is not a follow-up for any slash uuid
- **THEN** no `SlashCommand` SHALL be extracted from it; the message SHALL still contribute `tool_result` blocks to the pending assistant buffer per the `is_meta` filter rule

#### Scenario: Slash command with no following AIChunk
- **WHEN** a slash user message appears at the end of a session with no subsequent assistant response
- **THEN** the slash's `UserChunk` SHALL still be emitted, but the extracted `SlashCommand` SHALL be discarded without error

#### Scenario: Normal user message between slash and AI drops pending slash
- **WHEN** the stream is `slash user message → non-slash user message → assistant message`
- **THEN** the resulting `AIChunk.slash_commands` SHALL be empty; the slash MUST NOT be attached because a non-slash `UserChunk` was emitted between them

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

### Requirement: Embed teammate messages into AIChunk

The system SHALL detect user messages classified as teammate replies (per `team-coordination-metadata::Detect teammate messages`) and MUST NOT emit `UserChunk` for them. 检测到的 teammate 消息 SHALL 被解析为 `TeammateMessage` 并嵌入到下一个 flush 出的 `AIChunk.teammate_messages` 中——而不是丢弃（旧行为）。

实现 SHALL 在 chunk-building 主循环中维护 `pending_teammates: Vec<TeammateMessage>` 缓冲区：

1. 遇到 teammate user 消息时，调用 `parse_teammate_attrs` 解析 `teammate_id / color / summary / body`，叠加 `detect_noise` / `detect_resend` 预算结果与 `token_count` 估算，封装成 `TeammateMessage` 并 push 到 `pending_teammates`；**SHALL NOT** flush 当前 assistant buffer，**SHALL NOT** 产 `UserChunk`，主循环继续。
2. 在 `flush_buffer` 即将产出 `AIChunk` 之前，对 `pending_teammates` 内每条 teammate 调用 `link_teammate_to_send_message(&teammate, &chain)`（详见 `team-coordination-metadata::Link teammate messages to triggering SendMessage`），把 `reply_to_tool_use_id` 字段填好。
3. 把 pending teammates 整批 move 到新构造的 `AIChunk.teammate_messages` 字段；清空 `pending_teammates`。
4. 若主循环结束时 `pending_teammates` 非空（无后续 AIChunk 接收），SHALL 把它们追加到最后一个已 emit 的 AIChunk 的 `teammate_messages`；若整个 session 没有任何 AIChunk，SHALL 静默丢弃这些 pending teammate（罕见边界）。

回滚开关：`cdt-analyze::chunk::builder` 顶部 `const EMBED_TEAMMATES: bool = true;`；为 `false` 时 SHALL 退回旧行为——teammate user 消息直接 `continue`，`AIChunk.teammate_messages` 永远为空。该常量 MUST 与 enrichment 函数（`apply_teammate_embed`）和调用点同一轮 Edit 落地，不得分批提交（避免 clippy `dead_code`）。

`TeammateMessage` 结构体定义在 `cdt-core::chunk` 模块（与 `AIChunk` 共生命周期），字段含义详见 `ipc-data-api::Expose teammate messages on AIChunk`。

#### Scenario: Teammate message does not produce UserChunk
- **WHEN** the input is a real user message followed by an assistant turn whose tail user message is `<teammate-message teammate_id="alice" summary="hi">body</teammate-message>`
- **THEN** the output chunks SHALL contain exactly one `UserChunk` (the real user) and one `AIChunk`，且 SHALL NOT 包含任何代表 teammate 消息的 `UserChunk`

#### Scenario: Teammate message embedded into next AIChunk
- **WHEN** the message stream is `assistant(SendMessage→alice) → user(<teammate-message teammate_id="alice" summary="ok">body</teammate-message>) → assistant("got it")`
- **THEN** the resulting chunk list SHALL contain two `AIChunk` entries: 第一个含 SendMessage tool execution；第二个的 `teammate_messages` SHALL 含一条 `TeammateMessage { teammate_id: "alice", summary: Some("ok"), body: "body", reply_to_tool_use_id: Some(<sendmessage_tool_use_id>) }`

#### Scenario: Trailing teammate message attaches to last AIChunk
- **WHEN** the stream ends with a teammate user message after the last AIChunk has been emitted, with no further assistant message
- **THEN** the teammate SHALL be appended to the last emitted `AIChunk.teammate_messages` (instead of being dropped)

#### Scenario: Orphan teammate when no AIChunk exists
- **WHEN** the entire session contains only teammate user messages and no assistant message
- **THEN** the chunk list SHALL be empty, and the teammate messages SHALL be silently discarded; the system SHALL NOT panic

#### Scenario: EMBED_TEAMMATES=false reverts to legacy drop behavior
- **WHEN** the constant `EMBED_TEAMMATES` is set to `false` and chunk-building runs over a session with teammate messages
- **THEN** every teammate user message SHALL be skipped (legacy `continue` behavior), and every produced `AIChunk.teammate_messages` SHALL be empty

