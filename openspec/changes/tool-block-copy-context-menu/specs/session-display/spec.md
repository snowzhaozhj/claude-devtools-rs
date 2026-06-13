## MODIFIED Requirements

### Requirement: 消息 chunk 右键菜单

`SessionDetail.svelte` 渲染的用户消息 chunk 与 AI 消息 chunk SHALL 通过 `use:contextMenu` action 挂载右键菜单，让用户在不离开会话视图的前提下完成"复制纯文本 / 复制为 Markdown / 复制选中文本（有选区时）"等核心操作。菜单 items 由 frontend-context-menu capability 的 `buildUserMessageItems` / `buildAssistantMessageItems` factory 构造，遵循该 capability 定义的视觉契约（无 icon、separator 分组、shortcut hint 右对齐）；action 内部 SHALL 调用 `e.stopPropagation()`，让子元素（工具块、code block、subagent ExecutionTrace）的右键菜单事件不被 bubbling 到消息容器。

AI 消息组内承载单段 markdown 源文本的工具展开块——slash/SKILL 指令（复制源 `item.slash.instructions`）、Output 块、Thinking 块、User message 块（复制源均为各自的 `item.text`）——SHALL 各自通过 `use:contextMenu` action 挂载右键菜单，items 由 `buildMarkdownBlockItems(text, ctx)` factory 构造，输出「复制纯文本 / 复制为 Markdown」（有选区时融合「复制选中文本」）。这些块的 action `stopPropagation` SHALL 阻止事件冒泡到父 AI 消息 chunk，使右键这些块时仅弹出**该块**的复制菜单，复制内容为该块自身文本而非整条 AI 消息。块文本为空时 SHALL 不挂菜单或不弹出空菜单。

#### Scenario: 右键用户消息 chunk

- **WHEN** 用户在 `SessionDetail` 内任意 `.user-bubble` 上右键
- **THEN** SHALL 弹出包含"复制纯文本"、"复制为 Markdown"等 items 的浮层菜单
- **AND** 触发位置遵循 `frontend-context-menu` viewport 边界 clamp 规则
- **AND** 点击"复制为 Markdown" SHALL 把 `userChunkToMarkdown(chunk)` 结果写入 clipboard 并显示"已复制!"反馈 600ms 后关闭菜单

#### Scenario: 右键 AI 消息 chunk

- **WHEN** 用户在 `SessionDetail` 内任意 `.ai-msg-container` 上右键
- **THEN** SHALL 弹出包含"复制纯文本"、"复制为 Markdown"等 items 的浮层菜单
- **AND** "复制为 Markdown" 调用 `aiChunkToMarkdown(chunk)`，从 `chunk.semanticSteps` 中 `kind==="text"` 步骤的 `text` 字段拼接（用 `\n\n` 分隔）

#### Scenario: 有选区时融合"复制选中文本"

- **WHEN** 用户先 drag-select 一段文本，再在同一 chunk 容器内右键
- **THEN** 调用方 SHALL 在 oncontextmenu 触发瞬间读取选区文本并通过 `ctx.selectionText` 传入 factory（factory 本身不读 DOM，与 `frontend-context-menu::menu-items 函数库` 契约一致）
- **AND** factory 据 `ctx.selectionText` 非空在首段首项前动态插入"复制选中文本"item（`shortcut: "⌘C"`）
- **AND** 用户无需先清除选区再右键

#### Scenario: 子元素右键不触发消息层菜单

- **WHEN** 用户在 AI 消息 chunk 内的工具块（`BashToolViewer` 等已挂 `use:contextMenu` 的子组件）上右键
- **THEN** 子组件 action 的 `stopPropagation` SHALL 阻止事件冒泡
- **AND** AI 消息层菜单 SHALL **不**触发，仅子组件菜单弹出

#### Scenario: 右键 Output 工具展开块弹该块菜单

- **WHEN** 用户在 AI 消息组内某 Output 块（`item.type==="output"` 的展开 prose）上右键
- **THEN** SHALL 弹出由 `buildMarkdownBlockItems(item.text, ctx)` 构造的菜单（含"复制纯文本"、"复制为 Markdown"）
- **AND** 该块 action 的 `stopPropagation` SHALL 阻止冒泡，AI 消息层菜单 SHALL **不**触发
- **AND** 点击"复制为 Markdown" SHALL 把该 Output 块的 `item.text` 写入 clipboard（**不**是整条 AI 消息文本）

#### Scenario: 右键 Thinking / User message 工具展开块弹该块菜单

- **WHEN** 用户在 AI 消息组内某 Thinking 块（`item.type==="thinking"`）或 User message 块（`item.type==="user_message"`）上右键
- **THEN** SHALL 弹出由 `buildMarkdownBlockItems(item.text, ctx)` 构造的该块复制菜单
- **AND** 该块 action 的 `stopPropagation` SHALL 阻止冒泡到 AI 消息层菜单
- **AND** 复制内容为该块自身 `item.text`

#### Scenario: 右键 slash/SKILL 指令块弹该块菜单

- **WHEN** 用户在 AI 消息组内某 slash 指令块（`item.type==="slash"` 且 `item.slash.instructions` 非空的展开 prose）上右键
- **THEN** SHALL 弹出由 `buildMarkdownBlockItems(item.slash.instructions, ctx)` 构造的该块复制菜单
- **AND** 复制内容为该 slash 块的 `instructions` 文本
- **AND** 当 `item.slash.instructions` 为空（块不可展开）时 SHALL 不挂菜单
