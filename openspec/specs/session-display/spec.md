# session-display Specification

## Purpose

定义会话详情页面（SessionDetail）的渲染规则：Chunk 类型渲染、AI 组展开 / 折叠行为、语义步骤（SemanticStep）与工具执行的展示逻辑、Subagent 卡片彩色标识体系、teammate 消息按时序穿插、Markdown / Mermaid / 代码高亮、Context Panel 双视图、CLAUDE.md 目录树、自动刷新与 Ongoing banner、`OMIT_*` 路径下的懒加载（subagent trace / image asset / tool output / lazy markdown）。本 spec 聚焦前端渲染行为，数据结构由 `chunk-building`、`tool-execution-linking`、`team-coordination-metadata`、`ipc-data-api` spec 定义。
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

文本内容 SHALL 通过 Markdown 渲染器转为 HTML。代码块 SHALL 进行语法高亮。渲染结果 SHALL 经过 XSS 防护处理。**渲染时机由 lazy markdown 控制器决定（详见 `Lazy markdown rendering for first paint performance`）；XSS 防护与代码高亮规则在懒渲染触发时仍 MUST 严格执行。**

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

Markdown 中的 mermaid 代码块 SHALL 渲染为 SVG 图表。**`processMermaidBlocks` 的触发时机 SHALL 紧跟在该 markdown 区被 lazy 渲染之后（详见 `Lazy markdown rendering for first paint performance`），而非首屏 effect 全树扫描。**

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

### Requirement: Dashboard 项目概览

当无 active tab 时，主区域 SHALL 显示 Dashboard 项目概览页替代空状态。

#### Scenario: 无 tab 时显示 Dashboard
- **WHEN** 无 active tab
- **THEN** 主区域 SHALL 显示项目卡片网格，每张卡片包含项目名、路径缩写、会话数量

#### Scenario: 卡片点击选择项目
- **WHEN** 用户点击项目卡片
- **THEN** SHALL 在 Sidebar 中选中该项目并加载其会话列表

#### Scenario: Dashboard 本地搜索
- **WHEN** 用户在 Dashboard 搜索框中输入文本
- **THEN** 项目卡片 SHALL 按 displayName 或 path 过滤（大小写不敏感）

#### Scenario: 无项目
- **WHEN** 无可用项目
- **THEN** Dashboard SHALL 显示空状态提示

### Requirement: Context Panel 视图模式

Context Panel SHALL 支持 Category（按类别分组）和 Ranked（按 token 排序）两种主视图模式；Ranked 模式下 SHALL 提供 Grouped（按 category 颜色块分组）与 Flat（纯 token 排序平铺）子模式切换。Category 视图 SHALL 把 injections 拆为 6 个独立 Section（User Messages / CLAUDE.md Files / Mentioned Files / Tool Outputs / Task Coordination / Thinking + Text），每个 Section 用专属模板呈现关键字段而非通用 `label + preview`。所有 Section 默认 SHALL 处于展开状态；空 Section（无对应 injection）SHALL NOT 渲染。

#### Scenario: 默认 Category 视图 + 6 Section 全展开

- **WHEN** Context Panel 打开
- **THEN** SHALL 默认显示 Category 视图
- **AND** 6 个 Section 中所有非空 Section SHALL 默认展开
- **AND** 空 Section SHALL NOT 出现在 DOM 中

#### Scenario: 切换到 Ranked 视图

- **WHEN** 用户点击 "Ranked" 按钮
- **THEN** SHALL 切到 Ranked 视图并默认 Grouped 子模式
- **AND** Ranked 视图顶部 SHALL 出现 "Grouped" / "Flat" 子切换按钮

#### Scenario: Ranked Grouped 子模式

- **WHEN** Ranked 视图选中 Grouped 子模式
- **THEN** SHALL 按 category 分块，块内按 `estimatedTokens` 降序，每块顶部带 category 颜色 chip

#### Scenario: Ranked Flat 子模式

- **WHEN** Ranked 视图选中 Flat 子模式
- **THEN** SHALL 把所有 injection 平铺，按 `estimatedTokens` 降序排列，每行左侧带 category 颜色 chip

#### Scenario: 分类颜色系统

- **WHEN** Ranked 视图中渲染注入项
- **THEN** 各类别 SHALL 使用对应颜色标签：`claude-md` 紫蓝、`mentioned-file` 绿、`tool-output` 黄、`thinking-text` 紫、`task-coordination` 橙、`user-message` 蓝

#### Scenario: ToolOutputs Section 展示 tool breakdown

- **WHEN** Category 视图的 Tool Outputs Section 展开
- **THEN** 每条 `ToolOutputInjection` SHALL 展示其 `toolBreakdown` 中每个 tool 的名字、token 数、`isError` 标记
- **AND** 每个 tool 行 SHALL 是可点击的，触发 `onNavigateToTool(aiGroupId, toolUseId)`

#### Scenario: ThinkingText Section 拆分 thinking / text

- **WHEN** Category 视图的 Thinking + Text Section 展开
- **THEN** 每条 `ThinkingTextInjection` SHALL 拆开显示 `breakdown` 中 `thinking` 与 `text` 各自 token 数

#### Scenario: TaskCoordination Section 拆分各 kind

- **WHEN** Category 视图的 Task Coordination Section 展开
- **THEN** 每条 `TaskCoordinationInjection` SHALL 拆开显示 `breakdown` 中 `send-message` / `task-tool` / `teammate-message` 各 item 的 `label` + `tokenCount`

#### Scenario: UserMessages Section 显示 turn 序号

- **WHEN** Category 视图的 User Messages Section 渲染一条 `UserMessageInjection`
- **THEN** SHALL 显示 `Turn <turnIndex>` 标识 + `textPreview` + `estimatedTokens`

### Requirement: CLAUDE.md DirectoryTree

Category 视图中的 CLAUDE.md Files Section SHALL 按 `scope` 把文件分为 Global（含 `enterprise` + `user`）/ Project / Directory 三组，每组内 SHALL 以递归目录树形式展示文件路径；空组 SHALL NOT 渲染。Mentioned Files SHALL 拆到独立的 Mentioned Files Section 而非附在 CLAUDE.md Section 下。

#### Scenario: 三组分组渲染

- **WHEN** CLAUDE.md Files Section 渲染
- **THEN** SHALL 按 `scope` 分为 Global / Project / Directory 三组
- **AND** Global 组 SHALL 聚合 `scope == "enterprise"` 与 `scope == "user"` 的所有文件
- **AND** Project 组 SHALL 包含 `scope == "project"` 的文件
- **AND** Directory 组 SHALL 包含 `scope == "directory"` 的文件
- **AND** 任一组无文件时 SHALL NOT 渲染该组的 header 与 tree

#### Scenario: 目录树渲染

- **WHEN** 某一分组下有多个文件
- **THEN** SHALL 构建目录树，按路径层级递归渲染，目录可折叠/展开

#### Scenario: 文件节点信息

- **WHEN** 目录树中的文件节点渲染
- **THEN** SHALL 显示文件名和估计 token 数

#### Scenario: 目录排序

- **WHEN** 同级目录和文件排列
- **THEN** 文件 SHALL 排在目录之前，同类按名称字母排序

### Requirement: Subagent 内联展开 ExecutionTrace

每个 `AIChunk` 中的 subagent（`SemanticStep.kind === "subagent_spawn"` 或 `DisplayItem.type === "subagent"`）SHALL 以内联卡片形式渲染；用户 SHALL 能在**当前 tab 内**展开查看其 Dashboard 与完整执行链，SHALL NOT 自动跳转到新 tab。

**首屏 IPC 返回的 `Process.messages` 默认为空（`messagesOmitted=true`）。SubagentCard 在用户首次展开时 MUST 调 `getSubagentTrace(rootSessionId, process.sessionId)` 拉取完整 trace 并缓存到本地 `$state`；之后 traceItems 渲染 SHALL 用本地缓存。** 若 `messagesOmitted=false`（回滚开关或老后端），SHALL 直接用 `process.messages` 不发额外 IPC。SubagentCard MUST 接收 `rootSessionId: string` prop（由 SessionDetail 传入；嵌套 SubagentCard 一路向下传递不变）。

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
- **THEN** SHALL 显示该 subagent 完整的 DisplayItem 流（thinking / tool / output / 嵌套 subagent），与父卡片展开状态独立保存

#### Scenario: 嵌套 subagent 递归渲染与各自 lazy load
- **WHEN** 一个已展开的 SubagentCard 的 trace 含嵌套 SubagentCard B（B 也带 `messagesOmitted=true`）
- **THEN** 内层 B SHALL 作为可独立展开的 SubagentCard 渲染，渲染深度 SHALL 不超过 8 层
- **AND** 用户展开 B 时 SHALL 用 B 的 sessionId 单独调 `getSubagentTrace(rootSessionId, B.sessionId)`，不复用外层 A 的结果

#### Scenario: 不产生"打开新 tab"副作用
- **WHEN** 用户点击 subagent 卡片的任意区域
- **THEN** 应用 SHALL NOT 创建新 tab，也 SHALL NOT 调用 `openTab(subagent.sessionId, ...)`

### Requirement: Subagent 彩色标识体系

每个 subagent 卡片 SHALL 根据所属类别选用颜色：

1. **Team 成员**：`Process.team.member_color` → 通过 `getTeamColorSet` 映射到 border/badge/text 三色
2. **已知 subagentType 且有 agent config**：`AgentConfig.color` 对应调色板 → 通过 `getSubagentTypeColorSet` 返回
3. **已知 subagentType 无 config**：对 `subagent_type` 做 djb2 hash，映射到 14 色调色板任一槽位（确定性映射）
4. **未知类型（`subagent_type = None` 且非 team）**：使用中性 muted 色 + Bot 图标，不显示彩色圆点与 badge

#### Scenario: Team 成员使用 team 颜色
- **WHEN** subagent `process.team.member_color = "#8b5cf6"`
- **THEN** Header 圆点背景色 SHALL 为 `#8b5cf6`；badge SHALL 显示 `process.team.member_name` 文本且使用同色系 background/border

#### Scenario: agent config 匹配
- **WHEN** `subagent_type = "code-reviewer"` 且 agentConfigs 中存在同名条目 `color = "purple"`
- **THEN** Header 圆点与 badge SHALL 使用调色板 `purple` 槽位颜色；badge 文本 SHALL 为 `"code-reviewer"`（uppercase 样式由 CSS 控制）

#### Scenario: agent config 未命中走 hash
- **WHEN** `subagent_type = "unknown-type-xyz"` 且 agentConfigs 无对应条目
- **THEN** Header 圆点与 badge SHALL 使用 `djb2("unknown-type-xyz") % 14` 对应的调色板槽位颜色

#### Scenario: 完全无类型信息
- **WHEN** `subagent_type = None` 且 `team = None`
- **THEN** SHALL 使用中性 `--color-text-muted` Bot 图标，不渲染彩色圆点与 badge

### Requirement: Subagent MetricsPill 多维度展示

Subagent 卡片 Header SHALL 显示 MetricsPill，根据数据可用性展示以下维度：

- **Main Context**：`process.main_session_impact.total_tokens` 格式化；仅非 team 成员显示
- **Subagent Context**：**优先取 `process.lastIsolatedTokens`（后端预算字段）；若为 0 / 缺失则 fallback 用 `process.messages` 内最后一条 assistant `usage` 累加（兼容老后端）**。计算口径仍为 `input + output + cache_read + cache_creation`
- **Duration**：`process.duration_ms` 使用 `formatDuration` 格式化（秒/分钟）

若某维度数据缺失（`None` 或零值），对应槽位 SHALL 不渲染。

**Header 显示的 model 名 MUST 优先取 `process.headerModel`（后端预算字段，已跑过 `parse_model_string` 简化）；缺失时 fallback 用 `process.messages` 派生（兼容老后端）。** Shutdown-only 特例（team 成员 + 单一 SendMessage shutdown_response 调用）MUST 优先取 `process.isShutdownOnly` flag；缺失时 fallback 派生。

