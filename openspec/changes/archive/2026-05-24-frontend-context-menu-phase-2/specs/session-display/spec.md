## ADDED Requirements

### Requirement: 消息 chunk 右键菜单

`SessionDetail.svelte` 渲染的用户消息 chunk 与 AI 消息 chunk SHALL 通过 `use:contextMenu` action 挂载右键菜单，让用户在不离开会话视图的前提下完成"复制纯文本 / 复制为 Markdown / 复制选中文本（有选区时）/ 复制 deeplink"等核心操作。菜单 items 由 `ui/src/lib/contextMenu/menu-items.ts` 内的 `buildUserMessageItems` / `buildAssistantMessageItems` factory 构造，遵循 `frontend-context-menu` capability 定义的视觉契约（无 icon、separator 三段分组、shortcut hint 右对齐）；action 内部 SHALL 调用 `e.stopPropagation()`，让子元素（工具块、code block、subagent ExecutionTrace）的右键菜单事件不被 bubbling 到消息容器。

#### Scenario: 右键用户消息 chunk

- **WHEN** 用户在 `SessionDetail` 内任意 `.user-bubble` 上右键
- **THEN** SHALL 弹出包含"复制纯文本"、"复制为 Markdown"、"复制 deeplink"等 items 的浮层菜单
- **AND** 触发位置遵循 `frontend-context-menu` viewport 边界 clamp 规则
- **AND** 点击"复制为 Markdown" SHALL 把 `userChunkToMarkdown(chunk)` 结果写入 clipboard 并显示"已复制!"反馈 600ms 后关闭菜单

#### Scenario: 右键 AI 消息 chunk

- **WHEN** 用户在 `SessionDetail` 内任意 `.ai-msg-container` 上右键
- **THEN** SHALL 弹出包含"复制纯文本"、"复制为 Markdown"、"复制完整对话上下文"、"复制 deeplink"等 items 的浮层菜单
- **AND** "复制为 Markdown" 调用 `aiChunkToMarkdown(chunk)`，从 `chunk.semanticSteps` 中 `kind==="text"` 步骤的 `text` 字段拼接（用 `\n\n` 分隔）

#### Scenario: 有选区时融合"复制选中文本"

- **WHEN** 用户先 drag-select 一段文本，再在同一 chunk 容器内右键
- **THEN** factory SHALL 检测 `window.getSelection()?.toString().length > 0`
- **AND** 在首段首项前动态插入"复制选中文本"item（`shortcut: "⌘C"`）
- **AND** 用户无需先清除选区再右键

#### Scenario: 子元素右键不触发消息层菜单

- **WHEN** 用户在 AI 消息 chunk 内的工具块（`BashToolViewer` 等已挂 `use:contextMenu` 的子组件）上右键
- **THEN** 子组件 action 的 `stopPropagation` SHALL 阻止事件冒泡
- **AND** AI 消息层菜单 SHALL **不**触发，仅子组件菜单弹出

#### Scenario: 复制 deeplink 写入 hash route 形态

- **WHEN** 用户点击"复制 deeplink"
- **THEN** clipboard SHALL 写入 `${location.origin}${location.pathname}#/session/<sessionId>/chunk/<chunkId>`
- **AND** `chunkId` SHALL 来自渲染时的 `chunk.chunkId`（基于 message uuid 稳定）

### Requirement: 消息 chunk DOM 锚点 `data-chunk-id`

`SessionDetail.svelte` 的 chunk 渲染循环 SHALL 给每个 chunk 容器 div 加 `data-chunk-id={chunk.chunkId}` 属性，让 deeplink watcher 通过 `document.querySelector('[data-chunk-id="<id>"]')` 定位目标 chunk 并 `scrollIntoView` 滚动 + 高亮。该属性 SHALL 同时存在于用户消息 chunk 与 AI 消息 chunk 容器，覆盖所有可被 deeplink 引用的 chunk 类型。

#### Scenario: chunk 容器渲染时挂载 data 属性

- **WHEN** `SessionDetail` 渲染任意 chunk
- **THEN** chunk 顶层 DOM 元素 SHALL 含 `data-chunk-id` 属性，值为 `chunk.chunkId`
- **AND** 用户在浏览器 DevTools 内 `document.querySelectorAll('[data-chunk-id]')` SHALL 命中所有已渲染 chunk

#### Scenario: deeplink 跳转滚动到目标 chunk

- **WHEN** App 接收到形如 `#/session/<sid>/chunk/<cid>` 的 hash 变更
- **AND** 对应 session tab 已 mount 且 chunks 已加载
- **THEN** SHALL 调用 `document.querySelector('[data-chunk-id="<cid>"]')?.scrollIntoView({ behavior: 'smooth', block: 'center' })`
- **AND** 给目标 chunk 添加 `.chunk-highlight` class 触发 1.5s fade-out 高亮动画

#### Scenario: pendingScrollChunkId 绑定 tab lifecycle 消费一次

- **WHEN** deeplink 触发后 tab 已 focused + SessionDetail mount + chunks 加载完成
- **THEN** SHALL 检查 `getTabUIState(tabId).pendingScrollChunkId` 并消费一次（scroll + 高亮 + clear）
- **AND** 后续用户来回切到同 tab SHALL **不**重复 scroll（pendingScrollChunkId 已 clear）

#### Scenario: 目标 chunk 不存在时弹 toast

- **WHEN** chunks 加载完成但 `chunkId` 在 session 内不存在（chunkId 拼写错 / session JSONL 已被清空）
- **THEN** SHALL 弹 toast "deeplink target not found in this session"
- **AND** clear `pendingScrollChunkId`（避免后续重试）

#### Scenario: 用户始终未激活 tab 时保持 pending

- **WHEN** deeplink 打开 session tab 但用户从未切到该 tab（保持 pending 状态）
- **THEN** `pendingScrollChunkId` SHALL **不**被超时清除
- **AND** 用户后续切到该 tab + chunks 加载完成时仍 SHALL 触发 scroll（不丢失意图）

#### Scenario: tab 关闭随 tabUIState 一起清

- **WHEN** 用户关闭含 pendingScrollChunkId 的 tab
- **THEN** `pendingScrollChunkId` SHALL 随 tabUIState 自动清除（无残留 state）
