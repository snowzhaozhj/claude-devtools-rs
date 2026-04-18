## MODIFIED Requirements

### Requirement: 打开 session tab

用户从 Sidebar 点击会话时，系统 SHALL 在当前 focused pane 内打开一个 session tab。若该 sessionId 已有打开的 tab（无论在哪个 pane），系统 SHALL 切换焦点到已有 tab 所在 pane 并激活该 tab 而非创建重复 tab。新 tab 的 label SHALL 为 session 标题（截断至 50 字符），id SHALL 为唯一标识符。

#### Scenario: 首次打开 session
- **WHEN** 用户点击 Sidebar 中一个尚未打开的 session
- **THEN** 系统 SHALL 在 `focusedPaneId` 对应的 pane 中创建新 tab 并设为该 pane 的 activeTabId，对应 PaneView 的 TabBar SHALL 显示该 tab

#### Scenario: 重复点击已打开的 session（同 pane）
- **WHEN** 用户点击 Sidebar 中一个已在 focused pane 内的 session
- **THEN** 系统 SHALL 切换 focused pane 的 activeTabId 到该 tab，不创建新 tab

#### Scenario: 重复点击已打开的 session（其他 pane）
- **WHEN** 用户点击 Sidebar 中一个 tab 位于其他 pane 的 session
- **THEN** 系统 SHALL 把 `focusedPaneId` 切到该 tab 所在 pane，并将该 pane 的 activeTabId 设为该 tab，不创建新 tab

#### Scenario: Tab label 截断
- **WHEN** session 标题超过 50 字符
- **THEN** tab label SHALL 截断到 50 字符并追加省略号

### Requirement: 关闭 tab

用户点击 tab 的关闭按钮时，系统 SHALL 从该 tab 所在 pane 移除该 tab 并清理其关联的 UI 状态和 session 缓存。若关闭后 pane 变空且不是唯一 pane，该 pane SHALL 被自动关闭。

#### Scenario: 关闭非活跃 tab
- **WHEN** 用户关闭某 pane 内一个非该 pane active 的 tab
- **THEN** 该 tab SHALL 从 pane.tabs 中移除，该 pane 的 activeTabId 不变

#### Scenario: 关闭活跃 tab 且同 pane 还有其他 tab
- **WHEN** 用户关闭某 pane 的 activeTab 且该 pane 中还有其他 tab
- **THEN** 系统 SHALL 自动把该 pane 的 activeTabId 设为相邻 tab（优先同位置索引，否则前一个）

#### Scenario: 关闭 pane 中最后一个 tab（非唯一 pane）
- **WHEN** 用户关闭某 pane 中唯一的 tab，且该 pane 不是唯一 pane
- **THEN** 该 pane SHALL 被移除，相邻 pane 中的一个 SHALL 成为新的 focused pane（优先右邻，否则左邻）

#### Scenario: 关闭 pane 中最后一个 tab（唯一 pane）
- **WHEN** 用户关闭唯一 pane 中唯一的 tab
- **THEN** 该 pane SHALL 保留但 activeTabId 变为 null，Main 区域 SHALL 显示 Dashboard 占位

#### Scenario: 关闭时清理资源
- **WHEN** 任何 tab 被关闭
- **THEN** 该 tab 的 per-tab UI 状态和 session 数据缓存 SHALL 被删除

### Requirement: 切换 tab

用户点击某 pane 的 TabBar 中的 tab 时，系统 SHALL 切换该 pane 的 activeTabId 并恢复目标 tab 的 UI 状态，同时 SHALL 将 `focusedPaneId` 设为该 pane。

#### Scenario: 切换到有缓存的 tab
- **WHEN** 用户点击一个已加载过 session 数据的 tab
- **THEN** 系统 SHALL 从缓存恢复 session 数据，不发起 API 请求

#### Scenario: 切换时保存当前 tab 状态
- **WHEN** 用户从 tab A 切换到 tab B（无论同 pane 或跨 pane）
- **THEN** tab A 的展开/折叠状态、搜索状态、Context Panel 状态和滚动位置 SHALL 被保存

#### Scenario: 点击 tab 时同时 focus 该 pane
- **WHEN** 用户点击一个非 focused pane 内的 tab
- **THEN** `focusedPaneId` SHALL 更新为该 pane，Sidebar 高亮与快捷键作用域 SHALL 立即跟随

### Requirement: Per-tab UI 状态隔离

每个 tab SHALL 维护独立的 UI 状态（expandedChunks、expandedItems、searchVisible、contextPanelVisible、scrollTop）。UI 状态 SHALL 按 tabId 索引，不随 pane 迁移而重置。不同 tab 的 UI 操作 SHALL 互不影响。

