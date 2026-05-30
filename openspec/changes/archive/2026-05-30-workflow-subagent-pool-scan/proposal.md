## Why

Workflow 内嵌子代理的 JSONL 文件（`subagents/workflows/wf_<runId>/agent-*.jsonl`）当前完全不被扫描/解析。`WorkflowAgent` 仅有 summary 级字段（label/state/tokens），无法关联到具体子代理 session，前端无从下钻查看子代理对话内容。这是 Epic #397 二期"agent chip 下钻内嵌 SubagentCard"(PR 5) 的数据前置。

## What Changes

- `WorkflowAgent` 新增 `session_id: Option<String>` 字段，关联子代理 JSONL 文件的 agentId
- Workflow manifest 解析路径（`parse_manifest`）从 `workflowProgress[].agentId` 提取 session_id
- Workflow 运行态降级路径（journal 合成）从 journal `started` 条目的 `agentId` 填充 session_id
- 新增 IPC `get_workflow_agent_trace(parentSessionId, runId, agentSessionId)` 懒加载端点，复用现有 `parse_file + build_chunks` 路径解析 workflow 子代理 JSONL
- 前端 TypeScript 类型同步 `WorkflowAgent.sessionId`（本 PR 仅同步类型，不做下钻 UI）

## Capabilities

### New Capabilities

（无新 capability）

### Modified Capabilities

- `ipc-data-api`: 新增 `get_workflow_agent_trace` 懒加载端点；`WorkflowAgent` 返回值新增 `sessionId` 字段

## Impact

- `crates/cdt-core/src/workflow.rs` — `WorkflowAgent` struct 加字段
- `crates/cdt-api/src/ipc/workflow_manifest.rs` — manifest 解析填充 session_id
- `crates/cdt-api/src/ipc/local.rs` — 新增 IPC handler + 分池扫描辅助函数
- `src-tauri/src/lib.rs` — 注册新 Tauri command
- `src-tauri/capabilities/default.json` — 允许新 command
- `ui/src/lib/api.ts` — TypeScript 类型同步
- `crates/cdt-api/tests/ipc_contract.rs` — 新端点 contract test
