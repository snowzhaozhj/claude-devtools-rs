## Why

原版支持最多 4 个水平分栏的 Pane，每个 Pane 独立管理 tab 列表与状态，支持跨 Pane 拖拽 tab、边缘放置创建新 Pane、拖拽调节宽度。Rust port 当前是扁平的 tabs[] + 单 activeTabId + 单 main 容器布局，无法并排对比两个 session，也无法同时看 session + notifications。补齐多 Pane 后 UI 层基本对齐原版，关闭路线图最后一个大项。

## What Changes

- **BREAKING** `tabStore.svelte.ts` 数据模型：`tabs: Tab[] + activeTabId` 重构为 `paneLayout: { panes: Pane[]; focusedPaneId: string }`，每个 `Pane { id, tabs, activeTabId, widthFraction }`（暂不实现 `selectedTabIds` 多选，原版该功能使用率低）
- `tabUIStates` / `tabSessionCache` 仍按 tabId 索引（无需按 pane 改造，因为 tab 唯一）
- 新增 pane 生命周期操作：`splitPane(sourcePaneId, tabId, direction)`、`closePane`、`focusPane`、`resizePanes`
- 新增跨 Pane tab 移动：`moveTabToPane`、`moveTabToNewPane`、`reorderTabInPane`
- App.svelte 布局变更：原 `TabBar + main-content` 替换为 `PaneContainer`，内部按 `paneLayout.panes` 横向 flex 渲染多个 `PaneView`，中间插入可拖拽 `PaneResizeHandle`
- TabContextMenu 新增 "Split Right" / "Split Left" 两项；SessionContextMenu（右键 Sidebar session）新增 "Open in New Pane (Right)" 项
- Tab DnD 升级：已有 TabBar 内重排扩展为"拖到另一 Pane TabBar → moveTabToPane / 拖到 Pane 边缘 DropZone → moveTabToNewPane"
- 全局快捷键扩展：`Cmd+\` = split 当前 tab 到右侧；`Cmd+Option+←/→` = focus 上一/下一 Pane
- 限制：`MAX_PANES = 4`、`MIN_FRACTION = 0.1`
- `openTab` / `openSettingsTab` / `openNotificationsTab` 改为作用于 `focusedPaneId` 对应的 pane
- Sidebar 选中 session 时 → 走 focused pane 的 openTab（不是全局）

## Capabilities

### New Capabilities

（无新能力；所有行为都扩展自现有 UI 能力）

### Modified Capabilities

- `tab-management`：tabs/activeTabId 升级为多 Pane 隔离模型；打开/关闭/切换/UI 状态隔离/缓存/Sidebar 联动等 requirement 全部 delta 到 "focused pane 范畴内"；新增 Pane 生命周期 + Pane 间 tab 移动 + Pane resize 三组 requirement
- `sidebar-navigation`：Sidebar 的"打开 session"从"全局 openTab"改为"focused pane 的 openTab"；"Sidebar 高亮同步当前 activeTab"改为"同步 focused pane 的 activeTab"
- `session-display`：补明 per-pane 独立渲染契约——同一 session 若同时在多个 tab（例如不同 pane 里）打开，SessionDetail 渲染实例与 UI 状态 SHALL 各自独立

## Impact

- **代码**：
  - `ui/src/lib/tabStore.svelte.ts` 重构（破坏性 API 变更，tabStore 消费者全部要改）
  - 新增 `ui/src/components/layout/PaneContainer.svelte` / `PaneView.svelte` / `PaneResizeHandle.svelte` / `PaneSplitDropZone.svelte`
  - 新增 `ui/src/lib/paneHelpers.ts`（findPane / updatePane / insertPane / removePane / createEmptyPane 等）
  - `App.svelte` 布局重构（Sidebar + PaneContainer）
  - `TabBar.svelte` 接 paneId prop，DnD 支持跨 pane drop
  - `SessionContextMenu.svelte` + `TabContextMenu` 新增 split 项
  - `Sidebar.svelte` 的 onSelectSession 走 focused pane
- **测试**：paneHelpers 单元测试（纯函数）；tabStore 迁移后所有消费者 svelte-check 必须通过
- **无后端变更**：Tauri IPC / Rust crate 完全不动
- **兼容性**：旧 tabStore API 全 breaking，但没有外部消费者，只有 UI 内部调用
- **不做**：多选 tab（selectedTabIds）、pane 持久化到磁盘、vertical split（上下分屏）
