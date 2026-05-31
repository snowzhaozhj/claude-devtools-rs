# chunk-building Specification

## Purpose

把 `ParsedMessage` 流转换为 `UserChunk` / `AIChunk` / `SystemChunk` / `CompactChunk` 四类独立 chunk：合并连续 assistant、按 sidechain / hard-noise 过滤、提取 SemanticStep（thinking / text / tool / subagent spawn / interruption）、识别 slash 命令、嵌入 teammate 消息、在 compact 边界处 flush 并产出 `CompactChunk`。本 capability 是数据 pipeline 的核心算法层，决定了前端 UI 的对话流外观。
## Requirements
### Requirement: Build independent chunks from classified messages

系统 SHALL 把一段 `ParsedMessage` 序列转换为四类独立 chunk 的序列：`UserChunk`、`AIChunk`、`SystemChunk`、`CompactChunk`。chunk 之间 SHALL NOT 形成配对——`UserChunk` 不"拥有"其后的 `AIChunk`。连续的 assistant 消息 SHALL 被合并到同一个 `AIChunk.responses` 中，直到遇到真实用户消息、`SystemChunk` 对应的 `<local-command-stdout>` 消息、`CompactChunk` 对应的 compact summary 消息或输入末尾时 flush。

`AIChunk` SHALL 暴露 `slash_commands: Vec<SlashCommand>` 字段，包含紧邻前一条 slash user 消息提取出的 slash 命令（详见 `Emit slash commands as both UserChunk and AIChunk.slash_commands` Requirement）。默认为空数组。

`AIChunk` SHALL 同时暴露 `teammate_messages: Vec<TeammateMessage>` 字段，包含被嵌入到该 turn 的 teammate user 消息（详见 `Embed teammate messages into AIChunk` Requirement）。默认为空数组——序列化时通过 `skip_serializing_if = "Vec::is_empty"` 在无 teammate 时省略字段，以保持老快照与老 IPC payload 兼容。

#### Scenario: User question followed by AI response
- **WHEN** 输入是一条真实 user 消息后跟一条 assistant 消息
- **THEN** 输出 SHALL 为按输入顺序排列的一条 `UserChunk` 和一条独立 `AIChunk`

#### Scenario: Multiple assistant turns before next user input
- **WHEN** 若干 assistant 消息连续出现而其间无真实 user 消息
- **THEN** 它们 SHALL 被合并到同一个 `AIChunk`，`responses` 字段按时间序保留所有 assistant 消息

#### Scenario: Assistant buffer flushed by following user message
- **WHEN** 累积了 N 条响应的 assistant buffer 之后到达一条真实 user 消息
- **THEN** 系统 SHALL 在新的 `UserChunk` 之前先 emit 已累积的 `AIChunk`

#### Scenario: Command output appears inline
- **WHEN** 输入流中出现 content 严格被 `<local-command-stdout>...</local-command-stdout>` 包裹的 user 消息
- **THEN** 系统 SHALL 为它单独 emit 一个 `SystemChunk`，不并入相邻 `AIChunk`，且任何在途 assistant buffer SHALL 先被 flush

#### Scenario: AIChunk includes slash commands from preceding slash user message
- **WHEN** 一条 slash user 消息（content 以 `<command-name>/xxx</command-name>` 起首、`is_meta=false`）紧随其后是一条 assistant 响应
- **THEN** 产出的 `AIChunk.slash_commands` 字段 SHALL 含抽取出的 slash 命令

#### Scenario: AIChunk teammate_messages defaults to empty
- **WHEN** 一个 turn 中无任何 teammate reply，由其构造的 `AIChunk`
- **THEN** 其 `teammate_messages` 字段 SHALL 为空 `Vec`，IPC 序列化结果 SHALL 不包含 `teammateMessages` 键（由 `skip_serializing_if = "Vec::is_empty"` 控制）

### Requirement: Filter sidechain and hard-noise messages

系统 SHALL 在构造 chunk 之前排除 `is_sidechain == true` 的消息以及 `MessageCategory::HardNoise(_)` 的消息。被过滤掉的消息 SHALL NOT 影响 chunk 顺序、指标或语义步骤。`MessageCategory::Interruption` 类别的消息 MUST NOT 被此过滤器排除——它们在 chunk-building 主循环中以语义步骤形式处理（详见 `Emit interruption semantic step for interrupt-marker messages` Requirement）。

#### Scenario: Sidechain subagent messages in main stream
- **WHEN** 输入含 `is_sidechain = true` 的消息
- **THEN** 这些消息 SHALL NOT 出现在主线程任意 chunk 中，SHALL NOT 计入任何 `ChunkMetrics`

#### Scenario: Hard-noise messages dropped before chunk construction
- **WHEN** 输入含被分类为 `MessageCategory::HardNoise(_)` 的消息（synthetic assistant 占位、空 command output 等）
- **THEN** 系统 SHALL 在构造 chunk 之前丢弃它们，SHALL NOT 为它们 emit chunk

#### Scenario: Interruption category is not filtered as noise
- **WHEN** 输入含 `MessageCategory::Interruption` 的消息
- **THEN** 该消息 SHALL NOT 被 sidechain / hard-noise 过滤器丢弃，chunk-building SHALL 按 interruption 语义步骤规则处理它

### Requirement: Compute per-chunk metrics

每个 chunk SHALL 暴露 `timestamp`、可选的 `duration` 与 `metrics`，其中 `ChunkMetrics` 包含：`input_tokens`、`output_tokens`、`cache_creation_tokens`、`cache_read_tokens`、`tool_count` 与可选 `cost_usd`。