#### Scenario: 两个 tab 打开同一 session（不同 pane）
- **WHEN** 同一 session 在两个 pane 各开一个 tab（通过 "Open in New Pane" 或 split 操作）
- **THEN** 两个 tab 的展开状态、滚动位置 SHALL 各自独立

#### Scenario: 滚动位置恢复
- **WHEN** 用户在 tab A 滚动到某位置，切换到 tab B 后再切回 tab A
- **THEN** tab A 的 conversation 滚动位置 SHALL 恢复到之前保存的值

#### Scenario: 跨 pane 移动后 UI 状态保留
- **WHEN** 用户把 tab A 从 pane 1 拖到 pane 2
- **THEN** tab A 的 scrollTop / expanded 等 UI 状态 SHALL 原样保留

### Requirement: Session 数据缓存

已加载的 session 数据 SHALL 以 tab 为粒度缓存，按 tabId 索引。切换 tab（含跨 pane）时若缓存命中 SHALL 跳过 API 调用。关闭 tab 时缓存 SHALL 被释放。

#### Scenario: 缓存命中
- **WHEN** 切换到一个之前已加载完成的 tab
- **THEN** SessionDetail 数据 SHALL 直接从缓存读取，loading 状态 SHALL 不出现

#### Scenario: 缓存未命中
- **WHEN** 切换到一个首次打开的 tab
- **THEN** 系统 SHALL 调用 getSessionDetail API 加载数据，显示 loading 状态，加载完成后存入缓存

#### Scenario: 跨 pane 移动后缓存保留
- **WHEN** 用户把已加载 session 的 tab 从 pane 1 拖到 pane 2
- **THEN** 在 pane 2 中激活该 tab SHALL 命中缓存，不再发起 API 请求

### Requirement: TabBar 渲染

每个 pane SHALL 在其内容区顶部渲染一个独立的 TabBar，列出该 pane 的 tabs。pane.tabs 为空的 pane SHALL 不渲染 TabBar（若仍存在，说明是唯一 pane 的空状态）。

#### Scenario: 有 tab 时显示 TabBar
- **WHEN** pane.tabs 非空
- **THEN** 该 pane 的 TabBar SHALL 可见，高度固定约 36px，显示该 pane 的所有 tab 项

#### Scenario: 无 tab 时隐藏 TabBar
- **WHEN** pane.tabs 为空
- **THEN** 该 pane SHALL NOT 渲染 TabBar（唯一 pane 的空状态显示 Dashboard 占位）

#### Scenario: Active tab 视觉区分
- **WHEN** 某 pane 的 activeTabId 等于某 tab 的 id
- **THEN** 该 tab SHALL 有区别于非 active tab 的视觉样式（背景色、底部边框等）

#### Scenario: Focused pane 视觉区分
- **WHEN** 存在多个 pane 且其中一个 `pane.id === focusedPaneId`
- **THEN** 该 pane 的 TabBar 或边框 SHALL 有可见的 focused 视觉标识（例如更亮的顶部 accent 线或边框色）

### Requirement: Sidebar 与 Tab 联动

Sidebar 的会话高亮 SHALL 跟随 focused pane 的 activeTabId 对应的 sessionId。切换 focused pane 或 focused pane 的 activeTab 时 Sidebar 高亮 SHALL 同步更新。

#### Scenario: 切换 focused pane 后 Sidebar 同步
- **WHEN** 用户点击另一 pane 的 tab 使其成为 focused pane
- **THEN** Sidebar 中对应 focused pane 的 activeTab session 项 SHALL 高亮，之前的高亮 SHALL 移除

#### Scenario: 同 pane 内切换 tab 后 Sidebar 同步
- **WHEN** 用户在 focused pane 内切换 activeTab
- **THEN** Sidebar 高亮 SHALL 同步到新 activeTab 的 sessionId

#### Scenario: 无 active tab 时 Sidebar 无高亮
- **WHEN** focused pane 的 activeTabId 为 null
- **THEN** Sidebar 中 SHALL 无 session 项被高亮

## ADDED Requirements

### Requirement: Pane 生命周期

