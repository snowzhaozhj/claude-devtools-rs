## MODIFIED Requirements

### Requirement: 工具专化查看器路由

展开的工具项 SHALL 根据 toolName 路由到专化查看器。未知工具 SHALL 使用默认查看器。每个工具查看器 SHALL 在 header 区域提供 inline copy 按钮用于一键复制工具内容。

#### Scenario: Read 工具
- **WHEN** toolName 为 "Read" 且无错误
- **THEN** SHALL 使用 ReadToolViewer 渲染（显示文件路径、行号范围、代码内容）
- **AND** header SHALL 包含 copy 按钮，点击复制文件内容（strip 行号前缀后的纯文本）

#### Scenario: Edit 工具
- **WHEN** toolName 为 "Edit"
- **THEN** SHALL 使用 EditToolViewer 渲染（显示 old_string / new_string 对比）

#### Scenario: Write 工具
- **WHEN** toolName 为 "Write" 且无错误
- **THEN** SHALL 使用 WriteToolViewer 渲染（显示文件路径和内容）
- **AND** header SHALL 包含 copy 按钮，点击复制文件写入内容全文

#### Scenario: Bash 工具
- **WHEN** toolName 为 "Bash" 或 "bash"
- **THEN** SHALL 使用 BashToolViewer 渲染（显示命令和输出）
- **AND** header SHALL 包含 copy 按钮，点击复制命令输出文本

#### Scenario: 未知工具
- **WHEN** toolName 不匹配任何专化查看器
- **THEN** SHALL 使用 DefaultToolViewer 渲染（显示 JSON 输入和文本输出）
