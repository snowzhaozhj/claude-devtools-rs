## MODIFIED Requirements

### Requirement: Per-tab UI 状态隔离

每个 tab SHALL 维护独立的 UI 状态（`expandedChunks`、`expandedItems`、`searchVisible`、`contextPanelVisible`、滚动状态）。UI 状态 SHALL 按 tabId 索引，不随 pane 迁移而重置。不同 tab 的 UI 操作 SHALL 互不影响。

滚动状态 SHALL 以「视觉位置」语义而非绝对 scrollTop 数值持久化，记录三件套 `{ atBottom: boolean; anchorChunkId: string | null; anchorOffsetPx: number }`：

- `atBottom`：保存时点 conversation 容器是否粘底（`distanceFromBottom <= 16`）
- `anchorChunkId`：仅在 `atBottom=false` 时填，记录视口顶第一个可见 chunk 的 `chunkId`（即 conversation 容器中第一个 `getBoundingClientRect().bottom > containerRect.top + 1` 的 `[data-chunk-id]` 元素）
- `anchorOffsetPx`：anchor 元素 `getBoundingClientRect().top - container.getBoundingClientRect().top` 像素差，用于恢复时还原视口内子像素偏移

恢复策略 SHALL 按以下分支执行：

- 若 `atBottom=true` → 系统 SHALL 单次写入 `scrollTop = scrollHeight`，并 SHALL 启动「粘底 pin」状态机：用 `MutationObserver` 监听 conversation 容器子树（`subtree: true` + `attributes: true` + `attributeFilter: ['data-rendered']` + `childList: true` + `characterData: true`，覆盖 lazy markdown hydrate 时的 `dataset.rendered = "1"` 与 `innerHTML` 写入），每次 mutation 触发时若仍处 pin 期则重写 `scrollTop = scrollHeight`；pin 在以下任一条件触发时 SHALL 终止——用户主动滚动（`distanceFromBottom > 16`）/ 200 ms 内无 mutation / 5 s 上限超时；终止时 SHALL `disconnect` MutationObserver 并解绑 scroll listener 与 timer
- 若 `atBottom=false` 且 `anchorChunkId` 命中 conversation 内某 `[data-chunk-id]` 元素 → 系统 SHALL 调用 `el.scrollIntoView({ block: 'start' })`，再执行 `scrollTop -= anchorOffsetPx` 还原视口内偏移
- 若 `anchorChunkId` 未命中（chunk 因 refresh 后顺序变化或被合并丢失） → 系统 SHALL 降级到首屏顶部（不写 scrollTop）并 SHALL 在 console 记 warn，**不**回退到旧 scrollTop 数值方案

#### Scenario: 两个 tab 打开同一 session（不同 pane）
- **WHEN** 同一 session 在两个 pane 各开一个 tab（通过 "Open in New Pane" 或 split 操作）
- **THEN** 两个 tab 的展开状态、滚动状态 SHALL 各自独立

#### Scenario: 滚动位置恢复 - 切走时粘底
- **WHEN** 用户在 tab A 滚动到底部（`distanceFromBottom <= 16`），切换到 tab B 后再切回 tab A
- **THEN** tab A 的 conversation SHALL 仍处于底部（`distanceFromBottom <= 16`）
- **AND** 此后 lazy markdown 持续 hydrate 触发 conversation 子树 mutation 期间，conversation SHALL 持续保持在底部直到「粘底 pin」状态机终止

#### Scenario: 滚动位置恢复 - 切走时位于中间
- **WHEN** 用户在 tab A 滚动到非底部位置（如某个 chunk 在视口顶），切换到 tab B 后再切回 tab A
- **THEN** tab A 的视口顶 SHALL 仍是切走时记录的同一个 chunk
- **AND** 该 chunk 相对视口顶的像素偏移 SHALL 与切走时差异不超过 50 px

#### Scenario: 滚动位置恢复 - 切回时 lazy chunks 尚未 hydrate
- **WHEN** 用户切回 tab A，此时 conversation 容器内多数 lazy markdown 节点处于占位高度状态（`data-rendered != "1"`，scrollHeight 远小于切走时点）
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
