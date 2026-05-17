# session-detail-header-bottom-divider

## Why

用户反馈："会话详情页头部左侧竖条多余 + 头部没有分隔线和底部内容不容易区分"。

前者（装饰竖条 `.top-rail`）直接删除即可，纯样式细节，无 spec 约束。

后者（头部与下方对话区缺分隔线）触到 `app-chrome` capability 的 spec：`openspec/specs/app-chrome/spec.md` 第 105–109 行 Scenario "SessionDetail 顶部不与 TabBar 行底 border 重叠" 以**字面 SHALL**（绑定到 `SessionDetail.svelte:1072` 处 `border-bottom: 1px solid var(--color-border)`）禁止 top-bar 加任何 border-bottom。

但该 SHALL 写得过于绑死实施细节——D8 的真实意图（见 unified-title-bar design 第 8 条 + Requirement 第 89 行）是"避免 SessionDetail 顶部章节与 TabBar 行底 border 视觉上紧贴成 ≥ 2 px 加粗"。SessionDetail top-bar 自身高度 ~65 px，其 **下方** 的 border-bottom（用于分隔 top-bar 与 conversation）与 TabBar 行底之间隔了整个 top-bar，物理上不构成"紧贴叠线"——但字面 SHALL 仍把这种合理用法一并禁掉。

## What Changes

放宽 `app-chrome` Scenario "SessionDetail 顶部不与 TabBar 行底 border 重叠" 的字面 SHALL：

- 禁止的仍是**与 TabBar 行底紧贴**的 border（即 SessionDetail 最顶部 border-top 或 padding-top=0 时的 border-bottom 等会与上方 TabBar 行底相邻的 border）
- **允许** top-bar 自身**下方**的 border-bottom 用于分隔 top-bar 与下方 conversation／content 区域

同步在 `SessionDetail.svelte`：
- 移除装饰竖条 `.top-rail`（无 spec 约束，纯视觉冗余）
- 在 `.top-bar` 加 `border-bottom: 1px solid var(--color-border)` 与下方对话区分隔
- 左 padding 从 28px 改 24px（原 28px 给装饰竖条留空间）

## Impact

- **affected specs**: `app-chrome`（仅修 1 个 Scenario，不动 Requirement）
- **affected code**: `ui/src/routes/SessionDetail.svelte`（已在前一 commit 改）
- **affected users**: 所有 SessionDetail 用户——视觉上更清晰的头部/内容分隔
- **行为契约**：无 IPC/算法/状态机改动；纯视觉规范修订
