# tool-viewer-routing Specification (delta)

## MODIFIED Requirements

### Requirement: Tool summary 生成

getToolSummary SHALL 为每种已知工具名生成人类可读的 header 摘要文本。

#### Scenario: Workflow tool summary
- **WHEN** toolName 为 "Workflow"
- **THEN** SHALL 优先显示 input.name（截断到 50 字符）
- **AND** 若 name 缺失但 run_id/runId 存在 SHALL 显示 "run {runId}"（截断到 20 字符）
- **AND** 若两者都缺失 SHALL 显示 "Workflow"
