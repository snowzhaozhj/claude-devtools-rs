## 1. turn-model：derive_turns 单一权威（cdt-analyze）

- [x] 1.1 在 `cdt-analyze` 定义 `Turn { index, driver, member_chunk_ids, ... }` 与 `TurnDriver = User | Teammate(Vec) | Headless`（D3）
- [x] 1.2 实现 `derive_turns(chunks) -> Vec<Turn>`：按 driver 切时间线（D4）——UserChunk / teammate-carrying AIChunk 开 turn；无驱动续写折叠；首驱动前归 turn 0 headless；Compact/System 不开 turn
- [x] 1.3 AI-only group 折叠 + turn 跨 phase（D5）：被 compact/中断切出的无驱动续写折进所属 turn
- [x] 1.4 单测覆盖 4 场景：compact 跨界折叠 / headless 前缀 / teammate 会话（含 N 条批量 = 1 turn）/ 被打断 turn
- [x] 1.5 corpus 守卫：把 `crates/cdt-api/tests/corpus_q2_aionly.rs` 收敛为正式守卫，断言 `turn 计数 == 驱动输入数（被打断算、headless 不算）`

## 2. context-tracking 改为消费 derive_turns（session.rs + aggregator.rs）

- [x] 2.1 `context::session.rs::process_session_context_with_phases` 改为先 `derive_turns` 建 `chunk_id -> turn.index` 映射，循环里给 injection 标 `turnIndex = map[...]`，不再内联自增 `turn_index`
- [x] 2.2 `context::aggregator.rs` 修 injection id 派生：`*-ai-{turn_index}` → 按 chunkId 派生（D7，codex C2），保证折叠后唯一
- [x] 2.3 确认 phase 追踪 / compaction token delta / backfill 逻辑不变（独立于 turn_index，D 不变量 2）
- [x] 2.4 实现 D9（`[User, Compact, AIChunk]`）：A0 折进 U 的 turn（turn 跨 phase）；U 的 user-message injection 承载缺口效果不变（仍不出现在 contextInjections），归因为 phase 重置而非中断
- [x] 2.5 被打断 turn 的 user-message injection id 锚 `UserChunk.chunkId`（AIChunk-scoped 之外的派生规则，codex F7）

## 3. 测试与契约同步

- [x] 3.1 `crates/cdt-analyze/tests/context_tracking.rs`：key=ai_group_id 的断言不变；断言 turn_index 具体值 / AI-only 占号的 Scenario 按折叠后新值更新
- [x] 3.2 `crates/cdt-api/tests/ipc_contract.rs`：turnIndex / aiGroupId / injection id 相关 fixture 同步
- [x] 3.3 `crates/cdt-api/tests/corpus_turn_fidelity.rs`：确认 #541「丢 turn→0」守卫继续通过（不变量 3）
- [x] 3.4 `openspec validate first-class-turn --strict` 通过

## 4. CHANGELOG 与文档

- [x] 4.1 CHANGELOG `## [Unreleased]` 的 `### Changed`：桌面 "Turn N" 在重压缩会话标签纠错（compact 续写归入所属提问轮）

## 5. 验证（真实数据）

- [x] 5.1 在本机语料跑 corpus 守卫，确认 turn 计数收敛、`471bc334` 类会话从 15→2 turn
- [ ] 5.2 `just dev` 桌面端眼验：重压缩会话的 Context Panel "Turn N" 标签正确、聊天流渲染/导航无变化

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（跨 capability + 改既有 spec + 状态机，高风险禁豁免；如发现 bug：修 → push → 回 N.2）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再 wait-ci 全绿）
