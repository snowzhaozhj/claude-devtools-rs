## MODIFIED Requirements

### Requirement: Anchor turns on real user messages

系统 SHALL 用共享派生的 turn 结构（capability `turn-model`）为每条 injection 标注其所属 turn 序号，而非在 context 累计循环内独立自增计数。turn 序号 SHALL 取自该共享派生：每个**驱动输入**（一条真实用户消息；或无用户消息驱动时一条进入的 teammate 消息）开启一个 turn 并占一个单调递增的 turn 序号，**无驱动的续写 assistant 响应**（如自动压缩后未等待新输入即继续的部分）SHALL 折叠进其所属 turn、**不**单独占一个序号。

被打断的 turn——一条真实用户消息之后，在下一个驱动输入、压缩边界或会话结束之前**没有**产出任何 AI group——SHALL 仍占一个 turn 序号并产出一条 `user-message` injection 进入累积 injection 链。

turn 与 phase 是正交的两条轴：一个 turn SHALL 允许跨越一个压缩（phase）边界（一次提问内部发生了一次压缩），压缩边界由 phase 历史与 `compaction_token_delta` 表达，不由 turn 序号编码。

对于有对应 AI group 的 turn，其产出的 `ContextInjection.aiGroupId` 字段 SHALL 与同一 `AIChunk.chunkId` 字节级相等，使 UI 可直接用 `aiGroupId` 在 DOM 中按 `data-chunk-id` 锚点定位，无需客户端映射层。对于被打断的 turn（无对应 AIChunk），其 `user-message` injection 的 `aiGroupId` SHALL 等于该 turn 的 `UserChunk.chunkId`。

被打断的 turn SHALL NOT 产出独立的 per-turn `ContextStats` 记录，也 SHALL NOT 出现在 per-turn stats map（`turnContextStats`）中——它没有 AI group，唯一贡献是其 `user-message` injection。

AIChunk-scoped 的 injection（`tool-output` / `task-coordination` / `thinking-text`，以及完成 turn 的 `user-message`）的稳定标识 SHALL 由其所属 AIChunk 的 chunkId（与类别）派生，而非由 turn 序号派生——这样多个 AIChunk 折叠进同一 turn 序号时其标识仍唯一。每类每 AIChunk SHALL 至多产一条此类 injection，故 `{类别}-{chunkId}` 唯一。被打断 turn 的 `user-message` injection 无对应 AIChunk，其标识 SHALL 由该 turn 的 `UserChunk.chunkId` 派生。`claude-md` / `mentioned-file` 等非 AIChunk-scoped 类别不适用此派生规则。

#### Scenario: Completed turn anchors on its user message

- **WHEN** 会话序列为 `[User(U1), AI(A1)]`，`A1` 是 `U1` 的完整响应
- **THEN** 系统 SHALL 产出一个 turn（turn 序号 `0`），其 `user-message` injection 的 `turnIndex == 0`
- **AND** 该 `user-message` injection 的 `aiGroupId` SHALL 等于 `A1.chunkId`

#### Scenario: Compaction-only continuation folds into its turn

- **WHEN** 会话序列为 `[User(U0), AI(A0), Compact, AI(A1)]`，`A1` 是压缩后未等待新输入即继续的续写
- **THEN** `A0` 与 `A1` 产出的 injection SHALL 共享同一 `turnIndex == 0`（同属 `U0` 开启的 turn）
- **AND** `A0` 与 `A1` 各自的 injection 标识 SHALL 仍唯一（按各自 chunkId 派生）
- **AND** `A1` 所在的压缩后 phase SHALL 照常记录，turn 跨越该 phase 边界

#### Scenario: Compaction before any AI group folds the post-compact response into the user turn

- **WHEN** 会话序列为 `[User(U1), Compact, AI(A0)]`，`U1` 与 `Compact` 之间没有任何 AI group，`A0` 之前没有新驱动输入
- **THEN** `A0` 产出的 injection 的 `turnIndex` SHALL 等于 `U1` 的 turn 序号（`A0` 折进 `U1` 的 turn，turn 跨越压缩边界），`U1` 不被判为被打断
- **AND** `U1` 的 `user-message` injection 在其无 AI group 的压缩前 phase 仍无承载点，SHALL NOT 出现在任何 phase 的 `contextInjections` 中（phase 重置导致的承载缺口，已知限制；与 `A0` 折入 `U1` turn 正交）

#### Scenario: Interrupted user message still opens a turn

- **WHEN** 会话序列为 `[User(U1), User(U2), AI(A2)]`，`U1` 与 `U2` 之间没有任何 AI group
- **THEN** 系统 SHALL 为 `U1` 产出一条 `user-message` injection 并占用 turn 序号 `0`
- **AND** `U2` SHALL 占用 turn 序号 `1`，其 turn 锚定 `A2`

