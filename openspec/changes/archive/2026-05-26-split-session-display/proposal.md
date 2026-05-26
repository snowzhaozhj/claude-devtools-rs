# Split session-display — 提取 markdown / tool-viewer-routing / edit-diff-view

## Why

session-display 主 spec 当前 1390 行 / 45 Requirement / 219 Scenario，体量 TOP 3，单一 cap 内混合"对话流编排骨架 + 通用 markdown 渲染 + 工具 viewer 路由 + Edit diff 视图"四类边界互不相同的契约。issue #303 9-PR plan 已在 D-7 决策拆为 4 个 cap：

- `session-display` 留对话流编排骨架（chunk 渲染 / Subagent 卡片 / Context Panel / 顶 bar / 锚点 / 多 pane）
- `markdown`（新建）承接通用 markdown 渲染管线
- `tool-viewer-routing`（新建）承接工具专化 viewer 路由 + 展开/折叠性能
- `edit-diff-view`（新建）承接 Edit 工具 Diff 视图

拆后利于后续按 cap 边界独立演进（markdown 升级 marked v15 / tool-viewer 加新工具支持 / edit-diff 加 inline bookmark 不互相影响）。

## What Changes

- session-display `## REMOVED Requirements` × 12（被移走的 12 个 Requirement 标题列出）
- session-display `## MODIFIED Requirements` × 1（`SubagentCard 在 ongoing 期间主动重拉 trace` 内 2 个 Scenario 标题去 codex 审批后缀 `（C1 修复）` `（C3 修复）`，违反 SPEC_GUIDE 反例 1）
- markdown `## ADDED Requirements` × 4（Markdown 渲染与代码高亮 / Mermaid 图表渲染 / Lazy markdown rendering for first paint performance / 无语言代码块高亮自动检测限制）
- tool-viewer-routing `## ADDED Requirements` × 6（工具专化查看器路由 / Lazy load tool output on expand / Tool row displays approximate token count / 大文本工具详情交互优先渲染 / Tool detail timing and failure visibility / Tool result expansion avoids eager heavy rendering）
- edit-diff-view `## ADDED Requirements` × 2（Edit 工具 Diff 视图 / Edit diff preview highlighting）
- tool-execution-linking `## MODIFIED Requirements` × 1（`Source tool output text from raw tool_result.content` body 内 "session-display capability 的 ReadToolViewer" 改 "tool-viewer-routing capability 的 ReadToolViewer"，1 处描述性引用更新）
- 新建三个 cap 各自的 Purpose 段（用户价值视角，不抄实现概要）

不变量：

- 4 cap Requirement 加和 = 33 + 4 + 6 + 2 = **45**（与原 session-display 严格相等）
- 4 cap Scenario 加和 = 162 + 18 + 31 + 8 = **219**（与原 session-display 严格相等）
- 行为契约 100% 不变 —— 所有 Requirement body / Scenario WHEN / THEN / AND 子句字符级对等（仅 D-5 例外的 2 个 Scenario 标题去后缀）
- 不改代码 / 测试 / 配置 / IPC 字段 / Tauri command 协议 —— 纯 spec 文档拆分

## Impact

- **Affected specs**: session-display, markdown (NEW), tool-viewer-routing (NEW), edit-diff-view (NEW), tool-execution-linking
- **Affected code**: 无 —— 纯 spec 拆分，前端组件 / IPC / 后端不需改动
- **OpenSpec Capability map**: `openspec/README.md` 内 capability list 增 3 项，session-display 描述更新（聚焦"对话流编排骨架"）
- **Spec-purity baseline**: `tests/spec-purity-baseline.txt` 内 session-display 行的反模式数减少（被 REMOVED 的 12 个 Requirement body 内的历史污染随之离开），新增 markdown / tool-viewer-routing / edit-diff-view 三行 baseline；总和不变
- **CI**: 需观察 `scripts/check-spec-purity.sh` 是否需要 baseline 更新；archive 时 4 cap 主 spec 自动 sync