在 `team-coordination-metadata` 能力把端到端 subagent 候选装载接入 `build_chunks` 默认路径之前，`tool_count` SHALL 统计 `AIChunk.responses` 中所有 `tool_use` 块（包含 `Task` 调用）；`cost_usd` SHALL 取 `None`。`team-coordination-metadata` 移植完成后，`tool_count` 将按 Task 过滤语义修正，`cost_usd` 在价格表引入后改为 `Some(_)`。

#### Scenario: AIChunk with multiple tool uses
- **WHEN** 一个 `AIChunk` 在所有 assistant responses 中合计含 3 个 `tool_use` 块
- **THEN** 其 `metrics.tool_count` SHALL 等于 3

#### Scenario: UserChunk without token usage
- **WHEN** 一个 `UserChunk` 无 usage 数据
- **THEN** 其 metrics 中所有 token 字段 SHALL 全为零，`cost_usd` SHALL 为 `None`

#### Scenario: UserChunk duration is unset
- **WHEN** 一个 `UserChunk` 由恰好一条 `ParsedMessage` 产出
- **THEN** 其 `duration` SHALL 为 `None`

### Requirement: Link tool uses to tool results

`AIChunk` SHALL 在默认 `build_chunks` 路径中暴露已填充的 `tool_executions: Vec<ToolExecution>` 字段：每个 `tool_use` 块都对应一条 `ToolExecution` 记录（由 `tool-execution-linking` capability 产出），未配对的 `tool_use` 以 orphan 形式保留（`output = Missing`、`end_ts = None`）。

#### Scenario: Tool executions populated during chunk build
- **WHEN** `build_chunks` 跑完一段含 assistant `tool_use` 块的 session
- **THEN** 每个所属 `AIChunk.tool_executions` SHALL 为每个 `tool_use` 各含一条 `ToolExecution`，按起源 assistant 消息 `uuid` 分发

#### Scenario: Tool result appears in a later user message
- **WHEN** 一条 assistant `tool_use` 后跟一条 user 消息含其匹配的 `tool_result`
- **THEN** 对应 `AIChunk.tool_executions` 条目 SHALL 同时暴露 `start_ts` 与 `end_ts`，并把 result content 保留为 `output`

#### Scenario: Tool use with no matching result (orphan)
- **WHEN** 一条 assistant `tool_use` 在 session 中无任何 `tool_result` 匹配
- **THEN** 对应 `AIChunk.tool_executions` 条目 SHALL 设 `output = Missing`、`end_ts = None`、`is_error = false`，不 panic

### Requirement: Filter Task tool uses when subagent data is available

`AIChunk.tool_executions` SHALL 在 subagent 候选可用时省略已解析的 `Task` `tool_use`；orphan Task（无匹配 subagent）SHALL 保留。chunk-building capability 自身不调用 filter——纯函数 `tool_linking::filter_resolved_tasks` 在 `port-tool-execution-linking` 内实现并由独立测试覆盖；把 filter 接入 `build_chunks` 默认路径、装载 subagent 候选、同步更新 `ChunkMetrics::tool_count` 的工作由 `team-coordination-metadata` 完成。

#### Scenario: Task call with resolved subagent
- **WHEN** 一条 `Task` `tool_use` 在调用方传入的 candidate 池中匹配到 subagent
- **THEN** `tool_linking::filter_resolved_tasks` SHALL 把该条目从可变 `Vec<ToolExecution>` 中移除，仅保留非 Task 与 orphan Task

#### Scenario: Task call with no matching subagent
- **WHEN** 一条 `Task` `tool_use` 解析为 `Resolution::Orphan`
- **THEN** 它 SHALL 保留在 `AIChunk.tool_executions` 列表中

#### Scenario: Default chunk build does not filter Tasks
- **WHEN** `build_chunks` 调用时未传任何 subagent candidate 池
- **THEN** Task tool execution SHALL 保留在 `AIChunk.tool_executions`；下游消费者 MAY 仍显式调用 `filter_resolved_tasks`；端到端默认路径过滤推迟到 `team-coordination-metadata`

### Requirement: Attach subagents to AIChunks

`AIChunk` SHALL 暴露一个稳定字段以挂载由该 chunk 生成的 subagent Process 记录。chunk-building 只负责结构占位：字段默认空列表；真实的 Process 归集由 `team-coordination-metadata` capability 履行。

#### Scenario: Structure slot exists
- **WHEN** 一个 `AIChunk` 仅在 chunk-building capability 下被构造
- **THEN** 其 subagents 字段 SHALL 存在且为空

#### Scenario: Single subagent spawn
- **WHEN** 一个 `AIChunk` 的 assistant 消息 spawn 了一个 subagent
- **THEN** 在 `team-coordination-metadata` 跑完后，`AIChunk.subagents` SHALL 含一条 Process 记录，附自身 session id、时间戳、metrics、可选 team 元数据（由该 capability 验证）

### Requirement: Extract semantic steps for AIChunks

系统 SHALL 为每个 `AIChunk` 按时间序提取一组 `SemanticStep`（thinking / text output / tool execution / subagent spawn / interruption）以驱动 UI 可视化。`Thinking` 与 `Text` 步骤从 `ParsedMessage.content` 中按 block 顺序抽取；`ToolExecution` 步骤以 `tool_use_id + tool_name + timestamp` 形式产出，与 `AIChunk.tool_executions` 中条目一一对应（可通过 `tool_use_id` 交叉查找真实 `ToolExecution`）；`Interruption` 变体由 `Emit interruption semantic step for interrupt-marker messages` Requirement 负责产出。

