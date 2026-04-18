## MODIFIED Requirements

### Requirement: Extract semantic steps for AIChunks

The system SHALL extract a list of `SemanticStep` (thinking、text output、tool execution、subagent spawn、interruption) from each `AIChunk` in chronological order for UI visualization. `Thinking` 与 `Text` 步骤从 `ParsedMessage.content` 中按 block 顺序抽取；`ToolExecution` 步骤以 `tool_use_id` + `tool_name` + `timestamp` 的形式生成，与 `AIChunk.tool_executions` 里的条目一一对应（可通过 `tool_use_id` 交叉查找真实 `ToolExecution`）；`Interruption` 变体由 `Emit interruption semantic step for interrupt-marker messages` Requirement 负责产出。

当 `build_chunks_with_subagents` 有已解析的 subagent 时，`SubagentSpawn` 步骤 MUST 按 subagent 对应的 Task `tool_use_id` 查找 `semantic_steps` 中的同 id `ToolExecution`，并被 insert 在该 step 之后（相邻位置）；SHALL NOT 统一追加到末尾。若找不到对应 Task `ToolExecution`（异常兜底），MAY 退化为 append 到末尾并记录一条 `tracing::warn!`。

#### Scenario: AIChunk with thinking + text + tool
- **WHEN** an assistant response contains a thinking block, a text block, then a `tool_use`
- **THEN** the semantic steps SHALL be emitted in that exact order: `Thinking` → `Text` → `ToolExecution`

#### Scenario: SubagentSpawn step inserted after the matching Task ToolExecution
- **WHEN** an `AIChunk` 的 `semantic_steps` 依次包含 `ToolExecution(Read)` → `ToolExecution(Task, tool_use_id=t_task)` → `ToolExecution(Grep)`，且对应 `Task` 解析出一个 subagent
- **THEN** 最终 `semantic_steps` 顺序 MUST 为 `ToolExecution(Read)` → `ToolExecution(Task, t_task)` → `SubagentSpawn(placeholder=subagent.session_id)` → `ToolExecution(Grep)`

#### Scenario: 多个 Task 各自插入对应 subagent
- **WHEN** 一个 AIChunk 的 responses 中依次出现 `Task(t1)`、`Task(t2)`，分别匹配到 subagent A / B
- **THEN** `semantic_steps` 中 `SubagentSpawn(A)` MUST 紧随 `ToolExecution(Task, t1)`，`SubagentSpawn(B)` MUST 紧随 `ToolExecution(Task, t2)`

#### Scenario: 未解析的 Task 不产生 SubagentSpawn
- **WHEN** 某个 `Task` `tool_use` 的 `Resolution::Orphan`
- **THEN** 该 Task 的 `ToolExecution` 保留在 `semantic_steps` 中，其后 SHALL NOT 出现与它对应的 `SubagentSpawn`
