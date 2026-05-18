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

- [ ] 4.1 `git add openspec/` + commit，message：`docs(opsx): sync tool-execution-linking spec with implementation`，body 列三条 followups + 5 个新加 scenario
- [ ] 4.2 push 分支

## N. 发布

- [ ] N.1 push 分支 + 开 PR（PR title 简短，body 列三条 gap + 5 个新加 scenario + 链接 followups）
- [ ] N.2 wait-ci 全绿（spec validate 在 CI 里跑；纯 markdown 改动 cargo / pnpm 流水线应该秒过）
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
