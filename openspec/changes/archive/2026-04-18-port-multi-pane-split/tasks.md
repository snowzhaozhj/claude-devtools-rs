## 1. 数据模型与纯函数 helpers

- [x] 1.1 新增 `ui/src/lib/paneTypes.ts`：定义 `Pane { id, tabs, activeTabId, widthFraction }` 与 `PaneLayout { panes, focusedPaneId }`，常量 `MAX_PANES = 4` / `MIN_FRACTION = 0.1`
- [x] 1.2 新增 `ui/src/lib/paneHelpers.ts`：纯函数 `createEmptyPane(id) / findPane(layout, id) / findPaneByTabId(layout, tabId) / updatePane(layout, pane) / insertPane(layout, anchorId, newPane, direction) / removePane(layout, id) / rebalanceWidths(panes) / resizeAdjacent(layout, paneId, newFraction) / getAllTabs(layout)`，全部返回新对象引用
- [x] 1.3 调整：UI 层无 vitest 基建，改在 `paneHelpers.ts` 末尾加 `import.meta.env.DEV` 自检 asserts（rebalanceWidths 均分、insertPane 左右、removePane focus fallback、resizeAdjacent sum 保持 / clamp）
- [x] 1.4 `npm run check --prefix ui` 通过（0 errors）

## 2. tabStore 重构（保对外 API 名字）

- [x] 2.1 重写 `ui/src/lib/tabStore.svelte.ts`：内部改为 `paneLayout = $state<PaneLayout>(...)`，初始为单 pane；`tabUIStates` / `tabSessionCache` 继续按 tabId 索引不变
- [x] 2.2 保留并重写 `getTabs() → focusedPane.tabs` / `getActiveTab()` / `getActiveTabId() → focusedPane.activeTabId` / `setActiveTab(tabId)`（自动 focus 所在 pane）
- [x] 2.3 重写 `openTab(sessionId, projectId, label) / openSettingsTab() / openNotificationsTab()`：若 sessionId 已在任何 pane 打开 → focus 该 pane + set activeTabId；否则在 `focusedPaneId` 对应 pane 追加新 tab
- [x] 2.4 重写 `closeTab(tabId)`：从 tab 所在 pane 移除；若 pane 变空且非唯一 pane → removePane + rebalance；若是唯一 pane 的最后 tab → activeTabId 置 null
- [x] 2.5 新增导出：`getPaneLayout() / getFocusedPaneId() / focusPane(paneId) / splitPane(sourcePaneId, tabId, direction) / closePane(paneId) / moveTabToPane(tabId, sourcePaneId, targetPaneId, insertIndex?) / moveTabToNewPane(tabId, sourcePaneId, adjacentPaneId, direction) / reorderTabInPane(paneId, from, to) / resizePanes(paneId, newFraction) / getAllTabs()`
- [x] 2.6 保留 `reorderTab(from, to)` 作为 `reorderTabInPane(focusedPaneId, from, to)` 的便捷包装，避免外部调用点批量改
- [x] 2.7 `npm run check --prefix ui` 通过（0 errors，所有旧消费者兼容）

## 3. 布局组件

- [x] 3.1 新增 `ui/src/components/layout/PaneContainer.svelte`：横向 flex 容器，`{#each layout.panes as pane (pane.id)}` 渲染 `<PaneView>` + 相邻间 `<PaneResizeHandle>`
- [x] 3.2 新增 `ui/src/components/layout/PaneView.svelte`：渲染 `<TabBar paneId>` + 内容区（按 activeTab.type switch）+ 左右 `<PaneSplitDropZone>`；宽度 `flex: pane.widthFraction`；pointerdowncapture 自动 focusPane
- [x] 3.3 新增 `ui/src/components/layout/PaneResizeHandle.svelte`：pointerdown 拖拽，相对容器 fraction 减去前序 cumulative 得 leftPane 新 fraction，调 `resizePanes`
- [x] 3.4 新增 `ui/src/components/layout/PaneSplitDropZone.svelte`：dragging 时 pointer-events:auto；命中时高亮；data-pane-id / data-side 供 dragSession.hitTest
- [x] 3.5 调整：DnD 统一 pointer events 方案（macOS WKWebView HTML5 drag 不可靠）；新增 `ui/src/lib/dragSession.svelte.ts` 管理全局 drag + document-level pointermove/up，通过 elementFromPoint 命中 `.tab-item` 或 `.pane-drop-zone`，pointerup 自动派发 reorder/moveTo/moveToNew

## 4. App.svelte 布局替换

