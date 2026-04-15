# session-display Specification

## Purpose

定义会话详情页面的渲染规则：Chunk 类型渲染、AI 组展开/折叠行为、语义步骤（SemanticStep）和工具执行的展示逻辑。本 spec 聚焦前端渲染行为，数据结构由 `chunk-building` 和 `tool-execution-linking` spec 定义。

## Requirements

### Requirement: 按 Chunk 类型渲染对话流

SessionDetail SHALL 按顺序渲染 chunks 数组中的每个 Chunk。不同 kind 的 Chunk SHALL 使用不同的视觉布局。

#### Scenario: UserChunk 渲染
- **WHEN** chunk.kind 为 "user"
- **THEN** SHALL 渲染为右对齐气泡，显示消息文本（Markdown 渲染）、时间戳和 "You" 标签

#### Scenario: AIChunk 渲染
- **WHEN** chunk.kind 为 "ai"
- **THEN** SHALL 渲染为左对齐区块，包含 AI header（头像、模型名、token 统计、时间戳）和 body（文本+思考块）

#### Scenario: SystemChunk 渲染
- **WHEN** chunk.kind 为 "system"
- **THEN** SHALL 渲染为等宽字体预格式化块，带 Terminal 图标和 "System" 标签

#### Scenario: CompactChunk 渲染
- **WHEN** chunk.kind 为 "compact"
- **THEN** SHALL 渲染为居中摘要行，带 "Compact" 标签

#### Scenario: 空内容消息不渲染
- **WHEN** UserChunk 的文本经清洗后为空
- **THEN** 该 chunk SHALL 不产出任何 DOM 元素

### Requirement: AI 组工具展开/折叠

每个 AIChunk 的工具执行区域 SHALL 默认收起。用户 SHALL 能通过点击 AI header 中的 summary toggle 展开/收起工具列表。

#### Scenario: 默认收起
- **WHEN** AIChunk 首次渲染
- **THEN** 工具执行区域（ai-tools-section）SHALL 不可见

#### Scenario: 点击 summary 展开
- **WHEN** 用户点击 AI header 中的工具 summary 文本
- **THEN** 工具执行区域 SHALL 展开，chevron 图标 SHALL 旋转指示展开状态

#### Scenario: 再次点击收起
- **WHEN** 用户再次点击已展开的工具 summary
- **THEN** 工具执行区域 SHALL 收起

#### Scenario: 无工具时不显示 toggle
- **WHEN** AIChunk 没有 toolExecutions 和 slashCommands
- **THEN** AI header SHALL 不显示 summary toggle 控件

### Requirement: AI header 统计信息

AI header SHALL 显示该 AIChunk 的汇总信息：模型名称、工具/思考/subagent 计数摘要、输入/输出 token、时间戳。

#### Scenario: 模型名称简化
- **WHEN** AIChunk 的最后一个 response 有 model 字段
- **THEN** SHALL 显示简化后的模型名（移除 "claude-" 前缀和日期后缀）

#### Scenario: Summary 文本格式
- **WHEN** AIChunk 包含工具执行、思考块和 subagent
- **THEN** summary SHALL 包含各类型的计数（如 "3 tools · 1 thinking · 1 subagent"）

### Requirement: SemanticStep 渲染

AIChunk 的 semanticSteps SHALL 按类型分区渲染：tool_execution 和 subagent_spawn 在工具区域（需展开），thinking 和 text 在 body 区域（始终可见）。

#### Scenario: Thinking 步骤渲染
- **WHEN** semanticStep.kind 为 "thinking"
- **THEN** SHALL 渲染为可折叠的 BaseItem，带 Brain 图标和 "Thinking" 标签，展开后显示 Markdown 渲染的思考内容

#### Scenario: Text 步骤渲染
- **WHEN** semanticStep.kind 为 "text"
- **THEN** SHALL 直接渲染 Markdown 内容（不可折叠）

#### Scenario: Tool execution 步骤渲染
- **WHEN** semanticStep.kind 为 "tool_execution" 且工具区域已展开
- **THEN** SHALL 渲染为可折叠的 BaseItem，带 Wrench 图标、工具名、输入摘要和状态指示

#### Scenario: Subagent spawn 步骤渲染
- **WHEN** semanticStep.kind 为 "subagent_spawn"
- **THEN** SHALL 渲染为 BaseItem，带 Bot 图标、成员名称（或 "Subagent"）和描述摘要

### Requirement: 工具专化查看器路由

