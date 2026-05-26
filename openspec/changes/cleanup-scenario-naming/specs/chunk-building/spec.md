# chunk-building Spec Delta

## MODIFIED Requirements

### Requirement: Link tool uses to tool results

`AIChunk` SHALL 在默认 `build_chunks` 路径中暴露已填充的 `tool_executions: Vec<ToolExecution>` 字段：每个 `tool_use` 块都对应一条 `ToolExecution` 记录（由 `tool-execution-linking` capability 产出），未配对的 `tool_use` 以 orphan 形式保留（`output = Missing`、`end_ts = None`）。

#### Scenario: Tool executions populated during chunk build
- **WHEN** `build_chunks` 跑完一段含 assistant `tool_use` 块的 session
- **THEN** 每个所属 `AIChunk.tool_executions` SHALL 为每个 `tool_use` 各含一条 `ToolExecution`，按起源 assistant 消息 `uuid` 分发

#### Scenario: Tool result appears in a later user message
- **WHEN** 一条 assistant `tool_use` 后跟一条 user 消息含其匹配的 `tool_result`
- **THEN** 对应 `AIChunk.tool_executions` 条目 SHALL 同时暴露 `start_ts` 与 `end_ts`，并把 result content 保留为 `output`

#### Scenario: Tool use with no matching result (orphan)
- **WHEN** 一条 assistant `tool_use` 在 session 中无任何 `tool_result` 匹配
- **THEN** 对应 `AIChunk.tool_executions` 条目 SHALL 设 `output = Missing`、`end_ts = None`、`is_error = false`，不 panic

### Requirement: Filter Task tool uses when subagent data is available

`AIChunk.tool_executions` SHALL 在 subagent 候选可用时省略已解析的 `Task` `tool_use`；orphan Task（无匹配 subagent）SHALL 保留。chunk-building capability 自身不调用 filter——纯函数 `tool_linking::filter_resolved_tasks` 在 `port-tool-execution-linking` 内实现并由独立测试覆盖；把 filter 接入 `build_chunks` 默认路径、装载 subagent 候选、同步更新 `ChunkMetrics::tool_count` 的工作由 `team-coordination-metadata` 完成。

#### Scenario: Task call with resolved subagent
- **WHEN** 一条 `Task` `tool_use` 在调用方传入的 candidate 池中匹配到 subagent
- **THEN** `tool_linking::filter_resolved_tasks` SHALL 把该条目从可变 `Vec<ToolExecution>` 中移除，仅保留非 Task 与 orphan Task

#### Scenario: Task call with no matching subagent
- **WHEN** 一条 `Task` `tool_use` 解析为 `Resolution::Orphan`
- **THEN** 它 SHALL 保留在 `AIChunk.tool_executions` 列表中

#### Scenario: Default chunk build does not filter Tasks in this port
- **WHEN** `build_chunks` 调用时未传任何 subagent candidate 池
- **THEN** Task tool execution SHALL 保留在 `AIChunk.tool_executions`；下游消费者 MAY 仍显式调用 `filter_resolved_tasks`；端到端默认路径过滤推迟到 `team-coordination-metadata`
