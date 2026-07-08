# tab-management Specification

## Purpose

定义桌面应用的多 Tab 管理行为：Tab 生命周期（打开 / 关闭 / 切换）、per-tab UI 状态隔离、session 数据缓存、多 Pane 分屏（最多 4 pane）、tab 跨 pane 拖拽与 pane 宽度 resize。Sidebar 高亮 SHALL 始终跟随 focused pane 的 activeTab。Tab 跨进程持久化留作后续扩展。
## Requirements
### Requirement: 打开 session tab

用户从 Sidebar 点击会话时，系统 SHALL 在当前 focused pane 内打开一个 session tab。若该 sessionId 已有打开的 tab（无论在哪个 pane），系统 SHALL 切换焦点到已有 tab 所在 pane 并激活该 tab 而非创建重复 tab。新 tab 的 `label` 字段 SHALL 为 **完整的** session 标题（来自 `SessionSummary.title`，由后端按 `TITLE_MAX_CHARS = 500` 截断；前端 JS SHALL NOT 在此基础上再做任何不可逆截断），`id` SHALL 为唯一标识符。

视觉截断 SHALL 由 TabBar 渲染层的 CSS 实现：`.tab-label` 元素 SHALL 同时设置 `max-width`（合理的桌面 tab 视觉宽度，建议 150-200 px）+ `overflow: hidden` + `text-overflow: ellipsis` + `white-space: nowrap`。

Tab 容器 SHALL 在 `<button>` / `<span>` 等可 hover 的根元素上设置 HTML `title` 属性，值 SHALL 等于 **完整未截断的 tab label**，让浏览器原生 tooltip 在 hover 时显示全文。

`tabStore::shortLabel`（或等价的 JS 截断函数）SHALL 被移除，或改为透传 `(label) => label`；任何 `label.slice(0, N) + "…"` 形式的不可逆截断 SHALL NOT 出现在前端代码中——理由：JS 截断让 hover tooltip 也只能拿到截断版，造成信息丢失，无法通过拉宽 / hover 恢复。

#### Scenario: 首次打开 session
- **WHEN** 用户点击 Sidebar 中一个尚未打开的 session
- **THEN** 系统 SHALL 在 `focusedPaneId` 对应的 pane 中创建新 tab 并设为该 pane 的 activeTabId，对应 PaneView 的 TabBar SHALL 显示该 tab

#### Scenario: 重复点击已打开的 session（同 pane）
- **WHEN** 用户点击 Sidebar 中一个已在 focused pane 内的 session
- **THEN** 系统 SHALL 切换 focused pane 的 activeTabId 到该 tab，不创建新 tab

#### Scenario: 重复点击已打开的 session（其他 pane）
- **WHEN** 用户点击 Sidebar 中一个 tab 位于其他 pane 的 session
- **THEN** 系统 SHALL 把 `focusedPaneId` 切到该 tab 所在 pane，并将该 pane 的 activeTabId 设为该 tab，不创建新 tab

#### Scenario: Tab label 长度由后端控制 不再 JS 截断
- **WHEN** 后端 `SessionSummary.title` 长度为 120 字符
- **THEN** 对应新 tab 的 `label` 字段 SHALL 也是 120 字符（一字不少）
- **AND** TabBar 渲染时 SHALL 通过 CSS `max-width` + `text-overflow: ellipsis` 视觉截断超出部分
- **AND** 用户 hover tab 时浏览器原生 tooltip SHALL 显示完整 120 字符

#### Scenario: Tab tooltip 显示完整 label
- **WHEN** 任意 tab 的 label 含超出 CSS `max-width` 的内容
- **THEN** Tab 容器 HTML `title` 属性 SHALL 等于完整 `tab.label` 字符串
- **AND** 用户 hover 时 SHALL 看到完整字符串的原生 tooltip

#### Scenario: 不允许 JS 不可逆截断
- **WHEN** 在前端代码中搜索 `label.slice(0, ` / `label.substring(0, ` / `tab.label.slice` 等模式
- **THEN** SHALL NOT 出现任何作用于 `tab.label` 的不可逆字符截断（含 "…" 拼接）

#### Scenario: 点击会话打开标签页
- **WHEN** 用户点击一个会话项
- **THEN** 系统 SHALL 在当前聚焦窗格打开或激活该会话对应的标签页

