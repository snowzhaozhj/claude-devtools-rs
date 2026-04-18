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

#### Scenario: Subagent 默认折叠
- **WHEN** 一条 AI 组首次渲染，其中包含一个 subagent
- **THEN** subagent 卡片 SHALL 以单行 Header 形式展示，Dashboard 与 ExecutionTrace 均不可见

#### Scenario: 点击 Header 展开 Dashboard
- **WHEN** 用户点击 subagent 卡片的 Header 区域
- **THEN** SHALL 展开显示 Dashboard（meta 行 + Context Usage 列表）与 Execution Trace 折叠头；chevron SHALL 旋转 90°

#### Scenario: Execution Trace 内独立展开
- **WHEN** 用户点击已展开卡片中的 "Execution Trace" 折叠头
- **THEN** SHALL 显示该 subagent 完整的 DisplayItem 流（thinking / tool / output / 嵌套 subagent），与父卡片展开状态独立保存

#### Scenario: 嵌套 subagent 递归渲染
- **WHEN** subagent 的 ExecutionTrace 中包含另一个 subagent（`process.messages` 内再有 Task tool 调用结果）
- **THEN** 内层 subagent SHALL 作为可独立展开的 SubagentCard 渲染；渲染深度 SHALL 不超过 8 层，超过时内层只显示 Header 不再递归

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
- **Subagent Context**：最后一条 assistant 消息 `usage` 的 `input + output + cache_read + cache_creation` 之和
- **Duration**：`process.duration_ms` 使用 `formatDuration` 格式化（秒/分钟）

若某维度数据缺失（`None` 或零值），对应槽位 SHALL 不渲染。

#### Scenario: 非 team subagent 显示两维
- **WHEN** subagent 有 `main_session_impact.total_tokens = 5000` 与最新 usage 合计 12000
- **THEN** MetricsPill SHALL 显示 `Main: 5.0k` 与 `Ctx: 12.0k` 两个槽位

#### Scenario: Team 成员隐藏 Main Context
- **WHEN** subagent 是 team 成员（`team != None`）
- **THEN** MetricsPill SHALL NOT 显示 Main Context 槽位，仅显示 Context Window（最新 usage 合计）

#### Scenario: 数据全缺失
- **WHEN** 两个维度均为 `None` 或 0
- **THEN** MetricsPill SHALL 整体不渲染，但 Duration 显示逻辑不受影响

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

