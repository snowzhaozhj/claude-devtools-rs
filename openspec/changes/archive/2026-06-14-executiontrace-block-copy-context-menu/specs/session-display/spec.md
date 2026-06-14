## MODIFIED Requirements

### Requirement: Subagent 内联展开 ExecutionTrace

每个 `AIChunk` 中的 subagent（`SemanticStep.kind === "subagent_spawn"` 或 `DisplayItem.type === "subagent"`）SHALL 以内联卡片形式渲染；用户 SHALL 能在**当前 tab 内**展开查看其 Dashboard 与完整执行链，SHALL NOT 自动跳转到新 tab。

**首屏 IPC 返回的 `Process.messages` 默认为空（`messagesOmitted=true`）。SubagentCard 在用户首次展开时 MUST 调 `getSubagentTrace(rootSessionId, process.sessionId)` 拉取完整 trace 并缓存到本地 `$state`；之后 traceItems 渲染 SHALL 用本地缓存。** 若 `messagesOmitted=false`（回滚开关或老后端），SHALL 直接用 `process.messages` 不发额外 IPC。SubagentCard MUST 接收 `rootSessionId: string` prop（由 SessionDetail 传入；嵌套 SubagentCard 一路向下传递不变）。

ExecutionTrace 的 DisplayItem 流由 `buildDisplayItemsFromChunks(chunks)` 从 subagent 的 `Process.messages: Chunk[]` 构建。该函数 SHALL 对每个 `kind === "ai"` 的 AIChunk 平铺其 DisplayItem；对 `kind === "user"` 的 UserChunk SHALL 提取文本并产出一个 `user_message` DisplayItem（承载父会话给 subagent 的 prompt 及任何真实用户输入），但 SHALL 跳过 slash 命令 UserChunk（其 slash 信息已由后续 AIChunk 的 slash item 渲染，重复渲染须避免）与清洗后为空的 UserChunk；`kind === "system"` / `kind === "compact"` 的 chunk SHALL 跳过。

ExecutionTrace 内承载单段 markdown 源文本的展开块——Thinking（`item.type==="thinking"`）、Output（`item.type==="output"`）、User message（`item.type==="user_message"`），复制源均为各自的 `item.text`——SHALL 各自在其展开后的 `.prose` 容器上通过 `use:contextMenu` action 挂载右键菜单，items 由 frontend-context-menu capability 的 `buildMarkdownBlockItems(text, ctx)` factory 构造，输出「复制纯文本 / 复制为 Markdown」（有选区时融合「复制选中文本」）。该契约 SHALL 适用于 ExecutionTrace 的所有渲染路径——经 `SubagentCard` 渲染的 subagent 执行链与经 `WorkflowCard` 渲染的 workflow agent 执行链一致覆盖，含嵌套 subagent。`use:contextMenu` action 内置的 `stopPropagation` SHALL 阻止事件冒泡到外层 trace 或父消息 chunk 菜单，使右键这些块时仅弹出**该块**的复制菜单，复制内容为该块自身 `item.text` 而非整条消息。块文本为空时 `buildMarkdownBlockItems` 返回空数组，SHALL NOT 弹出空菜单。ExecutionTrace 内的 slash 块（`collapsible={false}` 无展开 body）与 tool 块（由各 ToolViewer 自带复制路径覆盖）不在本契约范围。

调用方 SHALL 在 ExecutionTrace 内构造 `MenuItemContext`：`sessionId` 用 trace 所属会话 id（`sessionId ?? rootSessionId`）、`projectId` 用 props `projectId`（嵌套场景可为空字符串）、`selectionText` 在 oncontextmenu 触发瞬间读取当前选区、`settings` 与 `dispatch` 复用全局 helper。`buildMarkdownBlockItems` 对纯 markdown 块复制仅消费 `ctx.selectionText` 与 `ctx.dispatch.copyToClipboard`，不消费 `sessionId` / `projectId` / `settings`，故 `projectId` 为空不影响复制正确性。

#### Scenario: Subagent 默认折叠
- **WHEN** 一条 AI 组首次渲染，其中包含一个 subagent
- **THEN** subagent 卡片 SHALL 以单行 Header 形式展示，Dashboard 与 ExecutionTrace 均不可见
- **AND** SHALL NOT 触发 `getSubagentTrace` IPC

#### Scenario: 点击 Header 展开 Dashboard
- **WHEN** 用户点击 subagent 卡片的 Header 区域，且 `process.messagesOmitted=true`
- **THEN** SHALL 调 `getSubagentTrace(rootSessionId, process.sessionId)` 拉取 trace，并把结果写入本地 `$state` messages 缓存
- **AND** SHALL 展开显示 Dashboard（meta 行 + Context Usage 列表）与 Execution Trace 折叠头；chevron SHALL 旋转 90°
- **AND** trace 拉取过程中 SHALL 显示加载占位（如骨架行 / spinner）