- [x] 4.1 `App.svelte` 用 `<PaneContainer>` 替换原 `<TabBar> + <main class="main-content">`（清理冗余 CSS）
- [x] 4.2 `selectSession` 继续调 `openTab`（tabStore 内部 resolve 到 focused pane）
- [x] 4.3 Cmd+1~9 / Cmd+W / Cmd+[/] 自动作用于 focused pane（因为 `getTabs/getActiveTabId` 已代理到 focused pane）
- [x] 4.4 新增 `Cmd+\\` split activeTab 到右侧；`Cmd+Option+←/→` focus 上/下一个 pane（循环）
- [x] 4.5 DashboardView 空状态条件：`PaneView` 根据 `isSolePane` 且无 activeTab 渲染 `<DashboardView>`；非唯一 pane 空显示 "此 Pane 暂无 Tab"

## 5. TabBar DnD 升级

- [x] 5.1 `TabBar.svelte` 新增 `paneId: string` prop，所有 tabs 渲染从 `getTabs()` 改为 `getPaneById(paneId).tabs`
- [x] 5.2 调整：DnD 改 pointer events 方案。TabBar `onpointerdown` 调 `beginDrag(tabId, paneId, index, startX)` 把控制权交 dragSession
- [x] 5.3 调整：命中 & 派发统一由 `dragSession` 在 document 层做（.tab-item 跨 pane → moveTabToPane；.pane-drop-zone → moveTabToNewPane；同 index → noop）
- [x] 5.4 tab 点击走 dragSession pointerup 路径：未越阈值视为点击，自动 `setActiveTab`（会先 focusPane）；键盘 Enter/Space 直接 `setActiveTab`
- [x] 5.5 tab 关闭按钮 `onclick` 调 `closeTab(tab.id)`（tabStore 内部按 tabId 找到所在 pane）

## 6. 右键菜单扩展

- [x] 6.1 新增 `openTabInNewPane` 便捷方法；`SessionContextMenu` 加 `onOpenInNewPane` + `canSplit` prop（禁用态视觉）；Sidebar 传入
- [x] 6.2 新建 `ui/src/components/TabContextMenu.svelte`：关闭 / 关闭其他 / Split Left / Split Right；达 MAX_PANES 时 Split 禁用
- [x] 6.3 TabBar 的 tab 绑 `oncontextmenu` 弹 TabContextMenu，Split 调 `splitPane(paneId, tabId, direction)`

## 7. 视觉细节

- [x] 7.1 Focused pane 顶部 accent：仅多 pane（`:not(.sole)`）时渲染 `box-shadow: inset 0 2px 0 0 var(--color-border-emphasis)`，与 active tab 的 bottom border 同语义同色。单 pane 不渲染任何 focus 视觉。颜色统一用主题 token（放弃初版蓝色 #3b82f6，与 Soft Charcoal 暖灰配色不搭）
- [x] 7.2 `PaneSplitDropZone` 宽 48px，dragging 时 `pointer-events:auto`，hit 时 10% 文字色 + border-emphasis 边框
- [x] 7.3 `PaneResizeHandle` hover/active 底色 `var(--color-border-emphasis)`
- [x] 7.4 唯一 pane + 空 tab 时 `PaneView` 渲染 `<DashboardView>`；非唯一 pane 空时显示 "此 Pane 暂无 Tab"
- [x] 7.5 拖拽体验优化：`dragSession` 越阈值时全局 `body.userSelect=none` + `cursor=grabbing`，避免跨 pane 拖 tab 时目标 pane 文本被选中；cleanup 时还原
- [x] 7.6 TabBar action 按钮（🔔/⚙）用 lucide SVG path（`BELL`/`SETTINGS` 加到 icons.ts）替换 emoji；hover 底色用 `var(--color-surface-raised)`；每 pane 都渲染一份对齐原版

## 8. 验收与归档

- [x] 8.1 `npm run check --prefix ui` 0 errors（5 warnings 全部是既有 SessionDetail 闭包 + SettingsView a11y，跟本 change 无关）
- [x] 8.2 `cargo tauri dev` 启动桌面应用手测（已验证通过）：
  - 单 pane 状态下所有原行为不变（openTab / closeTab / Cmd+1~9 / Cmd+W / Cmd+K）
  - Sidebar 右键 "Open in New Pane" 创建第二个 pane
  - Tab 右键 "Split Right" 创建新 pane
  - 两个 pane 各自独立滚动、expandedChunks、search
  - 跨 pane 拖拽 tab（到 TabBar 中间 / 到左右 DropZone）
  - 拖拽 PaneResizeHandle 调整宽度
  - 关闭 pane 内最后一个 tab 自动移除 pane
  - 达到 4 pane 时 Split Right 禁用
  - Cmd+\\ 快捷键触发 split
  - Cmd+Option+←/→ focus 上下 pane
- [x] 8.3 sync 本次 change 的三份 spec delta 回主 spec（随 archive 进行）
- [x] 8.4 `openspec validate port-multi-pane-split --strict` 通过
- [x] 8.5 `openspec archive port-multi-pane-split -y`