当 `build_chunks_with_subagents` 持有已解析的 subagent 时，`SubagentSpawn` 步骤 MUST 按 subagent 对应的 Task `tool_use_id` 在 `semantic_steps` 中查找同 id 的 `ToolExecution`，并被 insert 在该 step 之后（相邻位置）；SHALL NOT 一律 append 到末尾。若找不到对应 Task `ToolExecution`（异常兜底），MAY 退化为 append 到末尾并记录一条 `tracing::warn!`。

#### Scenario: AIChunk with thinking + text + tool
- **WHEN** 一个 assistant response 含一个 thinking 块、一个 text 块、再一个 `tool_use`
- **THEN** 语义步骤 SHALL 按这个顺序 emit：`Thinking` → `Text` → `ToolExecution`

#### Scenario: SubagentSpawn step inserted after the matching Task ToolExecution
- **WHEN** 一个 `AIChunk` 的 `semantic_steps` 依次为 `ToolExecution(Read)` → `ToolExecution(Task, tool_use_id=t_task)` → `ToolExecution(Grep)`，且对应 `Task` 解析出一个 subagent
- **THEN** 最终 `semantic_steps` 顺序 MUST 为 `ToolExecution(Read)` → `ToolExecution(Task, t_task)` → `SubagentSpawn(placeholder=subagent.session_id)` → `ToolExecution(Grep)`

#### Scenario: 多个 Task 各自插入对应 subagent
- **WHEN** 一个 AIChunk 的 responses 中依次出现 `Task(t1)`、`Task(t2)`，分别匹配到 subagent A / B
- **THEN** `semantic_steps` 中 `SubagentSpawn(A)` MUST 紧随 `ToolExecution(Task, t1)`，`SubagentSpawn(B)` MUST 紧随 `ToolExecution(Task, t2)`

#### Scenario: 未解析的 Task 不产生 SubagentSpawn
- **WHEN** 某个 `Task` `tool_use` 解析为 `Resolution::Orphan`
- **THEN** 该 Task 的 `ToolExecution` 仍保留在 `semantic_steps` 中，其后 SHALL NOT 出现对应的 `SubagentSpawn`

### Requirement: Emit CompactChunks at compaction boundaries

系统 SHALL 在遇到 `is_compact_summary == true` 的 `ParsedMessage` 时 emit 一条 `CompactChunk`，保留 summary 文本与边界时间戳。在产出 `CompactChunk` 之前，任何在途 `AIChunk` buffer SHALL 先被 flush。

#### Scenario: Session with one compaction
- **WHEN** session 中恰好含一条 `is_compact_summary == true` 的 `ParsedMessage`
- **THEN** SHALL 在该位置 emit **恰好**一条 `CompactChunk`，附带该消息时间戳与 summary 文本

#### Scenario: Compaction flushes pending assistant buffer
- **WHEN** 在 assistant buffer 已累积 2 条响应时到达 compact summary 消息
- **THEN** 系统 SHALL 先 flush 已 buffer 的 `AIChunk`，再 emit `CompactChunk`

### Requirement: Emit slash commands as both UserChunk and AIChunk.slash_commands

slash 命令消息（content 以 `<command-name>/xxx</command-name>` 起首的**非 `isMeta`** user 消息，可附加 `<command-message>` 与 `<command-args>`）SHALL 同时产出两处表示，对齐原版 TS 的 UserGroup 气泡 + AIGroup SlashItem 双重渲染：

1. 作为独立 `UserChunk` emit（content 保留原始 XML，UI 侧由 `cleanDisplayText` 清洗为 `/name args` 气泡显示）；
2. 把抽取出的 `SlashCommand { name, message, args, message_uuid, timestamp, instructions }` 挂到紧邻下一个 `AIChunk` 的 `slash_commands` 字段。

`SlashCommand.instructions` SHALL 取自 `is_meta=true` 且 `parent_uuid` 指向该 slash user 消息 `uuid` 的 follow-up user 消息的首个非空 text block；不存在时为 `None`。实现 SHALL 在 chunk-building 入口预扫一次消息序列建立 `parent_uuid → text` 映射，避免依赖消息到达顺序。

**紧邻约束**：pending slash MUST 只能挂到紧随其后的 `AIChunk`。若在 pending slash 与下一个 `AIChunk` 之间出现一条普通 user 消息（非 slash、非 `tool_result`-only、非 `<local-command-stdout>`），实现 SHALL 在产出该普通 `UserChunk` 之前清空 pending slash——对齐原版 `extractPrecedingSlashInfo` "只看紧邻前 UserGroup" 的语义。

#### Scenario: Slash message adjacent to assistant response
- **WHEN** 一条 slash user 消息 content 为 `<command-name>/commit</command-name><command-message>commit</command-message>` 紧随一条 assistant 响应，中间无任何非 slash user 消息
- **THEN** 输出 SHALL 含一条 `UserChunk`（保留原始 XML content）后跟一条 `AIChunk`，其 `slash_commands` 含一条 `name="commit"`、`message=Some("commit")`、`args=None`、`message_uuid` 等于该 slash 消息 uuid 的条目

#### Scenario: Slash arguments extracted
- **WHEN** 一条 slash user 消息含 `<command-name>/review-pr</command-name><command-message>review-pr</command-message><command-args>123</command-args>`
- **THEN** 抽取得到的 `SlashCommand` SHALL 为 `name="review-pr"`、`message=Some("review-pr")`、`args=Some("123")`

