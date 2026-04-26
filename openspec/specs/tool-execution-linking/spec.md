# tool-execution-linking Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Pair tool_use with tool_result by id

The system SHALL pair every `tool_use` block with its corresponding `tool_result` block by matching `tool_use_id`, regardless of how many messages separate them. 配对算法 SHALL 为纯同步函数，单次遍历输入消息即可完成；无匹配的 `tool_use` SHALL 作为 orphan 保留，标记 `output = Missing` 且 `end_ts = None`，不抛错。

#### Scenario: Immediate result
- **WHEN** a `tool_use` is followed in the next user message by a `tool_result` with matching id
- **THEN** the pair SHALL be linked and expose a `ToolExecution` record with both start and end timestamps

#### Scenario: Delayed result
- **WHEN** a `tool_use` is followed by several other messages before its `tool_result` appears
- **THEN** the pair SHALL still be linked once the matching result is seen

#### Scenario: Duplicate result ids
- **WHEN** two `tool_result` blocks share the same `tool_use_id`
- **THEN** the system SHALL link the first encountered result, increment a `duplicates_dropped` counter in the returned `ToolLinkingResult`, and emit `tracing::warn!` with the offending id

#### Scenario: Orphan tool_use has no matching result
- **WHEN** an assistant `tool_use` has no matching `tool_result` anywhere in the session
- **THEN** the system SHALL emit a `ToolExecution` record with `output = Missing`, `end_ts = None`, and `is_error = false`, and SHALL NOT panic

### Requirement: Build tool execution records with error state

Each linked pair SHALL produce a `ToolExecution` record exposing: `tool_use_id`、`tool_name`、`input`、`output`（`Text` / `Structured` / `Missing`）、`is_error`、`start_ts`（assistant 消息时间）、`end_ts`（tool_result 消息时间，orphan 为 `None`）、`source_assistant_uuid`（用于回填所属 `AIChunk`）。

#### Scenario: Tool returned an error
- **WHEN** the `tool_result` has `is_error = true`
- **THEN** the `ToolExecution` record SHALL set `is_error = true` and preserve the error content verbatim as `output`

#### Scenario: Bash tool with stdout and stderr
- **WHEN** the `tool_result` content is a structured JSON object carrying both stdout and stderr streams
- **THEN** the record SHALL store the original JSON as `ToolOutput::Structured` without flattening or discarding streams

#### Scenario: Text tool_result
- **WHEN** the `tool_result` content is a plain string (legacy shape)
- **THEN** the record SHALL store it as `ToolOutput::Text`

### Requirement: Resolve Task subagents with three-phase fallback matching

The system SHALL resolve `Task` tool calls to their corresponding subagent sessions using a three-phase fallback strategy, in order, implemented as a pure synchronous function over externally supplied candidates:

1. **Result-based**: 若 Task 对应的 `ToolExecution.output` 是结构化 JSON 且包含 `teammate_spawned` 或 `session_id` 字段，直接从 `candidates` 中按 session id 取出 `Process`。
2. **Description-based**: 用 Task 的 `description` 与 `candidate.description_hint` 做匹配，要求 `|task_ts − candidate.spawn_ts|` 落在 60 秒窗口内；若某 Task 唯一匹配到一个 candidate 则 link。
3. **Positional**: 若 phase 2 结束仍有未分配 Task 且"未分配 Task 数 == 未分配 candidate 数"，则按 spawn order 一一配对。

Unresolved Task calls SHALL remain as `Resolution::Orphan`。候选集合的装载不属本 capability——它由下游能力（例如 `project-discovery` 与 `team-coordination-metadata`）负责预过滤后传入。

#### Scenario: teammate_spawned result links directly
- **WHEN** a `Task` call has a matching `ToolExecution` whose structured output carries a `teammate_spawned` hint with subagent session id, and that session id is present in `candidates`
- **THEN** the function SHALL return `Resolution::ResultBased(Process)` for that task without evaluating later phases

#### Scenario: No result-based link, description matches one subagent
- **WHEN** the `Task` call has no usable `teammate_spawned` hint, and its description uniquely matches exactly one candidate within the 60s spawn-time window
- **THEN** the function SHALL return `Resolution::DescriptionBased(Process)`

#### Scenario: Description ambiguous, positional fallback applies
- **WHEN** description-based matching yields zero unique matches but the count of unresolved Task calls equals the count of unresolved candidates
- **THEN** the function SHALL return `Resolution::Positional(Process)` for each in spawn order

#### Scenario: Task call matches no subagent
- **WHEN** all three phases fail to produce a match for a given Task call
- **THEN** the function SHALL return `Resolution::Orphan` for that task, and the corresponding `ToolExecution` SHALL be retained as-is

