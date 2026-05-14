## ADDED Requirements

### Requirement: 大文本工具详情交互优先渲染

Read、Write、Edit 工具详情在展开较大文本内容时 SHALL 避免一次性对所有行执行重型同步语法高亮或 HTML 清洗；小/中等 Read、Write 内容 SHALL 保留完整语法高亮，大文本才 SHALL 降级到轻量高亮。展开交互 MUST 先让 header、容器、滚动和点击目标保持可响应。任何通过 `{@html}` 注入的工具内容 MUST 来自受控内部渲染器输出或经过 XSS 防护清洗。

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

#### Scenario: omitted Read 输出 ready 后再展开
- **WHEN** 用户首次展开一个 `outputOmitted=true` 且尚未缓存输出的 Read 工具项
- **THEN** SessionDetail SHALL 先拉取完整输出
- **AND** SHALL 在输出可用后再展开 ReadToolViewer，避免空内容或占位在已展开区域内被大块文件内容替换

#### Scenario: 嵌套 ExecutionTrace 的 omitted Read 输出 ready 后再展开
- **WHEN** 用户在 SubagentCard 的 ExecutionTrace 中首次展开一个 `outputOmitted=true` 且尚未缓存输出的 Read 工具项
- **THEN** ExecutionTrace SHALL 先拉取完整输出
- **AND** SHALL 在输出可用后再展开 ReadToolViewer，避免空内容或占位在已展开区域内被大块文件内容替换

#### Scenario: 工具详情展开状态局部更新
- **WHEN** 用户展开或收起单个工具项
- **THEN** SessionDetail SHALL 保持其他 chunk 与工具项的展开状态不变
- **AND** SHALL 避免因该单项状态变化重新执行与该工具无关的 Markdown、Mermaid 或 diff 渲染工作
