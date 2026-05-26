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