#### Scenario: Unrelated candidate does not trigger positional match
- **WHEN** a Task call has no description match, and the candidate pool contains subagent sessions belonging to unrelated parents such that the equality check fails
- **THEN** the function SHALL NOT positionally link and SHALL return `Resolution::Orphan`

### Requirement: Enrich subagent processes with team metadata

The system SHALL enrich Process records with `team` metadata (teamName, memberName, memberColor) when the spawning Task input or the `teammate_spawned` tool result carries team information. 此外，SHALL 填充以下 UI 必需字段，用于对齐原版 `SubagentItem` 视觉：

- `subagent_type: Option<String>`：从 Task tool_use `input.subagent_type` 字段读取（例如 `"code-reviewer"`）；未声明时 `None`
- `messages: Vec<Chunk>`：将 subagent session 的 `ParsedMessage` 流通过 `build_chunks` 转换后写入，用于前端内联展示 ExecutionTrace
- `main_session_impact: Option<MainSessionImpact>`：记录此 subagent 对父 session 贡献的 token 合计（来自 parent session 内 Task tool_result 的 usage 聚合）；新结构体字段为 `total_tokens: u64`（本次实现仅此一字段，为未来 `breakdown` 预留）
- `is_ongoing: bool` MUST 由装载层在 `parse_file(path)` 得到 `Vec<ParsedMessage>` 后立即调用 `cdt_analyze::check_messages_ongoing(&msgs)` 计算得出，与主 session `get_session_detail.isOngoing` / `extract_session_metadata` 走同一套五信号活动栈算法（text / interrupt / ExitPlanMode / tool rejection / SendMessage shutdown_response）。**禁止**仅用 `end_ts.is_none()` 或 "末行 timestamp > 首行 → done" 之类的时间戳简化判定——subagent 中断后无 assistant 收尾（例如末尾 `user/tool_result` 但无后续 assistant response）时时间戳简化判定会误判 done，导致 UI `SubagentCard` 错显 ✓。resolver 层 `compute_is_ongoing(cand) = cand.is_ongoing || cand.end_ts.is_none()` OR 兜底保留——装载层判 true 时强制 ongoing；判 false 时仍允许 `end_ts=None` 兜底（parse 失败 / 空 session 等 edge case）
- `duration_ms: Option<u64>`：`end_ts - spawn_ts` 的毫秒差；未结束时 `None`。**注意**：`end_ts` 仍按 JSONL 末行 timestamp 填充，与 `is_ongoing` 判定独立——`duration_ms` 是"已流逝时长"，与"是否仍在跑"是两回事
- `parent_task_id: Option<String>`：关联的 Task/Agent tool_use 的 `tool_use_id`，由 `resolve_subagents` 在匹配成功时回填
- `description: Option<String>`：Task tool_use 的 `input.description` 字段（独立于 `root_task_description`，后者保留为 subagent session root prompt）

#### Scenario: Team member spawned via TaskCreate
- **WHEN** a subagent was spawned via a TaskCreate call carrying team metadata
- **THEN** the Process.team SHALL be populated with the team name, member name, and color

#### Scenario: subagent_type 从 Task input 抽取
- **WHEN** Task tool_use input 包含 `subagent_type: "code-reviewer"`
- **THEN** 对应 Process SHALL 设置 `subagent_type = Some("code-reviewer".into())`

#### Scenario: messages 字段填充 subagent session chunks
- **WHEN** resolver 成功匹配到 subagent session，其 ParsedMessage 数量 > 0
- **THEN** Process.messages SHALL 为 `build_chunks(&subagent_parsed_messages)` 的结果；空 session 时 messages SHALL 为空数组

#### Scenario: parent_task_id 回填
- **WHEN** resolver 通过任一 phase 匹配到 Process
- **THEN** Process.parent_task_id SHALL 设置为触发匹配的 Task/Agent tool_use 的 `tool_use_id`

#### Scenario: duration_ms 计算
- **WHEN** subagent session 有 spawn_ts 与 end_ts
- **THEN** Process.duration_ms SHALL = `(end_ts - spawn_ts).num_milliseconds() as u64`

#### Scenario: is_ongoing 判定走 check_messages_ongoing 算法
- **WHEN** subagent session 的 `Vec<ParsedMessage>` 经 `cdt_analyze::check_messages_ongoing` 判定仍在进行（末尾活动栈里最后一个 ending 信号之后仍有 AI 活动，或从未出现 ending 但有 AI 活动）
- **THEN** `SubagentCandidate.is_ongoing` MUST 为 `true`，对应 `Process.is_ongoing` 经 resolver OR 兜底后 MUST 为 `true`

