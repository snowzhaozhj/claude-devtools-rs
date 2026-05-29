## ADDED Requirements

### Requirement: Extract Workflow scriptPath from toolUseResult

系统 SHALL 在 `tool_use` 的 `toolName == "Workflow"` 时，按以下优先级抽取 script 绝对路径到 `ToolExecution.workflow_script_path`：先取配对 user 消息顶层 `toolUseResult.scriptPath`（字符串类型）；该处缺失或非字符串时 SHALL 回退取 `tool_use.input.scriptPath`（字符串类型）。两处都无（例如 inline `{script, description}` 调用形态）时为 `None`。抽取 SHALL 发生在 output trim 之前，与 `workflow_run_id` 抽取同处，确保即使 `outputOmitted=true` 后端仍可用该绝对路径定位运行态 script 文件。

序列化 SHALL 用 camelCase `workflowScriptPath`，`#[serde(default, skip_serializing_if = "Option::is_none")]` 让无 Workflow 的 `ToolExecution` IPC payload 不含此字段。

#### Scenario: Workflow tool_result 含 scriptPath 字段

- **WHEN** `tool_use.name == "Workflow"` 且配对的 user 消息 `toolUseResult` 为 JSON 对象含 `"scriptPath": "/abs/path/workflows/scripts/foo-wf_abc123.js"`
- **THEN** 配对产出的 `ToolExecution.workflow_script_path` SHALL 为 `Some("/abs/path/workflows/scripts/foo-wf_abc123.js".into())`
- **AND** IPC 序列化 JSON SHALL 含 `"workflowScriptPath": "/abs/path/workflows/scripts/foo-wf_abc123.js"`

#### Scenario: toolUseResult 无 scriptPath 时回退 tool_use.input

- **WHEN** `tool_use.name == "Workflow"`，`toolUseResult` 无 `scriptPath`，但 `tool_use.input` 含 `"scriptPath": "/abs/path/foo-wf_abc123.js"`（字符串）
- **THEN** `ToolExecution.workflow_script_path` SHALL 为 `Some("/abs/path/foo-wf_abc123.js".into())`

#### Scenario: 两处都无 scriptPath（inline script 调用）

- **WHEN** `tool_use.name == "Workflow"` 但 `toolUseResult` 与 `tool_use.input` 均无 `scriptPath` 字段（inline `{script, description}` 调用形态），或值非字符串
- **THEN** `ToolExecution.workflow_script_path` SHALL 为 `None`
- **AND** IPC 序列化 JSON SHALL 不含 `workflowScriptPath` 键

#### Scenario: 非 Workflow 工具不抽取 scriptPath

- **WHEN** `tool_use.name` 为 "Bash"、"Read"、"Task" 等非 "Workflow" 值
- **THEN** `ToolExecution.workflow_script_path` SHALL 为 `None`，无论 `toolUseResult` 内容如何