#### Scenario: 非 team subagent 显示两维（用预算字段）
- **WHEN** subagent `mainSessionImpact.totalTokens = 5000`、`lastIsolatedTokens = 12000`、`messagesOmitted = true`、`messages = []`
- **THEN** MetricsPill SHALL 显示 `Main: 5.0k` 与 `Ctx: 12.0k` 两个槽位（不依赖 `messages` 派生）

#### Scenario: Team 成员隐藏 Main Context
- **WHEN** subagent 是 team 成员（`team != None`）
- **THEN** MetricsPill SHALL NOT 显示 Main Context 槽位，仅显示 Context Window（最新 usage 合计）

#### Scenario: 数据全缺失
- **WHEN** 两个维度均为 `None` 或 0
- **THEN** MetricsPill SHALL 整体不渲染，但 Duration 显示逻辑不受影响

#### Scenario: 老后端 fallback
- **WHEN** `lastIsolatedTokens` 缺失或为 0，但 `process.messages` 含完整 chunks
- **THEN** SHALL fallback 到 `messages` 派生（与改造前行为一致）

#### Scenario: headerModel 预算字段优先
- **WHEN** `process.headerModel = "haiku4.5"` 且 `process.messages = []`
- **THEN** SubagentCard header SHALL 显示 `haiku4.5`（不需 messages 即可正常显示）

### Requirement: Auto refresh on file change

SessionDetail SHALL 在收到命中当前 `(projectId, sessionId)` 的 `file-change`
事件时自动重拉 `getSessionDetail` 并刷新渲染，无需用户手动操作。**同一会话**
短时间内的多次 file change SHALL 合并成一次刷新（in-flight dedupe）。

#### Scenario: 文件追加新消息时自动刷新
- **WHEN** 用户已经打开 session tab `(projectA, sessionX)`
- **AND** 后端 `FileWatcher` 检测到 `~/.claude/projects/projectA/sessionX.jsonl`
  被追加新行，emit `file-change` payload `{ projectId: "projectA",
  sessionId: "sessionX", deleted: false }`
- **THEN** SessionDetail SHALL 调用 `getSessionDetail("projectA", "sessionX")`
  并把返回的 chunks 替换到 `tabStore` 缓存与组件 `$state`
- **AND** 新消息 SHALL 在视觉上追加到对话流末尾

#### Scenario: 非当前会话的事件不触发刷新
- **WHEN** 用户打开 session tab `(projectA, sessionX)`
- **AND** 收到 `file-change` payload `{ projectId: "projectA",
  sessionId: "sessionY", deleted: false }`（同 project 但不同 session）
- **THEN** 当前 SessionDetail SHALL NOT 触发 `getSessionDetail` 重拉

#### Scenario: 同会话多次 file-change 合并刷新
- **WHEN** 同一 session 在 < 200 ms 内连续收到 3 次 `file-change` 事件
- **THEN** SessionDetail SHALL 只发起 1 次 `getSessionDetail` 网络/IPC 调用
  （后续事件复用 in-flight Promise 直至 resolve）

#### Scenario: 用户贴底时刷新后保持贴底
- **WHEN** 刷新触发的瞬间，对话容器满足
  `scrollTop + clientHeight >= scrollHeight - 16`（视为 pinned-to-bottom）
- **AND** `getSessionDetail` 返回新内容并完成渲染
- **THEN** SessionDetail SHALL 在下一帧 (`tick`) 把 `scrollTop` 设为
  `scrollHeight`，让用户继续看到最新消息

#### Scenario: 用户已向上滚动时刷新不抢焦点
- **WHEN** 刷新触发的瞬间，用户已经向上滚动（不满足 pinned-to-bottom 条件）
- **AND** `getSessionDetail` 返回新内容并完成渲染
- **THEN** SessionDetail SHALL NOT 修改 `scrollTop`，用户视图位置保持不变

#### Scenario: 刷新失败保留旧 detail
- **WHEN** 自动刷新过程中 `getSessionDetail` 抛错
- **THEN** SessionDetail SHALL 继续显示旧 `detail`，SHALL NOT 切到 error
  状态；错误 SHALL 通过 `console.warn` 记录但不阻断后续刷新

#### Scenario: tab 关闭后不再刷新
- **WHEN** 用户关闭一个 session tab
- **THEN** 该 tab 对应的 file-change handler SHALL 被注销，后续命中同
  `(projectId, sessionId)` 的事件 SHALL NOT 触发任何刷新

### Requirement: Ongoing banner at session bottom

SessionDetail SHALL 在 `detail.isOngoing === true` 时于对话流底部渲染
`<OngoingBanner />`——内容为蓝色背景的胶囊区块，含 spinner 图标与
文案 "Session is in progress..."。`isOngoing` 为 false / undefined
时 SHALL NOT 渲染该横幅。横幅的出现与消失 MUST 随自动刷新
（`Auto refresh on file change` Requirement）切换，无需用户手动操作。

#### Scenario: Banner shown when ongoing
- **WHEN** 当前打开的 session `detail.isOngoing === true`
- **THEN** SessionDetail SHALL 在对话容器尾部渲染 `<OngoingBanner />`，
  图标 SHALL 有 `animate-spin` 动画，文案 SHALL 为 "Session is in
  progress..."

#### Scenario: Banner hidden when ended
- **WHEN** `detail.isOngoing === false`
- **THEN** SessionDetail SHALL NOT 渲染 `<OngoingBanner />`

#### Scenario: Banner toggled by auto refresh
- **WHEN** session 收到一个 `file-change` 事件并刷新后，后端返回的
  `detail.isOngoing` 从 `true` 变为 `false`（用户按 Esc 插入
  interrupt marker）
- **THEN** 横幅 SHALL 在该次重渲染中消失，不需要用户切 tab 或其他操作

### Requirement: Interruption semantic step rendering

AIChunk 的 `semantic_steps` 中 `kind === "interruption"` 的项 SHALL
以独立的红色 badge 块渲染——文案 "Session interrupted by user"
（或取 step.text 作 tooltip）。该块 SHALL 位于 AIChunk 语义步骤的
自然位置，不参与工具区域展开/折叠切换。

#### Scenario: Interruption step rendering position
- **WHEN** 一个 `AIChunk.semantic_steps` 末尾含一个
  `{ kind: "interruption", text: "[Request interrupted by user for tool use]",
  timestamp: "..." }` 条目
- **THEN** 该 AIChunk 正文（非工具展开区）SHALL 渲染一行红色背景的
  "Session interrupted by user" 块，位置在本 chunk 最末一个 Thinking
  / Text 步骤之后

#### Scenario: Interruption block does not depend on tools-expanded state
- **WHEN** 用户未展开 AIChunk 的工具区域
- **THEN** 中断块 SHALL 仍然可见（与 Thinking / Text 步骤同层次）

#### Scenario: No interruption step means no block
- **WHEN** AIChunk.semantic_steps 不含 `kind === "interruption"` 的条目
- **THEN** SessionDetail SHALL NOT 渲染任何中断相关的块

### Requirement: 多 Pane 并排时 SessionDetail 实例独立

当多个 pane 中各自打开 tab（含不同 pane 打开同一 sessionId 的场景）时，每个 pane 的 SessionDetail 实例 SHALL 独立渲染并维护各自 per-tab UI 状态（expandedChunks、expandedItems、searchVisible、contextPanelVisible、scrollTop）与 session 数据缓存（按 tabId 索引）。一个 pane 内的操作 SHALL NOT 影响另一 pane 的渲染结果。

#### Scenario: 同一 session 在两个 pane 各开一个 tab
- **WHEN** 用户通过 Sidebar "Open in New Pane" 或 tab 拖拽创建了两个 tab 指向同一 sessionId，分别位于 pane 1 与 pane 2
- **THEN** 两个 SessionDetail 实例 SHALL 各自独立渲染，expanded 状态 SHALL 各自独立

#### Scenario: pane A 滚动不影响 pane B
- **WHEN** 用户在 pane A 的 SessionDetail 滚动 conversation 区域
- **THEN** pane B 的 SessionDetail scrollTop SHALL 保持不变

#### Scenario: pane A 展开某 chunk 不影响 pane B
- **WHEN** 用户在 pane A 展开某 chunk 的工具执行详情
- **THEN** pane B 中对应 chunk（若同 tab 打开）的展开状态 SHALL 保持其自身值

#### Scenario: 关闭一个 pane 的 tab 不影响另一 pane
- **WHEN** 用户关闭 pane A 的某 tab（sessionId 同时在 pane B 的 tab 中打开）
- **THEN** pane B 的对应 SessionDetail SHALL 继续渲染，其 UI 状态与缓存 SHALL 不受影响

#### Scenario: 非 focused pane 的 file-change 自动刷新仍生效
- **WHEN** pane A 是 focused pane，pane B 打开了 sessionId=X 的 tab
- **AND** 后端 `file-change` 事件命中 sessionId=X
- **THEN** pane B 的 SessionDetail SHALL 按 `Auto refresh on file change` requirement 所述刷新，不因非 focused 而被跳过

### Requirement: Subagent 卡片与 Task tool 就地交错渲染

SessionDetail 渲染 `AIChunk` 时 MUST 按 `semantic_steps` 顺序依次输出 DisplayItem，使 subagent 卡片与其对应 Task / Agent 调用**时序相邻**；同时 UI SHALL 跳过与 subagent 已关联的 `Task` 或 `Agent` `tool_execution`，避免同一个逻辑调用同时以"工具调用行"和"Subagent 卡片"两种形式重复显示。

**前端跳过判定的工具名集合 MUST 与后端 `cdt-parse::is_task`（`crates/cdt-parse/src/parser.rs`）保持一致——当前为 `{ "Task", "Agent" }`**。后端 `resolve_subagents` 把这两类工具识别为 task 调用并尝试关联 SubagentProcess，前端 `displayItemBuilder` 在判定"已关联 subagent 的工具跳过 ToolItem"时 SHALL 同时识别这两个工具名；增减工具名时 SHALL 同步前后端两处实现并补对应 Scenario。

#### Scenario: Task 调用后紧随 Subagent 卡片
- **WHEN** AIChunk 含 `Read` → `Task(t_task)` → `Grep` 三个 tool_execution，且 `Task(t_task)` 已解析出 subagent A
- **THEN** UI SHALL 依序渲染：Read tool item → Subagent 卡片（A）→ Grep tool item；SHALL NOT 在 Grep 之后再输出 Subagent

#### Scenario: Task 去重
- **WHEN** `chunk.subagents` 中某 subagent 的 `parentTaskId = t_task`，且 `chunk.toolExecutions` 也含 `toolUseId = t_task, toolName = "Task"`
- **THEN** `displayItemBuilder` SHALL NOT 为该 `tool_execution` 步骤输出 `DisplayItem.type === "tool"`；subagent 卡片 SHALL 是该 Task 的唯一可见代表

#### Scenario: Agent 去重
- **WHEN** `chunk.subagents` 中某 subagent 的 `parentTaskId = t_agent`，且 `chunk.toolExecutions` 也含 `toolUseId = t_agent, toolName = "Agent"`
- **THEN** `displayItemBuilder` SHALL NOT 为该 `tool_execution` 步骤输出 `DisplayItem.type === "tool"`；subagent 卡片 SHALL 是该 Agent 调用的唯一可见代表

#### Scenario: Orphan Task 保留显示
- **WHEN** 某 Task `tool_use` 未匹配任何 subagent（`Resolution::Orphan`）
- **THEN** 对应的 `tool_execution` SHALL 照常渲染为 Tool item（Default viewer），不受去重影响

#### Scenario: Orphan Agent 保留显示
- **WHEN** 某 Agent `tool_use` 未匹配任何 subagent（`Resolution::Orphan`）
- **THEN** 对应的 `tool_execution` SHALL 照常渲染为 Tool item（Default viewer），不受去重影响

### Requirement: Subagent 卡片 Header badge 固定 TASK 文案

