## MODIFIED Requirements

### Requirement: 大文本工具详情交互优先渲染

Read、Write、Edit 工具详情在展开较大文本内容时 SHALL 避免一次性对所有行执行重型同步语法高亮或 HTML 清洗；小/中等 Read、Write 内容 SHALL 保留完整语法高亮，大文本才 SHALL 降级到轻量高亮。展开交互 MUST 先让 header、容器、滚动和点击目标保持可响应。任何通过 `{@html}` 注入的工具内容 MUST 来自受控内部渲染器输出或经过 XSS 防护清洗。`outputOmitted=true` 时，对依赖 `exec.output` 渲染的工具（Read / Bash / DefaultToolViewer 路径）SHALL 先 IPC 拉取输出再展开；不依赖 `exec.output` 渲染的工具（Edit / Write，仅渲染 `exec.input` 字段）SHALL 立即展开，不为不会被消费的 output 引入额外延迟。

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
- **WHEN** 用户展开一个 Edit 或 Write 工具项，无论 `outputOmitted` 是否为 `true`
- **THEN** SessionDetail 与 ExecutionTrace SHALL 立即把该工具项加入展开集合，SHALL NOT 等待 `getToolOutput` IPC
- **AND** 渲染 SHALL 仅依赖 `exec.input` 字段（Edit 的 `old_string` / `new_string`、Write 的 `content`），与 `exec.output` 无关

#### Scenario: 展开 AIChunk 不主动 prefetch Bash 与 Default
- **WHEN** 用户点击 AIChunk header 把工具区域展开
- **THEN** SessionDetail SHALL NOT 对该 chunk 内的 Bash 或 Default 路径工具触发 `getToolOutput` IPC（`prefetchReadOutputs` 仅命中 Read）
- **AND** Bash / Default 工具的 output 拉取 SHALL 仅在用户点击该具体工具项展开时按需触发，避免一次 chunk 展开引起并发 IPC 把"展开工具列表"交互拖慢

#### Scenario: 工具详情展开状态局部更新
- **WHEN** 用户展开或收起单个工具项
- **THEN** SessionDetail SHALL 保持其他 chunk 与工具项的展开状态不变
- **AND** SHALL 避免因该单项状态变化重新执行与该工具无关的 Markdown、Mermaid 或 diff 渲染工作
