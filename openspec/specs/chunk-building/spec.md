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

#### Scenario: Tool executions populated by build_chunks
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

#### Scenario: Default build_chunks does not filter Tasks in this port
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

系统 SHALL 检测被分类为 teammate reply 的 user 消息（按 `team-coordination-metadata::Detect teammate messages`），且 MUST NOT 为它们 emit `UserChunk`。检测到的 teammate 消息 SHALL 被解析为 `TeammateMessage` 并嵌入到下一个 flush 出的 `AIChunk.teammate_messages` 中——而非丢弃（旧行为）。

实现 SHALL 在 chunk-building 主循环中维护 `pending_teammates: Vec<TeammateMessage>` 缓冲：

1. 遇到 teammate user 消息时，调用 `parse_teammate_attrs` 解析 `teammate_id` / `color` / `summary` / `body`，叠加 `detect_noise` / `detect_resend` 预算结果与 `token_count` 估算，封装成 `TeammateMessage` 并 push 到 `pending_teammates`；**SHALL NOT** flush 当前 assistant buffer，**SHALL NOT** 产 `UserChunk`，主循环继续。
2. 在 `flush_buffer` 即将产出 `AIChunk` 之前，对 `pending_teammates` 中每条 teammate 调用 `link_teammate_to_send_message(&teammate, &chain)`（详见 `team-coordination-metadata::Link teammate messages to triggering SendMessage`），把 `reply_to_tool_use_id` 字段填好。
3. 把 pending teammates 整批 move 到新构造的 `AIChunk.teammate_messages` 字段；清空 `pending_teammates`。
4. 若主循环结束时 `pending_teammates` 非空（无后续 AIChunk 接收），SHALL 把它们追加到最后一条已 emit 的 AIChunk 的 `teammate_messages`；若整个 session 没有任何 AIChunk，SHALL 静默丢弃这些 pending teammate（罕见边界）。

回滚开关：`cdt-analyze::chunk::builder` 顶部 `const EMBED_TEAMMATES: bool = true;`；为 `false` 时 SHALL 退回旧行为——teammate user 消息直接 `continue`，`AIChunk.teammate_messages` 永远为空。该常量 MUST 与 enrichment 函数（`apply_teammate_embed`）以及调用点同一轮 Edit 落地，不得分批提交（避免 clippy `dead_code`）。

`TeammateMessage` 结构定义在 `cdt-core::chunk` 模块（与 `AIChunk` 共生命周期），字段含义详见 `ipc-data-api::Expose teammate messages on AIChunk`。

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

#### Scenario: EMBED_TEAMMATES=false reverts to legacy drop behavior
- **WHEN** 常量 `EMBED_TEAMMATES` 设为 `false`，chunk-building 跑过含 teammate 消息的 session
- **THEN** 每条 teammate user 消息 SHALL 被跳过（旧 `continue` 行为），每条产出的 `AIChunk.teammate_messages` SHALL 为空