非 team 成员的 subagent 卡片 Header badge MUST 显示固定文本 `TASK`（由 CSS uppercase 样式控制），`process.subagentType` 仅用于决定 badge 颜色（通过 `getSubagentTypeColorSet`），不再作为 badge 文字。Team 成员保持显示 `process.team.memberName`。展开视图的 Meta 行 Type 字段仍显示 `subagentType ?? (team ? "Team" : "Task")` 原值。

#### Scenario: 已知 subagent_type
- **WHEN** subagent `subagentType = "code-reviewer"`、无 team
- **THEN** Header badge 文字 MUST 为 `TASK`；badge 颜色 MUST 使用 agent config 或 djb2 hash 解析出的调色板颜色；展开 Meta 行 Type 字段 MUST 显示 `code-reviewer`

#### Scenario: 无类型信息
- **WHEN** `subagentType = None` 且 `team = None`
- **THEN** Header SHALL 渲染 Bot 图标 + 中性 badge "TASK"（已有中性样式路径）；展开 Meta 行 Type 字段 SHALL 显示 `Task`

#### Scenario: Team 成员保留成员名
- **WHEN** subagent `team.memberName = "reviewer"`
- **THEN** Header badge 文字 MUST 为 `reviewer`，不变为 `TASK`

### Requirement: Subagent 模型名对齐 parseModelString

Subagent 卡片 Header 与展开 Meta 行显示的模型名 MUST 通过 `parseModelString(rawModel)` 产出的 `name` 字段渲染：去 `claude-` 前缀、去 `-YYYYMMDD` 日期 suffix 后，把 `-` 分隔的 family/版本段以 `.` 连接（`haiku-4-5` → `haiku4.5`、`sonnet-4-6` → `sonnet4.6`、`opus-4-7` → `opus4.7`）。模型为 `<synthetic>` 或缺失时 SHALL 不渲染模型名。

#### Scenario: 版本号压缩
- **WHEN** rawModel = `claude-haiku-4-5-20251001`
- **THEN** Header 显示文本 MUST 为 `haiku4.5`

#### Scenario: synthetic 模型隐藏
- **WHEN** rawModel = `<synthetic>`
- **THEN** Header SHALL NOT 渲染模型名元素

### Requirement: Lazy markdown rendering for first paint performance

SessionDetail SHALL 把所有 markdown 内容（user prose / AI lastOutput / Thinking 展开体 / Output 展开体 / Slash instructions 展开体 / System pre 文本）的 `renderMarkdown` 调用延迟到节点进入视口（含 `200 px` rootMargin 余量）后再触发；视口外的对应区域 SHALL 仅渲染高度估算占位（背景色块），不调用 marked / highlight.js / DOMPurify。Mermaid block 的 `processMermaidBlocks` SHALL 在该 markdown 区真正渲染**之后**再被触发，不在占位阶段扫描。lazy markdown 控制器 MUST 对外暴露 `flushAll()` 同步方法，用于全文 DOM 操作场景（搜索 / 打印 / 导出）触发所有 pending 占位的强制渲染。

#### Scenario: 视口外 markdown 不渲染

- **WHEN** SessionDetail 首次挂载，detail 含 96 条 chunk，初始视口只覆盖前 5 条
- **THEN** 仅前 5 条 + 200 px rootMargin 内的 chunk 的 markdown 占位 SHALL 被替换为真实 HTML
- **AND** 其余 chunk 的 markdown 占位 SHALL 保持空背景色块，DOM 中 SHALL NOT 出现 `<pre><code class="hljs">` / `marked` 产出节点

#### Scenario: 滚动进入视口后渲染

- **WHEN** 用户向下滚动，未渲染的 markdown 占位首次进入视口（含 rootMargin）
- **THEN** 该占位 SHALL 在同一帧内调用 `renderMarkdown(text)`，把 HTML 注入容器
- **AND** SHALL 标记 `data-rendered="1"` 防重复，从 IntersectionObserver `unobserve` 该节点

#### Scenario: Mermaid 渲染时机

- **WHEN** 一个 markdown 占位首次渲染，且文本含 ```mermaid 代码块
- **THEN** SHALL 在占位渲染**之后**对**该占位元素**调用 `processMermaidBlocks(el)`，不扫整个 conversation 容器
- **AND** 已 `mermaid-done` 标记的 block SHALL 不重复渲染

#### Scenario: 视口外不进入 highlight.js / DOMPurify

- **WHEN** 一个 chunk 的 markdown 占位从未进入视口
- **THEN** 该文本 SHALL NOT 经过 `marked.parse`、`hljs.highlight`、`DOMPurify.sanitize` 任一处理

#### Scenario: 占位高度估算避免 layout 跳

- **WHEN** SessionDetail 首屏 mount 完，未渲染的占位高度按文本长度估算（如 `Math.max(60, text.length / 80 * 22)` px）
- **THEN** 进入视口后真实内容渲染产生的高度差 SHALL 不超过 `200 px` rootMargin（即不会让用户感知"跳一下"导致当前阅读位置丢失）

#### Scenario: file-change 自动刷新不打破 lazy 状态

- **WHEN** 当前 SessionDetail 已 lazy 渲染了部分 chunk，收到 `file-change` 触发 `refreshDetail` 重拉
- **THEN** 新 detail 替换后，已渲染过的 chunk（按 `chunkKey` 一致性）SHALL 保留渲染态；新增 chunk SHALL 默认占位、入视口后再渲染

#### Scenario: 紧急回滚开关

- **WHEN** `lazyMarkdown.svelte.ts` 顶部常量 `LAZY_MARKDOWN_ENABLED = false`
- **THEN** SessionDetail SHALL 退化为首屏直接渲染所有 markdown（旧行为），用于发现严重回归时一行切回

#### Scenario: flushAll 强制渲染所有 pending 占位

- **WHEN** 调用方对 lazy markdown 控制器调用 `flushAll()`
- **THEN** 所有处于 pending 状态（已 `observe` 但未进入视口）的占位元素 SHALL 按 `observe` 注册顺序同步调用 `renderMarkdown(text)` 注入 HTML
- **AND** 每个被 flush 的元素 SHALL 标记 `data-rendered="1"` 防重复
- **AND** 控制器内部的 pending map SHALL 被清空，IntersectionObserver SHALL `unobserve` 这些元素
- **AND** `flushAll` 是幂等的：再次调用时若无 pending 元素 SHALL 立即返回不做任何工作

#### Scenario: flushAll 在回滚开关关闭时为 no-op

- **WHEN** `LAZY_MARKDOWN_ENABLED = false` 时 SessionDetail 创建 lazy markdown 控制器并调用 `flushAll()`
- **THEN** 该方法 SHALL 立即返回不做任何工作（因为该分支下 `observe()` 已在注册时同步渲染，不存在 pending 元素）
- **AND** 接口签名 SHALL 与 enabled 分支一致，调用方无需分支判断

### Requirement: Skeleton placeholder while loading

SessionDetail 在 IPC `getSessionDetail` 进行中（`detail == null && loading == true`）SHALL 渲染骨架卡片占位（5 条不同高度的灰色矩形，对应 user / AI / system 视觉密度），而非纯文本 "加载中..."。骨架仅在初次加载（无缓存）显示；file-change 自动刷新走 `silent` 路径不显示骨架。

#### Scenario: 初次打开 session 显示骨架

- **WHEN** 用户首次点开一个 session tab，无 `tabStore` 缓存
- **THEN** SessionDetail SHALL 立即渲染 `<SessionDetailSkeleton />`（5 条灰色卡片），SHALL NOT 显示纯文本 "加载中..."

#### Scenario: 缓存命中不显示骨架

- **WHEN** 用户切回已打开过的 session tab（`getCachedSession(tabId)` 命中）
- **THEN** SessionDetail SHALL 直接渲染缓存的 detail，SHALL NOT 显示骨架

#### Scenario: file-change 刷新不显示骨架

- **WHEN** 已打开的 SessionDetail 收到 `file-change` 触发 `refreshDetail()`
- **THEN** 旧 detail 视图 SHALL 保留至新数据返回；过程中 SHALL NOT 切到骨架占位（保持反闪烁三原则）

#### Scenario: 骨架卡片无 shimmer 动画

- **WHEN** 骨架占位渲染
- **THEN** 卡片背景 SHALL 为静态 `var(--color-border)`，SHALL NOT 有 shimmer / pulse 动画（避免与 OngoingIndicator 视觉竞争 + 节省 GPU）

### Requirement: Inline image lazy load via asset protocol

User message 内的 `ContentBlock::Image` 块 MUST 通过视口懒加载渲染：首屏不携带 base64 字符串（后端 `get_session_detail` 默认裁剪，`source.dataOmitted=true`），ImageBlock 组件 SHALL 用 `IntersectionObserver`（与 lazy markdown 同节奏，`rootMargin: 200px`）监听自身 DOM 节点，进入视口才调用 `getImageAsset(rootSessionId, sessionId, blockId)` 拉取 Tauri `asset://` URL，赋值到 `<img src>` 由浏览器原生加载。

行为约束：
- 加载完成前 SHALL 显示占位（如固定高度 + media type 文案的 placeholder div），避免布局抖动。
- `dataOmitted=false`（回滚开关或老后端）时 SHALL 直接用 `data:<media_type>;base64,<source.data>` URI 路径，SHALL NOT 调 `getImageAsset`。
- 同一 ImageBlock 重复进出视口 SHALL 复用首次拉取的 URL（前端组件级缓存或 Svelte `$state` 留存）。
- `getImageAsset` 失败（IPC 异常 / 后端返回 fallback `data:` URI）时 SHALL 直接把返回值赋给 `<img src>`——浏览器渲染失败时显示 broken-image 图标即可，不需额外重试 UI。
- `blockId` 由前端从 chunk 内 ContentBlock 数组拼接：`<chunkUuid>:<blockIndex>`（chunkUuid 取所属 UserChunk / AIChunk response 的 uuid；blockIndex 是 image 在 `MessageContent::Blocks` 中的位置）。

#### Scenario: 首屏不加载视口外的 image

- **WHEN** SessionDetail 首屏渲染，含 5 个 ImageBlock，其中只有最上面 1 个在视口内
- **THEN** 仅视口内那 1 个 ImageBlock SHALL 调用 `getImageAsset`
- **AND** 其余 4 个 SHALL 显示占位 div，`<img>` 元素的 `src` SHALL 为空或未设置

#### Scenario: 滚动进入视口时按需加载

- **WHEN** 用户向下滚动使一个原本不在视口的 ImageBlock 进入视口
- **THEN** 该 ImageBlock SHALL 调用一次 `getImageAsset`，拿到 URL 后赋给 `<img src>`，浏览器加载并显示图片
- **AND** SHALL NOT 再次调用 `getImageAsset`（即使再次进出视口）

#### Scenario: 老后端 / 回滚开关 fallback 到 data URI

- **WHEN** ImageBlock 的 `source.dataOmitted` 为 `false` 或字段缺失，且 `source.data` 非空
- **THEN** ImageBlock SHALL 直接用 `data:<media_type>;base64,<source.data>` 作为 `<img src>`
- **AND** SHALL NOT 调用 `getImageAsset`

#### Scenario: 加载失败显示 broken-image 占位

- **WHEN** `getImageAsset` 返回的 URL `<img>` 加载失败（404 / asset 协议拒绝 / 数据损坏）
- **THEN** 浏览器原生 broken-image 图标 SHALL 显示，UI 不报错也不崩溃
- **AND** 用户 SHALL 能继续浏览 session 其他内容

### Requirement: Lazy load tool output on expand

ExecutionTrace 渲染的每个 tool item SHALL 在 `toggle(key)` 展开时检查对应 `exec.outputOmitted`：若为 `true` 且本地 `outputCache` 未命中，SHALL 调 IPC `getToolOutput(rootSessionId, sessionId, toolUseId)` 拉取 `ToolOutput` 并写入本地缓存；ToolViewer 渲染 SHALL 优先用本地 `outputCache.get(toolUseId)`，fallback 到 `exec.output`（兼容 `outputOmitted=false` 老后端 / 回滚路径）。本机制对主 SessionDetail 与 SubagentCard 内嵌套 ExecutionTrace 同等适用，sessionId 参数 SHALL 由所在 trace 的 sessionId 提供（嵌套 subagent trace 用 subagent 的 sessionId）。

