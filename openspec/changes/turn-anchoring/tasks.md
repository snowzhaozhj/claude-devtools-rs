## 1. turn 锚点重构（cdt-analyze/context）

- [ ] 1.1 `session.rs`：把 `previous_user_chunk` 重构为 `pending_user` 语义；`Chunk::User` 分支检测已有 pending（被打断）→ emit 被打断 turn（产 injection + 推累积链 + `turn_index += 1`）后再设新 pending
- [ ] 1.2 `session.rs`：`Chunk::Compact` 分支在原 compact 逻辑前先 flush pending 被打断 turn；循环结束后 flush 末尾 pending 被打断 turn
- [ ] 1.3 `aggregator.rs`：`create_user_message_injection` 的 `ai_group_id` 入参在被打断 turn 时传 `UserChunk.chunkId`（完整 turn 仍传 AIChunk chunkId）；确认 `user_message_id(turn_index)` 不与完整 turn 撞 id
- [ ] 1.4 `stats.rs`：透传所需参数，确认被打断 turn **不**写入 `stats_map`

## 2. 单元测试（spec scenario → test）

- [ ] 2.1 `Completed turn anchors on its user message`：`[User, AI]` → 1 turn，injection.aiGroupId == AIChunk.chunkId
- [ ] 2.2 `Interrupted user message still opens a turn`：`[User(U1), User(U2), AI(A2)]` → U1 占 turn 0 产 injection、U2 占 turn 1 锚 A2、U1 injection 在 A2 的 accumulatedInjections 中
- [ ] 2.3 `Interrupted turn at end of session`：`[User, AI, User]` → 末尾 User 经 backfill 出现在 last AI group 的 accumulatedInjections
- [ ] 2.4 `Interrupted turn produces no per-turn stats entry`：stats_map / turnContextStats 不含以 UserChunk chunkId 为 key 的条目
- [ ] 2.5 `Interrupted turn anchors its user-message injection to the UserChunk chunkId`：injection.aiGroupId == UserChunk.chunkId 且不等于任何 AIChunk.chunkId

## 3. 回归守卫（corpus 诊断）

- [ ] 3.1 把 `corpus_turn_fidelity` 诊断从 `investigate/turn-anchoring` 分支迁入本 change（倾向落 `cdt-analyze/tests/`，纯同步）；`#[ignore]` 手动跑，CI 无 corpus 自动空跑
- [ ] 3.2 修复后重跑诊断，确认"真实对话消息丢 turn"（B 计数）从 ~597 趋近 0；记录前后数字进 PR 描述

## 4. 下游一致性复核

- [ ] 4.1 复核 + 更新 `cdt-api/tests/ipc_contract.rs` 中涉及 turn 序号 / aiGroupId 的断言
- [ ] 4.2 跑前端 `vitest --run` 复核 ContextPanel / UserMessagesSection 相关单测与快照；漂移则 `INSTA_UPDATE` / 快照更新
- [ ] 4.3 `just dev` 手动 smoke：打开 issue 示例 session `21ea4d75-...`，确认 Context Panel "User Messages" 出现被打断消息的 Turn

## 5. follow-up 登记

- [ ] 5.1 把"保留 synthetic partial 内容"与"interruption marker 错位（append_interruption_to_last_ai）"两项写入 `openspec/followups.md`；评估是否各开 GitHub Issue

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
