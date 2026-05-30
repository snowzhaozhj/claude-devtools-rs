## 1. WorkflowAgent 加 session_id 字段

- [ ] `crates/cdt-core/src/workflow.rs`：`WorkflowAgent` 新增 `pub session_id: Option<String>`，带 `#[serde(default, skip_serializing_if = "Option::is_none")]`
- [ ] grep 所有 `WorkflowAgent {` 构造点，补 `session_id: None` 或实际值

## 2. manifest 解析填充 session_id

- [ ] `crates/cdt-api/src/ipc/workflow_manifest.rs::parse_manifest`：从 `entry_val.get("agentId")` 提取，填入 `WorkflowAgent { session_id: ... }`

## 3. 运行态降级路径填充 session_id

- [ ] `workflow_manifest.rs::resolve_running_state`：journal `started` 条目已解析 `agentId`，将其传递到合成的 `WorkflowAgent.session_id`

## 4. 新增 IPC get_workflow_agent_trace

- [ ] `crates/cdt-api/src/ipc/local.rs`：实现 `get_workflow_agent_trace(session_id, run_id, agent_id) -> Vec<Chunk>`
  - 在 `projects_dir` 下找含 `<session_id>` 目录的 project
  - 拼接 `<project>/<session_id>/subagents/workflows/<run_id>/agent-<agent_id>.jsonl`
  - 走 `parse_file_via_fs + build_chunks` 管线
  - 文件不存在返回空 Vec
- [ ] `DataApi` trait 加 `get_workflow_agent_trace` 方法（含默认空实现）

## 5. Tauri command 注册

- [ ] `src-tauri/src/lib.rs`：新增 `get_workflow_agent_trace` command 包装
- [ ] `src-tauri/capabilities/default.json`：允许该 command

## 6. 前端类型同步

- [ ] `ui/src/lib/api.ts`：`WorkflowAgent` interface 加 `sessionId?: string`
- [ ] `ui/src/lib/api.ts`：新增 `getWorkflowAgentTrace(sessionId, runId, agentId)` 函数（调用 IPC invoke）

## 7. 测试

- [ ] `crates/cdt-api/tests/ipc_contract.rs`：`WorkflowAgent` 序列化 round-trip 含 `sessionId`
- [ ] manifest 解析单测：含 `agentId` → `session_id = Some(...)`
- [ ] manifest 解析单测：缺 `agentId` → `session_id = None`
- [ ] `get_workflow_agent_trace` 集成测试：fixture 含 workflow 子代理 JSONL → 返回非空 chunks
- [ ] `get_workflow_agent_trace` 集成测试：文件不存在 → 返回空 Vec

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change
