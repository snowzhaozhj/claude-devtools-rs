## MODIFIED Requirements

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
