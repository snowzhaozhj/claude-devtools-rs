## Context

`process_session_context_with_phases`（`crates/cdt-analyze/src/context/session.rs`）按 chunk 序列计算 per-turn context stats。当前 `turn_index` 的递增锚在 AIChunk 上：`Chunk::Ai` 分支末尾 `turn_index += 1`；配对用的 `previous_user_chunk` 在 `Chunk::User` 分支被无条件覆盖，并在 `Chunk::Ai` 消费后清空。

chunk pipeline 在到达本函数前已完成噪声过滤：被打断响应的 partial 消息 `model == "<synthetic>"` 被 `cdt-parse::noise.rs` 判为 `HardNoise(SyntheticAssistant)`，不产 AIChunk。于是「用户消息 → 被打断（无 AIChunk）→ 下一条用户消息」这一序列里，第一条用户消息的 `UserChunk` 在被任何 AIChunk 消费前就被第二条覆盖，从不调用 `create_user_message_injection` → 从 turn 视图消失。

下游消费路径（`crates/cdt-api/src/ipc/local.rs:961-997`）：Context Panel 的 injection 列表取自**选中 phase 的 last AI group 的 `accumulatedInjections`**。每个 turn 的 injection 通过 `accumulatedInjections` 滚动链累积到该组。这意味着被打断 turn 的 injection 只要进入累积链即可显现，**无需** `stats_map` 条目。

context-tracking spec 的本意（Requirement `Classify context injections into six categories` 的 `Real user prompt in a new turn` Scenario）本就是「一条真实用户消息开启新 turn」——本 change 让实现忠实于该本意。

## Goals / Non-Goals

**Goals:**
- 每条真实用户消息开启一个 turn；被打断的用户消息（无后继 AIChunk）仍占一个 turn 序号并产出 `user-message` injection，进入累积链 → Context Panel "User Messages" 可见。
- 被打断 turn 的导航锚到其 `UserChunk.chunkId`（用户气泡），而非任何 AIChunk。
- 把 `corpus_turn_fidelity` 诊断纳入 change 作回归守卫，修复后"真实消息丢 turn"计数趋近 0。

**Non-Goals（划入 `openspec/followups.md`）:**
- 保留被打断响应的 partial 内容（放宽 `<synthetic>` 过滤）——触及 noise.rs + chunk-building 数据模型。
- 修复中断标记 `[Request interrupted by user]` 错位追加到更早 AIChunk（`append_interruption_to_last_ai`）——chunk-building 数据模型问题。
- 不改 IPC payload 字段形状（不增删字段）；仅改 turn 序号与 injection 列表内容。

## Decisions

### D1：turn 锚点从 AIChunk 改为真实用户消息

主循环改为：把 `previous_user_chunk` 语义化为 `pending_user`（一个尚未被 AI 响应消费的用户消息）。

- `Chunk::User(u)`：若已有 `pending_user`（说明上一条用户消息无 AI 响应 = 被打断），先为它 emit 一个被打断 turn（产 `user-message` injection、推入累积链、`turn_index += 1`），再 `pending_user = Some(u)`。
- `Chunk::Ai(ai)`：照常用 `pending_user` 作 `user_chunk` 计算 stats，`pending_user = None`，`turn_index += 1`。
- `Chunk::Compact`：若有 `pending_user`，先 emit 被打断 turn 再走原 compact 逻辑（compact 边界前的被打断 turn 归属当前 phase）。
- 循环结束：若仍有 `pending_user`，emit 末尾被打断 turn。

**候选对比**：
- *(选中) D1：用户消息锚定 + 被打断 turn 显式 emit*。对齐 spec 本意；改动集中在 session.rs 一个函数；turn 序号变为真实对话轮号。
- *(否决) 仅在覆盖前补发 injection、不改 turn_index 语义*：能让消息重新出现，但 turn 序号仍锚 AIChunk，与规划中 CLI/MCP turn 模型语义不一致，治标不治本。
- *(否决) 在 chunk-building 阶段为被打断响应造一个空 AIChunk 占位*：侵入 chunk 数据模型、影响所有 AIChunk 消费者（远超 context-tracking），风险面过大。

### D2：被打断 turn 的 injection `aiGroupId` 锚到 `UserChunk.chunkId`

被打断 turn 无 AIChunk，其 `user-message` injection 需要一个导航锚。选 `UserChunk.chunkId`——该用户气泡在主对话流中已被渲染（chunk-building 已产 `Chunk::User`），DOM 锚点存在，点击 turn 跳转到用户气泡语义正确。

**代价**：放宽现有「所有 injection 的 `aiGroupId` 字节级等于某 `AIChunk.chunkId`」不变量（spec MODIFIED + ipc_contract 断言更新）。

