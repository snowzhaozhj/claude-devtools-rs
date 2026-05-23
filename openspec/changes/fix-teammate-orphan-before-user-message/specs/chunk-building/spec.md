## MODIFIED Requirements

### Requirement: Embed teammate messages into AIChunk

系统 SHALL 检测被分类为 teammate reply 的 user 消息（按 `team-coordination-metadata::Detect teammate messages`），且 MUST NOT 为它们 emit `UserChunk`。检测到的 teammate 消息 SHALL 被解析为 `TeammateMessage` 并嵌入到下一个 flush 出的 `AIChunk.teammate_messages` 中——而非丢弃（旧行为）。

实现 SHALL 在 chunk-building 主循环中维护 `pending_teammates: Vec<TeammateMessage>` 缓冲：

1. 遇到 teammate user 消息时，调用 `parse_teammate_attrs` 解析 `teammate_id` / `color` / `summary` / `body`，叠加 `detect_noise` / `detect_resend` 预算结果与 `token_count` 估算，封装成 `TeammateMessage` 并 push 到 `pending_teammates`；**SHALL NOT** flush 当前 assistant buffer，**SHALL NOT** 产 `UserChunk`，主循环继续。
2. 在 `flush_buffer` 即将产出 `AIChunk` 之前，对 `pending_teammates` 中每条 teammate 调用 `link_teammate_to_send_message(&teammate, &chain)`（详见 `team-coordination-metadata::Link teammate messages to triggering SendMessage`），把 `reply_to_tool_use_id` 字段填好。
3. 把 pending teammates 整批 move 到新构造的 `AIChunk.teammate_messages` 字段；清空 `pending_teammates`。
4. 若主循环结束时 `pending_teammates` 非空（无后续 AIChunk 接收），SHALL 把它们追加到最后一条已 emit 的 AIChunk 的 `teammate_messages`；若整个 session 没有任何 AIChunk，SHALL 静默丢弃这些 pending teammate（罕见边界）。
5. 当 `flush_buffer` 被任意"产生 user-side chunk"路径触发（普通 user message / `<local-command-stdout>` SystemChunk / Compact 边界 / Slash user message / Interruption），且 `assistant_buffer` 为空但 `pending_teammates` 非空时，SHALL emit 一条 `responses` 为空、`teammate_messages` 非空的 `AIChunk` 收容这些 teammate，再让调用方处理后续 chunk。该 `AIChunk` 的 `chunk_id` base 取 `pending_teammates[0].uuid`、`timestamp` 取 `pending_teammates[0].timestamp`、`metrics` 为 `ChunkMetrics::zero()`、`duration_ms` 为 `None`、`semantic_steps` / `tool_executions` / `subagents` 全为空，`slash_commands` 由调用方既有的 `pending_slashes` 通过 `std::mem::take` 消费（与 buffer 非空路径一致）。该规则保证 teammate-message 的 emit 顺序在 chunk 列表中严格遵循 timestamp 早于后续 user-side chunk，避免序列倒置。

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

#### Scenario: Teammate message before non-AI user message produces standalone empty-AI chunk
- **WHEN** 消息流为 `user(<teammate-message ...>body</teammate-message>) → user("real input")`，其间无任何 assistant 消息或所有 assistant 都被 `MessageCategory::HardNoise(_)`（典型 `<synthetic>` model + `isApiErrorMessage=true`）过滤
- **THEN** 产出的 chunk 列表 SHALL 含两条按时序排列的 chunk：
  - `chunks[0]` SHALL 为 `AIChunk { responses: [], teammate_messages: [TeammateMessage { body: "body", .. }], chunk_id: <pending_teammates[0].uuid>:0, timestamp: <teammate.timestamp>, metrics: zero, semantic_steps: [], tool_executions: [], subagents: [], slash_commands: [] }`
  - `chunks[1]` SHALL 为 `UserChunk { content: "real input", .. }`
- **AND** `chunks[0].teammate_messages[0].reply_to_tool_use_id` SHALL 为 `None`（无前驱 SendMessage 可配对）

#### Scenario: Teammate message before SystemChunk-triggering user message produces standalone empty-AI chunk
- **WHEN** 消息流为 `user(<teammate-message ...>body</teammate-message>) → user("<local-command-stdout>ls output</local-command-stdout>")`，其间无 assistant
- **THEN** 产出的 chunk 列表 SHALL 含两条按时序排列的 chunk：先 `AIChunk { responses: [], teammate_messages: [..], slash_commands: [] }`，再 `SystemChunk { content_text: "ls output", .. }`

#### Scenario: Teammate message before Compact boundary produces standalone empty-AI chunk
- **WHEN** 消息流为 `user(<teammate-message ...>body</teammate-message>) → compact_summary("conversation summary text")`，其间无 assistant
- **THEN** 产出的 chunk 列表 SHALL 含两条按时序排列的 chunk：先 `AIChunk { responses: [], teammate_messages: [..] }`，再 `CompactChunk { summary_text: "conversation summary text", .. }`

#### Scenario: Slash command then teammate then real user emits empty-AI with both slash and teammate
- **WHEN** 消息流为 `user(slash "<command-name>/clear</command-name>") → user(<teammate-message ...>body</teammate-message>) → user("real input")`，其间无 assistant
- **THEN** 产出的 chunk 列表 SHALL 含三条按时序排列的 chunk：
  - `chunks[0]` SHALL 为 slash 的 `UserChunk`
  - `chunks[1]` SHALL 为 `AIChunk { responses: [], slash_commands: [{ name: "clear", .. }], teammate_messages: [{ body: "body", .. }] }`
  - `chunks[2]` SHALL 为真实 user 的 `UserChunk`
- **AND** `chunks[1].slash_commands` 来自调用方 `pending_slashes` 通过 `std::mem::take` 消费

