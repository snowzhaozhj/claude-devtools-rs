## 1. spec delta 起稿

- [x] 1.1 在 `openspec/changes/tool-execution-linking-spec-sync/specs/tool-execution-linking/spec.md` 写 MODIFIED `Pair tool_use with tool_result by id` Requirement，**保留**原有 4 个 scenario（Immediate / Delayed / Duplicate result ids / Orphan tool_use）+ **新加** `Duplicate tool_use ids` scenario
- [x] 1.2 在同 delta 文件写 MODIFIED `Format readable summaries for team coordination tools` Requirement，**保留**原有 1 个 scenario（SendMessage with recipient and body）+ **新加** 4 个 scenario：
  - `SendMessage shutdown_response approve=true → "Shutdown approved"`
  - `SendMessage shutdown_response approve=false → "Shutdown denied"`
  - `SendMessage broadcast type → "Broadcast: <truncated>"`
  - `SendMessage default type without recipient → truncate(type)`

## 2. followups.md 状态同步

- [x] 2.1 在 `openspec/followups.md::tool-execution-linking` 第一条 `[spec-gap] 重复 tool_use_id 的处理没被实现` 行末追加 ` ✅ 已在 \`tool-execution-linking-spec-sync\` 同步` + Rust 实现 bullet（指向 `pair.rs::pair_tool_executions` 与对应单测 + 新加 scenario 名）
- [x] 2.2 同章节第二条 `[spec-gap] SendMessage summary 格式细节与实现不一致` 行末追加 ` ✅ 已在 \`tool-execution-linking-spec-sync\` 同步` + Rust 实现 bullet（指向 `team/summary.rs::format_send_message` 与对应单测 + 新加 4 个 scenario 名）
- [x] 2.3 同章节第三条 `[coverage-gap] Task→subagent 的三阶段匹配` 行末追加 ` ✅ 已在 \`tool-execution-linking-spec-sync\` 同步` + Rust 实现 bullet（指向 `tool_linking/resolver.rs` 与现有 spec Requirement `Resolve Task subagents with three-phase fallback matching` 的 6 个 Scenario 名）
- [x] 2.4 D6 提到的 "default 无 recipient" 单测缺失作为新 [coverage-gap] 条目追加到同章节末尾，标注未修 + 留给后续 change

## 3. validate

- [x] 3.1 `openspec validate tool-execution-linking-spec-sync --strict` 通过——确认每个 scenario 有 WHEN/THEN，每个 Requirement 第一段含 SHALL/MUST
- [x] 3.2 `just preflight` 运行（虽然只动 spec，但 fmt / lint / test / spec-validate 跑一遍兜底）

## 4. commit + push

- [x] 4.1 `git add openspec/` + commit，message：`docs(opsx): sync tool-execution-linking spec with implementation`，body 列三条 followups + 5 个新加 scenario
- [x] 4.2 push 分支

## 5. codex 反转 D6 后补单测（design.md D6b）

- [x] 5.1 `crates/cdt-analyze/src/tool_linking/pair.rs::tests::duplicate_tool_use_id_warns_and_keeps_first`：两条 assistant `tool_use_id == "t1"`（`Bash` / `Read`）+ 一条 user result，断言 `executions.len() == 1` + `duplicates_dropped == 1` + `tool_name == "Bash"` + `start_ts` 来自首条 assistant
- [x] 5.2 `crates/cdt-analyze/src/team/summary.rs::tests::send_message_shutdown_denied_explicit_false`：`{"type":"shutdown_response","approve":false}` → `"Shutdown denied"`
- [x] 5.3 `crates/cdt-analyze/src/team/summary.rs::tests::send_message_shutdown_missing_approve`：`{"type":"shutdown_response"}` → `"Shutdown denied"`
- [x] 5.4 `crates/cdt-analyze/src/team/summary.rs::tests::send_message_default_type_without_recipient`：`{"type":"reminder","message":"..."}` → `"reminder"`
- [x] 5.5 `crates/cdt-analyze/src/team/summary.rs::tests::send_message_missing_type_without_recipient_uses_message_default`：`{}` → `"message"`，验证 `unwrap_or("message")` 默认值
- [x] 5.6 `crates/cdt-analyze/src/team/summary.rs` MODIFIED Requirement spec delta 引言段加 `unwrap_or("message")` 默认值措辞 + 加 `SendMessage missing type without recipient uses default literal` scenario
- [x] 5.7 design.md 加 D6b 反转决策块（保留原 D6 不删，按 `openspec/CLAUDE.md::apply 阶段反转 design 决策时三处同步` 约束）
- [x] 5.8 proposal.md 与本文件同步更新（"无源码改动"反转为"仅测试模块改动"）
- [x] 5.9 `openspec/followups.md` 第 1 条 ✅ bullet 修正测试名（保留 `duplicate_tool_use_id_warns_and_keeps_first`，因为现在该单测真实存在）；删除 D6 原留的 [coverage-gap]
- [x] 5.10 `cargo test -p cdt-analyze` 全绿（新加 5 个单测全 pass）
- [x] 5.11 `openspec validate tool-execution-linking-spec-sync --strict` 通过
- [x] 5.12 `just preflight` 全绿

## N. 发布

- [ ] N.1 push 分支 + 开 PR（PR title 简短，body 列三条 gap + 5 个新加 scenario + 链接 followups）
- [ ] N.2 wait-ci 全绿（spec validate 在 CI 里跑；纯 markdown 改动 cargo / pnpm 流水线应该秒过）
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
