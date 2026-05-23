## 1. cdt-analyze（chunk-building）

- [ ] 1.1 在 `crates/cdt-analyze/src/chunk/builder.rs::flush_buffer` 内把 `if buffer.is_empty() { return; }` 改为：buffer 空 + `pending_teammates` 非空时构造 `responses: Vec::new()` / `metrics: ChunkMetrics::zero()` / `duration_ms: None` / `semantic_steps: Vec::new()` / `tool_executions: Vec::new()` / `subagents: Vec::new()` 的 `AIChunk`，`chunk_id` base 用 `pending_teammates[0].uuid` 走 `next_chunk_id`，`timestamp` 用 `pending_teammates[0].timestamp`，`slash_commands` 用 `std::mem::take(pending_slashes)` 消费，`teammate_messages` 走既有 `link_against_chunks` 链接逻辑（`out` 中 `last_emitted_idx` + 自身 `new_chunk` placeholder 不存在则 fallback 单链）；buffer 空 + `pending_teammates` 空时仍直接 `return`。
- [ ] 1.2 复用既有 `link_against_chunks` 接受 `pending_chunk: Option<&AIChunk>` 参数；empty 路径下传 `Some(&new_chunk)` 让 self-scan 仍能命中（即使 `tool_executions` 空也保持调用一致性）。
- [ ] 1.3 把 spec delta 的 5 个新增 Scenario 各落到 `crates/cdt-analyze/src/chunk/builder.rs::tests` 模块新增单测，外加 1 个真实序列回归测试，共 6 个：
  - `teammate_before_real_user_emits_empty_ai_then_user`（spec Scenario "Teammate message before non-AI user message produces standalone empty-AI chunk"）
  - `teammate_before_local_command_stdout_emits_empty_ai_then_system`（spec Scenario "Teammate message before SystemChunk-triggering user message produces standalone empty-AI chunk"）
  - `teammate_before_compact_summary_emits_empty_ai_then_compact`（spec Scenario "Teammate message before Compact boundary produces standalone empty-AI chunk"）
  - `slash_then_teammate_then_user_emits_empty_ai_with_slash_and_teammate`（spec Scenario "Slash command then teammate then real user emits empty-AI with both slash and teammate"）
  - `teammate_before_interrupt_appends_to_empty_ai`（spec Scenario "Teammate message before interrupt marker appends to empty-AI"）
  - `synthetic_api_error_between_teammate_and_user_does_not_break_order`（命中真实 sessionId=`6290f9d4...` 序列：teammate → synthetic hard-noise → user "继续"；不在 spec 中，作为完整数据回归守门）
- [ ] 1.4 跑红→绿 cycle 验证 1.3 测试是有效防回归：先 `git stash` 保留测试代码、临时把 1.1 的修改 revert 回旧 `if buffer.is_empty() { return; }`，跑测试应**全部 fail**；再 `git stash pop` 恢复 fix，跑测试应全部 pass。
- [ ] 1.5 跑 `cargo test -p cdt-analyze` 确认既有 3 个 teammate Scenario 测试（`teammate_message_does_not_produce_user_chunk` / `teammate_message_embedded_into_ai_chunk_with_reply_to` / `trailing_teammate_attaches_to_last_ai_chunk` / `orphan_teammate_with_no_ai_chunk_is_silently_dropped` / `multiple_teammates_grouped_under_their_send_message`）全部仍 pass，无回归。

## 2. cdt-api（IPC contract）

- [ ] 2.1 在 `crates/cdt-api/tests/ipc_contract.rs` 加一个 round-trip 单测 `aichunk_with_empty_responses_and_teammate_messages_round_trips`：构造 `AIChunk { responses: Vec::new(), teammate_messages: vec![sample_teammate], .. }` → `serde_json::to_string` → `serde_json::from_str` → 字段对比。守住「empty responses + 非空 teammate_messages 的 AIChunk 可序列化 + 反序列化等价」。
- [ ] 2.2 在 `crates/cdt-api/tests/get_session_detail_with_teammate.rs` 增一条 fixture+测试 `orphan_teammate_session_emits_empty_ai_chunk_first`：构造命中 hard-noise 路径的 fixture jsonl（参考真实 sessionId 的 line 1-7 头），断言 `detail.chunks[0]` 是 empty-responses AIChunk + teammate_messages 非空，`detail.chunks[1]` 是 UserChunk。

## 3. UI 自验（前端兼容性）

- [ ] 3.1 直接 grep `ui/src/routes/SessionDetail.svelte` 与 `ui/src/lib/displayItemBuilder.ts` 的所有 `chunk.responses[`/`response.responses[` 访问点，确认每处都在 `length > 0` / `find` / 空安全 reduce 下；不安全处加守卫。
- [ ] 3.2 用 `pnpm --dir ui run dev` 起浏览器 mock server，在 `__fixtures__/` 加一个 minimal `orphan-teammate` fixture（一个 empty-responses AIChunk + UserChunk 两条）+ `?fixture=orphan-teammate&mock=1` 验证渲染：teammate 卡片可见、aiModel 显示 "Claude"、无 console error。
- [ ] 3.3 在 `ui/src/lib/displayItemBuilder.test.ts`（如不存在则新建）加单测 `empty_responses_aichunk_with_teammate_message_emits_only_teammate_item`：断言 `buildDisplayItems(emptyAIChunk).items` 只含 `{ type: "teammate_message", .. }`，无 thinking / output / tool / subagent / slash items。

## 4. spec validation + 本地 preflight

- [ ] 4.1 `openspec validate fix-teammate-orphan-before-user-message --strict` 通过
- [ ] 4.2 `just preflight` 全绿（fmt + clippy + test + spec-validate）
- [ ] 4.3 `bash scripts/run-perf-bench.sh` 跑一遍确保 perf baseline 不回归（chunk-building 改动单点 + 仅在 orphan 边界触发新代码路径，预期影响 ≤ 1%）

## 5. codex 二审（按 `.claude/rules/codex-usage.md`）

- [ ] 5.1 propose 阶段强制调（含 IPC 字段相关 + 状态机改动）：调 `Agent({ subagent_type: "codex:codex-rescue" })` 让 codex 审 design.md 的 D1-D5 决策（empty-responses AIChunk 选型 vs 新 chunk variant vs prepend 到 UserChunk）+ spec delta 第 5 条规则与新增 5 个 Scenario 的覆盖完整性。
- [ ] 5.2 PR push 后默认调 codex commit-level 二审（按既有规则）。

## N. 发布

- [ ] N.1 push 分支 + 开 PR（标题：`fix(chunk-building): emit empty-responses AIChunk before user-side flush when teammate orphans (closes #...)`）
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
