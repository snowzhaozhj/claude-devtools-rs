## Context

Rust port 当前 `tabStore.svelte.ts` 是扁平模型：`tabs: Tab[]` + 单 `activeTabId` + 模块级 `tabUIStates: Map<tabId, TabUIState>` + `tabSessionCache: Map<tabId, SessionDetail>`。布局是 `Sidebar + (TabBar + main-content)`，`main-content` 根据 activeTab.type switch 渲染 SessionDetail/SettingsView/NotificationsView/DashboardView。

原版用 zustand `paneSlice` 管 `paneLayout: { panes: Pane[]; focusedPaneId }`，每个 Pane 自带 tabs 和 activeTabId。布局用 `PaneContainer`（横向 flex）包多个 `PaneView`（独立 TabBar + 内容区），中间插 `PaneResizeHandle`，Pane 边缘放置 `PaneSplitDropZone`（tab 拖到这里 → 创建新 pane）。

本 change 不改后端、不改 spec 外行为，只在前端重构状态模型 + 加布局层。

## Goals / Non-Goals

**Goals:**

- `paneLayout` 作为前端 tab 状态的唯一真相源；`tabs` 概念完全隔离到 pane 内
- 支持 split（最多 4 pane）/ close pane / 跨 pane 移动 tab / 拖拽 resize
- per-tab UI 状态（scroll / expanded / search visible）与 per-tab session cache **继续按 tabId 索引**，不随 pane 迁移
- 保留 `openTab / openSettingsTab / openNotificationsTab` 这套公共 API 名字，内部 resolve 到 `focusedPaneId`
- Sidebar session 点击、CommandPalette 打开均走 focused pane
- 键盘 `Cmd+1~9 / Cmd+W / Cmd+[/]` 作用于 **focused pane 内的 tab**（需改 App.svelte 里 handler 语义）

**Non-Goals:**

- 多选 tab（原版 `selectedTabIds`，当前使用率低）
- 垂直分屏（上下）
- Pane layout 持久化到磁盘（重启后回到单 pane 即可）
- 重写 DnD 库；继续用原生 `dragstart / dragover / drop` + dataTransfer
- 改动 Tauri IPC / Rust crate

## Decisions

### Decision 1: `tabUIStates` 和 `tabSessionCache` 不按 pane 索引

tabId 全局唯一，且 tab 迁移到另一个 pane 时应**保留** scroll 位置和 session cache。按 pane 索引会让 moveTab 时需要 migrate state，徒增复杂度。

备选（放弃）：Map<paneId, Map<tabId, State>> 两层索引。徒增 API 表面，无收益。

### Decision 2: Pane 默认 widthFraction 均分；resize 时只调相邻两个

resize 的实现限制：只能拖动两个相邻 pane 之间的 handle，交换它们的宽度权重（`a.width + b.width = const`），其他 pane 不动。这是原版行为，也是最直觉的。

算法：`resizePanes(paneId, newFraction)` → pane[i] 设为 newFraction（clamped to `[0.1, 1 - 0.1*(n-1)]`），pane[i+1] 补差额；其余不变。

### Decision 3: split 时的 pane 插入位置

原版：split 用 `insertPane(layout, anchorId, newPane, direction)`，在 anchor 左/右插入。新 pane 从均分宽度池里"借"一份：原 anchor 宽度减半，新 pane 拿一半。

实现：`insertPane` 重新计算宽度 = 1 / n（n = 新的 panes 数量），全部均分一遍。**简化**——原版按 anchor 宽度的一半分配更平滑，但均分足够 MVP。

### Decision 4: focusedPaneId 的职责

`focusedPaneId` 决定两件事：
1. Sidebar / CommandPalette 打开 session 时放到哪个 pane
2. 键盘 `Cmd+1~9 / Cmd+W` 作用的 pane

`focusedPaneId` 的切换时机：
- 用户点击某 pane 内任意 tab → focus 该 pane
- 新 split 出来的 pane 自动成为 focused
- closePane 后 focus 落到相邻 pane（优先同索引，否则左邻）

Sidebar 的 activeSessionId 高亮应跟 `focused pane.activeTabId` 走。

### Decision 5: TabBar / SessionContextMenu 的 DnD

- **TabBar 内重排**：已有行为，改为 `reorderTabInPane(paneId, from, to)`
- **跨 TabBar 移动**：drop target 是另一个 TabBar，payload 带 sourcePaneId + tabId → `moveTabToPane`
- **创建新 pane**：每个 PaneView 的左/右边缘渲染一个 `PaneSplitDropZone`（宽度 24px，默认透明，dragover 时显示蓝色提示），drop 时 → `moveTabToNewPane(tabId, sourcePaneId, adjacentPaneId, direction)`
- **已满 4 pane**：drop zone hover 时显示禁用光标（或直接不渲染 drop zone）

