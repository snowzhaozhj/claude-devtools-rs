## ADDED Requirements

### Requirement: Anchor turns on real user messages

系统 SHALL 以每条真实用户消息为 turn 锚点：每条真实 `UserChunk` SHALL 开启恰好一个 turn 并占用一个单调递增的 turn 序号，无论其 AI 响应是否产出。被打断的 turn——即一条真实用户消息之后，在下一条真实用户消息、compact 边界或会话结束之前**没有**产出任何 AI group——SHALL 仍产出一条 `user-message` injection 并占用一个 turn 序号；该 injection SHALL 进入累积 injection 链，使其在后续 AI group 的累计列表（以及由此驱动的 context panel injection 列表）中可见。

被打断的 turn SHALL NOT 产出独立的 per-turn `ContextStats` 记录，也 SHALL NOT 出现在 per-turn stats map（`turnContextStats`）中——它没有 AI group，因此不产生 per-turn badge；它对 context 的唯一贡献是其 `user-message` injection。

#### Scenario: Completed turn anchors on its user message

- **WHEN** 会话序列为 `[User(U1), AI(A1)]`，`A1` 是 `U1` 的完整响应
- **THEN** 系统 SHALL 产出恰好一个 turn（turn 序号 `0`），其 `user-message` injection 的 `turnIndex == 0`
- **AND** 该 `user-message` injection 的 `aiGroupId` SHALL 等于 `A1.chunkId`

#### Scenario: Interrupted user message still opens a turn

- **WHEN** 会话序列为 `[User(U1), User(U2), AI(A2)]`，`U1` 与 `U2` 之间没有任何 AI group（`U1` 的响应被打断、partial 响应作为 hard noise 被过滤）
- **THEN** 系统 SHALL 为 `U1` 产出一条 `user-message` injection 并占用 turn 序号 `0`
- **AND** `U2` SHALL 占用 turn 序号 `1`，其 turn 锚定 `A2`
- **AND** `U1` 的 `user-message` injection SHALL 出现在 `A2` 这个 AI group 的 `accumulatedInjections` 中（随累积链向前流动）

#### Scenario: Interrupted turn at end of session

- **WHEN** 会话序列为 `[User(U1), AI(A1), User(U2)]`，`U2` 在会话结束前没有产出任何 AI group
- **THEN** 系统 SHALL 为 `U2` 产出一条 `user-message` injection 并占用一个 turn 序号
- **AND** 该 injection SHALL 经 phase 末尾 backfill 出现在最后一个 AI group（`A1`）的 `accumulatedInjections` 中

#### Scenario: Interrupted turn produces no per-turn stats entry

- **WHEN** 一个被打断的 turn（无 AI group）被处理
- **THEN** `turnContextStats` map SHALL NOT 含以该 turn 的 `UserChunk` chunkId 为 key 的条目
- **AND** `stats_map` SHALL NOT 含以该 `UserChunk` chunkId 为 key 的 `ContextStats` 记录

## MODIFIED Requirements

### Requirement: Expose context stats to display surfaces

系统 SHALL 通过稳定的数据结构暴露每 turn context 统计、按类累计 token、phase 历史，使 UI badge、hover 细分、完整 context panel 可消费。对于有对应 AI group 的 turn，其产出的 `ContextInjection.aiGroupId` 字段 SHALL 与同一 `AIChunk.chunkId` 字节级相等（共享同一 ID 形态 `<base>:<n>`，不含类型前缀），使 UI 可直接用 `aiGroupId` 在 DOM 中按 `data-chunk-id` 锚点定位对应 AIChunk，无需任何客户端映射层。对于被打断的 turn（无对应 AIChunk），其 `user-message` injection 的 `aiGroupId` 字段 SHALL 等于该 turn 的 `UserChunk.chunkId`，使 UI 锚定到该用户消息气泡（同样无需客户端映射层）。

#### Scenario: Query context stats for a specific turn

- **WHEN** 调用方请求第 N 个 turn 的 context 统计
- **THEN** 结果 SHALL 包含 `tokensByCategory`、total token、当前活跃 phase id、该 turn 的底层 injection 列表

#### Scenario: aiGroupId equals the corresponding AIChunk chunkId

- **WHEN** 一个 turn 的 AI group 对应 `AIChunk { chunk_id: "abc-uuid:0", responses: [...] }`，且该 turn 产出至少一条 `ContextInjection`（如 `ToolOutputInjection` / `ThinkingTextInjection` / `UserMessageInjection`）
- **THEN** 所有由该 turn 产出的 injection 的 `aiGroupId` SHALL 等于 `"abc-uuid:0"`（与 `AIChunk.chunk_id` 字节级相等）
- **AND** 即使同会话内出现 `chunk_id` 冲突由 `next_ai_chunk_id` 递增解决（如 `"abc-uuid:1"`），对应 turn 的 injection `aiGroupId` SHALL 同步使用递增后的值

#### Scenario: Empty-response AIChunk reuses its synthesized chunk_id

- **WHEN** 某 AI group 对应 `AIChunk { responses: [], chunk_id: "empty:0" }`（`next_ai_chunk_id` 已为空 response 生成稳定 ID）
- **THEN** 该 turn 产出的 injections SHALL 复用 `chunk_id` 的值（`"empty:0"`）
- **AND** SHALL NOT 回退到 `responses[0].uuid` 或 `ai-<turn_index>` 等旧形态

#### Scenario: Interrupted turn anchors its user-message injection to the UserChunk chunkId

- **WHEN** 一个被打断的 turn 对应 `UserChunk { chunk_id: "u-abc:0" }` 且其后在下一条真实用户消息 / compact / 会话结束之前没有 AI group
- **THEN** 该 turn 产出的 `user-message` injection 的 `aiGroupId` SHALL 等于 `"u-abc:0"`（与 `UserChunk.chunk_id` 字节级相等）
- **AND** SHALL NOT 等于任何 `AIChunk.chunkId`