#### Scenario: 折叠状态不触发 IPC

- **WHEN** SessionDetail 首屏渲染含 N 个 tool execution，且 `outputOmitted=true`
- **THEN** 前端 SHALL NOT 调 `getToolOutput`，仅渲染 BaseItem header（label / summary / status）
- **AND** Network 面板 SHALL 显示 0 次 `get_tool_output` 调用

#### Scenario: 展开时按需拉

- **WHEN** 用户点击某个 tool item 触发 `toggle(key)`
- **AND** 对应 `exec.outputOmitted=true` 且本地 `outputCache` 未命中
- **THEN** 前端 SHALL 调 `getToolOutput(rootSessionId, sessionId, exec.toolUseId)` 一次
- **AND** 拉取成功后 SHALL 把结果写入本地 `outputCache.set(toolUseId, output)`，触发 ToolViewer 用新 output 重渲染

#### Scenario: 重复展开复用本地缓存

- **WHEN** 用户先展开后折叠再展开同一 tool item
- **THEN** 第二次展开 SHALL NOT 触发 `getToolOutput` IPC（直接用 `outputCache.get(toolUseId)`）

#### Scenario: 老后端 / 回滚开关 fallback

- **WHEN** 后端响应中 `outputOmitted=false` 或字段缺失（老后端）
- **THEN** 前端 SHALL 直接渲染 `exec.output`，SHALL NOT 调 `getToolOutput`

#### Scenario: 嵌套 subagent 内 tool 用 subagent sessionId 拉

- **WHEN** SubagentCard 展开后渲染嵌套 ExecutionTrace，用户点击其中某 tool
- **THEN** 前端 SHALL 调 `getToolOutput(rootSessionId, subagent.sessionId, toolUseId)`，sessionId 用 subagent 的，不复用 root 的

#### Scenario: IPC 失败不阻塞 UI

- **WHEN** `getToolOutput` IPC 抛错或返回 `ToolOutput::Missing`
- **THEN** 前端 SHALL 渲染 fallback 显示（如"output 加载失败"或空状态），SHALL NOT 阻塞其它 tool item 的展开

### Requirement: Render task notification cards in user bubble

UserChunk 的 `content` 含一或多个 `<task-notification>...</task-notification>` XML 块时，UI MUST 通过 `parseTaskNotifications(content)` 抽取每个 block 的 `taskId` / `status` / `summary` / `outputFile` 四字段，在 user 气泡内**追加**渲染为独立卡片（move 原版 `UserChatGroup.tsx:484-536` 布局）。`cleanDisplayText` SHALL 继续把 `<task-notification>` 整段 XML 从正文清洗掉；user 气泡的渲染条件 MUST 改为 `text || images.length > 0 || taskNotifications.length > 0`——即使文本被清空、无图片，只要 task-notification 非空气泡仍 MUST 渲染。

#### Scenario: user message 只含 task-notification
- **WHEN** 一条 `user` 消息 content 是完整的 `<task-notification>...</task-notification>` XML，清洗后文本为空
- **THEN** 该 UserChunk MUST 渲染为独立 user 气泡，气泡内 MUST 含至少一张 task-notification 卡片，卡片 MUST 显示 summary 抽出的 cmdName、status 标签、exitCode（若 summary 含 `(exit code N)`）、outputFile basename

#### Scenario: user message 含 task-notification 混合正文
- **WHEN** 一条 `user` 消息 content 含多个 `<task-notification>` 块 + 普通文本
- **THEN** 气泡 MUST 先渲染清洗后的文本（markdown），再渲染每张 task-notification 卡片；卡片顺序 MUST 与 XML 出现顺序一致

#### Scenario: 失败 / 完成状态 UI 区分
- **WHEN** task-notification 的 `status` 为 `"failed"` 或 `"error"`
- **THEN** 卡片 status icon MUST 显示 ✕（红色 `error-highlight-text`）；`"completed"` 显示 ✓（绿色 `badge-success-text`）；其他状态（如 `"running"`）显示空心圆

### Requirement: AI header token summary uses last response usage snapshot

AIChunk 的 header 右侧 token 展示 MUST 取该 chunk 内**最后一条**带 `usage` 的 `AssistantResponse` 的 `usage` 四项之和作为"该 AI turn 结束时的 context window snapshot"，格式为压缩形式（如 `65.5k`）。**禁止**累加 chunk 内多条 responses 的 usage——Anthropic API 的 `cache_read_input_tokens` 每次返回"从 session 开头至当前 call 已缓存的历史"，多次 tool_use turn 中累加会把同一段历史重复计数 N 次，导致 UI 数字远大于真实值。

Header 前缀 MUST 显示 lucide `Info` SVG icon（hover 视觉提示）；hover 时 MUST 在 header 下方弹出 popover 卡片，列出 5 行 breakdown：Total / Input / Output / Cache create / Cache read（每项以 `toLocaleString()` 千分位显示）。`AIChunk.responses` 为空或全部 `usage=null` 时，header MUST 不渲染 token 槽（不显示 0）。

#### Scenario: 多 tool_use turn 取 last usage
- **WHEN** AIChunk 内含 3 条 responses：r1.usage={input=10, output=20, cacheRead=1000, cacheCreation=100} / r2.usage={input=5, output=8, cacheRead=1100, cacheCreation=50} / r3.usage={input=3, output=12, cacheRead=1200, cacheCreation=30}
- **THEN** header token MUST 显示 `fk(3+12+1200+30)` = `1.2k`（取 r3），**不是** `fk((10+20+1000+100)+(5+8+1100+50)+(3+12+1200+30))` = `3.5k`

#### Scenario: last usage 跳过 null
- **WHEN** AIChunk 末尾 response.usage 为 null，但前一条 response.usage 非 null
- **THEN** MUST 取"最后一条 usage 非 null"的 response 的 usage 计算

#### Scenario: hover 展示 breakdown
- **WHEN** 用户 hover Info icon 或 token 数字
- **THEN** 气泡下方 MUST 立即（<200ms，无原生 title 延迟）弹出自定义 popover 卡片，显示 Total / Input / Output / Cache create / Cache read 5 行；popover 不得依赖 `title=` HTML 原生 tooltip

### Requirement: Tool row displays approximate token count

ExecutionTrace 与 AI chunk 内联工具渲染中，每个工具 row 的 `BaseItem` MUST 通过 `getToolContextTokens(exec)` 估算 token 总和并以 `~{formatTokens(N)} tokens` 文案显示，与原版 `BaseItem.tsx:150` 格式一致。估算算法 MUST 为：

- input 部分：`estimateContentTokens(exec.input)`——对象/数组先 `JSON.stringify` 后按 ~4 字符/token 启发式计算
- output 部分：按 `ToolOutput.kind` 分支——`text` 取 `text` 字段走 `estimateTokens`；`structured` 取 `value` 走 `estimateContentTokens`；`missing` 贡献 0

`~N` 数字槽 SHALL 在 status 圆点之前渲染；工具 row 同时 MUST 在 status 圆点之后显示 `durationMs`（如 `25ms`）。当 `getToolContextTokens` 返回 0（空 input + missing output）时 row SHALL 不显示 token 槽。

#### Scenario: Bash 工具 row 显示 token 与 duration
- **WHEN** 一条 Bash tool row 的 `input={command: "ls -la"}` + `output.kind="text"` + `output.text="foo.rs\nbar.rs\n..."` 约 200 字符，duration 25ms
- **THEN** row MUST 显示 `~50 tokens` 槽（`ceil((len(JSON.stringify(input)) + 200) / 4)`）+ status 圆点 + `25ms`

#### Scenario: missing output 工具仍显示 input token
- **WHEN** 工具 `output.kind="missing"`（IPC 懒裁剪前的初始状态），`input={file_path: "/tmp/x.txt"}` JSON 约 40 字符
- **THEN** row MUST 显示 `~10 tokens`（仅 input 部分，output 贡献 0）

### Requirement: Render teammate messages embedded in AIChunk

SessionDetail 渲染 `AIChunk` 时 MUST 把 `chunk.teammateMessages` 作为 AIChunk 内部展示流的一类 DisplayItem 注入：每条 teammate message **按 `timestamp` 与其它 displayItems（thinking / text / tool / subagent / teammate_spawn）整体稳定排序穿插**——同 timestamp 保留 push 顺序。slash 命令仍排最前（与 AI turn 整体绑定，不参与时序排序）。

`replyToToolUseId` 字段 MUST 仅作为 teammate 卡片 header 的 reply chip 文本展示（"↪ reply"），**不**决定渲染位置——位置完全由 `tm.timestamp` 决定。这样即使没有 SendMessage 配对（teammate 主动发起回信、idle 通知等），卡片也按时序自然穿插，不会全部堆在 turn 末尾。

`displayItemBuilder` SHALL 把 teammate message 落点为 DisplayItem 类型 `{ type: "teammate_message", teammateMessage: TeammateMessage }`；`SessionDetail.svelte` 在 AIChunk 渲染流的 switch 内新增 `{:else if item.type === "teammate_message"}` 分支，渲染 `<TeammateMessageItem teammateMessage={item.teammateMessage} attachBody={...} rootSessionId={sessionId} />`。

`TeammateMessageItem.svelte` MUST 实现以下视觉契约：

1. **左侧 3px 彩色边**：颜色取自 `teammateMessage.color` 经 `getTeamColorSet(color)` 映射到 14 色调色板的 `border` 槽；缺失时退化到 `var(--color-border)`。
2. **Header 紧凑一行**：`color dot + teammate badge (teammateId, 同色系背景) + "Message" type label + summary 截断 (80 字符) + reply-to chip (CornerDownLeft icon + recipient/summary 简写) + token count (~Nk tokens 灰色) + chevron 折叠/展开`。
3. **默认折叠**：仅显示 header；用户点击 header 任意位置展开后渲染 markdown body（走 `attachMarkdown(body, "teammate")` 走 lazy markdown 管线）。
4. **噪声态极简**：`isNoise === true` 时 SHALL **不**渲染卡片框，仅渲染单行（`color dot + teammateId + body 单行截断`），`opacity: 0.45`，无展开/折叠。
5. **Resend 标记**：`isResend === true` 时 header 追加 RefreshCw icon + "Resent" 文案，整卡 `opacity: 0.6`。
6. **Token count 容错**：`tokenCount == null` 或 0 时 token 槽 SHALL 不渲染。
7. **Reply-to chip 容错**：`replyToToolUseId == null` 时 chip 槽 SHALL 不渲染。

`lazyMarkdown.svelte.ts` 的 `Kind` union MUST 加 `"teammate"` 分支（与 user / ai 同样走 `marked + highlight.js + DOMPurify` 管线）。

#### Scenario: Teammate messages interleave with other items by timestamp
- **WHEN** AIChunk 的 displayItems 时序为 `t=1 Read → t=2 Output(team已建) → t=3 SendMessage→alice → t=4 teammate(alice reply, replyTo=tu-send-alice) → t=5 Output(完毕)`
- **THEN** UI DisplayItem 顺序 SHALL 严格按 timestamp 升序排列：`Read → Output(team已建) → SendMessage→alice → TeammateMessageItem(alice) → Output(完毕)`——teammate 卡片**因 timestamp** 紧贴 SendMessage，不依赖 reply_to 配对

#### Scenario: Multiple teammate replies interleave by timestamp
- **WHEN** AIChunk 时序：`t=1 SendMessage→alice → t=2 SendMessage→bob → t=3 alice reply → t=4 bob reply → t=5 Output`
- **THEN** UI 顺序 SHALL 为 `SendMessage→alice → SendMessage→bob → TeammateMessageItem(alice) → TeammateMessageItem(bob) → Output`——按时序，**不**因 reply_to 把 alice reply 强行拉到 alice 的 SendMessage 之后

