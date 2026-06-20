## MODIFIED Requirements

### Requirement: Lazy load subagent trace

新 IPC `get_subagent_trace(parentSessionId, subagentSessionId)` MUST 返回该 subagent 的完整 chunks 流，用于 SubagentCard 展开时按需拉取被 `messagesOmitted` 裁剪的 trace 数据。后端 SHALL **在 caller 所在 `projects_dir` 下所有 project 目录**扫描 `<parentSessionId>/subagents/agent-<subagentSessionId>.jsonl`（新结构），命中即返；未命中时 fallback 到旧结构兼容路径（仅在主 `project_dir` 内查找 flat `agent-<subagentSessionId>.jsonl`）。`parse_file` + `build_chunks` 后,后端 SHALL 对该 chunks 流执行 `promote_result_agent_tasks` 骨架升级(见 `chunk-building::Promote nested Agent calls to skeleton subagents`),把子 transcript 内携带 `result_agent_id` 的嵌套 `Agent` / `Task` 调用暴露为带 `messagesOmitted=true` 的可懒拉骨架 subagent,再返回 `Vec<Chunk>`。骨架升级 SHALL NOT 读取嵌套子文件、SHALL NOT 额外 parse——仅复用已解析的 `result_agent_id`，使展开成本与拉取一层 trace 同量级。

#### Scenario: 拉取存在的 subagent trace

- **WHEN** caller 调用 `get_subagent_trace("parent-uuid", "sub-uuid")`，对应 subagent jsonl 存在
- **THEN** 响应 SHALL 含完整的 `Vec<Chunk>`（与未裁剪时 `Process.messages` 内容一致）

#### Scenario: subagent jsonl 不存在

- **WHEN** caller 调用 `get_subagent_trace`，但目标 jsonl 不存在
- **THEN** 响应 SHALL 为空 `[]`，不报错（与"不存在"等价于"无 trace"——caller UI 显示空 trace 即可）

#### Scenario: 嵌套 subagent 各自独立拉取

- **WHEN** SubagentCard A 展开后含嵌套 SubagentCard B；用户展开 B
- **THEN** 前端 SHALL 用 B 的 sessionId 单独调 `get_subagent_trace(rootSessionId, B.sessionId)`，不复用 A 的结果

#### Scenario: 跨 project_dir 定位 subagent jsonl
- **WHEN** caller 调用 `get_subagent_trace("parent-uuid", "sub-uuid")`，subagent jsonl 物理位于非主 `project_dir`（例如 `<projects_dir>/B/parent-uuid/subagents/agent-sub-uuid.jsonl`）
- **THEN** 后端 SHALL 跨 `project_dir` 扫描定位到 B 下的 jsonl，并返回完整 `Vec<Chunk>`

#### Scenario: 返回的 trace 把嵌套 Agent 调用暴露为可展开 subagent

- **WHEN** caller 调用 `get_subagent_trace("root-uuid", "sub-a")`，`sub-a` 的 transcript 内含一个 `Agent` `ToolExecution`，其 `result_agent_id = "sub-b"`
- **THEN** 返回的 `Vec<Chunk>` 中对应 `AIChunk.subagents` SHALL 含一个 `session_id="sub-b"`、`messagesOmitted=true`、`parentTaskId` 为该 Agent 调用 `tool_use_id` 的骨架 subagent
- **AND** 前端据此可对 `sub-b` 再调 `get_subagent_trace("root-uuid", "sub-b")` 继续向下展开
