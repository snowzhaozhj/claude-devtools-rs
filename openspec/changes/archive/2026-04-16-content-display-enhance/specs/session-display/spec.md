# session-display Specification (Delta — content-display-enhance)

> Delta spec：新增 DiffViewer 和 Mermaid 渲染相关 Requirements。

## ADDED Requirements

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
- **WHEN** markdown 内容包含 \`\`\`mermaid 代码块
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