#### Scenario: Teammate without reply_to interleaves naturally
- **WHEN** AIChunk 含 `[t=1 Output, t=2 teammate(member-1, replyToToolUseId=null), t=3 Output]`（teammate 主动发起回信，无 SendMessage 配对）
- **THEN** UI 渲染 SHALL 为 `Output → TeammateMessageItem(member-1) → Output`——按 timestamp 穿插，**不**追加到 turn 末尾

#### Scenario: replyToToolUseId only affects chip text not position
- **WHEN** TeammateMessageItem 渲染时 `replyToToolUseId === "tu-x"`
- **THEN** 卡片 header SHALL 显示 reply chip（"↪ reply"），但卡片位置 SHALL 由 `timestamp` 决定，与 `tu-x` 在 displayItems 中的位置无关

#### Scenario: Noise teammate renders as minimal inline row
- **WHEN** teammate `isNoise === true`
- **THEN** SHALL 渲染单行（color dot + teammateId + body 截断），`opacity: 0.45`，SHALL NOT 渲染卡片框 / chevron / 展开区

#### Scenario: Resend teammate rendered with refresh badge and dimmed
- **WHEN** teammate `isResend === true` 且 `isNoise === false`
- **THEN** SHALL 渲染完整卡片，header 追加 RefreshCw icon + "Resent" 文案，整卡 `opacity: 0.6`

#### Scenario: Markdown body renders via lazy pipeline
- **WHEN** 用户首次展开一个 TeammateMessageItem（非 noise），body 含围栏代码块
- **THEN** 展开区 SHALL 通过 `attachMarkdown(body, "teammate")` 触发懒加载 markdown 渲染，含 highlight.js 语法高亮与 DOMPurify XSS 过滤；视口外的 teammate 卡片 SHALL 不消耗 markdown 渲染时间

### Requirement: SubagentCard 在 ongoing 期间主动重拉 trace

SubagentCard MUST 监听 `(process.isOngoing, process.endTs, process.messagesTotalCount)` 三元组组成的版本指纹；当版本变化**且**该卡片处于用户已展开状态（`isExpanded === true`，即用户已点击展开按钮）**且**`process.messagesOmitted === true` 时，SHALL 自动调用 `getSubagentTrace(rootSessionId, process.sessionId)` 重拉新 trace 并替换 `messagesLocal`。"已展开"判定 MUST 使用 `isExpanded` 而非 `messagesLocal !== null`——用 messagesLocal 判定会让首次展开期间（`ensureMessages` 的 `await` 进行中、`messagesLocal` 仍为 `null`）版本跳变后的新 fetch 不被触发，旧版本 fetch settle 后把 stale trace 写入 `messagesLocal`，UI 永久卡在旧快照（codex 二审 C1 发现）。

首次展开触发的 `ensureMessages` 与 effect 的版本主动重拉之间 SHALL 通过严格版本匹配协作：`ensureMessages` 在 IPC settle 时 MUST 检查 `currentVersion === fetchedVersion`，不匹配时 SHALL NOT 写入 `messagesLocal`（保持 `null`），让 effect 已发起的新版本 fetch 接管显示。早期实现里 `currentVersion === fetchedVersion || messagesLocal == null` 兜底语义 SHALL NOT 出现——`|| null` 兜底是 C1 的根本机制。

`getSubagentTrace` IPC 失败时 SHALL NOT 把 `messagesLocal` 写成空数组 `[]`——保留 `null` 让用户折叠重开时 `ensureMessages` 仍能命中 `messagesLocal == null` 通过 guard 重新尝试。早期实现把 `[]` 当作"显示空 trace"的兜底会让重试入口被永久封堵（codex 二审 C3 发现）。

未展开的 SubagentCard SHALL NOT 因版本变化主动发 IPC（仅清本地 stale 缓存或保持 `null`，等待用户下次展开时按既有 lazy 路径拉取），避免 ongoing 大会话内 N 个未展开卡片每次父 refresh 都触发 IPC 风暴。

同一 `process.sessionId` 同时收到多次版本变化 SHALL 通过 inflight 去重，但 inflight 复用 key MUST 为 `${sessionId}|${messagesVersion}` 联合 key，**不**仅按 sessionId 复用。理由：仅按 sessionId 复用时，旧版本（版本 N）的 Promise 在 pending 期间版本递增到 N+1，新触发的重拉若复用旧 Promise 会把版本 N 的旧 trace 写入 `messagesLocal`，且因 effect 认为"已在拉取中"而不再排第二轮——版本 N+1 的新 chunks 永远拿不到。等价替代实现：仅按 sessionId 复用但 Promise settle 后 SHALL 检查"当前版本 == fetch 时版本"，不等则视为 stale 并立即触发新一轮重拉。

#### Scenario: 已展开 ongoing subagent 在版本递增时主动重拉

- **WHEN** SubagentCard 已展开（`messagesLocal !== null`）且 `process.isOngoing === true`
- **AND** 父 session refresh 后 `process.messagesTotalCount` 从 5 变为 8
- **THEN** SubagentCard SHALL 自动调 `getSubagentTrace(rootSessionId, process.sessionId)` 重拉，并把返回的 `Vec<Chunk>` 替换到 `messagesLocal`，UI 渲染的 ExecutionTrace SHALL 立即反映新增的 chunks，**无需**用户折叠重开

#### Scenario: ongoing 翻转到 done 时同步最终状态

- **WHEN** SubagentCard 已展开，`process.isOngoing` 从 `true` 翻转到 `false`（subagent 收尾）
- **AND** `process.endTs` 从 `null` 变为具体时间戳
- **THEN** SubagentCard SHALL 触发最后一次 `getSubagentTrace` 重拉，让 UI 同步到 subagent 完成态的完整 trace

#### Scenario: 未展开卡片不主动重拉

- **WHEN** SubagentCard 未展开（`isExpanded === false`），`process.messagesTotalCount` 在多次父 refresh 中递增
- **THEN** SubagentCard SHALL NOT 发 `getSubagentTrace` IPC；用户首次展开时 SHALL 走既有 lazy 路径拉一次最新 trace

#### Scenario: 首次展开期间版本跳变由 effect 接管（C1 修复）

- **WHEN** 用户首次展开 SubagentCard：`isExpanded` 翻到 `true`，`ensureMessages` 启动 `getSubagentTrace`（版本 N，`messagesLocal` 仍为 `null`）
- **AND** pending 期间父 session refresh 让 `process.messagesTotalCount` 递增到 N+1
- **THEN** `$effect` SHALL 因 `isExpanded === true` 而触发新版本（N+1）的 `getSubagentTrace`，**不**因 `messagesLocal === null` 短路
- **AND** 旧版本（N）的 Promise settle 时 SHALL 严格判 `currentVersion === fetchedVersion`，不匹配则**不**写入 `messagesLocal`（保持 `null`），由新版本 fetch 接管显示

#### Scenario: IPC 失败后折叠重开能重试（C3 修复）

- **WHEN** SubagentCard 已展开，`ensureMessages` 调 `getSubagentTrace` 但 IPC 抛错
- **THEN** `messagesLocal` SHALL 保持 `null`（**不**写成 `[]`）；`isLoadingTrace` 复位为 `false`
- **AND** 用户折叠（`isExpanded=false`）再展开（`isExpanded=true`）时，`ensureMessages` SHALL 因 `messagesLocal == null` 通过 guard 重新调 `getSubagentTrace`

#### Scenario: 同 sessionId 同版本并发触发 inflight 复用

- **WHEN** SubagentCard 已展开，`messagesVersion = "1|_|5"` 触发 `getSubagentTrace`（尚未 settle）
- **AND** 同 sessionId 同版本 `"1|_|5"` 因 effect 重跑再次触发
- **THEN** 第二次 SHALL 复用第一次的 Promise（key `${sessionId}|1|_|5` 命中），SHALL NOT 并发发起第二次 IPC

#### Scenario: 同 sessionId 跨版本不复用旧 Promise

- **WHEN** SubagentCard 已展开，`messagesVersion = "1|_|5"` 触发 `getSubagentTrace`（Promise A 尚未 settle）
- **AND** pending 期间版本递增到 `"1|_|8"`，新一轮重拉触发
- **THEN** 第二次 SHALL 视为新版本 fetch（key `${sessionId}|1|_|8` 不命中旧 inflight），SHALL 发起 Promise B；Promise A settle 时**不应**把版本 5 的旧 trace 写入 `messagesLocal`（fetch 时版本与当前版本不等，结果 SHALL 被丢弃或被 Promise B 的结果覆盖）

#### Scenario: 老后端缺 messagesTotalCount 字段降级

- **WHEN** 旧后端响应不含 `messagesTotalCount`（JSON 反序列化为 `undefined`）
- **THEN** 版本指纹三元组中 `messagesTotalCount` 视为 `undefined`，版本永远是常量，主动重拉 effect SHALL NOT 触发；行为退化为既有 lazy 路径（用户折叠重开才能看到新内容），SHALL NOT 报错或卡死

### Requirement: 大文本工具详情交互优先渲染

Read、Write、Edit 工具详情在展开较大文本内容时 SHALL 避免一次性对所有行执行重型同步语法高亮或 HTML 清洗；小/中等 Read、Write 内容 SHALL 保留完整语法高亮，大文本才 SHALL 降级到轻量高亮。展开交互 MUST 先让 header、容器、滚动和点击目标保持可响应。任何通过 `{@html}` 注入的工具内容 MUST 来自受控内部渲染器输出或经过 XSS 防护清洗。`outputOmitted=true` 时，前端 SHALL 按工具实际路由的 viewer 决定是否先 IPC 拉取输出再展开：路由到 ReadToolViewer / BashToolViewer / DefaultToolViewer 的工具（含 `isError=true` 的 Read 与 Write —— 这两类走 DefaultToolViewer 渲染错误详情）依赖 `exec.output`，SHALL 先拉到再展开；路由到 EditToolViewer 的 Edit 工具（任意 `isError`）与路由到 WriteToolViewer 的 `isError=false` 的 Write 工具仅渲染 `exec.input` 字段，SHALL 立即展开，不为不会被消费的 output 引入额外延迟。

#### Scenario: Read 大文本展开不阻塞整页交互
- **WHEN** 用户展开一个 Read 工具项，且该工具输出包含数百行文本
- **THEN** 工具详情 SHALL 渲染路径、行号容器和文件内容
- **AND** SHALL 使用轻量高亮，SHALL NOT 对所有行同步执行 `highlight.js` + `DOMPurify` 后才允许交互

#### Scenario: Read 小/中等文本保留完整语法高亮
- **WHEN** 用户展开一个 Read 工具项，且该工具输出未达到大文本阈值
- **THEN** 代码行 SHALL 保留 `highlight.js` 语法高亮

#### Scenario: Write 大文本展开不阻塞整页交互
- **WHEN** 用户展开一个 Write 工具项，且输入内容包含数百行文本
- **THEN** 工具详情 SHALL 渲染文件路径和文件内容
- **AND** SHALL 使用轻量高亮，SHALL NOT 对所有行同步执行 `highlight.js` + `DOMPurify` 后才允许交互

#### Scenario: Write 小/中等文本保留完整语法高亮
- **WHEN** 用户展开一个 Write 工具项，且输入内容未达到大文本阈值
- **THEN** 代码行 SHALL 保留 `highlight.js` 语法高亮

#### Scenario: Edit diff 行不做重型语法高亮
- **WHEN** 用户展开一个 Edit 工具项，且 diff 包含多行 added、removed 或 context 内容
- **THEN** DiffViewer SHALL 保留统一 diff 结构、old/new 行号、增删背景与 `+`/`-` 前缀
- **AND** SHALL NOT 对每个 diff 行执行 `highlight.js` 语法高亮

#### Scenario: 工具详情 HTML 注入保持安全边界
- **WHEN** Read、Write 或 Edit 工具内容包含类似 HTML 或脚本片段的文本
- **THEN** 渲染结果 MUST 将其作为代码/文本展示，SHALL NOT 执行脚本或注入未清洗 HTML

