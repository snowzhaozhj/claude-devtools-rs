# app-chrome — session-detail-header-bottom-divider delta

## MODIFIED Requirements

### Requirement: chrome 与下方区域的分隔线只一条 1 px

`UnifiedTitleBar` 底部 SHALL 渲染**仅一条** `1 px solid var(--color-border)` 横向分隔线作为 chrome 与下方 sidebar / pane 区域的视觉边界。Pane 内 `TabBar.svelte` 的 active tab indicator MUST NOT 使用 `border-bottom` 实现（避免与 TabBar 行底 border 重叠成加粗视觉），SHALL 改用 `box-shadow: inset 0 -2px 0 var(--color-accent)` 在 tab 内部渲染。Pane 内 content 区（session detail / settings / notifications）的最顶部章节 SHALL NOT 渲染与上方 TabBar 行底 border 紧贴的另一条 border。

#### Scenario: chrome 底部仅一条分隔线

- **WHEN** `UnifiedTitleBar` 渲染
- **THEN** chrome 与下方 sidebar 顶部之间 SHALL 仅有一条 1 px 分隔线
- **AND** chrome 与下方 pane TabBar 顶部之间 SHALL 仅有一条 1 px 分隔线
- **AND** 该分隔线 SHALL NOT 与下方组件自身的 border 叠加形成 ≥ 2 px 视觉加粗

#### Scenario: active tab indicator 不与行底 border 拼缝

- **WHEN** 任一 tab 处于 active 状态
- **THEN** 该 tab 的 active indicator SHALL 通过 `box-shadow: inset 0 -2px 0 var(--color-accent)` 渲染在 tab 内部
- **AND** SHALL NOT 使用 `border-bottom: 2px` 实现
- **AND** 与 TabBar 行底 1 px border 在视觉上 SHALL 不连续拼接（indicator 仅 tab 内宽度、border 行宽度，二者分属不同层）

#### Scenario: SessionDetail 顶部不产生视觉双线

- **WHEN** pane content 加载 `SessionDetail.svelte`
- **THEN** SessionDetail 最顶部章节（top-bar）SHALL NOT 渲染与上方 TabBar 行底 border 紧贴的 border（即 SessionDetail 顶部首像素层 SHALL NOT 存在另一条 1 px border 与 TabBar 行底相邻而形成 ≥ 2 px 视觉加粗）
- **AND** SessionDetail 最顶部章节与 TabBar 之间用户视觉上 SHALL 仅看到一条来自 TabBar 行底的 1 px 分隔线
- **AND** 本 Scenario SHALL NOT 禁止 top-bar 自身**下方**（top-bar 与下方 conversation/content 区域之间）渲染 1 px `var(--color-border)` 分隔线用于区分头部章节与下方内容——该 border 与 TabBar 行底之间隔了整个 top-bar 高度，物理上不构成紧贴叠线

#### Scenario: 其它 view 顶部 border audit-only

- **WHEN** 实施期 audit `SettingsView.svelte` / `NotificationsView.svelte` / `DashboardView.svelte` 等其它 pane content view 顶部 border
- **THEN** 若该 border 用于 view 内部章节分隔（如 settings nav 与 setting body 之间、notifications header 与 list 之间），SHALL 保留不动
- **AND** 仅当某 view 顶部存在与 TabBar 行底紧贴 1 px 重叠加粗时，才 SHALL 移除该 view 顶部对应 border
