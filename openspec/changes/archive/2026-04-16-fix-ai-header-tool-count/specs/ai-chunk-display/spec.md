## ADDED Requirements

### Requirement: 从 AIChunk 构建 DisplayItem 列表

前端 SHALL 提供 `buildDisplayItems(chunk)` 函数，从 `AIChunk` 的 `semanticSteps`、`toolExecutions`、`subagents`、`slashCommands` 构建统一的 `DisplayItem[]` 列表。列表按 `semanticSteps` 的出现顺序排列，slash 命令排在最前。

#### Scenario: 基本构建——tool + text + thinking 混合 chunk
- **WHEN** AIChunk 包含 2 个 thinking step、3 个 tool_execution step、1 个 text step
- **THEN** `buildDisplayItems` SHALL 返回 6 个 DisplayItem，类型分别为 thinking / thinking / tool / tool / tool / text，顺序与 semanticSteps 一致

#### Scenario: Tool execution 关联——从 semanticSteps 查找 toolExecutions
- **WHEN** semanticStep 的 `kind === "tool_execution"` 且 `toolUseId` 在 `chunk.toolExecutions` 中有匹配
- **THEN** 对应的 DisplayItem SHALL 包含完整的 `ToolExecution` 信息（toolName、input、output、isError、startTs、endTs）

#### Scenario: Tool execution 无匹配——orphan semantic step
- **WHEN** semanticStep 的 `toolUseId` 在 `chunk.toolExecutions` 中无匹配
- **THEN** 该 step SHALL 被跳过，不产出 DisplayItem

#### Scenario: Subagent spawn 关联
- **WHEN** semanticStep 的 `kind === "subagent_spawn"` 且 `placeholderId` 在 `chunk.subagents` 中有匹配
- **THEN** 对应的 DisplayItem SHALL 类型为 subagent，包含 Process 的 `rootTaskDescription`、`spawnTs`、`endTs`、`metrics`、`team`

#### Scenario: Slash 命令排在最前
- **WHEN** AIChunk 有 `slashCommands` 列表
- **THEN** slash 类型的 DisplayItem SHALL 排在所有其他 DisplayItem 之前

#### Scenario: Last output 检测与跳过
- **WHEN** semanticSteps 中最后一个 `kind === "text"` 的 step 被识别为 last output
- **THEN** 该 step SHALL 不包含在 `buildDisplayItems` 返回的列表中（由外部 ai-body 区域始终可见地渲染）

### Requirement: 从 DisplayItem 列表生成 Header Summary

前端 SHALL 提供 `buildSummary(items)` 函数，统计 `DisplayItem[]` 中各类型的数量，生成人类可读的 summary 字符串。

#### Scenario: 混合类型统计
- **WHEN** DisplayItem 列表含 1 thinking、3 tool、2 output（message）、1 subagent
- **THEN** `buildSummary` SHALL 返回 `"3 tool calls, 2 messages, 1 subagent, 1 thinking"`（顺序：tool → slash → message → subagent → thinking）

#### Scenario: 空列表
- **WHEN** DisplayItem 列表为空
- **THEN** `buildSummary` SHALL 返回空字符串

#### Scenario: 单类型
- **WHEN** DisplayItem 列表只含 5 个 tool 类型
- **THEN** `buildSummary` SHALL 返回 `"5 tool calls"`

### Requirement: AI chunk 展开列表由 DisplayItem 驱动渲染

SessionDetail 的 AI chunk 展开区域 SHALL 遍历 `buildDisplayItems` 返回的 `DisplayItem[]` 列表渲染子项，每种类型对应独立的渲染方式。

#### Scenario: Tool 类型渲染
- **WHEN** DisplayItem 类型为 tool
- **THEN** SHALL 渲染为 BaseItem（WRENCH 图标 + toolName + summary），可展开查看 Tool Viewer

#### Scenario: Thinking 类型渲染
- **WHEN** DisplayItem 类型为 thinking
- **THEN** SHALL 渲染为 BaseItem（BRAIN 图标 + "Thinking" 标签），可展开查看 markdown 内容

#### Scenario: Output（message）类型渲染
- **WHEN** DisplayItem 类型为 output
- **THEN** SHALL 渲染为 markdown 文本块，展示 AI 的文本回复内容

#### Scenario: Subagent 类型渲染
- **WHEN** DisplayItem 类型为 subagent
- **THEN** SHALL 渲染为 SubagentCard 组件（独立于 BaseItem 的卡片样式），显示任务描述和执行时长

#### Scenario: Slash 类型渲染
- **WHEN** DisplayItem 类型为 slash
- **THEN** SHALL 渲染为 BaseItem（SLASH 图标 + 命令名 + 参数）

### Requirement: SubagentCard 独立组件

前端 SHALL 提供 `SubagentCard.svelte` 组件，以独立于工具卡片的样式渲染 subagent 信息。

#### Scenario: 基本渲染
- **WHEN** SubagentCard 收到 Process 数据
- **THEN** SHALL 显示任务描述（`rootTaskDescription`）、执行时长（`endTs - spawnTs`）、team 信息（如果有）

#### Scenario: 点击导航到 subagent session
- **WHEN** 用户点击 SubagentCard
- **THEN** SHALL 在新 tab 中打开该 subagent 的 session（通过 `process.sessionId`）
