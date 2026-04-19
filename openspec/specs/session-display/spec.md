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

Context Panel SHALL 支持 Category（按类别分组）和 Ranked（按 token 排序）两种视图模式。

#### Scenario: 默认 Category 视图
- **WHEN** Context Panel 打开
- **THEN** SHALL 默认显示 Category 视图，按类别分组展示注入项

#### Scenario: 切换到 Ranked 视图
- **WHEN** 用户点击 "Ranked" 按钮
- **THEN** SHALL 将所有注入项按 estimatedTokens 降序排列，平铺显示，每项带分类颜色标签

#### Scenario: 分类颜色系统
- **WHEN** Ranked 视图中渲染注入项
- **THEN** 各类别 SHALL 使用对应颜色标签：claude-md 紫蓝、file 绿、tool 黄、thinking 紫、team 橙、user 蓝

### Requirement: CLAUDE.md DirectoryTree

Category 视图中的 CLAUDE.md 类别 SHALL 以递归目录树形式展示文件路径。

#### Scenario: 目录树渲染
- **WHEN** CLAUDE.md 类别下有多个文件
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

SessionDetail 渲染 `AIChunk` 时 MUST 按 `semantic_steps` 顺序依次输出 DisplayItem，使 subagent 卡片与其对应 Task 调用**时序相邻**；同时 UI SHALL 跳过与 subagent 已关联的 Task `tool_execution`，避免同一个逻辑调用同时以"Task tool 调用行"和"Subagent 卡片"两种形式重复显示。

#### Scenario: Task 调用后紧随 Subagent 卡片
- **WHEN** AIChunk 含 `Read` → `Task(t_task)` → `Grep` 三个 tool_execution，且 `Task(t_task)` 已解析出 subagent A
- **THEN** UI SHALL 依序渲染：Read tool item → Subagent 卡片（A）→ Grep tool item；SHALL NOT 在 Grep 之后再输出 Subagent

#### Scenario: Task 去重
- **WHEN** `chunk.subagents` 中某 subagent 的 `parentTaskId = t_task`，且 `chunk.toolExecutions` 也含 `toolUseId = t_task, toolName = "Task"`
- **THEN** `displayItemBuilder` SHALL NOT 为该 `tool_execution` 步骤输出 `DisplayItem.type === "tool"`；subagent 卡片 SHALL 是该 Task 的唯一可见代表

#### Scenario: Orphan Task 保留显示
- **WHEN** 某 Task `tool_use` 未匹配任何 subagent（`Resolution::Orphan`）
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

SessionDetail SHALL 把所有 markdown 内容（user prose / AI lastOutput / Thinking 展开体 / Output 展开体 / Slash instructions 展开体 / System pre 文本）的 `renderMarkdown` 调用延迟到节点进入视口（含 `200 px` rootMargin 余量）后再触发；视口外的对应区域 SHALL 仅渲染高度估算占位（背景色块），不调用 marked / highlight.js / DOMPurify。Mermaid block 的 `processMermaidBlocks` SHALL 在该 markdown 区真正渲染**之后**再被触发，不在占位阶段扫描。

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