用 HTML5 `draggable` + `dataTransfer.setData("application/x-tab-id", tabId)` + `setData("application/x-pane-id", paneId)` 传递信息。

### Decision 6: Svelte 5 响应性

`paneLayout` 是 `$state({...})`。所有修改**必须**产生新对象引用（`panes: [...old, new]`），避免 mutate 导致 fine-grained 响应性失效。helpers 函数全部返回新 layout，不改原引用。这和原版 zustand immutable 风格一致。

### Decision 7: 迁移公共 API 表面

`tabStore.svelte.ts` 对外导出的 `openTab / closeTab / setActiveTab / getTabs / getActiveTab / getActiveTabId / reorderTab` 全部**保留签名**但内部重写：
- `openTab` → 作用于 `getFocusedPane()`
- `getTabs()` → 返回 `focusedPane.tabs`（或新增 `getAllTabs()` 给 CommandPalette 用）
- `setActiveTab(tabId)` → 找到 tab 所在 pane 并 focus 该 pane + set activeTabId

这样改后 TabBar / App.svelte / Sidebar 的调用点**大多数不用改**，只有需要 paneId 语义的地方（DnD、split 快捷键）要新增调用。

App.svelte 的快捷键 handler：`Cmd+1~9` 用 `getFocusedPane().tabs[idx]`；`Cmd+W` 关 focused pane 的 activeTab；`Cmd+[/]` 在 focused pane 内循环。

### Decision 8: 布局组件拆分

- `PaneContainer.svelte`：横向 flex 容器，遍历 `paneLayout.panes` 渲染 `<PaneView>` + `<PaneResizeHandle>`
- `PaneView.svelte`：单 pane 的内容（TabBar + main-content + 左右 DropZone）
- `PaneResizeHandle.svelte`：pane 之间的拖拽条
- `PaneSplitDropZone.svelte`：pane 边缘 tab drop 区域

TabBar 继续复用，新增 `paneId` prop。

## Risks / Trade-offs

- [tabStore breaking change] → 风险：所有消费者需回改。Mitigation：保留 `openTab` 等名字，只重写内部，扫一遍 `ui/src/` 的所有 import 统改。
- [Svelte 5 响应性与复杂对象] → 嵌套 `paneLayout.panes[i].tabs` 改动可能触发整层 re-render。Mitigation：`{#each panes as pane (pane.id)}` + `{#each pane.tabs as tab (tab.id)}` 稳定 key；若性能问题再引入 `SvelteMap` 或切片订阅。
- [DnD 跨 pane 数据传递] → HTML5 DnD 的 dataTransfer 在 dragover 事件里读取受限。Mitigation：用模块级 `$state` 存 drag session（draggingTabId / sourcePaneId），drop 时直接读，不依赖 dataTransfer。
- [pane resize 与内部虚拟滚动冲突] → 没做虚拟滚动，暂不存在；未来引入时需观察。
- [快捷键歧义] `Cmd+\` 在 macOS 某些环境被抢占。Mitigation：`preventDefault` 兜底；若冲突改 `Cmd+D`（原版也是 `Cmd+D`？待查）。
- [MAX_PANES=4 在窄窗口不可用] → 4 pane 每个只有 ~300px，可能无法用。Mitigation：用户自己决定是否 split，不做硬限制；若未来加 min width 校验则 reject split。

## Migration Plan

1. **先落 paneHelpers.ts 纯函数 + 单测**（无副作用，独立验证）
2. **重写 tabStore.svelte.ts**：内部切换到 paneLayout，对外 API 保兼容（openTab/getTabs 等保留名字）
3. **svelte-check 扫全量消费者**：修所有 breaking 点
4. **新增 PaneContainer / PaneView / PaneResizeHandle / PaneSplitDropZone 组件**
5. **App.svelte 布局替换** `TabBar + main-content` → `PaneContainer`
6. **TabBar DnD 升级**：跨 pane drop + split drop zone
7. **ContextMenu 新增 split 项**
8. **快捷键 `Cmd+\` + `Cmd+Option+←/→`**
9. **手测**：单 pane 保持原行为；split 后双 pane 各自独立；跨 pane 拖 tab；resize；close pane
10. 归档前同步 tab-management / sidebar-navigation / session-display spec delta 回主 spec

## Open Questions

- `Cmd+\` 与 VSCode 风格对齐，但原版实际用的快捷键是什么？归档前核对（不 block 实现，默认 `Cmd+\`）
- CommandPalette 的"Open in Split"操作要不要加？本次不做，留给后续
- Pane 折叠（最小化某 pane）？本次不做