#### Scenario: orphan tool_result without assistant reply → is_ongoing=true
- **WHEN** subagent JSONL 末尾是一条 `user` 消息含 `tool_result`，但其前驱 `assistant` 发出的 `tool_use` 无后续 `assistant` 收尾（即 subagent 被中断前 Claude 已执行 tool，但未在 result 之后继续发 text / ExitPlanMode / shutdown_response）
- **THEN** 即使 JSONL 末行 timestamp 晚于首行（按时间戳简化判定会判 done），`is_ongoing` MUST 为 `true`（对齐主 session `check_messages_ongoing` 的五信号算法）

#### Scenario: 装载层与主 session ongoing 判定一致
- **WHEN** 同一份 `Vec<ParsedMessage>` 被传给主 session 路径（`get_session_detail`→`check_messages_ongoing`）和 subagent 装载路径（`parse_subagent_candidate`→`check_messages_ongoing`）
- **THEN** 两路返回的 `is_ongoing` 值 MUST 相等（装载层与主 session 走同一算法）

#### Scenario: main_session_impact 聚合
- **WHEN** parent session 中与此 subagent 对应的 Task tool_result 携带 usage（`input_tokens` + `output_tokens` + `cache_*`）
- **THEN** Process.main_session_impact.total_tokens SHALL 为该 usage 四项之和

### Requirement: Format readable summaries for team coordination tools

The system SHALL produce a short human-readable summary string for every team coordination tool (TeamCreate, TaskCreate, TaskUpdate, TaskList, TaskGet, SendMessage, TeamDelete), capturing the most salient parameters.

#### Scenario: SendMessage with recipient and body
- **WHEN** a SendMessage tool_use has a `to` and a `message` parameter
- **THEN** the summary SHALL include both the recipient and a truncated message preview

### Requirement: Detect teammate-spawned tool results

`tool_linking::pair` 在配对 `tool_use` 与对应 user 消息的 `tool_result` 时 MUST 检测 user 消息顶层 `toolUseResult.status` 字段。当 `status == "teammate_spawned"` 时，从 `toolUseResult` 抽出 `name` 与 `color` 字段封装为 `cdt_core::TeammateSpawnInfo` 并赋给 `ToolExecution.teammate_spawn`。其它情况 `teammate_spawn` SHALL 保持 `None`。

`TeammateSpawnInfo.name` MUST 来自 `toolUseResult.name`（必填，命中即必有）。`TeammateSpawnInfo.color` MUST 来自 `toolUseResult.color`（可选，缺失时 `None`）。

UI 端按此字段决定渲染：非空时把整条 `tool_execution` displayItem 替换为 `teammate_spawn` 极简单行（圆点 + member-X badge + "Teammate spawned" 文案），对齐原版 `claude-devtools/src/renderer/components/chat/items/LinkedToolItem.tsx::isTeammateSpawned`；为空时保留普通 tool item 渲染。

序列化 SHALL 用 camelCase（`teammateSpawn`），`#[serde(skip_serializing_if = "Option::is_none")]` 让无 spawn 信息的 tool execution IPC payload 不含此字段，老前端兼容。

#### Scenario: Status teammate_spawned populates TeammateSpawnInfo
- **WHEN** user 消息 `tool_use_result` 为 `{"status":"teammate_spawned","name":"member-1","color":"blue"}`，对应 `tool_use_id` 配对到一条 `Agent` tool use
- **THEN** 配对产出的 `ToolExecution.teammate_spawn` SHALL 为 `Some(TeammateSpawnInfo { name: "member-1", color: Some("blue") })`

#### Scenario: Status teammate_spawned without color
- **WHEN** `tool_use_result` 为 `{"status":"teammate_spawned","name":"member-2"}`（无 color 字段）
- **THEN** `ToolExecution.teammate_spawn` SHALL 为 `Some(TeammateSpawnInfo { name: "member-2", color: None })`

#### Scenario: Other status values leave teammate_spawn None
- **WHEN** `tool_use_result.status` 为其它值（如 `"ok"`、缺失或非字符串）
- **THEN** `ToolExecution.teammate_spawn` SHALL 为 `None`

#### Scenario: No tool_use_result leaves teammate_spawn None
- **WHEN** user 消息无 `toolUseResult` 顶层字段
- **THEN** 配对产出的 `ToolExecution.teammate_spawn` SHALL 为 `None`

#### Scenario: Empty teammate_spawn omitted from IPC payload
- **WHEN** `ToolExecution.teammate_spawn = None`
- **THEN** 序列化 JSON SHALL 不含 `teammateSpawn` 键（`skip_serializing_if = "Option::is_none"` 控制）