#### Scenario: Interrupted turn produces no per-turn stats entry

- **WHEN** 一个被打断的 turn（无 AI group）被处理
- **THEN** `turnContextStats` map SHALL NOT 含以该 turn 的 `UserChunk` chunkId 为 key 的条目
- **AND** `stats_map` SHALL NOT 含以该 `UserChunk` chunkId 为 key 的 `ContextStats` 记录

#### Scenario: turnContextStats and contextInjections remain consistent under folding

- **WHEN** SessionDetail 返回 `turnContextStats` 与 `contextInjections`，且某个 turn 含多个 AI group（折叠）
- **THEN** 对 `contextInjections` 中 `aiGroupId` 属于 AIChunk chunkId 集合的条目按 `aiGroupId` 分组计数
- **AND** 每组的 count SHALL 与 `turnContextStats` 中对应 entry（key=chunkId）的 `newCount` 一致——一致性校验按 `aiGroupId`（chunkId）分组，不受多个 AI group 共享同一 `turnIndex` 影响

### Requirement: Compute cumulative context statistics per turn

系统 SHALL 为每个 AI group（`AIChunk`，按 `chunkId` 标识）计算上下文窗口当前可见的 token 总数，按六类分项细分。即使 AI group 为空（无 step、无 response、无前置 user group），SHALL 仍产出一条 `ContextStats` 记录：六类 token 全为 0，total 为 0，而非跳过。**本统计是 per-AI-group 粒度，不是 per-conversation-turn**：在 turn-model 的 conversation-turn 概念下，一个 turn 可含多个 AI group（如跨压缩折叠），各 AI group SHALL 各产一条 `ContextStats`（key=`chunkId`），SHALL NOT 因同属一个 turn 而合并。（Requirement title 保留 `per turn` 措辞——title 抽象为 `per AI group` 是 design F2/F3/F5 记录的 cleanup followup，本 change 只在 body 澄清粒度，不改 title。）

#### Scenario: AI group with CLAUDE.md + two tool outputs + user message

- **WHEN** 一个 AI group 含上述四条 injection
- **THEN** 该 AI group 的统计 SHALL 把对应 token 数累加到匹配类别字段，并暴露一个 total

#### Scenario: Empty AI group still produces a zeroed stats record

- **WHEN** 一个 AI group 没有任何 step、response，也没有前置 user 消息
- **THEN** 该 AI group 的统计 SHALL 仍被产出：`tokens_by_category.*` 全部等于 `0`，`total_estimated_tokens == 0`，`new_injections` 为空数组，而非从 stats map 中缺失

### Requirement: Per-turn context stats exposure

SessionDetail IPC SHALL 返回精简的 per-AI-group context stats map（字段 `turnContextStats`），让前端无需遍历完整 injection 列表即可渲染 per-AI-group badge。

Stats 结构包含：`newCount`、`newTokens`、`newTokensByCategory`、`countsByCategory`、`cumulativeEstimatedTokens`、`cumulativeTokensByCategory`。

Map 为稀疏结构：只包含 `newCount > 0` 的 AI group，key MUST equal `AIChunk.chunkId` byte-for-byte。**该 key 是 per-AI-group 标识，不是 conversation-turn 序号**——消费方 SHALL NOT 假设存在 `turnContextStats[turnIndex]`；一个 conversation-turn（含跨压缩折叠的多个 AI group）SHALL 对应该 map 中的多个 entry（每 AI group 一条）。被打断的 turn（无 AI group）SHALL NOT 出现在该 map 中——其 `user-message` injection 的 `aiGroupId` 等于 `UserChunk.chunkId`，不属于 AIChunk chunkId 集合，故 `turnContextStats` 与 `contextInjections` 的一致性校验 SHALL 仅覆盖 `aiGroupId` 属于 AIChunk chunkId 集合的 injection。

#### Scenario: SessionDetail 返回 per-AI-group context stats

- **GIVEN** 一个包含多个 AI group 的 session
- **WHEN** 调用 get_session_detail
- **THEN** 返回的 `turnContextStats` 字段包含每个有新 context 注入的 AI group 的精简 stats
- **AND** 没有新 context 注入的 AI group 不在 map 中

#### Scenario: stats 只计算新增注入

- **GIVEN** 一个 AI group 使用了 3 个 tool 并引用了 1 个 @file
- **AND** @file 在之前已被引用过
- **WHEN** 计算该 AI group 的 stats
- **THEN** `newCount = 3`（只有 tool outputs 是本轮新增），`newTokensByCategory.mentionedFile = 0`

#### Scenario: stats key 等于 AIChunk chunkId

- **GIVEN** SessionDetail 返回了 `turnContextStats` 和 chunks
- **WHEN** 收集所有 AI chunk 的 chunkId
- **THEN** `turnContextStats` 的所有 key 均属于 AI chunkId 集合的子集
