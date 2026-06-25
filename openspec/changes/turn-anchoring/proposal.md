## Why

被打断的用户消息（用户发了消息、AI 开始响应但被打断）从 turn 视图中**完全丢失**——桌面端 Context Panel 的 "User Messages" 不显示这条消息的 Turn（issue #540）。本机真实语料诊断：~597/9095 ≈ **6.5%** 的真实对话消息丢 turn，在"反复催继续 / 打断"的会话里成片出现。

根因是实现漂移：context-tracking spec 的本意是**"一条真实用户消息开启新 turn"**（Requirement `Classify context injections into six categories` 的 `Real user prompt in a new turn` Scenario），但 `process_session_context_with_phases` 的 `turn_index` 实际锚在 AIChunk 上（`Chunk::Ai` 分支才 `turn_index += 1`），且配对用的 `previous_user_chunk` 会被下一条用户消息覆盖。当一个用户消息的响应被打断（partial 响应 `model == "<synthetic>"` 被过滤、不产 AIChunk）时，该用户消息既没拿到 turn、也没配对任何 AIChunk，于是从不产 `UserMessage` injection。

turn 定义是规划中 AI-friendly CLI/MCP API 重设计（change `redesign-cli-mcp-api`）的地基，此修复是其显式前置项。

## What Changes

- turn 锚点从 AIChunk 改为**真实用户消息**：每条真实 `UserChunk` 开启一个 turn，无后继 AIChunk（被打断）的用户消息照样占一个 `turn_index` 并产出 `UserMessage` injection，流入 `accumulated_injections` 累积链 → 在 Context Panel "User Messages" 中可见。
- 被打断的 turn（无 AI group）其 `UserMessage` injection 的 `aiGroupId` 锚到该 `UserChunk` 的 `chunkId`（导航跳转到用户气泡），而非任何 AIChunk。这放宽现有"`aiGroupId` 字节级等于某 `AIChunk.chunkId`"不变量——**调整 context-tracking spec 契约**（非 BREAKING：仅新增"被打断 turn"分支，已有完整 turn 行为不变）。
- 前端导航分流：Context Panel 点击被打断 turn 的 user-message injection SHALL 直接定位到该 `UserChunk`（而非沿用"向前找前置 UserChunk"逻辑回溯到上一条消息）——**调整 session-display 导航契约**。
- 被打断的 turn **不**进 `stats_map` / `turnContextStats`（无 AI group，无 per-turn badge），仅通过累积链显现于 injection 列表。
- 回归守卫：把 `corpus_turn_fidelity` 诊断（现于 `investigate/turn-anchoring` 分支）正式纳入本 change 的测试，修复后"真实消息丢 turn"计数 SHALL 趋近 0。

显式划为 **follow-up**（不在本 change 范围，记入 `openspec/followups.md`）：
- 子问题 2：保留被打断响应的 partial 内容（放宽 `<synthetic>` 过滤）——触及 noise.rs + chunk-building 数据模型，独立风险面。
- 根因 #4：中断标记 `[Request interrupted by user]` 错位追加到更早一个 AIChunk（`append_interruption_to_last_ai`）——chunk-building 数据模型问题，独立于 turn 会计。

## Capabilities

### New Capabilities
<!-- 无新增 capability -->

### Modified Capabilities
- `context-tracking`: turn 锚点语义从 AIChunk-anchored 改为 user-message-anchored；显式补上"被打断的 turn（无 AI group）仍产出 `UserMessage` injection 并占一个 turn"的契约；放宽 `aiGroupId` 不变量以容纳被打断 turn 锚到 `UserChunk.chunkId`；MODIFY `turnContextStats`↔`contextInjections` 一致性校验排除被打断 injection。
- `session-display`: Context Panel turn 锚点导航按命中 chunk 类型分流——被打断 turn 的 user-message injection 直接定位到该 `UserChunk`，完整 turn 仍向前找前置 UserChunk。

## Impact

- **代码（后端）**：`crates/cdt-analyze/src/context/session.rs`（主循环 turn_index / user chunk 配对）、`crates/cdt-analyze/src/context/aggregator.rs`（`create_user_message_injection` 接受被打断 turn 的锚 id）、`crates/cdt-analyze/src/context/stats.rs`（参数透传）。
- **代码（前端）**：`ui/src/routes/SessionDetail.svelte::handleNavigateToUserGroup`（按命中 chunk 类型分流导航）。
- **下游（行为对齐，无 API 字段增减）**：桌面端 Context Panel "User Messages" / "Turn N" 编号——含被打断 turn 后的 turn 序号会右移（更忠实于真实对话轮）；`get_session_detail` IPC 的 `contextInjections` 多出被打断 turn 的 `UserMessage` 条目。
- **测试**：`corpus_turn_fidelity` 回归守卫纳入；现有 context-tracking 单测 + ipc_contract 涉及 turn 序号的断言需复核更新；前端 ContextPanel 快照可能漂移。
- **前置/后继**：解锁 change `redesign-cli-mcp-api` 的 turn 模型 spec。
