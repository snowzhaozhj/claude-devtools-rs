# Tasks — split-session-display

## 1. session-display delta

- [x] 1.1 写 `specs/session-display/spec.md` `## REMOVED Requirements` 段，列 12 个 Requirement 标题
- [x] 1.2 写 `specs/session-display/spec.md` `## MODIFIED Requirements` 段：`SubagentCard 在 ongoing 期间主动重拉 trace` Requirement 完整重写（仅 2 个 Scenario 标题去 codex 审批后缀，body 与其他 Scenario 字符级保持）

## 2. markdown 新 cap

- [x] 2.1 写 `specs/markdown/spec.md` Purpose 段（用户价值视角）
- [x] 2.2 `## ADDED Requirements` × 4：Markdown 渲染与代码高亮 / Mermaid 图表渲染 / Lazy markdown rendering for first paint performance / 无语言代码块高亮自动检测限制
- [x] 2.3 每个 Requirement body 与 Scenario 字符级搬运自原 session-display

## 3. tool-viewer-routing 新 cap

- [x] 3.1 写 `specs/tool-viewer-routing/spec.md` Purpose 段（用户价值视角）
- [x] 3.2 `## ADDED Requirements` × 6：工具专化查看器路由 / Lazy load tool output on expand / Tool row displays approximate token count / 大文本工具详情交互优先渲染 / Tool detail timing and failure visibility / Tool result expansion avoids eager heavy rendering
- [x] 3.3 每个 Requirement body 与 Scenario 字符级搬运自原 session-display

## 4. edit-diff-view 新 cap

- [x] 4.1 写 `specs/edit-diff-view/spec.md` Purpose 段（用户价值视角）
- [x] 4.2 `## ADDED Requirements` × 2：Edit 工具 Diff 视图 / Edit diff preview highlighting
- [x] 4.3 每个 Requirement body 与 Scenario 字符级搬运自原 session-display

## 5. tool-execution-linking 跨 spec 引用更新

- [x] 5.1 写 `specs/tool-execution-linking/spec.md` `## MODIFIED Requirements` 段：`Source tool output text from raw tool_result.content` 完整重写，仅 body 内 "session-display capability 的 ReadToolViewer" 改 "tool-viewer-routing capability 的 ReadToolViewer"；2 个 Scenario 字符级保持

## 6. 校验

- [x] 6.1 `openspec validate split-session-display --strict` 通过
- [x] 6.2 4 cap Requirement 加和 = 45（手工 grep `^### Requirement:` 各 cap 主 spec + 本 change delta）
- [x] 6.3 4 cap Scenario 加和 = 219（手工 grep `^#### Scenario:` 各 cap 主 spec + 本 change delta）
- [x] 6.4 `bash scripts/check-spec-purity.sh` 通过（必要时同步 baseline）
- [x] 6.5 `just preflight` 通过（fmt / lint / test / spec-validate 一把梭）

## 7. Codex design 二审

- [x] 7.1 调 `Agent({ subagent_type: "codex:codex-rescue", ... })` 跑 design 二审 —— codex companion classifier 平台故障未跑完整 GPT-5.4 二审；改本地自审 6/8 checkpoint 全部通过（D-3 Edit diff 高亮冲突 pre-existing 非本 PR 引入 / D-2 REMOVED+ADDED 工艺通过 openspec validate strict / D-5 SubagentCard 8 Scenario 字符级对照 / D-4 跨 spec 引用单点改 / D-6 跟随 175fd7e 先例 / spec-purity baseline 同步）。剩 #4 D-1 字符级对等 + #7 deltaCount 概念分歧将在 N.3 PR codex 二审 + spec-guide-reviewer 上覆盖
- [x] 7.2 处理 codex 反馈（如需）—— 本地自审 0 hard finding

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex PR 二审通过 + spec-guide-reviewer 通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 同 commit 内直接编辑 3 个新 cap 主 spec 插 `## Purpose` 段，按 design.md::D-6 草稿；再次 wait-ci 全绿）
