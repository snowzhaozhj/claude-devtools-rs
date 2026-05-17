# app-chrome Specification

## Purpose
TBD - created by archiving change unified-title-bar. Update Purpose after archive.
## Requirements
### Requirement: UnifiedTitleBar 单条 chrome

系统 SHALL 在应用窗口最顶部（macOS 上为窗口最顶；Windows / Linux 上为 OS 原生 title bar 下方）渲染 `UnifiedTitleBar.svelte` 组件，作为应用层窗口 chrome 行。`UnifiedTitleBar` 自身高度 MUST 恒为 44 px，三平台一致。`UnifiedTitleBar` MUST 在所有应用状态下持续渲染（包括空项目、加载中、错误态），SHALL NOT 被任何瞬态横幅 / 模态 / 错误条推挤或替换。

**跨平台 chrome 总高度说明（非可测断言，仅澄清）**：macOS 用 `titleBarStyle: "Overlay"` 隐藏原生 title bar 后，窗口顶部唯一 chrome 即 `UnifiedTitleBar` 自身（44 px，含 traffic light 浮绘层）；Windows / Linux 由 OS 在 `UnifiedTitleBar` 之上额外绘制原生 title bar（含 minimize / maximize / close 按钮，约 28-32 px），本 change 不为 Win/Linux 自绘窗口控件。

#### Scenario: 应用启动后 chrome 必现

- **WHEN** 应用启动完成，`App.svelte` 挂载
- **THEN** `UnifiedTitleBar` SHALL 渲染在 `<div class="app-root">` 的第一个子节点位置
- **AND** `UnifiedTitleBar` 自身高度 SHALL 为 44 px
- **AND** chrome 下方 SHALL 直接是 `<div class="app-layout">`（Sidebar + main-area），中间无任何 banner / 间隙

#### Scenario: 更新或 Rosetta 提示出现不推挤页面

- **WHEN** 后端 emit `updater://available` 或 Rosetta 检测命中
- **THEN** `UnifiedTitleBar` 自身高度 SHALL 保持 44 px 不变
- **AND** `<div class="app-layout">` 相对 chrome 底部的 top 偏移量 SHALL 不变
- **AND** SHALL NOT 渲染 `RosettaBanner.svelte` 或 `UpdateBanner.svelte` 全宽横幅

### Requirement: chrome 四 zone 布局

`UnifiedTitleBar` 内部 SHALL 按 `[zone-platform-padding] [zone-left-center] [zone-drag-flex] [zone-status]` 四段 flex 横向布局：

- `zone-platform-padding`：仅 macOS 渲染，宽度 80 px，用于避让系统 traffic-light 按钮
- `zone-left-center`：放置主导航控件（项目选择下拉 + sidebar 折叠按钮），左对齐
- `zone-drag-flex`：flex: 1 弹性空白区，承载 `data-tauri-drag-region` 拖窗
- `zone-status`：右对齐的 status 容器，承载 status pill / status icon / notification button / settings button

#### Scenario: macOS 平台 chrome 起始 padding

- **WHEN** 平台为 macOS（`navigator.userAgent.includes("Macintosh") == true`）
- **THEN** `zone-platform-padding` SHALL 渲染，宽度 SHALL 为 80 px
- **AND** `zone-left-center` 的第一个控件左边缘 SHALL 距窗口左边缘 80 px

#### Scenario: Windows / Linux 平台 chrome 起始 padding

- **WHEN** 平台为 Windows 或 Linux（`navigator.userAgent.includes("Macintosh") == false`）
- **THEN** `zone-platform-padding` SHALL NOT 渲染
- **AND** `zone-left-center` 的第一个控件左边缘 SHALL 距窗口左边缘 ≤ 8 px（仅保留 chrome 内边距）

#### Scenario: zone-drag-flex 拖窗

- **WHEN** 用户在 chrome 的非按钮区域按住鼠标左键拖动
- **THEN** Tauri SHALL 调用窗口拖动（基于 `data-tauri-drag-region` 属性）
- **AND** 在按钮 / 下拉 / pill 上按下 SHALL NOT 触发拖窗（由 `data-tauri-drag-region="false"` 子树覆盖）

### Requirement: chrome 右侧 status zone 容纳契约