#### Scenario: 重复展开复用本地缓存

- **WHEN** 用户已展开过一次 SubagentCard 后折叠，再次展开
- **THEN** SHALL 直接用本地 `$state` 缓存的 messages，SHALL NOT 再次调 `getSubagentTrace`

#### Scenario: 老后端兼容
- **WHEN** `process.messagesOmitted` 缺失或为 `false`，且 `process.messages` 非空
- **THEN** SubagentCard 展开时 SHALL 直接用 `process.messages`，SHALL NOT 调 `getSubagentTrace`

#### Scenario: Execution Trace 内独立展开
- **WHEN** 用户点击已展开卡片中的 "Execution Trace" 折叠头
- **THEN** SHALL 显示该 subagent 完整的 DisplayItem 流（父会话给 subagent 的 prompt / user_message、thinking、tool、output、嵌套 subagent），与父卡片展开状态独立保存

#### Scenario: ExecutionTrace 显示父会话给 subagent 的 prompt
- **WHEN** subagent 的 `Process.messages` 首条为 UserChunk（父会话给它的 prompt，非 slash、清洗后非空）
- **THEN** ExecutionTrace SHALL 在轨迹中产出一个 `user_message` DisplayItem 显示该 prompt 文本
- **AND** 该 item SHALL 渲染为带 User 图标的可展开条目，body 以 prose 显示完整 prompt

#### Scenario: ExecutionTrace 不重复渲染 slash 输入
- **WHEN** subagent 的 `Process.messages` 含一个 slash 命令 UserChunk（`<command-name>/x</command-name>`），且其 slash 信息已挂到后续 AIChunk 的 `slash_commands`
- **THEN** `buildDisplayItemsFromChunks` SHALL NOT 为该 UserChunk 产出 `user_message` DisplayItem（避免与 slash item 重复）
- **AND** 该 slash SHALL 仅由 AIChunk 的 slash item 渲染一次

#### Scenario: 嵌套 subagent 递归渲染与各自 lazy load
- **WHEN** 一个已展开的 SubagentCard 的 trace 含嵌套 SubagentCard B（B 也带 `messagesOmitted=true`）
- **THEN** 内层 B SHALL 作为可独立展开的 SubagentCard 渲染，渲染深度 SHALL 不超过 8 层
- **AND** 用户展开 B 时 SHALL 用 B 的 sessionId 单独调 `getSubagentTrace(rootSessionId, B.sessionId)`，不复用外层 A 的结果

#### Scenario: 不产生"打开新 tab"副作用
- **WHEN** 用户点击 subagent 卡片的任意区域
- **THEN** 应用 SHALL NOT 创建新 tab，也 SHALL NOT 调用 `openTab(subagent.sessionId, ...)`

#### Scenario: 右键 subagent ExecutionTrace 内 Output 块弹该块菜单
- **WHEN** 用户展开一个 SubagentCard 的 Execution Trace，并在其中某 Output 块（`item.type==="output"` 的展开 prose）上右键
- **THEN** SHALL 弹出由 `buildMarkdownBlockItems(item.text, ctx)` 构造的菜单（含「复制纯文本」「复制为 Markdown」）
- **AND** action 内置 `stopPropagation` SHALL 阻止冒泡，父 AI 消息 chunk 菜单 SHALL **不**触发
- **AND** 点击「复制为 Markdown」SHALL 把该 Output 块的 `item.text` 写入 clipboard（**不**是整条消息文本）

#### Scenario: 右键 ExecutionTrace 内 Thinking / User message 块弹该块菜单
- **WHEN** 用户在已展开 trace 内某 Thinking 块（`item.type==="thinking"`）或 User message 块（`item.type==="user_message"`）上右键
- **THEN** SHALL 弹出由 `buildMarkdownBlockItems(item.text, ctx)` 构造的该块复制菜单
- **AND** 复制内容为该块自身 `item.text`，事件 SHALL NOT 冒泡到父消息菜单

#### Scenario: 右键 workflow agent ExecutionTrace 内块弹该块菜单
- **WHEN** 用户展开一个 WorkflowCard 的某 agent drilldown trace（经 ExecutionTrace 渲染），并在其中 Thinking / Output / User message 块上右键
- **THEN** SHALL 弹出由 `buildMarkdownBlockItems(item.text, ctx)` 构造的该块复制菜单，行为与 subagent 执行链一致
- **AND** 复制内容为该块自身 `item.text`