系统 SHALL 维护 `paneLayout: { panes: Pane[]; focusedPaneId: string }` 作为 tab 状态的唯一真相源。初始状态 SHALL 为单 pane，MAX_PANES SHALL 为 4。用户 SHALL 可通过 tab 右键菜单 "Split Left" / "Split Right"、Sidebar 会话右键菜单 "Open in New Pane"、或快捷键 `Cmd+\` 创建新 pane。closePane 操作 SHALL 仅在 `panes.length > 1` 时允许。

#### Scenario: 初始单 pane
- **WHEN** 应用启动
- **THEN** `paneLayout.panes.length` SHALL 等于 1，`focusedPaneId` SHALL 指向该唯一 pane

#### Scenario: Split 创建新 pane（向右）
- **WHEN** 用户在 tab 右键菜单选择 "Split Right" 或按 `Cmd+\`
- **AND** `paneLayout.panes.length < 4`
- **THEN** 系统 SHALL 创建一个新 pane 插入到当前 pane 右侧，把触发 split 的 tab 移动到新 pane，新 pane SHALL 成为 focused

#### Scenario: Split 达到 MAX_PANES 上限
- **WHEN** 用户尝试 split 但 `paneLayout.panes.length === 4`
- **THEN** 系统 SHALL NOT 创建新 pane，操作 SHALL 静默忽略或展示禁用视觉

#### Scenario: 关闭 pane
- **WHEN** 用户关闭某个非唯一 pane（通过关闭该 pane 内最后一个 tab 或显式关闭 pane）
- **THEN** 该 pane SHALL 从 `paneLayout.panes` 中移除，相邻 pane 的 widthFraction SHALL 重新均分，`focusedPaneId` SHALL 切到相邻 pane

#### Scenario: 唯一 pane 不可关闭
- **WHEN** 只剩一个 pane 且用户关闭其最后一个 tab
- **THEN** 该 pane SHALL 保留（activeTabId 变为 null），`focusedPaneId` 保持指向它

### Requirement: 跨 Pane 拖拽 tab

用户 SHALL 可通过拖拽把 tab 从一个 pane 的 TabBar 移动到另一个 pane。支持三种 drop 行为：同 pane 重排、跨 pane 插入、拖到 pane 边缘 drop zone 创建新 pane。

#### Scenario: 同 pane 内重排
- **WHEN** 用户把 tab 拖到同一个 TabBar 的不同位置
- **THEN** 系统 SHALL 调用 `reorderTabInPane(paneId, from, to)` 更新 pane.tabs 顺序，不改变 activeTabId 所指 tab（但 activeTabId 可能对应新索引）

#### Scenario: 跨 pane 移动
- **WHEN** 用户把 tab 从 pane A 的 TabBar 拖到 pane B 的 TabBar（非边缘 drop zone）
- **THEN** 系统 SHALL 调用 `moveTabToPane(tabId, paneA.id, paneB.id)`，tab 被从 pane A 移除并追加/插入到 pane B，pane B 的 activeTabId SHALL 设为该 tab，`focusedPaneId` SHALL 切到 pane B

#### Scenario: 拖到 pane 边缘创建新 pane
- **WHEN** 用户把 tab 拖到另一 pane 的左/右边缘 `PaneSplitDropZone`
- **AND** `paneLayout.panes.length < 4`
- **THEN** 系统 SHALL 调用 `moveTabToNewPane(tabId, sourcePaneId, adjacentPaneId, direction)`，在 adjacent pane 的指定方向插入新 pane，tab 迁入新 pane，新 pane 成为 focused

#### Scenario: 源 pane 因拖空而自动关闭
- **WHEN** 跨 pane 移动后源 pane.tabs 变空，且 `paneLayout.panes.length > 1`
- **THEN** 源 pane SHALL 被自动移除，相邻 pane 的 widthFraction 重新均分

### Requirement: Pane 宽度拖拽 resize

相邻 pane 之间 SHALL 渲染一个可拖拽的 `PaneResizeHandle`。拖动 handle SHALL 仅调整相邻两个 pane 的 widthFraction，其他 pane 宽度不变。单个 pane 的 widthFraction SHALL clamp 到 `[0.1, 1 - 0.1 * (n - 1)]`（n = 总 pane 数）。

#### Scenario: 拖动相邻 handle
- **WHEN** 用户在 pane i 与 pane i+1 之间的 handle 上拖拽
- **THEN** pane i 的 widthFraction SHALL 跟随鼠标位置更新，pane i+1 SHALL 补差额以保持二者之和不变

#### Scenario: clamp 到最小宽度
- **WHEN** 拖拽使 pane i 的 fraction 将小于 0.1
- **THEN** 系统 SHALL 把 pane i 的 fraction clamp 到 0.1 并停止进一步缩小

#### Scenario: 不影响非相邻 pane
- **WHEN** 在 pane 1 和 pane 2 之间 resize
- **THEN** pane 3、pane 4 的 widthFraction SHALL 保持不变

#### Scenario: 视觉反馈
- **WHEN** 鼠标悬停或拖动 resize handle
- **THEN** handle SHALL 展示可拖拽视觉（例如更亮底色或 col-resize 光标）