#### Scenario: omitted output 工具输出 ready 后再展开
- **WHEN** 用户首次展开一个 `outputOmitted=true` 且尚未缓存输出的工具项，且该工具的查看器使用 `exec.output` 渲染（Read / Bash / DefaultToolViewer 路径）
- **THEN** SessionDetail SHALL 先调 `getToolOutput(rootSessionId, sessionId, exec.toolUseId)` 拉取完整输出
- **AND** SHALL 在输出可用后再把该工具项加入展开集合，避免空 OUTPUT 区域被实际内容跳变替换
- **AND** IPC 失败或返回 `kind = "missing"` 时 SHALL 保持工具项折叠，让用户重试

#### Scenario: 嵌套 ExecutionTrace 的 omitted output 工具输出 ready 后再展开
- **WHEN** 用户在 SubagentCard 的 ExecutionTrace 中首次展开一个 `outputOmitted=true` 且尚未缓存输出的工具项，且该工具的查看器使用 `exec.output` 渲染（Read / Bash / DefaultToolViewer 路径）
- **THEN** ExecutionTrace SHALL 先调 `getToolOutput(rootSessionId, traceSessionId, exec.toolUseId)` 拉取完整输出
- **AND** SHALL 在输出可用后再把该工具项加入展开集合
- **AND** `traceSessionId` SHALL 为该 trace 所属 session 的 sessionId（嵌套 subagent 时为 subagent 自己的 sessionId）

#### Scenario: 不依赖 output 的工具立即展开
- **WHEN** 用户展开一个 Edit 工具项（`isError` 任意），或 `isError=false` 的 Write 工具项
- **THEN** SessionDetail 与 ExecutionTrace SHALL 立即把该工具项加入展开集合，SHALL NOT 等待 `getToolOutput` IPC
- **AND** 渲染 SHALL 仅依赖 `exec.input` 字段（Edit 的 `old_string` / `new_string`、Write 的 `content`），与 `exec.output` 无关
- **AND** `isError=true` 的 Write 与 Read 工具项 SHALL NOT 走本场景，而是落到上一条 "omitted output 工具输出 ready 后再展开" Scenario —— 它们走 DefaultToolViewer 渲染错误详情，需要 `exec.output`

#### Scenario: 展开 AIChunk 不主动 prefetch Bash 与 Default
- **WHEN** 用户点击 AIChunk header 把工具区域展开
- **THEN** SessionDetail SHALL NOT 对该 chunk 内的 Bash 或 Default 路径工具触发 `getToolOutput` IPC（`prefetchReadOutputs` 仅命中 Read）
- **AND** Bash / Default 工具的 output 拉取 SHALL 仅在用户点击该具体工具项展开时按需触发，避免一次 chunk 展开引起并发 IPC 把"展开工具列表"交互拖慢

#### Scenario: 工具详情展开状态局部更新
- **WHEN** 用户展开或收起单个工具项
- **THEN** SessionDetail SHALL 保持其他 chunk 与工具项的展开状态不变
- **AND** SHALL 避免因该单项状态变化重新执行与该工具无关的 Markdown、Mermaid 或 diff 渲染工作

### Requirement: Tool detail timing and failure visibility

SessionDetail SHALL 在所有工具明细展示路径中显示可用的时间统计与失败原因。该规则适用于主会话工具列表和 subagent ExecutionTrace 内的工具项。

#### Scenario: Completed tool shows duration

- **WHEN** 一个工具执行同时具有 `startTs` 与 `endTs`
- **THEN** 工具明细 Header SHALL 显示由二者差值格式化得到的耗时

#### Scenario: Pending tool shows waiting state

- **WHEN** 一个工具执行具有 `startTs` 但缺少 `endTs`
- **THEN** 工具明细 Header SHALL 显示等待或进行中状态，而不是空白时间统计

#### Scenario: Failed tool shows readable reason

- **WHEN** 一个工具执行 `isError=true` 且 `output` 含文本或结构化错误内容
- **THEN** 展开工具明细 SHALL 显示失败原因
- **AND** 失败原因 SHALL 保留 raw 文本或格式化 JSON fallback，避免只显示失败状态

#### Scenario: Subagent trace tool uses same metadata display

- **WHEN** subagent ExecutionTrace 中渲染一个工具项
- **THEN** 该工具项 SHALL 使用与主会话工具项相同的耗时、等待状态与失败原因展示规则

### Requirement: Edit diff preview highlighting

Edit 工具展开内容 SHALL 保持统一 diff 视图，并根据 `file_path` 推断语言后对 diff 行内容进行语法高亮；无法推断或高亮失败时 MUST 降级为纯文本 diff，不影响 added/removed/context 样式和行号。

#### Scenario: Edit diff highlights by file extension

- **WHEN** Edit 工具 input 含 `file_path="src/lib.rs"`、`old_string` 与 `new_string`
- **THEN** DiffViewer SHALL 以 Rust 语言规则高亮 diff 行内容
- **AND** added/removed/context 背景、前缀与 old/new 双列行号 SHALL 保持可见

#### Scenario: Unknown extension falls back to plain diff

- **WHEN** Edit 工具 input 的文件扩展名无法映射到高亮语言
- **THEN** DiffViewer SHALL 渲染纯文本 diff
- **AND** SHALL NOT 抛错或显示空白预览

#### Scenario: Pure insert still previews content

- **WHEN** Edit 工具只有 `new_string` 或 `old_string` 为空
- **THEN** DiffViewer SHALL 显示所有新增行并应用可用的语言高亮

#### Scenario: Trailing newline does not create phantom diff row

- **WHEN** Edit 工具对比 `old_string="a\n"` 与 `new_string="b\n"`
- **THEN** DiffViewer SHALL 只显示一条 removed `a` 与一条 added `b`
- **AND** SHALL NOT 显示额外空白 context 行

### Requirement: Tool result expansion avoids eager heavy rendering

工具调用结果 SHALL 只在用户展开对应工具项后渲染重内容；重复展开同一工具项 SHALL 复用已计算的渲染结果。大型 markdown、代码高亮或 JSON 输出 SHALL 遵循 lazy 渲染策略，避免折叠状态和首次展开时造成明显主线程卡顿。

#### Scenario: Collapsed tool does not render heavy output

- **WHEN** 一个工具项处于折叠状态且 output 很大
- **THEN** SessionDetail SHALL NOT 为该 output 执行 markdown 渲染、语法高亮或大 JSON DOM 构建

#### Scenario: First expansion renders on demand

- **WHEN** 用户首次展开该工具项
- **THEN** 工具详情 SHALL 渲染可见内容
- **AND** 大型文本 SHALL 继续使用 lazy markdown 或等价的分帧/视口触发机制

#### Scenario: Re-expansion reuses cached render result

- **WHEN** 用户展开工具项、折叠后再次展开同一工具项
- **THEN** UI SHALL 复用已缓存的渲染结果或派生数据，避免重复执行昂贵转换

### Requirement: SessionDetail 滚动路径渲染隔离

SessionDetail SHALL 在对话流的 chunk / message 级稳定块容器上使用浏览器原生渲染隔离策略，降低视口外 DOM 对滚动过程 layout / paint 的影响。该策略 MUST NOT 改变 chunk 的 DOM 顺序、展开状态、搜索语义、lazy markdown 触发时机、Mermaid 渲染结果或 header popover 可见性；不支持相关 CSS 属性的平台 SHALL 退化为旧渲染行为。

#### Scenario: 离屏 chunk 使用内容可见性优化
- **WHEN** SessionDetail 渲染包含大量 `UserChunk` / `AIChunk` / `SystemChunk` / `CompactChunk` 的会话
- **THEN** User / System / Compact 的 chunk 外层容器 SHALL 带有用于 `content-visibility: auto` 的样式类或等价样式
- **AND** AIChunk SHALL NOT 在包含 header popover 的 AI row 外层启用 `content-visibility: auto`，而 SHALL 仅在 `ai-tools-section` / `ai-body` 等子区域启用渲染隔离
- **AND** 启用该策略的容器 SHALL 提供 `contain-intrinsic-size` 估算高度，减少离屏内容首次进入视口时的滚动跳变

#### Scenario: 渲染隔离不改变 lazy markdown
- **WHEN** 一个 markdown 占位尚未进入视口
- **THEN** 渲染隔离策略 SHALL NOT 强制调用 `renderMarkdown(text)`
- **AND** markdown 仍 SHALL 仅在 lazy markdown 控制器观察到进入视口或调用 `flushAll()` 时渲染

#### Scenario: 搜索强制渲染后仍能命中文本
- **WHEN** 用户在 SessionDetail 打开搜索并输入只存在于离屏 chunk 中的文本
- **THEN** 搜索流程 SHALL 继续通过 lazy markdown 控制器 `flushAll()` 渲染 pending 内容
- **AND** 渲染隔离策略 SHALL NOT 阻止浏览器在 DOM 中找到该文本

#### Scenario: Mermaid 首次可见后仍渲染
- **WHEN** 一个含 ```mermaid 代码块的 markdown 区域因滚动进入视口而触发 lazy markdown 渲染
- **THEN** `processMermaidBlocks(el)` SHALL 仍在该 markdown 区真实渲染后执行
- **AND** 渲染隔离策略 SHALL NOT 让 Mermaid 图表停留在纯代码视图
- **AND** 含 `.mermaid-block` 的 contained 区域 SHALL 退出 `content-visibility: auto`，避免图表异步替换高度时触发滚动锚点跳动

#### Scenario: Header popover 不被容器裁剪
- **WHEN** 用户 hover AI header 的 token summary
- **THEN** 自定义 token breakdown popover SHALL 能溢出 chunk 容器显示
- **AND** 渲染隔离策略 SHALL NOT 通过 `contain: paint` 或等价裁剪边界遮挡 popover

### Requirement: 无语言代码块高亮自动检测限制

Markdown 代码块高亮 SHALL 避免对未声明语言的大块内容执行同步 `highlightAuto` 语言检测。声明语言且 highlight.js 支持时 MUST 继续使用指定语言高亮；未声明语言或超过自动检测阈值的代码块 SHALL 按 plaintext 安全渲染，仍经过 Markdown 渲染与 DOMPurify 清洗链路。

#### Scenario: 声明语言代码块保持高亮
- **WHEN** Markdown 内容包含 ```rust 或其他 highlight.js 支持的声明语言代码块
- **THEN** renderer SHALL 使用对应语言调用 highlight.js 高亮
- **AND** 输出 SHALL 保留 `hljs` token class 以应用 Soft Charcoal 主题颜色

#### Scenario: 未声明语言代码块按 plaintext 渲染
- **WHEN** Markdown 内容包含未声明语言的 fenced code block
- **THEN** renderer SHALL NOT 对该代码块调用不受限的 `highlightAuto`
- **AND** 输出 SHALL 保留代码文本内容并按 plaintext 安全渲染

#### Scenario: 大块代码不自动检测语言
- **WHEN** Markdown 内容包含字符数超过自动检测阈值的未声明语言代码块
- **THEN** renderer SHALL NOT 调用 `highlightAuto`
- **AND** 首次进入视口时 SHALL 避免因语言猜测造成主线程长任务

### Requirement: SessionDetail uses chunkId as chunk identity

SessionDetail SHALL 使用后端返回的 `chunk.chunkId` 作为 chunk 级身份标识。顶层 chunk `{#each}` key、chunk 级展开状态、滚动保存相关锚点和 chunk 级 DOM 标记 MUST 优先使用 `chunkId`，MUST NOT 继续依赖不保证全局唯一的 assistant response uuid；数组 index 仅可作为 chunk 内局部 item 的渲染后缀，不得作为 chunk 级长期身份。`openOrReplaceTab` 复用 `tabId` 切换 `sessionId` 时，旧 SessionDetail 实例保存状态 MUST 继续校验当前 `tabId` 仍指向同一 `sessionId`，避免旧 session 的展开或滚动状态写回污染新 session。

#### Scenario: 重复 response uuid 不导致 keyed each 崩溃

