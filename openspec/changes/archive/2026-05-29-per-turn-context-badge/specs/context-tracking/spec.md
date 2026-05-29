# context-tracking spec delta: per-turn-context-badge

## ADDED Requirements

### Requirement: Per-turn context stats exposure

SessionDetail IPC SHALL 返回精简的 per-turn context stats map（字段 `turnContextStats`），让前端无需遍历完整 injection 列表即可渲染 per-turn badge。

Stats 结构包含：`newCount`（本轮新增注入数）、`newTokens`（本轮新增 token 总数）、`newTokensByCategory`（本轮新增按 6 类分的 token）、`countsByCategory`（按 6 类分的 count）、`cumulativeEstimatedTokens`（到该轮为止的累积 context 大小）、`cumulativeTokensByCategory`（到该轮为止的累积按类分布）。

Map 为稀疏结构：只包含 `newCount > 0` 的 AI turn，key MUST equal `AIChunk.chunkId` byte-for-byte。

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
- **WHEN** 对 contextInjections 按 aiGroupId 分组并计数
- **THEN** 每组的 count 与 turnContextStats 中对应 entry 的 newCount 一致

#### Scenario: turn stats key 等于 AIChunk chunkId

- **GIVEN** SessionDetail 返回了 turnContextStats 和 chunks
- **WHEN** 收集所有 AI chunk 的 chunkId
- **THEN** turnContextStats 的所有 key 均属于 AI chunkId 集合的子集

### Requirement: Badge display rules

前端 SHALL 根据 turn context stats 的空态规则决定是否渲染 Context badge，避免每轮都显示无意义的 "Context +1"。空态规则同时考虑 count 和 token 阈值。

#### Scenario: 空 turn 不渲染 badge

- **GIVEN** 一个 AI turn 不在 turnContextStats map 中（或 newCount = 0）
- **THEN** 不渲染 badge

#### Scenario: 低 token thinking-only 时不渲染 badge

- **GIVEN** 一个 AI turn 的 turnContextStats
- **WHEN** newCount = 1 且 countsByCategory 只有 thinkingText > 0
- **AND** newTokens < 1000
- **THEN** 不渲染 badge

#### Scenario: 高 token thinking-only 仍渲染 badge

- **GIVEN** 一个 AI turn 的 turnContextStats
- **WHEN** newCount = 1 且 countsByCategory 只有 thinkingText > 0
- **AND** newTokens >= 1000
- **THEN** 渲染 badge（大 thinking block 值得用户注意）

#### Scenario: 有意义的注入渲染 badge

- **GIVEN** 一个 AI turn 的 turnContextStats
- **WHEN** newCount >= 1 且不满足上述空态条件
- **THEN** 渲染 "Context +{newCount}" badge
