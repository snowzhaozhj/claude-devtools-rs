## Why

当前应用顶部由 **3 个独立横向条**（`RosettaBanner` + `UpdateBanner` + `SidebarHeader` / `TabBar`）纵向堆叠，且左右两段头部（左 sidebar 一段、右 pane 一段，每段 40 px）各自处理 macOS traffic-light 避让，对齐靠肉眼。`UpdateBanner` 出现时整页向下推 40 px、整宽进度条贯穿，撕裂窗口 chrome 一体感；非 macOS 平台 traffic-light padding 缺失导致组件被挤到 chrome 下方。这与 Apple HIG 的 *Unified Toolbar* 模式（window controls 与 toolbar 共享同一行）、Linear / Notion / Cursor 等现代桌面 app 的"右上角 status zone"惯例都背道而驰。本 change 把窗口 chrome 拍平为一条 **Unified Title Bar**，把更新与 Rosetta 状态从横幅降级为右上 status pill + popover，让 chrome 在三平台都视觉一致、不被任何瞬态横幅推开。

## What Changes

- **新增** `app-chrome` capability 与组件 `UnifiedTitleBar.svelte`：窗口最顶一条 44 px 高的 chrome 行，左 / 左中 / 右中 / 右四个 zone，跨平台行为契约统一在此处定义（macOS traffic-light 避让 80 px、Windows / Linux 直接从 0 起 paint）。
- **新增** `UpdateStatusPill.svelte`：右侧 status zone 内的 update 状态药丸，状态机 `idle / available / downloading / downloaded / error`，downloading 时内嵌环形进度（不再整宽 banner 进度条），点击展开 popover 承载原 UpdateBanner 的三按钮（立即更新 / 稍后提醒 / 跳过此版本）+ release notes。
- **新增** `RosettaStatusIcon.svelte`：右侧 status zone 内的小三角警告 icon + tooltip，替代原全宽 RosettaBanner。
- **修改** `app-auto-update` capability：`UpdateBanner 三按钮交互` Requirement 改写为 `UpdateStatusPill 状态机与 popover`；"前端 SHALL 在主窗口顶部显示横幅"句式 SHALL 改为 "前端 SHALL 在 UnifiedTitleBar 右侧 status zone 显示 status pill"。所有按钮语义、IPC 调用、`skipped_update_version` 持久化、签名校验、跨平台覆盖策略**不变**。**BREAKING**（component 名）：`UpdateBanner.svelte` 文件 SHALL 删除，调用方 SHALL 改用 `UpdateStatusPill` + `updateStore`。
- **修改** `sidebar-navigation` capability：`项目选择` Requirement 句式 "Sidebar 顶部 SHALL 提供项目选择器" SHALL 改为 "UnifiedTitleBar 左中 zone SHALL 提供项目选择器"；项目选择行为契约（自动选中第一个、切换替换会话列表、空状态提示）**不变**。SidebarHeader 内剩余内容（搜索框 + filter chip）保留在 Sidebar 内。
- **删除** `RosettaBanner.svelte` 与 `UpdateBanner.svelte` 横幅；它们的可见性由 `UnifiedTitleBar` 子组件的状态机接管。
- **DOM 拓扑变更**：`App.svelte` 顶层 `<div class="app-root">` 从 `[RosettaBanner, UpdateBanner, app-layout]` 改为 `[UnifiedTitleBar, app-layout]`；`UnifiedTitleBar` 内部用 `[platform-padding-left] [sidebar-header-controls] [drag-region-flex] [status-zone]` 四段 flex 布局。
- **drag region**：原 SidebarHeader / TabBar 各自的 drag region 收敛到 UnifiedTitleBar 单一 drag-region-flex；pane 内 TabBar **保留自身的 drag region**（多 pane 时仍可从 tab strip 拖动窗口，对齐 Cursor 模型）。
- **未涉及**：`notification-ui` / `settings-ui` / `tab-management` / `app-auto-update` 的其它 Requirement（component 自身行为不变，只是被挂在新位置；后端 IPC、状态机、签名链均不动）。

## Capabilities

### New Capabilities
- `app-chrome`: 应用窗口顶部 chrome 行（unified title bar）的布局契约——四 zone 划分、跨平台 traffic-light 避让、drag region、status zone 容纳子组件的契约（update / rosetta / 通知 / 设置）、与 Sidebar / Pane / TabBar 的边界。

### Modified Capabilities
- `app-auto-update`: 把 `UpdateBanner 三按钮交互` Requirement 改名并改写为 `UpdateStatusPill 状态机与 popover`；"主窗口顶部横幅" → "右侧 status zone status pill + popover"。按钮语义 / IPC / 签名链 / 跨平台策略不变。
- `sidebar-navigation`: 把 `项目选择` Requirement 中"Sidebar 顶部"句式改为"UnifiedTitleBar 左中 zone"。其它 Requirement（会话列表 / 搜索 / Pin / Hide / 右键菜单 / 宽度拖拽）不变。

## Impact

**前端**：
- 新增 `ui/src/components/UnifiedTitleBar.svelte` + `ui/src/components/UpdateStatusPill.svelte` + `ui/src/components/RosettaStatusIcon.svelte`。
- 修改 `ui/src/App.svelte` 顶层 DOM 结构。
- 修改 `ui/src/components/Sidebar.svelte` 与 `ui/src/components/SidebarHeader.svelte`：移除项目下拉 + sidebar 折叠按钮 + macOS traffic-light padding，保留搜索 / filter；header 行高随之收紧或合并入 sidebar 内容区。
- 修改 `ui/src/components/TabBar.svelte`：移除 bell + 齿轮 + macOS traffic-light padding；只保留 tab 列表 + 展开 sidebar 按钮（折叠态）+ drag region。
- 删除 `ui/src/components/UpdateBanner.svelte` 与 `ui/src/components/RosettaBanner.svelte`。
- 状态 store：`ui/src/lib/updateStore.svelte.ts` 与 `ui/src/lib/rosettaStore.svelte.ts`（如存在）保留，被新组件消费；不增减 store 字段。
- 测试：`ui/tests/e2e/` 加 `unified-title-bar.spec.ts`（macOS UA / Windows UA 两套 viewport 截图断言 traffic-light 避让 + pill popover 行为）；vitest 单测加 `UpdateStatusPill` 状态机覆盖。

**后端**：
- **零改动**。无新 IPC command、无 `LocalDataApi` 公开方法签名变化、无 `ConfigData` 字段变化、无 `EXPECTED_TAURI_COMMANDS` 变化。`updater://available` / `updater://download-progress` 事件、`check_for_update` IPC、`skipped_update_version` 持久化均不变。

**视觉 / UX**：
- 应用顶部恒为单条 44 px chrome（无横幅推挤）。Rosetta 警告与更新提示在 status zone 内通过 status pill 状态呈现，可被忽略（icon-only）也可点击展开 popover。
- macOS：traffic-light 与 chrome 左侧控件视觉对齐；Windows / Linux 系统标题栏仍由 OS 绘制，UnifiedTitleBar 直接从窗口左缘 paint 起。
- 多 pane 时每 pane 内 TabBar 保留——chrome 与 content tabs 职责分离（Cursor 模型）。

**Perf impact**：
- 顶层 DOM 节点数微减（少两个 banner wrapper）；首屏 layout 触发次数减一（无 banner 显隐导致的 reflow）。无后端 / IPC payload 改动。按 `.claude/rules/perf.md` 此 PR 属于 UI-only 重构，PR 描述附四维数据校验冷启动 wall + RSS 未回归。
