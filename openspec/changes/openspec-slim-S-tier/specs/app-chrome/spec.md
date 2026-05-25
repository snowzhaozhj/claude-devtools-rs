## MODIFIED Requirements

### Requirement: UnifiedTitleBar 单条 chrome

系统 SHALL 在应用窗口最顶部（macOS 上为窗口最顶；Windows / Linux 上为 OS 原生 title bar 下方）渲染应用层窗口 chrome 行（`UnifiedTitleBar.svelte`）。chrome 自身高度 MUST 恒为 44 px，三平台一致。chrome MUST 在所有应用状态下持续渲染（包括空项目、加载中、错误态），SHALL NOT 被任何瞬态横幅 / 模态 / 错误条推挤或替换。

**跨平台 chrome 总高度说明（非可测断言，仅澄清）**：macOS 用 `titleBarStyle: "Overlay"` 隐藏原生 title bar 后，窗口顶部唯一 chrome 即应用层 chrome 自身（44 px，含 traffic light 浮绘层）；Windows / Linux 由 OS 在应用层 chrome 之上额外绘制原生 title bar（含 minimize / maximize / close 按钮，约 28-32 px），本 capability 不为 Win/Linux 自绘窗口控件。

#### Scenario: 应用启动后 chrome 必现

- **WHEN** 应用启动完成
- **THEN** 应用层 chrome SHALL 渲染在应用根布局的第一个子节点位置
- **AND** chrome 自身高度 SHALL 为 44 px
- **AND** chrome 下方 SHALL 直接是 sidebar + main-area 主布局，中间无任何 banner / 间隙

#### Scenario: 更新或 Rosetta 提示出现不推挤页面

- **WHEN** 后端 emit `updater://available` 或 Rosetta 检测命中
- **THEN** chrome 自身高度 SHALL 保持 44 px 不变
- **AND** 主布局相对 chrome 底部的 top 偏移量 SHALL 不变
- **AND** SHALL NOT 渲染全宽 update / Rosetta 横幅

### Requirement: chrome 四 zone 布局

应用层 chrome 内部 SHALL 按 `[zone-platform-padding] [zone-left-center] [zone-drag-flex] [zone-status]` 四段 flex 横向布局：

- `zone-platform-padding`：仅 macOS 渲染，宽度 80 px，用于避让系统 traffic-light 按钮
- `zone-left-center`：放置主导航控件（项目选择下拉 + sidebar 折叠按钮），左对齐
- `zone-drag-flex`：flex: 1 弹性空白区，承载 `data-tauri-drag-region` 拖窗
- `zone-status`：右对齐的 status 容器，承载 status pill / status icon / notification button / settings button

平台判定 SHALL 以"运行平台为 macOS"为单一布尔信号驱动 padding 渲染；具体 detection 实现（userAgent / Tauri runtime API / 其它）属实现细节，spec 不绑死。

#### Scenario: macOS 平台 chrome 起始 padding

- **WHEN** 平台判定为 macOS
- **THEN** `zone-platform-padding` SHALL 渲染，宽度 SHALL 为 80 px
- **AND** `zone-left-center` 的第一个控件左边缘 SHALL 距窗口左边缘 80 px

#### Scenario: Windows / Linux 平台 chrome 起始 padding

- **WHEN** 平台判定为 Windows 或 Linux
- **THEN** `zone-platform-padding` SHALL NOT 渲染
- **AND** `zone-left-center` 的第一个控件左边缘 SHALL 距窗口左边缘 ≤ 8 px（仅保留 chrome 内边距）

#### Scenario: zone-drag-flex 拖窗

- **WHEN** 用户在 chrome 的非按钮区域按住鼠标左键拖动
- **THEN** Tauri SHALL 调用窗口拖动（基于 `data-tauri-drag-region` 属性）
- **AND** 在按钮 / 下拉 / pill 上按下 SHALL NOT 触发拖窗（由 `data-tauri-drag-region="false"` 子树覆盖）

### Requirement: chrome 与下方区域的分隔线只一条 1 px

chrome 底部 SHALL 渲染**仅一条** 1 px 横向分隔线作为 chrome 与下方 sidebar / pane 区域的视觉边界（颜色取自 `--color-border` token）。Pane 内 TabBar 的 active tab indicator MUST NOT 使用 `border-bottom` 实现（避免与 TabBar 行底 border 重叠成加粗视觉），SHALL 改用 tab 内部叠加的视觉手段（如内阴影 / 顶部 / 底部 inset 实现的 accent 线）。Pane 内 content 区（session detail / settings / notifications）的最顶部章节 SHALL NOT 渲染与上方 TabBar 行底 border 紧贴的另一条 border。

#### Scenario: chrome 底部仅一条分隔线

- **WHEN** chrome 渲染
- **THEN** chrome 与下方 sidebar 顶部之间 SHALL 仅有一条 1 px 分隔线
- **AND** chrome 与下方 pane TabBar 顶部之间 SHALL 仅有一条 1 px 分隔线
- **AND** 该分隔线 SHALL NOT 与下方组件自身的 border 叠加形成 ≥ 2 px 视觉加粗

#### Scenario: active tab indicator 不与行底 border 拼缝

- **WHEN** 任一 tab 处于 active 状态
- **THEN** 该 tab 的 active indicator SHALL 在 tab 内部渲染（不使用 `border-bottom`）
- **AND** 与 TabBar 行底 1 px border 在视觉上 SHALL 不连续拼接（indicator 仅 tab 内宽度、border 行宽度，二者分属不同层）

#### Scenario: SessionDetail 顶部不产生视觉双线

- **WHEN** pane content 加载 SessionDetail
- **THEN** SessionDetail 最顶部章节（top-bar）SHALL NOT 渲染与上方 TabBar 行底 border 紧贴的 border
- **AND** SessionDetail 最顶部章节与 TabBar 之间用户视觉上 SHALL 仅看到一条来自 TabBar 行底的 1 px 分隔线
- **AND** 本 Scenario SHALL NOT 禁止 top-bar 自身**下方**（top-bar 与下方 conversation/content 区域之间）渲染 1 px 分隔线用于区分头部章节与下方内容——该 border 与 TabBar 行底之间隔了整个 top-bar 高度，物理上不构成紧贴叠线

#### Scenario: 其它 view 顶部 border audit-only

- **WHEN** 实施期 audit Settings / Notifications / Dashboard 等其它 pane content view 顶部 border
- **THEN** 若该 border 用于 view 内部章节分隔，SHALL 保留不动
- **AND** 仅当某 view 顶部存在与 TabBar 行底紧贴 1 px 重叠加粗时，才 SHALL 移除该 view 顶部对应 border
