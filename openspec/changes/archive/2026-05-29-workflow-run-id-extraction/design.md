## Context

`pair_tool_executions`（`cdt-analyze/src/tool_linking/pair.rs`）在配对 tool_use/tool_result 时，已从 user 消息顶层 `toolUseResult`（`Option<serde_json::Value>`）抽取 `agentId`（line 82-83）和 `teammate_spawn` 信息。Workflow 工具的 `toolUseResult` 含 `runId` 字段（`"wf_797e9bdf-994"` 格式），是后续定位 `workflows/wf_<runId>.json` manifest 的唯一 key。

tool output 在 `get_session_detail` 序列化前会被 trim（`local.rs:335-351`，`outputOmitted=true`），所以 runId 必须在 trim 前抽出、作为 `ToolExecution` 的独立字段持久化到 IPC payload。

## Goals / Non-Goals

**Goals:**
- 在 pair 阶段从 Workflow `toolUseResult.runId` 抽取值写入 `ToolExecution.workflow_run_id`
- IPC 序列化为 `workflowRunId`，无值时不出现在 JSON
- 通过 ipc_contract test 锁字段名

**Non-Goals:**
- 不读 manifest 文件
- 不改 chunk-building / 不产 WorkflowItem
- 不改前端
- 不改 scan_subagent_candidates

## Decisions

### D1: 抽取位置选 pair 阶段，不选 chunk-building

pair 阶段能直接访问 `msg.tool_use_result`（`Option<serde_json::Value>`），且在 output trim 之前。如果延后到 chunk-building 阶段，output 已被清空，只能从 output text 正则提取（脆弱）。

候选方案：
- A. pair 阶段从 `toolUseResult.get("runId")` 抽（选此）
- B. chunk-building 阶段从 `exec.output` 正则匹配 "Run ID: wf_xxx"（脆弱，依赖文本格式）
- C. 专门新建 "Workflow 预处理" pass（过度设计）

### D2: 条件抽取——仅 toolName == "Workflow" 时才尝试读 runId

避免对所有 tool_result 做额外 `.get("runId")` 访问。实际上 `.get()` 对 Value 是 O(1) hashmap lookup（~10ns），非 Workflow 不做是为了**语义清晰**而非性能。

### D3: 字段类型选 `Option<String>` + skip_serializing_if

- Workflow 工具尚未普及，绝大多数 ToolExecution 该字段为 None → 不序列化 → 零 payload 增量
- 与 `result_agent_id: Option<String>` 同层同模式，一致性好

## Risks / Trade-offs

- [runId 字段缺失/非 string] → 降级为 None，不影响既有逻辑；ipc_contract test 覆盖此 case
- [toolUseResult 为 None（老后端/回滚）] → 同上降级为 None
- [resume 场景 input.resumeFromRunId 与 result.runId 不同] → 取 result 返回的为准（代表实际执行的 run）
- [ToolExecution 加非 Option 字段破坏构造点] → 选 `Option<T> + serde(default)` 避免此问题