#### Scenario: Slash instructions sourced from isMeta follow-up
- **WHEN** uuid 为 `"s1"` 的 slash user 消息后跟一条 `is_meta=true` 的 user 消息（其 `parent_uuid == "s1"`）携带文本块 `"Review this session..."`，再后跟 assistant 响应
- **THEN** 产出的 `AIChunk.slash_commands[0].instructions` SHALL 等于 `Some("Review this session...")`

#### Scenario: Non-slash isMeta message is not a slash source
- **WHEN** 一条 `is_meta=true` 的 user 消息含 system-reminder 注入但无 `<command-name>` 标签，且不是任何 slash uuid 的 follow-up
- **THEN** SHALL NOT 从中抽出 `SlashCommand`；该消息 SHALL 仍按 `is_meta` 过滤规则把 `tool_result` 块贡献给 pending assistant buffer

#### Scenario: Slash command with no following AIChunk
- **WHEN** 一条 slash user 消息出现在 session 末尾，其后无 assistant 响应
- **THEN** slash 的 `UserChunk` SHALL 仍被 emit，但抽取出的 `SlashCommand` SHALL 被丢弃，不抛错

#### Scenario: Normal user message between slash and AI drops pending slash
- **WHEN** 流为 `slash user 消息 → 非 slash user 消息 → assistant 消息`
- **THEN** 产出的 `AIChunk.slash_commands` SHALL 为空；slash MUST NOT 被挂载——因为两者之间已 emit 了一条非 slash `UserChunk`

### Requirement: Emit interruption semantic step for interrupt-marker messages

系统 SHALL 在 chunk-building 遇到 `MessageCategory::Interruption` 消息时，把一条 `SemanticStep::Interruption { text, timestamp }` 追加到紧邻前一个（或当前正 buffer 的）`AIChunk`。若该 interruption 到达时无任何活跃或 buffer 中的 `AIChunk`，则 SHALL 静默丢弃（对齐原版：独立中断不产出新 chunk 类型）。MUST NOT 为 interruption emit 独立 `Chunk` variant——保持 chunk 列表的四种类型不变。

#### Scenario: Interruption appended to the current assistant buffer
- **WHEN** 消息流为 `assistant(text) → user("[Request interrupted by user for tool use]")`
- **THEN** 产出的单一 `AIChunk.semantic_steps` 末尾 SHALL 为 `SemanticStep::Interruption { text: <原始 interrupt 文本>, timestamp: <user 消息 ts> }`，`AIChunk` 在其后 SHALL 被 flush

#### Scenario: Interruption appended to the last AIChunk when buffer is empty
- **WHEN** 消息流先 flush 一条 `assistant(text)`，再到达一条 `user("[Request interrupted by user]")`，期间无新 assistant 消息
- **THEN** interruption SHALL 被追加到最近一条已 emit `AIChunk` 的 `semantic_steps`

#### Scenario: Interruption without any prior assistant
- **WHEN** 消息流以一条 `MessageCategory::Interruption` 起首，其前无任何 `AIChunk`
- **THEN** SHALL NOT 为它 emit chunk，chunk 列表 SHALL 与无 interruption 时一致

#### Scenario: Multiple interruptions in a row
- **WHEN** 一条 assistant 响应后跟两条连续 interrupt 消息
- **THEN** 每条 interruption SHALL 在同一个 `AIChunk.semantic_steps` 中产生一条 `SemanticStep::Interruption`，原始顺序保持

### Requirement: Embed teammate messages into AIChunk

系统 SHALL 检测被分类为 teammate reply 的 user 消息（按 `team-coordination-metadata::Detect teammate messages`）且 MUST NOT 为它们 emit `UserChunk`。检测到的 teammate 消息 SHALL 被解析为 `TeammateMessage` 并嵌入到下一个 flush 出的 `AIChunk.teammate_messages`，而非丢弃。

`TeammateMessage` 字段语义详见 `ipc-data-api::Expose teammate messages on AIChunk`。每条 `TeammateMessage` SHALL 携带 `reply_to_tool_use_id` 字段（在 flush 之前由 chunk-building 协同 `team-coordination-metadata::Link teammate messages to triggering SendMessage` 填充）。

**flush 行为契约**：

1. 主循环遇到 teammate user 消息时 SHALL 缓冲该 message，**不**立刻 flush 在途 assistant buffer，**不**产生 `UserChunk`。
2. 在产出下一条 `AIChunk` 时 SHALL 把全部缓冲 teammate 整批挂入该 `AIChunk.teammate_messages`。
3. 主循环结束时若仍有缓冲 teammate（无后续 `AIChunk` 接收），SHALL 把它们追加到最后一条已 emit 的 `AIChunk.teammate_messages`；若整个 session 没有任何 `AIChunk`，缓冲 teammate SHALL 被静默丢弃。
4. 当其它 flush 路径（普通 user 消息 / `<local-command-stdout>` SystemChunk / Compact 边界 / Slash user / Interruption marker 五处）触发 flush 时，若 assistant buffer 为空但 teammate 缓冲非空，SHALL 产出一条 `responses` 为空、`teammate_messages` 非空的 empty-AI `AIChunk` 收容这些 teammate。该 empty-AI 的 `chunk_id` base 取首条缓冲 teammate 的 uuid、`timestamp` 取首条缓冲 teammate timestamp、`metrics` 为零、`semantic_steps` / `tool_executions` / `subagents` 为空、`slash_commands` 由调用方既有 pending slash 缓冲消费。该规则保证 teammate-message emit 顺序在 chunk 列表中严格遵循 timestamp 早于后续 chunk，避免序列倒置。
5. Interruption 分支触发 empty-AI 后 SHALL 把 `SemanticStep::Interruption` 追加到刚产出的 empty-AI 的 `semantic_steps`——这是既有 "interrupt 挂到最近 AIChunk" 契约的自然延续。

