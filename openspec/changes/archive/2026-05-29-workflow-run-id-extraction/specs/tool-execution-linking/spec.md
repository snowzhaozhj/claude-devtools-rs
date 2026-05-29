## ADDED Requirements

### Requirement: Extract Workflow runId from toolUseResult

系统 SHALL 在 `tool_use` 的 `toolName == "Workflow"` 且配对的 user 消息顶层 `toolUseResult` 含 `runId` 字段（字符串类型）时，把该值抽取到 `ToolExecution.workflow_run_id`。抽取 SHALL 发生在 output trim 之前，确保即使 `outputOmitted=true` 后前端仍可消费该字段。

序列化 SHALL 用 camelCase `workflowRunId`，`#[serde(default, skip_serializing_if = "Option::is_none")]` 让无 Workflow 的 ToolExecution IPC payload 不含此字段。

#### Scenario: Workflow tool_result 含 runId 字段

- **WHEN** `tool_use.name == "Workflow"` 且配对的 user 消息 `toolUseResult` 为 JSON 对象含 `"runId": "wf_797e9bdf-994"`
- **THEN** 配对产出的 `ToolExecution.workflow_run_id` SHALL 为 `Some("wf_797e9bdf-994".into())`
- **AND** IPC 序列化 JSON SHALL 含 `"workflowRunId": "wf_797e9bdf-994"`

#### Scenario: Workflow tool_result runId 缺失或非字符串

- **WHEN** `tool_use.name == "Workflow"` 但 `toolUseResult` 无 `runId` 字段，或 `runId` 值非字符串
- **THEN** `ToolExecution.workflow_run_id` SHALL 为 `None`
- **AND** IPC 序列化 JSON SHALL 不含 `workflowRunId` 键

#### Scenario: 非 Workflow 工具不抽取 runId

- **WHEN** `tool_use.name` 为 "Bash"、"Read"、"Task" 等非 "Workflow" 值
- **THEN** `ToolExecution.workflow_run_id` SHALL 为 `None`，无论 `toolUseResult` 内容如何

#### Scenario: toolUseResult 为 None 时不抽取

- **WHEN** user 消息无顶层 `toolUseResult` 字段（老后端 / 回滚）
- **THEN** `ToolExecution.workflow_run_id` SHALL 为 `None`
