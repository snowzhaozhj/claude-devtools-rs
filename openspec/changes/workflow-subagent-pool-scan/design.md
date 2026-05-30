## Context

Workflow 子代理（`subagents/workflows/wf_<runId>/agent-*.jsonl`）的 JSONL 文件当前完全不被扫描。`WorkflowAgent` 仅有 summary 字段（label/state/tokens），不含 `session_id`。前端无法关联到子代理的完整对话内容。

现有架构中：
- `get_subagent_trace(rootSessionId, subagentSessionId)` 已实现普通子代理的懒加载（跨 project_dir 扫 `{root}/subagents/agent-<sub>.jsonl`）
- `parse_manifest` 解析 `workflowProgress[]` 时已读取 `agentId` 字段但未存入 `WorkflowAgent`
- 运行态降级路径（journal 合成）读取 journal `started` 条目的 `agentId` 但未存入
- Workflow 子代理路径固定：`<session_dir>/subagents/workflows/<runId>/agent-<agentId>.jsonl`

## Goals / Non-Goals

**Goals:**
- `WorkflowAgent` 新增 `session_id` 字段，传递到前端
- 新增 IPC 端点 `get_workflow_agent_trace` 懒加载 workflow 子代理完整对话
- 路径查找直接拼接（不需跨 project 扫描——workflow 子代理固定在父 session_dir 下）
- 前端 TS 类型同步

**Non-Goals:**
- 前端下钻 UI（SubagentCard 渲染）——PR 5
- workflow 子代理的 `messagesOmitted` 瘦身——首次打开 workflow agent trace 是用户显式点击，不在首屏 payload 路径
- 跨 project_dir 的 workflow 子代理查找——子代理固定在父 session 的 `subagents/workflows/` 下

## Decisions

**D1：路径查找策略——直接拼接 vs 跨目录扫描**

选直接拼接。Workflow 子代理路径完全确定：`<session_dir>/subagents/workflows/<runId>/agent-<agentId>.jsonl`。无需像普通 subagent 那样跨 project_dir 遍历（那是因为 worktree 场景子代理可能在不同 project_dir 下）。直接拼接零额外 I/O。

**D2：IPC 端点签名——复用 `get_subagent_trace` vs 新建独立端点**

新建 `get_workflow_agent_trace(session_id, run_id, agent_id)` 独立端点。理由：
- 路径构造逻辑不同（workflow 子代理在 `subagents/workflows/<runId>/` 下而非 `subagents/` 下）
- 调用方需要显式传 `run_id` 来定位正确的 workflow 子目录
- 复用 `get_subagent_trace` 需要 hack 参数语义（把 run_id 藏到某个参数里），不如直接新建

内部复用 `parse_file_via_fs + build_chunks` 管线（同 `get_subagent_trace`）。

**D3：`session_id` 来源——manifest agentId vs JSONL 文件名**

从 manifest `workflowProgress[].agentId` 直接取。实证确认该字段与 JSONL 文件名的 `agent-<agentId>.jsonl` 部分一致。运行态降级路径同理从 journal `started` 条目的 `agentId` 字段取。

**D4：`session_id` 可选性——`Option<String>` vs 必填**

用 `Option<String>`（`skip_serializing_if = "Option::is_none"`）。原因：
- manifest 里某些旧格式 agent 条目可能没有 `agentId`
- 运行态降级路径 journal 条目也可能缺 `agentId`（边界情况）
- 前端按有无 `sessionId` 决定是否显示下钻入口（PR 5）

**D5：IPC 端点需要 `session_id`（父 session）来定位 session_dir**

端点签名：`get_workflow_agent_trace(session_id: String, run_id: String, agent_id: String)`。
- `session_id`：父 session，用于查找 session_dir（同 `get_subagent_trace` 的 `root_session_id` 逻辑）
- `run_id`：workflow run ID（如 `wf_797e9bdf-994`）
- `agent_id`：workflow agent 的 session ID（如 `ad34cb14a1ae5b192`）

路径拼接：`<projects_dir>/<project>/<session_id>/subagents/workflows/<run_id>/agent-<agent_id>.jsonl`

查找父 session_dir：复用现有 `find_session_dir_cross_project` 或按 `get_subagent_trace` 同模式扫描 `projects_dir/*/` 找含 `<session_id>.jsonl` 的 project_dir。

## Risks / Trade-offs

1. **manifest 无 `agentId` 的旧格式**：workflow 工具很新（2026 Q2+），实际上所有 manifest 都带 `agentId`。用 `Option` 兜底已足够。
2. **性能**：`session_id` 字段增加 payload 几十字节/agent，在 WorkflowItem 级别可忽略。新 IPC 端点是用户显式触发，不在首屏路径。
3. **`get_workflow_agent_trace` 找不到文件时返空**：与 `get_subagent_trace` 行为一致——返回空 `Vec<Chunk>`，前端显示"无内容"。
