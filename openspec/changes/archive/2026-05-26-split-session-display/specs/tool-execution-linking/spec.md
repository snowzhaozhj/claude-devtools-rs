## MODIFIED Requirements

### Requirement: Source tool output text from raw tool_result.content

系统 SHALL 把 user 消息内 `message.content[i].content`（即 Anthropic API `ContentBlock::ToolResult` 的 `content` 字段）作为 `ToolExecution.output` 的唯一来源，按形态填入 `ToolOutput::Text { text }`（字符串）或 `ToolOutput::Structured { value }`（对象 / 数组）。系统 SHALL NOT 读取 user 消息顶层 `toolUseResult` 字段（Claude Code 独立写入的 enriched 数据，含 `file.content` / `file.numLines` / `file.startLine` 等不带行号前缀的 Read 工具元数据）来填充 `output` —— 该顶层字段 MUST 仅用于 `result_agent_id` 与 `teammate_spawn` 的元数据抽取（见 `Detect teammate-spawned tool results` Requirement）。

这意味着 Claude Read 工具的 `output.text` 完整保留原始 cat -n 风格 `<num>\t<text>` 行号前缀；UI 渲染层（`tool-viewer-routing` capability 的 `ReadToolViewer`）按需 strip 前缀，避免与 CSS `::before data-line` 双重显示行号。

#### Scenario: Read tool output preserves cat -n line prefixes
- **WHEN** Read 工具的 user 消息 `tool_result.content` 字符串以 cat -n 风格行号开头（例如 `"1\tline-a\n2\tline-b\n"`）
- **THEN** 配对产出的 `ToolExecution.output` SHALL 为 `ToolOutput::Text { text }`，`text` 完整保留原始行号前缀，不做 strip 也不切换到 enriched 来源

#### Scenario: Enriched toolUseResult fields are not used for output
- **WHEN** user 消息同时含 raw `tool_result.content`（含 cat -n 前缀）与顶层 `toolUseResult.file.content`（Claude Code enriched，无前缀）
- **THEN** `ToolExecution.output` SHALL 来自 raw 路径；顶层 `toolUseResult.file.*` 仅供 `teammate_spawn` 与 `result_agent_id` 抽取使用，SHALL NOT 参与 `output` 字段填充