- **WHEN** SessionDetail 渲染两个 `AIChunk`，且两个 chunk 的 `responses[0].uuid` 相同但 `chunkId` 不同
- **THEN** 顶层 chunk keyed each SHALL 使用两个不同的 `chunkId`
- **AND** Svelte SHALL NOT 因 duplicate key 抛错或中断渲染

#### Scenario: chunk 级展开状态绑定 chunkId

- **WHEN** 用户展开一个 chunk 级可折叠区域
- **AND** SessionDetail 因 file-change silent refresh 收到相同 session 文件内容对应的新 `chunks` 数组
- **THEN** 展开状态 SHALL 通过 `chunkId` 重新匹配到同一 chunk
- **AND** 展开状态 SHALL NOT 因数组对象重建或重复 response uuid 丢失

#### Scenario: openOrReplaceTab 不污染新 session 状态

- **WHEN** `openOrReplaceTab` 复用同一个 `tabId`，把 active tab 从 session A 替换为 session B
- **AND** session A 的旧 SessionDetail 实例随后 destroy 并尝试保存 `expandedChunks` / `scrollTop`
- **THEN** 保存逻辑 SHALL 检查该 `tabId` 当前仍指向 session A 后才写回
- **AND** session B 的 `expandedChunks` / `scrollTop` SHALL NOT 被 session A 的旧状态覆盖

### Requirement: Context Panel turn 锚点导航

Context Panel 内每条 injection SHALL 提供一个跳转动作，把 SessionDetail 主视图滚动到对应 `AIChunk` 容器（按 `aiGroupId == chunkId` 匹配 `data-chunk-id` DOM 属性）。点击 `ToolOutputs` Section 内某条 tool breakdown SHALL 先确保该 chunk 展开（`expandedChunks` 含 `chunkId`），再在目标 chunk 内查找该 tool 子节点（按 `data-tool-use-id == toolUseId` 匹配）并滚动到该子节点；若目标 chunk 内找不到 tool，则退化为滚到 chunk 本身。点击 `UserMessageInjection` SHALL 滚到该 turn 紧邻前的 `UserChunk`。

#### Scenario: 点击 injection 滚到 AIChunk

- **WHEN** 用户点击 Category 视图任一非 user-message Section 的 injection 行
- **THEN** SHALL 把对应 `chunkId` 加入 `expandedChunks`（已在则跳过）
- **AND** SHALL `await tick()` 一次后 `scrollIntoView({ block: "center", behavior: "smooth" })` 把 `[data-chunk-id="<aiGroupId>"]` 节点稳定滚动到 conversation 容器中部

#### Scenario: 点击 tool breakdown 行展开并滚到 tool

- **WHEN** 用户点击 Tool Outputs Section 内某 tool 行
- **THEN** SHALL 先把对应 AIChunk 的 `chunkId` 加入 `expandedChunks`（已在则跳过）
- **AND** SHALL `await tick()` 一次后滚动到该 chunk
- **AND** SHALL 再次 `await tick()` 后滚动到 `[data-tool-use-id="<toolUseId>"]` 节点

#### Scenario: 点击 user message injection 滚到 UserChunk

- **WHEN** 用户点击 User Messages Section 某条 user-message injection
- **THEN** SHALL 滚到与该 `aiGroupId` 相邻、紧贴其前的 `UserChunk` 容器（按时序在 `chunks` 数组中匹配）；若无前置 UserChunk 则 SHALL 退化为滚到该 AIChunk

#### Scenario: SessionDetail 渲染 chunk 时挂 DOM 锚点

- **WHEN** SessionDetail 渲染任意 `Chunk`
- **THEN** chunk 容器节点 SHALL 带 `data-chunk-id={chunk.chunkId}` 属性
- **AND** AIChunk 内每个 `ToolExecution` 渲染节点 SHALL 带 `data-tool-use-id={exec.toolUseId}` 属性

### Requirement: Context Panel Phase Selector

Context Panel Header SHALL 在 `SessionDetail.phaseInfo.phases.length > 1` 时显示 Phase 切换下拉控件；下拉项为 `Latest` + `Phase 1` + `Phase 2` + ... + `Phase N`（N = phases.length）。默认选中 `Latest`（对应内部 `selectedPhase = null`）。选中具体 phase `N` 时，Context Panel SHALL 从 `SessionDetail.injectionsByPhase[N]` 直接读取该 phase 的完整 accumulated injections；选中 `Latest` 时 SHALL 显示 `injectionsByPhase[最大 phaseNumber]`（即原 `contextInjections` 字段内容）。Context Panel Header `Visible: ~Xk tokens` SHALL 按当前过滤后的 injections 计算（Latest 即原行为）。

#### Scenario: 单 phase 会话不显示 selector

- **WHEN** `SessionDetail.phaseInfo` 缺失或 `phaseInfo.phases.length <= 1`
- **THEN** Context Panel Header SHALL NOT 渲染 Phase Selector

#### Scenario: 多 phase 会话默认 Latest

- **WHEN** `phaseInfo.phases.length > 1` 且 panel 首次打开
- **THEN** Phase Selector SHALL 显示并默认选中 `Latest`
- **AND** Context Panel SHALL 展示 `injectionsByPhase[最大 phaseNumber]`

#### Scenario: 切换到具体 phase 仅展示该 phase injections

- **WHEN** 用户从 Phase Selector 选中 `Phase 2`（`phaseNumber == 2`）
- **THEN** Context Panel SHALL 直接取 `SessionDetail.injectionsByPhase["2"]` 作为输入
- **AND** 不在该 phase 的 injections SHALL 不出现在任何 Section 中
- **AND** Header `Visible: ~Xk tokens` SHALL 计算该 phase injections 的 token 总和

#### Scenario: 选中的 phase 无 injections

- **WHEN** 选中某 phase 后 `injectionsByPhase[N]` 为空数组或 undefined
- **THEN** Context Panel body SHALL 显示占位文案"本 phase 无 injection"
- **AND** 所有 Section SHALL NOT 渲染
- **AND** Header `Visible` SHALL 显示 `~0`

### Requirement: Quick Anchor Navigation

SessionDetail SHALL 在长会话场景下提供「跳到最新消息」快速锚点：当 conversation scroll 容器距底距离 > 300px 时浮现右下角按钮 + `keyboard-shortcuts` registry 的 `session.jump-to-latest` 当前 binding（默认 mac `⌘↓` / Win+Linux `Ctrl+End`，双 binding spec）触发 smooth scroll 到末尾。该锚点 SHALL 仅作为**瞬时 affordance** 存在（动作不再适用即隐去），SHALL NOT 作为持久导航或装饰；SHALL NOT 引入除 neutral surface / border / text 外的色彩通道（不复用 Focus Blue / Execution Green / Failure Red / Compaction Amber / Thinking Purple）。

`session.jump-to-latest` spec 的 `allowInInput` SHALL 为 `false`（input focus 时让位给浏览器原生光标导航）；spec 的 `preventDefault` SHALL 为 `true`。

**注册位置**：该快捷键 SHALL 由 `PaneView`（或同层 controller）在 mount 阶段**只注册一次** shared handler，**不**由各 SessionDetail 实例分别注册（参见 `add-keyboard-shortcut-system::design.md::D8`：单 binding 单 spec 1:1 关系不允许多 instance 重复 `registerShortcut` 同一 ID）。shared handler 内部 SHALL 按 `getActiveTabId()` 找当前 focused pane 的 active tab，若该 tab 关联的是 SessionDetail 实例则调用其 `jumpToLatest()` 并返回 `true`；若 active tab 不是 SessionDetail（如 Dashboard / Settings）SHALL 返回 `false` 让 dispatcher 不 preventDefault、放行浏览器原生 `Cmd+↓` / `Ctrl+End` 行为。该快捷键 SHALL 由用户在 `Settings → Keyboard Shortcuts` 中自定义。

#### Scenario: 距底 ≤ 300px 时按钮隐藏
- **WHEN** conversation 容器满足 `scrollTop + clientHeight ≥ scrollHeight - 300`
- **THEN** 按钮 SHALL 不可见（`opacity: 0` 且 `pointer-events: none`），且 SHALL NOT 截获键盘 focus

#### Scenario: 距底 > 300px 时按钮显现
- **WHEN** 用户向上滚动使 `scrollTop + clientHeight < scrollHeight - 300`
- **THEN** 按钮 SHALL 在 conversation 容器右下角浮现（`position: absolute; bottom: 16px; right: 16px`）
- **AND** 进出动效 SHALL 为 `opacity + translateY(8px → 0)`，duration 200ms，曲线 `cubic-bezier(0.16, 1, 0.3, 1)`

#### Scenario: 点击按钮 smooth 滚动到末尾
- **WHEN** 用户点击按钮
- **THEN** conversation 容器 SHALL 调用 `scrollTo({ top: scrollHeight, behavior: 'smooth' })`
- **AND** 滚动期间 SHALL 设置 `isProgrammaticScroll = true` 抑制按钮重新显隐判定

#### Scenario: programmatic-scroll 状态机由 scrollend / 距底兜底 / 用户输入三路终止
- **WHEN** `isProgrammaticScroll = true` 期间，conversation 容器触发 `scrollend` 事件
- **THEN** SHALL 立即清 `isProgrammaticScroll = false` 并 `clearTimeout` 任何挂起的 fallback timer
- **AND-WHEN** 在 scrollend 不触发的边缘环境（如 `prefers-reduced-motion: reduce` 下的 `behavior: 'auto'` 路径），1500ms fallback timer SHALL 兜底清除该 flag
- **AND-WHEN** 滚动期间用户主动 `wheel` / `touchmove` / 非本快捷键 `keydown`（即用户打断 smooth scroll）
- **THEN** SHALL 立即清 `isProgrammaticScroll = false` 让按钮按当前距底距离重新派生可见性
- **AND-WHEN** 滚动期间 conversation 距底已 ≤ 16px
- **THEN** SHALL 立即清 `isProgrammaticScroll = false`（提前结束）

#### Scenario: 重复触发跳底不互相干扰
- **WHEN** `isProgrammaticScroll = true` 期间，用户再次点击按钮或再次按下 `session.jump-to-latest` 当前 binding
- **THEN** SHALL 先 `clearTimeout` 旧 fallback timer，再触发新 smooth scroll，重新 set `isProgrammaticScroll = true` 和新 fallback timer
- **AND** 旧 timer 不得提前清掉新 scroll 的 flag

#### Scenario: macOS 键盘快捷键触发跳底
- **WHEN** 平台为 macOS 且 `document.activeElement` 不是 `input` / `textarea` / `contenteditable` 元素
- **AND** 用户按下 `session.jump-to-latest` 当前 binding（默认 `⌘↓`）
- **THEN** registry dispatcher SHALL 命中 `session.jump-to-latest` spec
- **AND** PaneView shared handler SHALL 调 `getActiveTabId()` 找当前 focused SessionDetail 实例
- **AND** 若 active tab 是 SessionDetail，handler SHALL 调用其 `jumpToLatest()` 并返回 `true`，dispatcher SHALL 调用 `event.preventDefault()`
- **AND** SHALL 触发与按钮点击相同的 smooth 滚动到末尾路径

#### Scenario: Windows / Linux 键盘快捷键触发跳底
- **WHEN** 平台非 macOS 且 `document.activeElement` 不是 `input` / `textarea` / `contenteditable` 元素
- **AND** 用户按下 `session.jump-to-latest` 当前 binding（默认 `Ctrl+End`）
- **THEN** registry dispatcher SHALL 命中 `session.jump-to-latest` spec
- **AND** PaneView shared handler 同 macOS 路径，调 `getActiveTabId()` 找 active SessionDetail 实例并调用 `jumpToLatest()`
- **AND** dispatcher SHALL 调用 `event.preventDefault()`
- **AND** SHALL 触发与按钮点击相同的 smooth 滚动到末尾路径

#### Scenario: active tab 非 SessionDetail 时不消费
- **WHEN** focused pane 的 active tab 是 Dashboard / Settings / 其它非 SessionDetail
- **AND** 用户按下 `session.jump-to-latest` 当前 binding
- **THEN** PaneView shared handler SHALL 返回 `false`
- **AND** dispatcher SHALL NOT 调用 `event.preventDefault()`
- **AND** 浏览器原生 `Cmd+↓` / `Ctrl+End` 行为 SHALL 不被打断