展开的工具项 SHALL 根据 toolName 路由到专化查看器。未知工具 SHALL 使用默认查看器。

#### Scenario: Read 工具
- **WHEN** toolName 为 "Read" 且无错误
- **THEN** SHALL 使用 ReadToolViewer 渲染（显示文件路径、行号范围、代码内容）

#### Scenario: Edit 工具
- **WHEN** toolName 为 "Edit"
- **THEN** SHALL 使用 EditToolViewer 渲染（显示 old_string / new_string 对比）

#### Scenario: Write 工具
- **WHEN** toolName 为 "Write" 且无错误
- **THEN** SHALL 使用 WriteToolViewer 渲染（显示文件路径和内容）

#### Scenario: Bash 工具
- **WHEN** toolName 为 "Bash" 或 "bash"
- **THEN** SHALL 使用 BashToolViewer 渲染（显示命令和输出）

#### Scenario: 未知工具
- **WHEN** toolName 不匹配任何专化查看器
- **THEN** SHALL 使用 DefaultToolViewer 渲染（显示 JSON 输入和文本输出）

### Requirement: Slash 命令展示

AIChunk 的 slashCommands SHALL 在工具区域顶部渲染，先于 tool_execution 步骤。

#### Scenario: Slash 命令渲染
- **WHEN** AIChunk.slashCommands 非空且工具区域已展开
- **THEN** 每个 slash 命令 SHALL 渲染为不可展开的 BaseItem，带 Slash 图标、"/" + 命令名、参数或消息摘要

### Requirement: 会话标题提取

SessionDetail 顶部 SHALL 显示会话标题。标题 SHALL 取自 chunks 中第一条非命令的 user 消息文本。

#### Scenario: 正常标题
- **WHEN** chunks 中有非 "/" 开头的 user 消息
- **THEN** 标题 SHALL 为该消息文本（截断至 60 字符）

#### Scenario: 全部为命令消息
- **WHEN** 所有 user 消息都以 "/" 开头
- **THEN** 标题 SHALL fallback 到第一条 user 消息文本，或 sessionId 前 12 字符

### Requirement: Markdown 渲染与代码高亮

文本内容 SHALL 通过 Markdown 渲染器转为 HTML。代码块 SHALL 进行语法高亮。渲染结果 SHALL 经过 XSS 防护处理。

#### Scenario: 代码块语法高亮
- **WHEN** Markdown 内容包含围栏代码块且指定了语言
- **THEN** SHALL 使用 highlight.js 进行语法高亮，应用 Soft Charcoal 主题 token 颜色

#### Scenario: XSS 防护
- **WHEN** Markdown 渲染的 HTML 包含潜在 XSS 内容（script 标签等）
- **THEN** SHALL 通过 DOMPurify 清洗后再注入 DOM

### Requirement: Edit 工具 Diff 视图

Edit 工具的展开内容 SHALL 以统一 diff 格式显示 old_string 和 new_string 的行级差异。

#### Scenario: LCS diff 渲染
- **WHEN** 展开一个 Edit 工具项
- **THEN** SHALL 显示统一 diff 视图：context 行无背景色、added 行绿色背景 + "+" 前缀、removed 行红色背景 + "-" 前缀

#### Scenario: Diff 行号
- **WHEN** diff 视图渲染
- **THEN** 每行 SHALL 显示 old/new 双列行号（仅对应列有值）

#### Scenario: Diff Header
- **WHEN** diff 视图渲染
- **THEN** Header SHALL 显示文件名、语言标签、+N/-N 统计

#### Scenario: 纯新增（无 old_string）
- **WHEN** Edit 工具只有 new_string
- **THEN** SHALL 所有行以 added 样式显示

### Requirement: Mermaid 图表渲染

Markdown 中的 mermaid 代码块 SHALL 渲染为 SVG 图表。

#### Scenario: Mermaid 代码块渲染
- **WHEN** markdown 内容包含 ```mermaid 代码块
- **THEN** SHALL 动态加载 mermaid 库并渲染为 SVG 图表

#### Scenario: Code/Diagram 切换
- **WHEN** mermaid 图表已渲染
- **THEN** SHALL 提供 Code/Diagram 切换按钮，点击在源码和图表间切换

#### Scenario: 渲染失败降级
- **WHEN** mermaid 语法错误导致渲染失败
- **THEN** SHALL 显示错误提示并保留代码视图

#### Scenario: 主题适配
- **WHEN** 应用主题为 dark
- **THEN** mermaid 图表 SHALL 使用 dark 主题渲染