`zone-status` SHALL 按从左到右顺序容纳：`RosettaStatusIcon`（条件渲染）→ `UpdateStatusPill`（条件渲染）→ 通知按钮（含未读 badge）→ 设置按钮。任一 status 子组件 MUST 通过 `aria-label` 描述当前状态；可见时高度 SHALL ≤ 28 px 以保证 chrome 总高 44 px 不溢出。

#### Scenario: 各 status 子组件按状态独立显隐

- **WHEN** Rosetta 未检测命中且 update 状态为 `idle`
- **THEN** `zone-status` 内 SHALL 仅渲染通知按钮 + 设置按钮
- **AND** Rosetta icon 与 update pill SHALL NOT 渲染（不占布局空间）

#### Scenario: 多 status 同时可见时按顺序排列

- **WHEN** Rosetta 命中 AND update 状态为 `available`
- **THEN** `zone-status` 内从左到右顺序 SHALL 为：Rosetta icon、update pill、通知按钮、设置按钮
- **AND** 各组件间距 SHALL 为 8 px

### Requirement: 项目导航控件锚定 chrome 左中

`zone-left-center` SHALL 持续渲染项目选择下拉与 sidebar 折叠 / 展开按钮，按从左到右顺序为：项目下拉、折叠按钮。两者位置 MUST NOT 跟随 sidebar 宽度变化或折叠状态变化；sidebar 完全折叠时这两个控件 SHALL 仍可见且可点击。

#### Scenario: sidebar 折叠不影响 chrome 控件

- **WHEN** 用户点击折叠按钮把 sidebar 收起
- **THEN** 项目下拉 + 折叠按钮 SHALL 保持原位
- **AND** sidebar 宽度 SHALL 收缩到 0
- **AND** 折叠按钮 icon SHALL 切换为"展开 sidebar" 形态

#### Scenario: sidebar 展开不影响 chrome 控件

- **WHEN** 用户在折叠态点击展开按钮
- **THEN** sidebar SHALL 恢复到上次宽度
- **AND** 项目下拉 + 折叠按钮 SHALL 保持原位
- **AND** 折叠按钮 icon SHALL 切换为"折叠 sidebar" 形态

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

#### Scenario: SessionDetail 顶部不与 TabBar 行底 border 重叠

- **WHEN** pane content 加载 `SessionDetail.svelte`
- **THEN** `SessionDetail.svelte:1072` 处原有的 `border-bottom: 1px solid var(--color-border)` SHALL 移除
- **AND** SessionDetail 最顶部章节与 TabBar 之间用户视觉上 SHALL 仅看到一条来自 TabBar 行底的 1 px 分隔线

#### Scenario: 其它 view 顶部 border audit-only

- **WHEN** 实施期 audit `SettingsView.svelte` / `NotificationsView.svelte` / `DashboardView.svelte` 等其它 pane content view 顶部 border
- **THEN** 若该 border 用于 view 内部章节分隔（如 settings nav 与 setting body 之间、notifications header 与 list 之间），SHALL 保留不动
- **AND** 仅当某 view 顶部存在与 TabBar 行底紧贴 1 px 重叠加粗时，才 SHALL 移除该 view 顶部对应 border

### Requirement: chrome 与 pane 内 TabBar 职责分离

每个 pane 内 SHALL 保留独立的 `TabBar.svelte` 渲染该 pane 的 tab 列表（session / settings / notifications）。Pane 内 TabBar SHALL NOT 包含通知按钮、设置按钮、macOS traffic-light padding；这三者全部归属 `UnifiedTitleBar`。Pane 内 TabBar 仍保留 tab 列表 + 折叠态下的"展开 sidebar" 快捷按钮 + 自身的 drag region。

#### Scenario: TabBar 不再渲染通知 / 设置按钮

- **WHEN** 任一 pane 渲染 TabBar
- **THEN** TabBar 内 SHALL NOT 包含通知 button 或设置 button
- **AND** TabBar 高度 SHALL 由其内容（tab 列表 + 折叠展开按钮）决定，保持现有 40 px

#### Scenario: 多 pane chrome 仍为一份

- **WHEN** 用户在 sidebar 右键 "Open in New Pane" 触发 split
- **THEN** chrome SHALL 仍为单条 44 px 横向覆盖整个窗口宽度
- **AND** 每个 pane 内 SHALL 各自有独立 TabBar
- **AND** chrome 内 status zone（update / rosetta / 通知 / 设置）SHALL 全局一份，不 per-pane 复制