**候选对比**：
- *(选中) 锚到 `UserChunk.chunkId`*：导航语义正确；不变量放宽为「有 AI group 的 turn 锚 AIChunk，被打断 turn 锚 UserChunk」。
- *(否决) 锚到下一个承载它的 AI group chunkId（如示例的 A2）*：点击 turn 695 却跳到 709，语义错位。
- *(否决) 锚到空字符串 / 特殊 sentinel*：前端导航需特判，污染消费端。

### D3：被打断 turn 不进 `stats_map` / `turnContextStats`

被打断 turn 无 AI group，不产独立 `ContextStats`，不进 per-turn stats map（保 `turnContextStats` 的「key == AIChunk.chunkId」不变量纯净），不渲染 per-turn badge。它对 context 的唯一贡献是 `user-message` injection 进累积链。

**理由**：badge 表达"这一轮 AI 响应新引入多少 context"，被打断 turn 没有 AI 响应，无可 badge。让它进 stats_map 会迫使 local.rs:977 的 `turnContextStats` 构建特判过滤，得不偿失。

**连带契约修订（codex 二审 WARNING）**：主 spec `Per-turn context stats exposure` 的 `turn stats 与 context_injections 一致` scenario 要求"`contextInjections` 按 `aiGroupId` 分组后每组 count == `turnContextStats` 的 newCount"。被打断 turn 的 injection `aiGroupId`（= UserChunk id）不在 `turnContextStats` 里，会破坏该一致性。故 delta MODIFY 该 requirement：一致性校验**仅覆盖 `aiGroupId` 属于 AIChunk chunkId 集合的 injection**，显式排除被打断 injection；ipc_contract 对应断言同步更新。

### D5：被打断 turn 的前端导航分流（codex 二审 CRITICAL）

`handleNavigateToUserGroup(aiGroupId)`（`ui/src/routes/SessionDetail.svelte:756`）现逻辑：`findIndex(chunkId == aiGroupId)` 命中 idx 后**从 idx-1 向前**找 UserChunk。D2 后被打断 turn 的 `aiGroupId` 本身就是 UserChunk id → 向前回溯会跳到**上一条**用户消息（点 695 跳到更早消息），导航破裂。

**修法**：命中的 chunk 若 `kind === "user"`（被打断 turn）→ 直接 `handleNavigateToChunk(aiGroupId)`；否则（完整 turn，命中 AIChunk）→ 保持向前找前置 UserChunk。这使本 change 触及 `ui/`（前端 + e2e 点击断言），spec owner 是 session-display `Context Panel turn 锚点导航`（MODIFY + 新增"被打断 turn 直接定位"scenario）。

### D4：累积链承载的边界条件

被打断 turn 的 injection 通过 `accumulatedInjections` 滚动向前；phase 末尾 backfill（session.rs:182-188）把累积链写回 last AI group。**已知边界**：若一个 phase 内**完全没有** AI group（只有被打断的用户消息），`current_phase_last_ai_group_id` 为 `None`，无 backfill 目标，该 phase 的被打断 turn injection 不显现。此场景下 `contextInjections` 本就为空（无 AI group），属退化情形，记入 Risks 不在本 change 兜底。

## Risks / Trade-offs

- [turn 序号右移破坏现有快照] → 含被打断 turn 的会话其后续 turn 序号 +1。这是**预期的正确化**，非回归。Mitigation：更新受影响的 context-tracking 单测、ipc_contract turn 断言、前端 ContextPanel 快照；用 `corpus_turn_fidelity` 跨真实语料验证总体方向。
- [纯被打断 phase 的 injection 丢失 + phase 编号稀疏（D4 边界，codex 二审 WARNING）] → `[AI, compact, User-only, compact, AI]` 这类"phase 内无任何 AI group"的情形：(a) 该 phase 的被打断 injection 无 backfill 目标而丢失；(b) `current_phase_number` 在 compact 处仍递增但空 phase 不 push 进 `phases`，PhaseSelector 出现跳号（Phase 1、Phase 3）。**二者均为 pre-existing**——本 change 不改 phase push / number 逻辑，空 phase（无 AI group）在改前就不入 `phases`。Mitigation：记入 `openspec/followups.md`，不在本 change 兜底；不改 session-display PhaseSelector spec。
- [aiGroupId 不变量放宽影响其它消费者] → codex 全仓 grep 确认真实依赖仅 Context Panel 导航（D5 已修）+ ipc_contract / turnContextStats 一致性断言（D3 已 MODIFY）。无其它消费者用 aiGroupId 反查 stats_map。

## Migration Plan

无数据迁移/无 IPC 字段增删。纯算法行为变更，随版本发布生效。回滚：revert 单个 PR 即可（改动集中在 `cdt-analyze/src/context/`）。

## Open Questions

- 被打断 turn 的 `user-message` injection 是否应在 preview 文本上加视觉标记（如"(interrupted)"）以区别于完整 turn？倾向**否**（保持 injection 纯数据，视觉区分留给前端），apply 时复核前端 UserMessagesSection 是否需要。**apply 结论**：未加视觉标记（injection 保持纯数据，前端无新增需求）。

