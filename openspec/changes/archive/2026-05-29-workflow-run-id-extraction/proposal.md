## Why

Workflow 工具的 `tool_result` 含 `toolUseResult.runId`（`wf_` 前缀），是关联 `workflows/wf_<runId>.json` manifest 的唯一 key。但当前 `pair_tool_executions` 只从 `toolUseResult` 抽取 `agentId`，且 tool output 会被默认裁剪（`outputOmitted=true`）——前端拿不到 runId，无法定位 manifest，后续 WorkflowCard 渲染无数据来源。本 change 在裁剪前抽取 runId 挂到 `ToolExecution`，为整条链路打通第一块基石。

## What Changes

- `ToolExecution` 新增 `workflow_run_id: Option<String>` 字段（camelCase 序列化为 `workflowRunId`，`skip_serializing_if = "Option::is_none"`）
- `pair_tool_executions` 在配对 Workflow 工具的 `tool_result` 时，从 `toolUseResult.get("runId")` 抽取值写入该字段
- IPC contract test 锁字段名，确保前端可稳定消费

## Capabilities

### New Capabilities

（无新增 capability）

### Modified Capabilities

- `tool-execution-linking`: 新增 Scenario "Workflow tool_result 的 runId 被抽取到 ToolExecution.workflow_run_id"

## Impact

- `crates/cdt-core/src/tool_execution.rs`：加字段
- `crates/cdt-analyze/src/tool_linking/pair.rs`：加一行 `.get("runId")` 抽取
- `crates/cdt-api/tests/ipc_contract.rs`：加 round-trip test
- 前端无改动（字段有但本 change 不消费）
- 性能：无 Workflow session 零开销（`skip_serializing_if` + 条件抽取仅在 toolName=="Workflow" 时触发）