#### Scenario: Teammate message does not produce UserChunk

- **WHEN** 输入是一条真实 user 消息后跟一个 assistant turn，其末尾 user 消息为 `<teammate-message teammate_id="alice" summary="hi">body</teammate-message>`
- **THEN** 输出 chunk 列表 SHALL **恰好**含一条 `UserChunk`（真实 user）与一条 `AIChunk`，且 SHALL NOT 含任何代表 teammate 消息的 `UserChunk`

#### Scenario: Teammate message embedded into next AIChunk

- **WHEN** 消息流为 `assistant(SendMessage→alice) → user(<teammate-message teammate_id="alice" summary="ok">body</teammate-message>) → assistant("got it")`
- **THEN** 产出的 chunk 列表 SHALL 含两条 `AIChunk`：第一条带 SendMessage tool execution；第二条 `teammate_messages` SHALL 含一条 `TeammateMessage { teammate_id: "alice", summary: Some("ok"), body: "body", reply_to_tool_use_id: Some(<sendmessage_tool_use_id>) }`

#### Scenario: Trailing teammate message attaches to last AIChunk

- **WHEN** 流的末尾在最后一条 AIChunk 已 emit 之后到达一条 teammate user 消息，无后续 assistant 消息
- **THEN** teammate SHALL 被追加到最后一条已 emit 的 `AIChunk.teammate_messages`（而非被丢弃）

#### Scenario: Orphan teammate when no AIChunk exists

- **WHEN** 整个 session 仅含 teammate user 消息，无任何 assistant 消息
- **THEN** chunk 列表 SHALL 为空，teammate 消息 SHALL 被静默丢弃；系统 SHALL NOT panic

#### Scenario: Teammate message before non-AI user message produces standalone empty-AI chunk

- **WHEN** 消息流为 `user(<teammate-message ...>body</teammate-message>) → user("real input")`，其间无任何 assistant 消息或所有 assistant 都被 `MessageCategory::HardNoise(_)` 过滤
- **THEN** 产出的 chunk 列表 SHALL 含两条按时序排列的 chunk：先 empty-AI `AIChunk { responses: [], teammate_messages: [TeammateMessage { body: "body", .. }], slash_commands: [] }`，再 `UserChunk { content: "real input", .. }`
- **AND** empty-AI 的 `teammate_messages[0].reply_to_tool_use_id` SHALL 为 `None`（无前驱 SendMessage 可配对）

#### Scenario: Teammate message before SystemChunk-triggering user message produces standalone empty-AI chunk

- **WHEN** 消息流为 `user(<teammate-message ...>body</teammate-message>) → user("<local-command-stdout>ls output</local-command-stdout>")`，其间无 assistant
- **THEN** 产出的 chunk 列表 SHALL 含两条按时序排列的 chunk：先 empty-AI `AIChunk { responses: [], teammate_messages: [..] }`，再 `SystemChunk { content_text: "ls output", .. }`

#### Scenario: Teammate message before Compact boundary produces standalone empty-AI chunk

- **WHEN** 消息流为 `user(<teammate-message ...>body</teammate-message>) → compact_summary("conversation summary text")`，其间无 assistant
- **THEN** 产出的 chunk 列表 SHALL 含两条按时序排列的 chunk：先 empty-AI `AIChunk { responses: [], teammate_messages: [..] }`，再 `CompactChunk { summary_text: "conversation summary text", .. }`

#### Scenario: Slash command then teammate then real user emits empty-AI with both slash and teammate

- **WHEN** 消息流为 `user(slash "<command-name>/clear</command-name>") → user(<teammate-message ...>body</teammate-message>) → user("real input")`，其间无 assistant
- **THEN** 产出的 chunk 列表 SHALL 含三条按时序排列的 chunk：
  - `chunks[0]` SHALL 为 slash 的 `UserChunk`
  - `chunks[1]` SHALL 为 empty-AI `AIChunk { responses: [], slash_commands: [{ name: "clear", .. }], teammate_messages: [{ body: "body", .. }] }`
  - `chunks[2]` SHALL 为真实 user 的 `UserChunk`

#### Scenario: Teammate message before interrupt marker appends to empty-AI

- **WHEN** 消息流为 `user(<teammate-message ...>body</teammate-message>) → interrupt("[Request interrupted by user]")`，其间无 assistant 或所有 assistant 都被 hard-noise 过滤
- **THEN** 产出的 chunk 列表 SHALL 恰好含一条 chunk：empty-AI `AIChunk { responses: [], teammate_messages: [{ body: "body", .. }], semantic_steps: [SemanticStep::Interruption { text: "[Request interrupted by user]", .. }], tool_executions: [], subagents: [], slash_commands: [] }`

### Requirement: CompactChunk carries optional derived metadata

`CompactChunk` SHALL 提供两个可选派生槽位，让 IPC 组装层后置填充 compaction 元数据：

- `tokenDelta: Option<CompactionTokenDelta>` —— compact 边界对应的 token 数差值（含 `preCompactionTokens` / `postCompactionTokens` / `delta`）
- `phaseNumber: Option<u32>` —— 该 compact 在 chunks 中的 phase 编号

两个字段的派生算法与数据来源由 capability `ipc-data-api` 的 Requirement `Expose CompactChunk derived metadata in SessionDetail` 定义。本 capability 仅声明 `CompactChunk` 提供这两个 optional 槽位，并约束 chunk-building 算法层 emit `CompactChunk` 时把这两个字段填 `None`——chunk emission 算法行为不依赖任何 phase / token 派生数据源。