#### Scenario: input focused 时键盘不拦截
- **WHEN** `document.activeElement` 是 `input` / `textarea` / `contenteditable` 元素（典型如 SessionDetail 内 SearchBar 输入框）
- **AND** 用户按下平台对应的跳底快捷键
- **THEN** registry dispatcher SHALL 在 `allowInInput=false` 守卫处直接 return
- **AND** SessionDetail SHALL NOT `preventDefault()`，SHALL 让浏览器原生光标导航生效（`Cmd+↓` 移光标到 input 末尾、`Ctrl+End` 同义）

#### Scenario: 多 pane 场景仅 focused pane 内 active SessionDetail 触发滚动
- **WHEN** PaneView 有 ≥ 2 个 pane 且每个 pane 内都有 SessionDetail tab 处于 mount 状态
- **AND** 用户按下 `session.jump-to-latest` 当前 binding
- **THEN** PaneView 顶层注册的 shared handler SHALL 通过 `getActiveTabId()` 找出当前 focused pane 的 active tab
- **AND** 仅该 active tab 关联的 SessionDetail（其 `tabId === getActiveTabId()`）SHALL 被调用 `jumpToLatest()` 触发 smooth 滚到底
- **AND** 其它 pane 的 SessionDetail（不是 active tab）的 `jumpToLatest()` SHALL NOT 被调用，SHALL NOT 触发滚动，保留原视口位置
- **AND** registry SHALL 在整个应用生命周期内对 `session.jump-to-latest` 仅持有 1 个 spec（由 PaneView 注册），SHALL NOT 因 SessionDetail mount / unmount 重复 register / unregister

#### Scenario: ContextPanel 打开时按钮让位
- **WHEN** ContextPanel 处于打开状态（`contextPanelVisible = true`）
- **THEN** 按钮的 `right` 偏移 SHALL 为 `CONTEXT_PANEL_WIDTH + 16px`（与 ContextPanel 既有宽度常量保持一致）
- **AND** ContextPanel 关闭后 SHALL 恢复 `right: 16px`

#### Scenario: reduced-motion 降级
- **WHEN** 用户系统设置 `prefers-reduced-motion: reduce`
- **THEN** 按钮进出 SHALL 为即时显隐（不做 opacity / translateY 过渡）
- **AND** 滚动到末尾 SHALL 使用 `behavior: 'auto'` 而非 `'smooth'`

#### Scenario: 切 tab 来回时按钮可见性重新判定
- **WHEN** 用户从 SessionDetail tab 切走再切回
- **THEN** 按钮可见性 SHALL 根据切回时的 `scrollTop / scrollHeight` 重新派生（不持久化按钮显隐 flag）
- **AND** 既有 `uiState.scrollTop` 恢复机制 SHALL 保持不变（按钮可见性是 scrollTop 的 derived）

#### Scenario: 按钮形态遵循 floating affordance 契约
- **WHEN** 按钮处于 visible 态
- **THEN** 视觉 SHALL 为 28×28 hit area + 14px `chevrons-down` icon + `6px` radius
- **AND** 颜色 SHALL 用 `--color-surface-raised` bg + 1px `--color-border-emphasis` + `--color-text-secondary` 图标色（不复用 Focus Blue / 任何语义色）
- **AND** Elevation SHALL 为 `0 2px 8px rgba(0,0,0,0.06)`，hover 升至 `0 4px 12px rgba(0,0,0,0.08)`
- **AND** SHALL 提供 `aria-label`（如「跳到最新消息」）+ 平台分流的 `title` tooltip 显示 `formatShortcut(currentBinding)` 的输出

### Requirement: SessionDetail 顶 bar meta-action menu 入口

SessionDetail 顶 bar SHALL 在 `.top-meta` 区（与既有 `[Context N]` toggle 并列）渲染一个 icon-only overflow menu trigger（下文统称 "meta-action menu"），承载会话级 on-demand 操作。trigger SHALL 复用 `.top-badge` 样式 token（`13px` icon、padding `6px 10px`、`border-radius 6px`），与 `[Context]` 共享视觉语言。

#### Scenario: meta-action trigger 渲染位置与形态

- **WHEN** SessionDetail 加载完成
- **THEN** 顶 bar 右侧 `.top-meta` 区 SHALL 渲染一个 icon-only `MoreHorizontal` (`⋯`) button
- **AND** trigger SHALL 位于 `[Context N]` toggle 的左侧（trigger 在左，Context 在右），二者间距对齐 `.top-meta` 既有 `gap: 8px`
- **AND** trigger SHALL NOT 渲染数字 / pill / text label
- **AND** trigger 默认态 icon 颜色 SHALL 为 `text-muted`，hover 态升至 `text` 主色

#### Scenario: 点击 trigger 展开 menu

- **WHEN** 用户点击 meta-action trigger
- **THEN** SHALL 在 trigger 下方右对齐位置展开 menu overlay（top = trigger.bottom + 4px）
- **AND** menu SHALL 不绘制指向 trigger 的箭头
- **AND** menu SHALL 按以下顺序列出 action items：
  1. `在 Finder 中打开`（macOS）/ `在文件管理器中打开`（其他平台）—— 仅 Tauri runtime 渲染
  2. `复制工作目录路径`
  3. `复制 Session ID`
- **AND** 第 (1)(2) 项与第 (3) 项之间 SHALL 渲染 1px `border-subtle` 分隔线（仅当第 (1) 项存在时）

#### Scenario: 平台分支 — HTTP server mode 隐藏文件管理器项

- **WHEN** 应用运行在 HTTP server mode（无 Tauri runtime，`isTauriRuntime() === false`）
- **THEN** menu SHALL NOT 渲染「在文件管理器中打开」项
- **AND** menu 仅包含「复制工作目录路径」与「复制 Session ID」两项
- **AND** SHALL NOT 渲染任何分隔线

#### Scenario: 平台分支 — Tauri 桌面 mode 完整渲染

- **WHEN** 应用运行在 Tauri runtime 内（`isTauriRuntime() === true`）
- **THEN** menu SHALL 渲染全部三项 action items 并含分隔线

#### Scenario: 「在文件管理器中打开」调 plugin

- **WHEN** 用户在 Tauri runtime 下点击「在文件管理器中打开」menu 项 AND `detail.metadata.cwd` 非空
- **THEN** SHALL 调用 `@tauri-apps/plugin-opener` 的 `openPath(detail.metadata.cwd)` API 打开系统文件管理器并定位到该路径
- **AND** menu overlay SHALL 立即关闭

#### Scenario: 「在文件管理器中打开」失败反馈

- **WHEN** `openPath` 调用 reject（路径不存在 / 权限拒绝 / plugin 内部错误）
- **THEN** trigger 区 SHALL 临时显示 `打开失败` 红字反馈（`color: danger`）
- **AND** 1500ms 后 SHALL 自动恢复 idle 态
- **AND** SHALL NOT 弹出 modal / dialog / global toast
- **AND** SHALL 在浏览器 console 或后端 `tracing` 留错误日志

#### Scenario: 「复制工作目录路径」成功

- **WHEN** 用户点击「复制工作目录路径」menu 项 AND `navigator.clipboard.writeText(detail.metadata.cwd)` resolve
- **THEN** menu overlay SHALL 立即关闭
- **AND** trigger 区 SHALL 临时显示 `已复制` 文字反馈（`color: text-secondary`）
- **AND** 1500ms 后 SHALL 自动恢复 idle 态
- **AND** SHALL NOT 弹出 toast

#### Scenario: 「复制工作目录路径」失败

- **WHEN** `navigator.clipboard.writeText` reject（多见于 HTTP non-secure context 或权限拒绝）
- **THEN** trigger 区 SHALL 临时显示 `复制失败` 红字反馈（`color: danger`）
- **AND** 1500ms 后 SHALL 自动恢复 idle 态

#### Scenario: 「复制 Session ID」

- **WHEN** 用户点击「复制 Session ID」menu 项
- **THEN** SHALL 调 `navigator.clipboard.writeText(detail.sessionId)` 复制完整 session id 字符串
- **AND** 反馈状态 SHALL 与「复制工作目录路径」一致（成功 `已复制` / 失败 `复制失败`，1500ms 自动恢复）

#### Scenario: cwd 缺失时降级

- **WHEN** `detail.metadata.cwd` 为 `undefined` / 空字符串（如老 session jsonl 不含 cwd 字段）
- **THEN** menu trigger SHALL 仍渲染并可点击展开
- **AND** 「在文件管理器中打开」与「复制工作目录路径」两项 SHALL 渲染为 disabled 态（不响应点击 / `text-muted` 色 / `cursor: not-allowed`）
- **AND** 「复制 Session ID」项 SHALL 保持可用
- **AND** menu SHALL NOT 渲染额外提示文案（disabled 态本身已传达）

#### Scenario: menu overlay 关闭行为

- **WHEN** menu 处于 open 态 AND 用户点击 menu 外区域 OR 按 `Esc` 键 OR 点击任一可用 menu 项
- **THEN** menu overlay SHALL 关闭
- **AND** trigger 焦点 SHALL 保持（键盘焦点回到 trigger）

#### Scenario: trigger 键盘可达性

- **WHEN** 用户使用键盘 Tab 移动焦点到 meta-action trigger
- **THEN** SHALL 渲染 `focus-visible` 蓝色 outline ring
- **AND** 按 `Enter` 或 `Space` SHALL 展开 menu
- **AND** menu open 态下方向键 SHALL 在 enabled menu 项之间移动焦点（disabled 项 SHALL 跳过）

#### Scenario: menu container ARIA 语义

- **WHEN** menu overlay 处于 open 态
- **THEN** menu 容器元素 SHALL 设 `role="menu"` 与 `aria-orientation="vertical"`
- **AND** trigger 元素 SHALL 设 `aria-haspopup="menu"` 与 `aria-expanded="true"`（关闭态切 `aria-expanded="false"`）
- **AND** trigger 元素 SHALL 设 `aria-controls=<menu-id>` 指向 menu 容器 id
- **AND** menu 中每个分组（cwd 操作组与 session id 操作组）SHALL 用 `role="separator"` 元素分隔（仅当多于一组时）

### Requirement: SessionDetail 顶 bar 不渲染完整 cwd 文本

SessionDetail 顶 bar SHALL NOT 在 `.top-stats` 行、`.top-titles` 区或任何常驻位置直接渲染完整 `cwd` 路径文本。完整 cwd 路径 SHALL 仅通过 meta-action menu 的 on-demand 操作（在文件管理器打开 / 复制路径）暴露。

#### Scenario: top-stats 行不含 CWD chip

- **WHEN** SessionDetail 加载完成
- **THEN** `.top-stats` 行 SHALL NOT 渲染 `CWD` label
- **AND** `.top-stats` 行 SHALL NOT 渲染任何完整 cwd 字符串
- **AND** `.top-stats` 行 SHALL 仅包含定长 / 短数字量化指标（AI / USER / TOOLS / TOK / LAST）

#### Scenario: top-stats 单行不触发 wrap

- **WHEN** SessionDetail 顶 bar 渲染于任意窗口宽度（≥ 最小桌面宽度 `800px`）
- **THEN** `.top-stats` 行 SHALL 单行渲染所有指标，不触发 `flex-wrap`
- **AND** `.top-stats` CSS SHALL 设 `flex-wrap: nowrap`
- **AND** SHALL NOT 在第一行末尾出现孤悬分隔符 `·`

#### Scenario: LAST 时间精度降级

- **WHEN** `.top-stats` 行渲染 LAST 时间
- **THEN** SHALL 显示分钟级 `HH:MM` 精度（如 `19:50`）
- **AND** SHALL NOT 显示秒级 `HH:MM:SS` 精度
- **AND** 时间格式与 sidebar 「刚刚 / 18m / 1h / HH:MM」时间显示密度对齐

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

