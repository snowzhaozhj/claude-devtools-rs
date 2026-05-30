## ADDED Requirements

### Requirement: WorkflowAgent session_id 关联

`get_session_detail` 返回的 `WorkflowItem.agents[]` 中，每个 `WorkflowAgent` SHALL 携带 `session_id: Option<String>` 字段，值为该 agent 对应的 JSONL 文件标识符（manifest 路径的 `agentId`）。该字段 SHALL 在 manifest 解析路径从 `workflowProgress[].agentId` 提取，在运行态降级路径从 journal `started` 条目的 `agentId` 字段提取。

`session_id` 为 `None` 时（manifest 缺 `agentId` 字段的旧格式边界），前端 SHALL 隐藏该 agent 的下钻入口。

#### Scenario: manifest 含 agentId 时填充 session_id

- **WHEN** workflow manifest `workflowProgress[]` 含 `type=workflow_agent` 条目且 `agentId` 字段存在
- **THEN** 对应 `WorkflowAgent.session_id` SHALL 为 `Some(agentId 值)`

#### Scenario: manifest 缺 agentId 时 session_id 为 None

- **WHEN** workflow manifest `workflowProgress[]` 含 `type=workflow_agent` 条目但无 `agentId` 字段
- **THEN** 对应 `WorkflowAgent.session_id` SHALL 为 `None`

#### Scenario: 运行态降级路径填充 session_id

- **WHEN** manifest 缺失，journal `started` 条目含 `agentId` 字段
- **THEN** 对应 `WorkflowAgent.session_id` SHALL 为 `Some(agentId 值)`

### Requirement: Lazy load workflow agent trace

新 IPC `get_workflow_agent_trace(sessionId, runId, agentId)` SHALL 返回指定 workflow 子代理的完整 chunks 流（`Vec<Chunk>`），用于前端展开时按需拉取。

路径构造 SHALL 为：`<projects_dir>/<project_containing_session>/<sessionId>/subagents/workflows/<runId>/agent-<agentId>.jsonl`。

后端 SHALL 先在 `projects_dir` 下找到包含 `<sessionId>/` 子目录的 project，再拼接完整路径。找到文件后走 `parse_file + build_chunks` 管线返回结果。

#### Scenario: 拉取存在的 workflow agent trace

- **WHEN** caller 调用 `get_workflow_agent_trace("parent-session", "wf_abc-123", "agent-xyz")`
- **AND** 对应 JSONL 文件 `<session_dir>/subagents/workflows/wf_abc-123/agent-agent-xyz.jsonl` 存在
- **THEN** 返回非空 `Vec<Chunk>`，内容为该子代理的完整对话流

#### Scenario: workflow agent JSONL 不存在

- **WHEN** caller 调用 `get_workflow_agent_trace` 且对应 JSONL 文件不存在
- **THEN** 返回空 `Vec<Chunk>`（不报错）
