## MODIFIED Requirements

### Requirement: Compute per-chunk metrics

Each chunk SHALL expose `timestamp`、可选的 `duration` 和 `metrics`，其中 `ChunkMetrics` 包含：`input_tokens`、`output_tokens`、`cache_creation_tokens`、`cache_read_tokens`、`tool_count` 与可选 `cost_usd`。

在 `team-coordination-metadata` 能力把端到端 subagent 候选装载接入 `build_chunks` 默认路径之前，`tool_count` SHALL 统计 `AIChunk.responses` 中所有 `tool_use` 块（包含 `Task` 调用）；`cost_usd` SHALL 取 `None`。`team-coordination-metadata` 移植完成后，`tool_count` 将按 Task 过滤语义修正，`cost_usd` 在价格表引入后改为 `Some(_)`。

#### Scenario: AIChunk with multiple tool uses
- **WHEN** an `AIChunk` contains 3 `tool_use` blocks across its assistant responses
- **THEN** its `metrics.tool_count` SHALL equal 3

#### Scenario: UserChunk without token usage
- **WHEN** a `UserChunk` has no usage data
- **THEN** its metrics token fields SHALL all be zero and `cost_usd` SHALL be `None`

#### Scenario: UserChunk duration is unset
- **WHEN** a `UserChunk` is emitted from exactly one `ParsedMessage`
- **THEN** its `duration` SHALL be `None`

### Requirement: Link tool uses to tool results

`AIChunk` SHALL 在默认的 `build_chunks` 路径中暴露已填充的 `tool_executions: Vec<ToolExecution>` 字段：每个 `tool_use` 块都对应一条 `ToolExecution` 记录（由 `tool-execution-linking` capability 产出），未配对的 `tool_use` 以 orphan 形式保留（`output = Missing`、`end_ts = None`）。

#### Scenario: Tool executions populated by build_chunks
- **WHEN** `build_chunks` runs over a session containing assistant `tool_use` blocks
- **THEN** each owning `AIChunk.tool_executions` SHALL contain one `ToolExecution` per `tool_use`, distributed according to the originating assistant message `uuid`

#### Scenario: Tool result appears in a later user message
- **WHEN** an assistant `tool_use` is followed by a user message carrying its matching `tool_result`
- **THEN** the corresponding `AIChunk.tool_executions` entry SHALL expose both `start_ts` and `end_ts` and preserve the result content as `output`

#### Scenario: Tool use with no matching result (orphan)
- **WHEN** an assistant `tool_use` has no matching `tool_result` in the session
- **THEN** the `AIChunk.tool_executions` entry SHALL have `output = Missing`, `end_ts = None`, and `is_error = false`, without panic

### Requirement: Filter Task tool uses when subagent data is available

`AIChunk.tool_executions` SHALL 在 subagent 候选可用时省略已解析的 `Task` `tool_use`；orphaned Task 调用（没有匹配 subagent）SHALL 被保留。chunk-building capability 自身不调用 filter——纯函数 `tool_linking::filter_resolved_tasks` 在 `port-tool-execution-linking` 内实现并受独立测试覆盖，而把 filter 接入 `build_chunks` 默认路径、装载 subagent 候选、同步更新 `ChunkMetrics::tool_count` 的工作由 `team-coordination-metadata` 完成。

#### Scenario: Task call with resolved subagent
- **WHEN** a `Task` `tool_use` has a matching subagent entry in the caller-supplied candidate pool
- **THEN** `tool_linking::filter_resolved_tasks` SHALL remove that entry from a mutable `Vec<ToolExecution>`, leaving only non-Task executions and orphan Task calls

#### Scenario: Task call with no matching subagent
- **WHEN** a `Task` `tool_use` resolves to `Resolution::Orphan`
- **THEN** it SHALL remain in the `AIChunk.tool_executions` list

#### Scenario: Default build_chunks does not filter Tasks in this port
- **WHEN** `build_chunks` is invoked without passing any subagent candidate pool
- **THEN** Task tool executions SHALL remain in `AIChunk.tool_executions`, and downstream consumers MAY still invoke `filter_resolved_tasks` explicitly; end-to-end default-path filtering is deferred to `team-coordination-metadata`

### Requirement: Extract semantic steps for AIChunks

The system SHALL extract a list of `SemanticStep` (thinking、text output、tool execution、subagent spawn) from each `AIChunk` in chronological order for UI visualization. `Thinking` 与 `Text` 步骤从 `ParsedMessage.content` 中按 block 顺序抽取；`ToolExecution` 步骤以 `tool_use_id` + `tool_name` + `timestamp` 的形式生成，与 `AIChunk.tool_executions` 里的条目一一对应（可通过 `tool_use_id` 交叉查找真实 `ToolExecution`）；`SubagentSpawn` 变体先保留但不产出，留给 `team-coordination-metadata` 填充。

#### Scenario: AIChunk with thinking + text + tool
- **WHEN** an assistant response contains a thinking block, a text block, then a `tool_use`
- **THEN** the semantic steps SHALL be emitted in that exact order: `Thinking` → `Text` → `ToolExecution`

#### Scenario: SubagentSpawn step is reserved but not yet emitted
- **WHEN** chunk-building runs without the downstream subagent capability
- **THEN** no `SemanticStep::SubagentSpawn` SHALL be emitted, and the enum variant SHALL remain available for later ports