两个字段为 `None` 时序列化 SHALL 省略；非 `None` 时序列化 key SHALL 用 camelCase（`tokenDelta` / `phaseNumber`），与既有 `CompactionTokenDelta` 的 camelCase 序列化对齐。

#### Scenario: Builder emits CompactChunk with derived fields as None

- **WHEN** chunk-building 处理一条 `is_compact_summary == true` 的 `ParsedMessage`
- **THEN** emit 的 `CompactChunk` SHALL 包含 `tokenDelta: None` AND `phaseNumber: None`
- **AND** 既有 `summaryText` / `uuid` / `timestamp` / `metrics` 字段 SHALL 与既有 Requirement `Emit CompactChunks at compaction boundaries` 描述一致（不被本字段加影响）

#### Scenario: CompactChunk serializes derived fields as camelCase when present

- **WHEN** `CompactChunk { tokenDelta: Some(delta), phaseNumber: Some(3), .. }` 被序列化为 JSON
- **THEN** 输出 JSON SHALL 包含 key `tokenDelta`（驼峰）AND `phaseNumber`（驼峰），SHALL NOT 包含 snake_case 形式
- **AND** `tokenDelta` value 形如 `{"preCompactionTokens": ..., "postCompactionTokens": ..., "delta": ...}`

#### Scenario: CompactChunk serializes with optional fields omitted when None

- **WHEN** 一个 `tokenDelta: None` AND `phaseNumber: None` 的 `CompactChunk` 被序列化为 JSON
- **THEN** 输出 JSON object SHALL **不**包含 `tokenDelta` / `phaseNumber` key
- **AND** 反序列化 JSON object 时缺这两个 key SHALL 等价于 `tokenDelta: None` AND `phaseNumber: None`

### Requirement: Expose CompactChunk derived metadata in SessionDetail

`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `CompactChunk` SHALL 携带由 chunks 自身派生填充的两个可选字段（数据形态契约见 capability `chunk-building` 的 Requirement `CompactChunk carries optional derived metadata`）：

- `tokenDelta: Option<CompactionTokenDelta>`
- `phaseNumber: Option<u32>`

派生算法 SHALL 在 IPC 组装层（`cdt-api` 内 `SessionDetail` 构造路径）实现，**不**修改 `cdt-analyze::chunk::builder` 算法层、**不**依赖 `ContextPhaseInfo`。派生函数 signature SHALL 是 `apply_compact_derived(chunks: &mut [Chunk], enabled: bool)`，输入仅 chunks 序列与回滚开关。

具体规则：

- **`phaseNumber`**：派生函数内维护 `compact_counter: u32 = 1`，按 chunks 顺序遍历，每遇 `Chunk::Compact(c)` 就 `compact_counter += 1`，立即赋 `c.phase_number = Some(compact_counter)`。对齐原版 `groupTransformer.ts:295-303` 与 `cdt-analyze::context::session.rs:101` 的"compact 触发新 phase"语义
- **`tokenDelta`**：对每个 `Chunk::Compact(c)` at index `i`，独立查 chunks 自身：
  - `last_ai_before` = `chunks[..i]` 中最后一个 `Chunk::Ai`
  - `first_ai_after` = `chunks[i+1..]` 中第一个 `Chunk::Ai`
  - `pre_tokens` = `last_ai_before` 的 last response 的 `usage` 各字段总和（`input_tokens + output_tokens + cache_read_input_tokens + cache_creation_input_tokens`）；`responses` 全 `usage = None` 时 `pre_tokens = None`
  - `post_tokens` = `first_ai_after` 的 first response 的 `usage` 总和；同上 fallback
  - 若 `pre_tokens` 与 `post_tokens` 都有值 → `c.token_delta = Some(CompactionTokenDelta { pre_compaction_tokens: pre, post_compaction_tokens: post, delta: post as i64 - pre as i64 })`；任一缺值 → `c.token_delta = None`
  - 该算法对齐原版 `groupTransformer.ts:305-315` 的 `findLastAiBefore` + `findFirstAiAfter`，对**连续 compact** 给每个 compact 独立计算（虽然连续 compact 中所有 compact 的 `last_ai_before` / `first_ai_after` 命中同一对 AI，结果相同——这是与原版一致的行为）

序列化 SHALL 使用 camelCase（`tokenDelta` / `phaseNumber`）。`None` 时按 `#[serde(default, skip_serializing_if = "Option::is_none")]` 省略字段。

派生函数 SHALL 接收 `enabled: bool` 参数：调用方在生产代码传顶部 `const COMPACT_DERIVED_ENABLED: bool = true`（统一回滚点），测试代码可直接传 `false` 验回滚路径。`enabled = false` 时派生函数 SHALL 直接返回，不写入任何 `tokenDelta` / `phaseNumber`。

派生 SHALL 在 `get_session_detail` 共享路径（IPC 与 HTTP detail 共用同一组装入口）内调用一次。`list_sessions` / `list_sessions_sync` 等返回 `SessionSummary`（无 chunks）的入口 SHALL 不调用派生。

#### Scenario: Token delta computed from neighboring AI chunks

- **WHEN** session chunks 序列为 `[AIChunk(last response usage total = 30000), CompactChunk(uuid="c-1"), AIChunk(first response usage total = 5000)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `Some(CompactionTokenDelta { preCompactionTokens: 30000, postCompactionTokens: 5000, delta: -25000 })`
- **AND** 序列化 JSON SHALL 包含 `"tokenDelta":{"preCompactionTokens":30000,"postCompactionTokens":5000,"delta":-25000}`

#### Scenario: Token delta None when no AI before compact

- **WHEN** session chunks 序列为 `[UserChunk, CompactChunk(uuid="c-1"), AIChunk(...)]`（compact 之前无 AIChunk）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`
- **AND** 序列化 JSON SHALL **不包含** `tokenDelta` key