### Apply 阶段决策修订

- **D-apply1（诊断落点）**：`corpus_turn_fidelity` 最终**留在 `cdt-api/tests/`**，不迁 `cdt-analyze`。原因：诊断要扫全语料用 async `parse_file`（`cdt-analyze` 是 sync crate、无 tokio dev-dep），且度量 C 需 `process_session_context_with_phases` 全链路；`cdt-api` 已具备全部依赖。
- **D-apply2（回归度量方向修正）**：原 proposal/tasks 写"修复后 B（真实消息丢 turn）趋近 0"——**这是错的**。本修复**不改 chunk 流**（被打断响应本就无 AIChunk），所以 B（chunk 层 UserChunk 后无 AIChunk）修复后**不变**（实测仍 597）。真正归零→非零的是 context turn 层：新增度量 **C = 锚到 UserChunk 的 user-message injection 数**（修复前 0，修复后实测 **1193**）。C 才是"被打断消息被救回 turn injection"的正确证据。spec delta / tasks 5.x 已同步此修正。

## D-apply3：是否重构为 first-class Turn 模型——否（经影响面调研）

PR review 阶段质疑"本修复是否只是 band-aid，原链路若不合理应重构而非打补丁"。调研全部消费点后**否决重构**，保留本 change 的修法。证据：

- **`stats_map` 仅在 `cdt-api::local.rs::inject_context_annotations`（约 949-1003）一处展开**成 IPC 字段（contextInjections / injectionsByPhase / turnContextStats / phaseInfo）。
- **`turn_index` 不进 IPC**——它是后端 context-tracking 的临时递增计数；**前端完全无 turn 概念**，所有"turn"靠 **chunkId** 表达（DOM `data-chunk-id`、导航 `findIndex`、`turnContextStats[chunkId]` 反查）。
- 真实架构是**"injection 锚定 chunkId"**，不是"turn = AI group"。AIChunk / UserChunk 都有 chunk_id；本 change 让被打断 turn 的 injection 锚到 `UserChunk.chunk_id`——是该模型的**自然延伸**，不是例外补丁。
- 改成 `turns: Vec<Turn>`（turn_index 为主键）要把 turn_index 塞进 IPC + 前端建 turn_index→chunkId 反向索引 + 重写 15+ 后端测试 + IPC 契约 + 前端导航，**净亏**——只换"显式"边际收益，还破坏正在工作的 chunkId 导航模型。

**API 一等公民 turn 的正确落点是 `cdt-query`（CLI/MCP-only 层），从 chunk 流派生**——chunk 流里被打断的用户消息本就是 `Chunk::User`，API 直接枚举即可，无 AI 的 turn 在 API 层做完整一等公民，**不碰桌面端 core**（保"扩展锁 cdt-query、桌面端零影响"边界）。故本 change 与 `redesign-cli-mcp-api` 解耦：本 change 修桌面端 Context Panel，API turn 视图在 cdt-query 另建。

## PR review 收敛（codex + pr-review-toolkit 三 agent）

- **codex CRITICAL（spec overclaim）**：原 ADDED requirement 称被打断 injection"SHALL 可见"，但无 AI carrier 的退化 phase 会丢。→ spec 措辞改为可见性取决于承载点 + 显式 documented limitation + 2 个 scenario（pin 退化不 panic / compact 前 flush 落对 phase）。
- **codex WARNING（AI-only 消耗 turn_index）**：pre-existing。→ spec 加"turn 序号是对话轮序号，非用户消息序号；AI-only group 也占一个"。
- **codex NIT + test-analyzer（nav 防御 / compact 分支零覆盖 / 一致性正向未测）**：→ nav 仅 `kind==="ai"` 回溯 + system/compact 用例；补 compact-flush + 退化 characterization 单测；API 测试加 turnContextStats↔contextInjections 正向一致性断言。
- **comment-analyzer（模块头 stale-by-omission）**：→ session.rs 模块头补 compact flush 步骤 + turn 锚定说明。
- **code-reviewer**：0 高置信问题。

## 前瞻：CLI/MCP turn 模型的字段拆分（codex 二审魔鬼代言人）

codex 指出扩展瓶颈：`aiGroupId` 同时承担"AI group id"与"通用导航锚点"两个语义，被打断 turn 是被迫复用该字段的例外。规划中的 `redesign-cli-mcp-api` turn 模型若继续建在 `accumulated list + last AI backfill` 上，所有"无 AI 的真实 turn"都缺一等公民位置（无 stats row、无 phase row）。**建议**该 change 的 turn 数据模型显式拆 `turnId` / `anchorChunkId`（导航锚，可指 User 或 AI）/ `aiGroupId?`（可空，仅完整 turn 有），不要围绕例外继续打补丁。本 change 范围内不引入该拆分（仅修 bug + 放宽现有不变量），但作为 [[redesign-cli-mcp-api]] 的设计输入记此。