#### Scenario: 会话详情按所属工作树加载
- **WHEN** 用户点击 sidebar 某 session 行（所属 worktree id 为 "wt-X1"、session id 为 "sid"）
- **THEN** 会话标签页身份 SHALL 关联该会话所属 worktree、session 与 group，用于详情加载与 Sidebar 高亮
- **AND** session 详情视图 SHALL 使用 worktree id "wt-X1" 与 session id "sid" 请求详情数据，而不是使用 group id

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

每个 tab SHALL 维护独立的 UI 状态（`expandedChunks`、`expandedItems`、`searchVisible`、`contextPanelVisible`、滚动状态）。UI 状态 SHALL 按 tabId 索引，不随 pane 迁移而重置。不同 tab 的 UI 操作 SHALL 互不影响。

滚动状态 SHALL 以「视觉位置」语义而非绝对 scrollTop 数值持久化，记录三件套 `{ atBottom: boolean; anchorChunkId: string | null; anchorOffsetPx: number }`：

- `atBottom`：保存时点 conversation 容器是否粘底（`distanceFromBottom <= 16`）
- `anchorChunkId`：仅在 `atBottom=false` 时填，记录视口顶第一个可见 chunk 的 `chunkId`
- `anchorOffsetPx`：anchor 元素相对 conversation 容器顶的像素偏移，用于恢复时还原视口内子像素位置

恢复策略 SHALL 按以下分支执行：

- 若 `atBottom=true` → 系统 SHALL 一次写入 `scrollTop = scrollHeight`，并 SHALL 启动「粘底 pin」状态机：监听 conversation 容器子树变更（含 lazy markdown hydrate 引发的子树 / 文本 / 属性变化），每次变更触发时若仍处 pin 期则重写 `scrollTop = scrollHeight`；pin 在以下任一条件触发时 SHALL 终止——用户主动滚动（`distanceFromBottom > 16`）/ 200 ms 内无变更 / 5 s 上限超时；终止时 SHALL 释放观察器与 listener
- 若 `atBottom=false` 且 `anchorChunkId` 命中 conversation 内某 chunk 元素 → 系统 SHALL 把该元素滚动到视口顶并按 `anchorOffsetPx` 还原视口内偏移
- 若 `anchorChunkId` 未命中（chunk 因 refresh 后顺序变化或被合并丢失） → 系统 SHALL 降级到首屏顶部（不写 scrollTop）并 SHALL 在 console 记 warn，**不**回退到旧 scrollTop 数值方案

#### Scenario: 两个 tab 打开同一 session（不同 pane）

- **WHEN** 同一 session 在两个 pane 各开一个 tab（通过 "Open in New Pane" 或 split 操作）
- **THEN** 两个 tab 的展开状态、滚动状态 SHALL 各自独立

#### Scenario: 滚动位置恢复 - 切走时粘底

- **WHEN** 用户在 tab A 滚动到底部（`distanceFromBottom <= 16`），切换到 tab B 后再切回 tab A
- **THEN** tab A 的 conversation SHALL 仍处于底部（`distanceFromBottom <= 16`）
- **AND** 此后 lazy markdown 持续 hydrate 触发子树变更期间，conversation SHALL 持续保持在底部直到「粘底 pin」状态机终止

#### Scenario: 滚动位置恢复 - 切走时位于中间

- **WHEN** 用户在 tab A 滚动到非底部位置（如某个 chunk 在视口顶），切换到 tab B 后再切回 tab A
- **THEN** tab A 的视口顶 SHALL 仍是切走时记录的同一个 chunk
- **AND** 该 chunk 相对视口顶的像素偏移 SHALL 与切走时差异不超过 50 px

#### Scenario: 滚动位置恢复 - 切回时 lazy chunks 尚未 hydrate

- **WHEN** 用户切回 tab A，此时 conversation 容器内多数 lazy markdown 节点处于占位高度状态（scrollHeight 远小于切走时点）
- **THEN** 系统 SHALL **不**依赖切走时点的 scrollHeight 数值进行恢复
- **AND** 锚点 chunk 后续 hydrate 引起的高度变化 SHALL **不**让视觉位置偏离 anchor 元素自身位置

#### Scenario: 滚动位置恢复 - anchor chunk 失效降级

