## MODIFIED Requirements

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

#### Scenario: Split 达到 MAX_PANES 上限

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
