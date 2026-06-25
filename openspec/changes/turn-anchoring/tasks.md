## 1. turn 锚点重构（cdt-analyze/context）

- [x] 1.1 `session.rs`：把 `previous_user_chunk` 重构为 `pending_user` 语义；`Chunk::User` 分支检测已有 pending（被打断）→ emit 被打断 turn（产 injection + 推累积链 + `turn_index += 1`）后再设新 pending
- [x] 1.2 `session.rs`：`Chunk::Compact` 分支在原 compact 逻辑前先 flush pending 被打断 turn；循环结束后 flush 末尾 pending 被打断 turn
- [x] 1.3 `aggregator.rs`：`create_user_message_injection` 的 `ai_group_id` 入参在被打断 turn 时传 `UserChunk.chunkId`（完整 turn 仍传 AIChunk chunkId）；确认 `user_message_id(turn_index)` 不与完整 turn 撞 id（turn_index 全局单调 → id 唯一）
- [x] 1.4 `stats.rs`：被打断 turn 直接 push 到 accumulated 链、**不**经 compute_context_stats，确认不写入 `stats_map` / `turnContextStats`

## 2. 后端单元测试（spec scenario → test，`cdt-analyze/tests/context_tracking.rs`）

- [x] 2.1 `completed_turn_anchors_on_its_user_message`：`[User, AI]` → 1 turn，injection.aiGroupId == AIChunk.chunkId
- [x] 2.2 `interrupted_user_message_still_opens_a_turn`：`[U1, U2, A2]` → U1 占 turn 0 产 injection、U2 占 turn 1 锚 A2、U1 injection 在 A2 的 accumulatedInjections
- [x] 2.3 `interrupted_turn_at_end_of_session`：`[User, AI, User]` → 末尾 User 经 backfill 出现在 last AI group accumulated
- [x] 2.4 `interrupted_user_message_still_opens_a_turn` 含断言：stats_map 不含 UserChunk chunkId
- [x] 2.5 `interrupted_turn_anchor_is_userchunk_not_any_aichunk`：injection.aiGroupId == UserChunk.chunkId 且 ∉ AIChunk 集合
- [x] 2.6 `consecutive_interruptions_each_open_a_turn`：`[U1, U2, U3, A3]` → 前两条各占一 turn，第三条锚 AI，不丢不吞
- [x] 2.7 `interrupted_turn_before_compaction_lands_in_pre_compact_phase`（codex/test-analyzer：compact 分支 flush 路径）：`[U0, A0, U1, Compact, U2, A2]` → U1 落 compact 前 phase 的 A0
- [x] 2.8 `interrupted_turn_with_no_ai_carrier_phase_is_dropped`（D4 退化 characterization）：`[U1, Compact, A0]` → 不 panic，U1 无承载点丢失（pin 已知限制）

## 3. 前端导航分流（D5，codex CRITICAL）

- [x] 3.1 抽纯函数 `lib/contextNavigation.ts::resolveUserGroupNavTarget`（命中 chunk 是 user → 直接定位；是 ai → 向前找前置 UserChunk）；`SessionDetail.svelte::handleNavigateToUserGroup` 接入
- [x] 3.2 `lib/contextNavigation.test.ts` vitest 覆盖 4 分支：完整 turn / 被打断 turn 直接定位不回溯 / 无前置退化 / 命中不到返 null
- [x] 3.3 nav 防御（codex NIT）：仅 `kind==="ai"` 回溯，aiGroupId 异常命中 system/compact 退化为自身 + 对应用例

## 4. 下游一致性复核

- [x] 4.1 `cdt-api/tests/http_session_detail_global_lookup.rs::interrupted_user_message_surfaces_in_context_injections_not_turn_stats`：全链路（build_chunks→SessionDetail）证明被打断 injection 进 contextInjections、不进 turnContextStats；并断言正向一致性（turnContextStats key ⊆ AIChunk 集合 + 真实 turn newCount 与分组计数一致，test-analyzer #3）。ipc_contract 既有断言无破坏，148 全过
- [x] 4.2 vitest ContextPanel / contextExtractor 全过（22），无快照漂移（未改渲染）
- [ ] 4.3 `just dev` 手动 smoke：打开 issue 示例 session `21ea4d75-...`，确认 Context Panel "User Messages" 出现被打断消息 Turn 且点击跳到该消息本身。**注**：后端正确性已由 corpus 真实语料（C=1193 被救回）+ 全链路 ipc 测试证明；此项为最终桌面视觉确认，留人工

## 5. 回归守卫（corpus 诊断）

- [x] 5.1 `corpus_turn_fidelity` 迁入 `cdt-api/tests/`（保留 async parse_file 全链路），`#[ignore]` 手动跑；**修正**原诊断方向——B（chunk 层 UserChunk 后无 AIChunk）修复后不变，新增 C（context turn 层锚 UserChunk 的 injection）才是正确度量
- [x] 5.2 实跑：B=597（不变），**C=1193（修复前 0）**——被救回的被打断 turn injection，例子含真实消息（"继续"/"可以"）。数字记入诊断 baseline 注释 + PR 描述

## 6. follow-up 登记

- [x] 6.1 写入 `openspec/followups.md` `## turn-anchoring`：(a) 保留 synthetic partial 内容；(b) interruption marker 错位；(c) 纯被打断 phase injection 丢失 + PhaseSelector 跳号（pre-existing）

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