#### Scenario: Token delta None when no AI after compact

- **WHEN** session chunks 序列为 `[AIChunk(...), CompactChunk(uuid="c-1")]`（compact 在 chunks 末尾，之后无 AIChunk）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`

#### Scenario: Token delta None when neighboring AI lacks usage data

- **WHEN** session chunks 序列为 `[AIChunk(responses 全部 usage=None), CompactChunk(uuid="c-1"), AIChunk(...)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`（pre_tokens 无法计算）

#### Scenario: Consecutive compacts share identical token delta

- **WHEN** session chunks 序列为 `[AIChunk(last response usage total = 30000), CompactChunk(uuid="c-1"), CompactChunk(uuid="c-2"), AIChunk(first response usage total = 5000)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 等于 `CompactChunk(c-2).tokenDelta`（都是 `Some(CompactionTokenDelta { 30000, 5000, -25000 })`，因为两个 compact 的 `last_ai_before` 与 `first_ai_after` 命中同一对 AI；对齐原版 `groupTransformer.ts:305-315` 的 `findLastAiBefore`/`findFirstAiAfter` 独立查询语义，**不会**因 cdt-analyze 内部 `current_phase_compact_group_id` 覆盖问题让 c-1 拿到 None）

#### Scenario: Phase number assigned by compact ordinal

- **WHEN** session chunks 序列含 `[UserChunk, AIChunk(...), CompactChunk(uuid="c-1"), AIChunk(...)]`（chunks 中的第 1 个 compact）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)`（compact_counter 从 1 起，遇到 c-1 自增到 2）

#### Scenario: Consecutive compacts each get its own phase number

- **WHEN** session chunks 序列含 `[..., CompactChunk(uuid="c-1"), CompactChunk(uuid="c-2"), AIChunk(...)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)` AND `CompactChunk(c-2).phaseNumber` SHALL 为 `Some(3)`

#### Scenario: Phase number stable when compact at end of chunks

- **WHEN** session chunks 序列为 `[AIChunk(...), CompactChunk(uuid="c-1"), AIChunk(...), CompactChunk(uuid="c-2")]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)` AND `CompactChunk(c-2).phaseNumber` SHALL 为 `Some(3)`（派生不依赖 compact 之后是否有 AIChunk）

#### Scenario: Compact followed only by user and system chunks

- **WHEN** session chunks 序列为 `[AIChunk(...), CompactChunk(uuid="c-1"), UserChunk, SystemChunk]`（compact 之后仅 User/System，无 AIChunk）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)`（phaseNumber 派生与"compact 之后必须 AIChunk"无关）
- **AND** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`（tokenDelta 需要 first_ai_after，不存在时 None）

#### Scenario: Rollback flag disables derivation

- **WHEN** 调用派生函数 `apply_compact_derived(chunks, enabled = false)`
- **AND** `chunks` 中含若干 `CompactChunk` 与相邻 `AIChunk` 含完整 usage
- **THEN** 处理后所有 `CompactChunk.tokenDelta` SHALL 为 `None` AND `phaseNumber` SHALL 为 `None`
- **AND** 该 Scenario SHALL 可在单元测试中独立断言（派生函数接收 `enabled: bool` 参数而非依赖运行时不可改的 `const`）

### Requirement: Stable chunk identifiers in SessionDetail

`get_session_detail` 返回的 `SessionDetail.chunks` 中每个 `Chunk` SHALL 暴露 `chunkId` 字段（camelCase 序列化），且同一次返回内所有 `chunkId` MUST 唯一。同一 session 文件内容未变化时，重复调用 `get_session_detail(projectId, sessionId)` MUST 返回相同顺序、相同 `chunkId` 的 chunks。

**统一 `chunkId` 形态**（本 change 引入）：所有 `Chunk` 类型（`AIChunk` / `UserChunk` / `SystemChunk` / `CompactChunk`）的 `chunkId` MUST 形如 `<base>:<n>`（`n` 从 0 起的十进制整数）。`AIChunk` 的 `base` MUST 取 `responses[0].uuid`（空 responses 时 fallback 字面量 `"empty"`）；`UserChunk` / `SystemChunk` / `CompactChunk` 的 `base` MUST 取自身消息 `uuid`。**MUST NOT** 使用裸 `<uuid>` 形态（即使首次出现也必须带 `:0` 后缀），**MUST NOT** 使用 `ai:` / `user:` / `sys:` / `compact:` 等类型前缀——chunk 类型由 `Chunk::kind` 字段区分，**不**靠 `chunkId` 字面前缀。

**Collision-free 兜底**：后端在分配 `chunkId` 时 MUST 维护一个跨所有 chunk 类型共享的 build 阶段全局已分配集合（`HashSet<String>`），命中冲突时 MUST 继续递增 ordinal 后缀 `n` 直到 candidate 未被占用——以兜底 uuid 自身恰好形如 `<base>:<n>` 等极端上游输入下"跨形态撞车"以及"跨类型撞车"的 corner case，确保整体 `chunkId` 集合 MUST 唯一。

#### Scenario: 所有 chunk 首次出现使用 `<uuid>:0`

- **WHEN** `get_session_detail` 返回 `UserChunk` / `SystemChunk` / `CompactChunk` / `AIChunk`，且其 base（`uuid` 或 `responses[0].uuid`）在同一次返回的其余 chunk 中**未**出现过
- **THEN** 该 chunk 的 `chunkId` SHALL 等于 `format!("{base}:0")`
- **AND** SHALL NOT 等于裸 `base`（无后缀）
- **AND** SHALL NOT 含 `ai:` / `user:` / `sys:` / `compact:` 等类型前缀

#### Scenario: 重复 assistant response uuid 仍生成唯一 chunkId

- **WHEN** 一个 session 在 compact/replay 后产生两个 `AIChunk`，且两个 chunk 的 `responses[0].uuid` 相同（值 `"dup"`）
- **THEN** `get_session_detail` 返回的两个 `AIChunk.chunkId` SHALL 分别为 `"dup:0"` 与 `"dup:1"`
- **AND** 整体 `chunks.map(chunk => chunk.chunkId)` MUST 唯一

#### Scenario: 未变化 session 重复调用时 chunkId 稳定

- **WHEN** 同一 `projectId` / `sessionId` 对应的 session JSONL 文件内容未变化
- **AND** caller 连续两次调用 `get_session_detail(projectId, sessionId)`
- **THEN** 两次返回的 `chunks.map(chunk => chunk.chunkId)` SHALL 完全相同

#### Scenario: 重复 user uuid 仍生成唯一 chunkId

- **WHEN** 同一 sessionId 的 JSONL 在 `claude --bg` 启动子会话等场景下出现两条 `uuid` 相同的 user 消息（值 `"u-dup"`）
- **AND** `get_session_detail` 为这两条消息分别构造 `UserChunk`
- **THEN** 两个 `UserChunk.chunkId` SHALL 分别为 `"u-dup:0"` 与 `"u-dup:1"`
- **AND** 整体 `chunks.map(chunk => chunk.chunkId)` MUST 唯一，前端 `{#each ... as chunk (chunk.chunkId)}` MUST NOT 触发 duplicate key 错误

#### Scenario: uuid 与 `<uuid>:<n>` 后缀形态撞车时仍唯一

- **WHEN** 同一次 `get_session_detail` 返回内既有 `uuid == "abc"` 的 user chunk，又有另一条 `uuid == "abc:1"` 的 user chunk
- **AND** `uuid == "abc"` 的 chunk 第二次出现（按统一规则 candidate 应为 `"abc:1"`，但已被 `uuid == "abc:1"` 首次出现产出的 `"abc:1:0"` 之前的 candidate 占用）
- **THEN** 后端 MUST 校验 candidate 是否已被占用
- **AND** MUST 继续递增 ordinal 直到 candidate 未被占用（实际产 `"abc:0"` / `"abc:1:0"` / `"abc:1"` 三条互不撞）
- **AND** 整体 `chunks.map(chunk => chunk.chunkId)` MUST 唯一

#### Scenario: AI chunk 与 user chunk 跨类型不撞

- **WHEN** 同一次 `get_session_detail` 返回内有一条 `AIChunk`（`responses[0].uuid == "x"`）和一条 `UserChunk`（`uuid == "x"`）
- **THEN** 两个 chunk 的 `chunkId` 候选都是 `"x:0"`，全局集合检测冲突
- **AND** 后到的 chunk MUST 递增到 `"x:1"`
- **AND** 两个 `chunkId` SHALL 不相同

### Requirement: Inline embed queued user message as SemanticStep

系统 SHALL 在 chunk-building 时，对 `is_queued_input == true` 的 user 消息：
- 不 flush assistant buffer
- 不产出 UserChunk
- 不清除 pending_slashes
- 将其记录为 pending，在下一次 flush AIChunk 时作为 `SemanticStep::UserMessage { uuid, text, timestamp }` 插入 `semantic_steps` 序列的精确时序位置

时序位置定义：在 pending 记录的 timestamp 之后、第一个 timestamp 更晚的其它 step 之前；若无更晚 step 则追加到末尾。

连续多条 queued_command SHALL 各自产独立 `UserMessage` step，按 timestamp（进而行序）排列，不合并。

末尾 flush 时仍有 pending user messages 的，追加到最后一个 AIChunk 的 semantic_steps 末尾。无 AIChunk 时丢弃（与 orphan teammate 同策略）。

#### Scenario: Queued input does not flush buffer or produce UserChunk
- **WHEN** chunk-building 主循环遇到 `category == User` AND `is_queued_input == true`
- **THEN** assistant buffer 不 flush AND 无 UserChunk 产出 AND pending_slashes 不清除

#### Scenario: UserMessage step appears at correct timeline position
- **WHEN** 用户在 tool_use A（ts=1）和 tool_use B（ts=3）之间发送 queued_command（ts=2）
- **THEN** flush 产出的 AIChunk.semantic_steps 序列中 `UserMessage(ts=2)` 位于 `ToolExecution(A)` 之后、`ToolExecution(B)` 之前

#### Scenario: Multiple queued inputs produce multiple steps
- **WHEN** 同一 AI turn 内出现 2 条 queued_command（ts=2, ts=4）
- **THEN** AIChunk.semantic_steps 含 2 条独立 `UserMessage`，按 timestamp 排序

#### Scenario: Trailing queued input attaches to last AIChunk
- **WHEN** 文件末尾有 queued_command 且无后续 assistant 消息（buffer 空）
- **THEN** 该 UserMessage step 追加到最后一个已 emit 的 AIChunk 的 semantic_steps 末尾

#### Scenario: Orphan queued input without any AIChunk is dropped
- **WHEN** 全文件仅有 queued_command 无任何 assistant 消息
- **THEN** 不产出任何 chunk（静默丢弃）

