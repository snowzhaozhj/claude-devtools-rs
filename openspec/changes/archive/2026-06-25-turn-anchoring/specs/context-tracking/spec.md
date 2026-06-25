## ADDED Requirements

### Requirement: Anchor turns on real user messages

系统 SHALL 以每条真实用户消息为 turn 锚点：每条真实 `UserChunk` SHALL 开启恰好一个 turn 并占用一个单调递增的 turn 序号，无论其 AI 响应是否产出。被打断的 turn——即一条真实用户消息之后，在下一条真实用户消息、compact 边界或会话结束之前**没有**产出任何 AI group——SHALL 仍占用一个 turn 序号并产出一条 `user-message` injection 进入累积 injection 链。

该被打断 injection 的可见性取决于承载点：当该 turn 所在 phase 存在至少一个 AI group（在其前或其后）承载累积链时，该 injection SHALL 出现在该 phase 末尾 AI group 的累计列表（及由此驱动的 context panel injection 列表）中。当该 turn 所在 phase **完全没有任何 AI group**（退化情形，如整段 phase 仅由被打断的用户消息构成）时，累积链无承载点，该 injection SHALL NOT 出现在 context panel injection 列表中——此为**已知限制**（记入 `openspec/followups.md`），但该 turn 序号仍被占用。

turn 序号是单调递增计数：每条真实 `UserChunk` 占用一个；无前置用户消息的 AI group（AI-only group）也占用一个。因此 turn 序号 SHALL NOT 被解读为"用户消息序号"，而是"对话轮序号"。

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

#### Scenario: Interrupted turn whose phase has no AI group is a documented limitation

- **WHEN** 会话序列为 `[User(U1), Compact, AI(A0)]`，`U1` 所在的 compact 前 phase 没有任何 AI group 承载累积链
- **THEN** 处理 SHALL NOT panic，SHALL 产出良定义结果（`U1` 占用 turn 序号 `0`，`A0` 落在 compact 后的新 phase）
- **AND** `U1` 的 `user-message` injection SHALL NOT 出现在任何 phase 的 `accumulatedInjections` / `contextInjections` 中（无承载点，已知限制）

#### Scenario: Interrupted turn before a compaction lands in the pre-compact phase

- **WHEN** 会话序列为 `[User(U0), AI(A0), User(U1), Compact, User(U2), AI(A2)]`，`U1` 在 compact 前被打断（`A0` 与 compact 之间无 AI group 承载 `U1`）
- **THEN** `U1` 的 `user-message` injection SHALL 出现在 compact 前 phase 的末尾 AI group（`A0`）的 `accumulatedInjections` 中
- **AND** `U2` 的完整 turn SHALL 落在 compact 后的 phase，锚定 `A2`

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

### Requirement: Per-turn context stats exposure

SessionDetail IPC SHALL 返回精简的 per-turn context stats map（字段 `turnContextStats`），让前端无需遍历完整 injection 列表即可渲染 per-turn badge。

Stats 结构包含：`newCount`（本轮新增注入数）、`newTokens`（本轮新增 token 总数）、`newTokensByCategory`（本轮新增按 6 类分的 token）、`countsByCategory`（按 6 类分的 count）、`cumulativeEstimatedTokens`（到该轮为止的累积 context 大小）、`cumulativeTokensByCategory`（到该轮为止的累积按类分布）。

Map 为稀疏结构：只包含 `newCount > 0` 的 AI turn，key MUST equal `AIChunk.chunkId` byte-for-byte。被打断的 turn（无 AI group）SHALL NOT 出现在该 map 中——其 `user-message` injection 的 `aiGroupId` 等于 `UserChunk.chunkId`，不属于 AIChunk chunkId 集合，因此 `turnContextStats` 与 `contextInjections` 的一致性校验 SHALL 仅覆盖 `aiGroupId` 属于 AIChunk chunkId 集合的 injection。

#### Scenario: SessionDetail 返回 turn context stats

- **GIVEN** 一个包含多个 AI turn 的 session
- **WHEN** 调用 get_session_detail
- **THEN** 返回的 `turnContextStats` 字段包含每个有新 context 注入的 AI turn 的精简 stats
- **AND** stats 包含 `newCount`, `newTokens`, `newTokensByCategory`, `countsByCategory`, `cumulativeEstimatedTokens`, `cumulativeTokensByCategory`
- **AND** 没有新 context 注入的 turn 不在 map 中

#### Scenario: turn stats 只计算新增注入

- **GIVEN** 一个 AI turn 使用了 3 个 tool 并引用了 1 个 @file
- **AND** @file 在之前的 turn 已经被引用过
- **WHEN** 计算该 turn 的 TurnContextStats
- **THEN** newCount = 3（只有 tool outputs 是本轮新增）
- **AND** newTokensByCategory.toolOutput > 0
- **AND** newTokensByCategory.mentionedFile = 0（非首次出现不计为 new）

#### Scenario: turn stats 与 context_injections 一致

- **GIVEN** SessionDetail 返回了 turnContextStats 和 contextInjections
- **WHEN** 对 contextInjections 中 `aiGroupId` 属于 AIChunk chunkId 集合的条目按 aiGroupId 分组并计数（排除被打断 turn 的 user-message injection）
- **THEN** 每组的 count 与 turnContextStats 中对应 entry 的 newCount 一致
- **AND** `aiGroupId` 不属于 AIChunk chunkId 集合的被打断 injection SHALL NOT 参与该一致性校验

#### Scenario: turn stats key 等于 AIChunk chunkId

- **GIVEN** SessionDetail 返回了 turnContextStats 和 chunks
- **WHEN** 收集所有 AI chunk 的 chunkId
- **THEN** turnContextStats 的所有 key 均属于 AI chunkId 集合的子集