- **WHEN** 用户切回 tab A，但保存的 `anchorChunkId` 在当前 conversation 内找不到（如 refresh 后 chunk 被合并）
- **THEN** conversation SHALL 回到首屏顶部
- **AND** 系统 SHALL 在 console 记 warn 注明失败的 `anchorChunkId`
- **AND** 系统 SHALL **不**尝试用 scrollTop 数值或其它兜底位置

#### Scenario: 跨 pane 移动后 UI 状态保留

- **WHEN** 用户把 tab A 从 pane 1 拖到 pane 2
- **THEN** tab A 的滚动状态（`atBottom` / `anchorChunkId` / `anchorOffsetPx`）/ `expandedChunks` 等 UI 状态 SHALL 原样保留

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

#### Scenario: 高亮跟随当前窗格的活跃标签页
- **WHEN** focused pane 的 activeTabId 变化（无论通过 Sidebar 点击、TabBar 点击、跨 pane focus 切换还是快捷键）
- **THEN** Sidebar 中对应 sessionId 的会话项 SHALL 高亮，之前的高亮 SHALL 移除

### Requirement: Pane 生命周期

系统 SHALL 维护 `paneLayout: { panes: Pane[]; focusedPaneId: string }` 作为 tab 状态的唯一真相源。初始状态 SHALL 为单 pane，MAX_PANES SHALL 为 4。用户 SHALL 可通过 tab 右键菜单 "Split Left" / "Split Right"、Sidebar 会话右键菜单 "Open in New Pane"、或 `keyboard-shortcuts` registry 的 `pane.split` 当前 binding（默认 mac `⌘\` / 其他 `Ctrl+\`）创建新 pane。closePane 操作 SHALL 仅在 `panes.length > 1` 时允许。

`keyboard-shortcuts` registry SHALL 注册以下 spec id 并在 dispatcher 命中时执行对应操作；用户 SHALL 可在 `Settings → Keyboard Shortcuts` 中自定义任一 binding：

- `pane.split`（默认 `mod+\`）：触发 split right
- `pane.focus.next`（默认 `mod+alt+ArrowRight`）/ `pane.focus.prev`（默认 `mod+alt+ArrowLeft`）：循环切换 focused pane
- `tab.switch.<n>`（n=1..9，默认 `mod+1` ~ `mod+9`）：切到 focused pane 的第 n 个 tab；n > 当前 tab 数时静默忽略
- `tab.close`（默认 `mod+W`）：关闭 focused pane 的 active tab，遵循"关闭 tab" Requirement 既有路径
- `tab.next`（默认 `mod+]`）/ `tab.prev`（默认 `mod+[`）：循环切换 focused pane 的 active tab

#### Scenario: 初始单 pane

- **WHEN** 应用启动
- **THEN** `paneLayout.panes.length` SHALL 等于 1，`focusedPaneId` SHALL 指向该唯一 pane

#### Scenario: Split 创建新 pane（向右）

- **WHEN** 用户在 tab 右键菜单选择 "Split Right" 或按下 `pane.split` 当前 binding
- **AND** `paneLayout.panes.length < 4`
- **THEN** 系统 SHALL 创建一个新 pane 插入到当前 pane 右侧，把触发 split 的 tab 移动到新 pane，新 pane SHALL 成为 focused

#### Scenario: Split 达到最大 pane 数上限

- **WHEN** 用户尝试 split 但 `paneLayout.panes.length === 4`
- **THEN** 系统 SHALL NOT 创建新 pane，操作 SHALL 静默忽略或展示禁用视觉

#### Scenario: 关闭 pane

- **WHEN** 用户关闭某个非唯一 pane（通过关闭该 pane 内最后一个 tab 或显式关闭 pane）
- **THEN** 该 pane SHALL 从 `paneLayout.panes` 中移除，相邻 pane 的 widthFraction SHALL 重新均分，`focusedPaneId` SHALL 切到相邻 pane

#### Scenario: 唯一 pane 不可关闭

- **WHEN** 只剩一个 pane 且用户关闭其最后一个 tab
- **THEN** 该 pane SHALL 保留（activeTabId 变为 null），`focusedPaneId` 保持指向它

#### Scenario: 任一 pane / tab spec id 当前 binding 命中即触发对应操作

- **WHEN** 用户按下任一 pane / tab spec id 当前 binding（白名单：`pane.split` / `pane.focus.next` / `pane.focus.prev` / `tab.switch.1..9` / `tab.close` / `tab.next` / `tab.prev`）
- **AND** `document.activeElement` 不是 `<input>` / `<textarea>` / `[contenteditable="true"]`
- **THEN** registry dispatcher SHALL 命中该 spec
- **AND** 系统 SHALL 执行对应 pane / tab 操作（focus 切换 / tab 切换 / tab 关闭 / split 创建），副作用与等效鼠标操作一致
- **AND** 当操作前提不成立时 SHALL 静默忽略，包括但不限于：`pane.focus.next` / `pane.focus.prev` 在 `panes.length < 2` 时；`tab.next` / `tab.prev` 在 focused pane 仅 1 个 tab 时；`tab.switch.<n>` 在 n 超出当前 pane tab 数时；`tab.close` 在 focused pane 无 active tab 时；`pane.split` 在 `panes.length === 4` 时

#### Scenario: 用户改自定义 binding 后生效

- **WHEN** 用户在 `Settings → Keyboard Shortcuts` 把任一 spec id 的 binding 改为新组合
- **THEN** 后续按下新组合 SHALL 触发该 spec id 对应操作
- **AND** 按下原默认组合 SHALL NOT 触发该 spec id（除非另一 spec id 已占用该默认组合）

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

PaneResizeHandle SHALL 在 idle 态展示 1px 常驻分隔线（`--color-border-emphasis`），hover/active/focus-visible 态 SHALL 切换为整条半透明中性灰高亮（`color-mix(in oklch, var(--color-border-emphasis) 60%, transparent)`），此时分隔线 SHALL 消隐。视觉语言 SHALL 与 Sidebar resize handle 一致。

PaneResizeHandle SHALL 具有 `role="separator"` + `tabindex="0"` + `aria-orientation="vertical"` + `aria-label="拖动调整面板宽度"` ARIA 语义。`aria-valuemin` SHALL 为 `MIN_FRACTION * 100`，`aria-valuemax` SHALL 为 `(1 - MIN_FRACTION * (paneCount - 1)) * 100`（随 pane 数动态），`aria-valuenow` SHALL 为 `Math.round(leftPane.widthFraction * 100)`。

PaneResizeHandle SHALL 支持键盘 resize：ArrowLeft 减少 leftPane 的 widthFraction（步长 0.05），ArrowRight 增加（步长 0.05），Shift 修饰键 SHALL 加速步长至 0.15。Home SHALL 设 leftPane fraction 为 `MIN_FRACTION`，End SHALL 设为 `combined - MIN_FRACTION`（combined = leftPane + rightPane 的 widthFraction 之和）。键盘操作 SHALL 经过与拖拽相同的 `resizePanes` clamp 逻辑。

#### Scenario: 拖动相邻 handle
- **WHEN** 用户在 pane i 与 pane i+1 之间的 handle 上拖拽
- **THEN** pane i 的 widthFraction SHALL 跟随鼠标位置更新，pane i+1 SHALL 补差额以保持二者之和不变

#### Scenario: clamp 到最小宽度
- **WHEN** 拖拽使 pane i 的 fraction 将小于 0.1
- **THEN** 系统 SHALL 把 pane i 的 fraction clamp 到 0.1 并停止进一步缩小

#### Scenario: 不影响非相邻 pane
- **WHEN** 在 pane 1 和 pane 2 之间 resize
- **THEN** pane 3、pane 4 的 widthFraction SHALL 保持不变

#### Scenario: 常驻分隔线
- **WHEN** resize handle 处于 idle 态（无 hover / 无 focus / 无 drag）
- **THEN** handle SHALL 展示 1px 常驻分隔线（`--color-border-emphasis`），cursor 为 `col-resize`

#### Scenario: hover/active/focus-visible 视觉反馈
- **WHEN** 鼠标悬停、拖拽中、或键盘 focus-visible resize handle
- **THEN** handle SHALL 展示整条半透明 accent-blue 高亮，idle 态的 1px 分隔线 SHALL 消隐
- **AND** 高亮色 SHALL 为 `color-mix(in oklch, var(--color-border-emphasis) 60%, transparent)`，与 Sidebar resize handle 一致

#### Scenario: 键盘 resize
- **WHEN** handle 获焦且用户按 ArrowLeft
- **THEN** leftPane 的 widthFraction SHALL 减少 0.05（Shift 修饰时减少 0.15），rightPane 补差额
- **AND** ArrowRight SHALL 增加 0.05（Shift 修饰时增加 0.15）
- **AND** Home SHALL 设 leftPane fraction 为 MIN_FRACTION
- **AND** End SHALL 设 leftPane fraction 为 combined - MIN_FRACTION

#### Scenario: ARIA 语义
- **WHEN** PaneResizeHandle 渲染
- **THEN** 元素 SHALL 具有 `role="separator"` + `tabindex="0"` + `aria-orientation="vertical"` + `aria-label="拖动调整面板宽度"`
- **AND** `aria-valuenow` SHALL 反映 leftPane 的当前 widthFraction 百分比（四舍五入整数）

### Requirement: 打开 project-scoped Memory tab

用户从 Sidebar 点击 Memory 入口时，系统 SHALL 在当前 focused pane 内打开该项目的 Memory tab。Memory tab SHALL 绑定 `projectId`，同一项目重复打开时 SHALL 复用已有 Memory tab；不同项目的 Memory tab SHALL 可以同时存在。

#### Scenario: 首次打开 Memory tab
- **WHEN** 用户点击当前项目 Sidebar 中的 Memory 入口
- **THEN** 系统 SHALL 在 `focusedPaneId` 对应 pane 中创建 `type = "memory"` 的 tab，并设为该 pane 的 activeTabId

#### Scenario: 重复打开同一项目 Memory tab
- **WHEN** 用户再次点击同一项目的 Memory 入口
- **THEN** 系统 SHALL 激活已有 Memory tab，而不是创建重复 tab

#### Scenario: 不同项目 Memory tab 独立
- **WHEN** 用户先打开 project A 的 Memory tab，再切换到 project B 并打开 Memory tab
- **THEN** 系统 SHALL 创建另一个绑定 project B 的 Memory tab，不替换 project A 的 Memory tab

### Requirement: Jobs tab type with singleton semantics

系统 SHALL 支持 `"jobs"` 类型的 tab，具有单例语义（最多一个 Jobs tab 存在）。

#### Scenario: Open Jobs tab when none exists

- **WHEN** 用户点击 TitleBar jobs icon 或 ⌘K "Open Jobs"
- **AND** 当前无 Jobs tab
- **THEN** 创建并激活一个新的 Jobs tab

#### Scenario: Open Jobs tab when one already exists

- **WHEN** 用户点击 TitleBar jobs icon
- **AND** 已有一个 Jobs tab
- **THEN** 激活已有的 Jobs tab（不创建新的）

### Requirement: 数据根切换后的 tab 上下文重置

系统 SHALL 在数据根目录切换成功后关闭当前 root-scoped tabs，并让主工作区回到 Dashboard / 工作台空 pane 状态。root-scoped tabs 包括 session tab 与 memory tab；它们绑定旧数据根下的 project / session 身份，切换成功后继续展示会误导用户。配置保存失败时，系统 SHALL 保留当前 tabs 与已加载内容，不执行上下文重置。

#### Scenario: 切换数据根成功后回到工作台
- **WHEN** 用户在 Settings 中切换数据根目录
- **AND** 配置保存成功
- **THEN** 系统 SHALL 关闭已打开的 session tabs 与 memory tabs
- **AND** 主工作区 SHALL 回到 Dashboard / 工作台状态
- **AND** Sidebar 中 SHALL 不再高亮旧 session tab

#### Scenario: 多 pane 中的 root-scoped tabs 全部关闭
- **WHEN** 用户打开了多个 pane，且其中多个 pane 含 session 或 memory tab
- **AND** 数据根目录切换成功
- **THEN** 所有 pane 中的 session 与 memory tabs SHALL 被关闭
- **AND** 工作区 SHALL 收敛到可显示 Dashboard 的状态

#### Scenario: 切换失败保留旧上下文
- **WHEN** 用户尝试切换数据根目录
- **AND** 配置保存失败
- **THEN** 系统 SHALL 保留当前已打开 tabs
- **AND** 已加载的 session / memory 内容 SHALL 继续可见
- **AND** 主工作区 SHALL NOT 自动回到 Dashboard

#### Scenario: 关闭 root-scoped tabs 时释放旧内容
- **WHEN** 数据根目录切换成功导致 session 或 memory tab 被关闭
- **THEN** 系统 SHALL 释放这些 tab 关联的已加载内容与 UI 状态
- **AND** 用户后续重新打开任意 session SHALL 从当前数据根加载内容

