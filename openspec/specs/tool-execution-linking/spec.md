# tool-execution-linking Specification

## Purpose

把 assistant 消息中的 `tool_use` 块与 user 消息中的 `tool_result` 块按 `tool_use_id` 配对，产出 `ToolExecution` 记录（含 `is_error`、`output`、起止时间戳）；为 `Task` 工具调用通过三阶段 fallback 识别对应 subagent session 并把 Process 的 `team` 元数据、`subagent_type`、`messages`、`is_ongoing` 等字段填齐；为团队协作工具产出可读 summary。本 capability 是 chunk-building 与 UI subagent 卡片渲染的共享数据来源。

## Requirements

### Requirement: Pair tool_use with tool_result by id

系统 SHALL 把每个 `tool_use` 块与同 `tool_use_id` 的 `tool_result` 块配对，无视两者间隔多少条消息。配对算法 SHALL 为纯同步函数，对输入消息单次遍历即可完成；无匹配的 `tool_use` SHALL 作为 orphan 保留，标记 `output = Missing` 且 `end_ts = None`，不抛错。

#### Scenario: Immediate result
- **WHEN** 一个 `tool_use` 紧接的下一条 user 消息含同 id 的 `tool_result`
- **THEN** 这对 SHALL 被链接，并产出含起止时间戳的 `ToolExecution` 记录

#### Scenario: Delayed result
- **WHEN** `tool_use` 后还间隔了若干消息才出现其 `tool_result`
- **THEN** 一旦匹配到对应 result，配对 SHALL 仍然成立

#### Scenario: Duplicate result ids
- **WHEN** 两个 `tool_result` 块共享同一 `tool_use_id`
- **THEN** 系统 SHALL 链接首条遇到的 result，把返回的 `ToolLinkingResult` 中 `duplicates_dropped` 计数加 1，并以 `tracing::warn!` 上报这个 id

#### Scenario: Orphan tool_use has no matching result
- **WHEN** 一条 assistant `tool_use` 在整个 session 中无任何 `tool_result` 与之匹配
- **THEN** 系统 SHALL 产出一条 `ToolExecution` 记录，`output = Missing`、`end_ts = None`、`is_error = false`，SHALL NOT panic

### Requirement: Build tool execution records with error state

每对已链接的 `tool_use` / `tool_result` SHALL 产出一条 `ToolExecution` 记录，暴露：`tool_use_id`、`tool_name`、`input`、`output`（`Text` / `Structured` / `Missing`）、`is_error`、`start_ts`（assistant 消息时间）、`end_ts`（tool_result 消息时间，orphan 为 `None`）、`source_assistant_uuid`（用于回填所属 `AIChunk`）。

#### Scenario: Tool returned an error
- **WHEN** `tool_result` 的 `is_error = true`
- **THEN** `ToolExecution` 记录 SHALL 设 `is_error = true`，并把错误内容原样保留到 `output`

#### Scenario: Bash tool with stdout and stderr
- **WHEN** `tool_result` content 是携带 stdout 与 stderr 流的结构化 JSON 对象
- **THEN** 记录 SHALL 把原始 JSON 存为 `ToolOutput::Structured`，不展平、不丢流

#### Scenario: Text tool_result
- **WHEN** `tool_result` content 是纯字符串（旧版形态）
- **THEN** 记录 SHALL 把内容存为 `ToolOutput::Text`

### Requirement: Resolve Task subagents with three-phase fallback matching

系统 SHALL 用三阶段 fallback 策略把 `Task` 工具调用解析到对应 subagent session，按以下顺序，作为消费外部传入候选集的纯同步函数：

1. **Result-based**：若 Task 对应的 `ToolExecution.output` 是结构化 JSON 且包含 `teammate_spawned` 或 `session_id` 字段，直接从 `candidates` 中按 session id 取出 `Process`。
2. **Description-based**：用 Task 的 `description` 与 `candidate.description_hint` 做匹配，要求 `|task_ts − candidate.spawn_ts|` 落在 60 秒窗口内；若某 Task 唯一匹配到一个 candidate 则 link。
3. **Positional**：若 phase 2 结束后仍有未分配 Task 且"未分配 Task 数 == 未分配 candidate 数"，则按 spawn order 一一配对。

未解析的 Task 调用 SHALL 保留为 `Resolution::Orphan`。候选集合的装载不属本 capability——它由下游能力（例如 `project-discovery` 与 `team-coordination-metadata`）负责预过滤后传入。

#### Scenario: teammate_spawned result links directly
- **WHEN** 一个 `Task` 调用对应的 `ToolExecution` 的结构化 output 含 `teammate_spawned` hint 与 subagent session id，且该 session id 在 `candidates` 中存在
- **THEN** 函数 SHALL 直接返回 `Resolution::ResultBased(Process)`，不再评估后续阶段

#### Scenario: No result-based link, description matches one subagent
- **WHEN** 一个 `Task` 调用没有可用的 `teammate_spawned` hint，但其 description 在 60 秒 spawn 窗口内唯一匹配一个 candidate
- **THEN** 函数 SHALL 返回 `Resolution::DescriptionBased(Process)`

#### Scenario: Description ambiguous, positional fallback applies
- **WHEN** description 阶段无任何唯一匹配，但未解析 Task 数等于未解析 candidate 数
- **THEN** 函数 SHALL 按 spawn order 为每对返回 `Resolution::Positional(Process)`

#### Scenario: Task call matches no subagent
- **WHEN** 三阶段对某 Task 调用均无匹配
- **THEN** 函数 SHALL 对该 task 返回 `Resolution::Orphan`，对应 `ToolExecution` SHALL 原样保留

#### Scenario: Unrelated candidate does not trigger positional match
- **WHEN** Task 调用无 description 匹配，candidate 池含归属其它父 session 的 subagent，使等量 check 失败
- **THEN** 函数 SHALL NOT 走 positional 链接，SHALL 返回 `Resolution::Orphan`

### Requirement: Enrich subagent processes with team metadata

系统 SHALL 在 spawn 用的 Task input 或 `teammate_spawned` 工具结果含 team 信息时，把 Process 记录的 `team` 元数据（`teamName`、`memberName`、`memberColor`）填上。此外，SHALL 同时填充以下 UI 必需字段，以对齐原版 `SubagentItem` 视觉：

- `subagent_type: Option<String>`：从 Task tool_use `input.subagent_type` 字段读取（例如 `"code-reviewer"`）；未声明时为 `None`。
- `messages: Vec<Chunk>`：把 subagent session 的 `ParsedMessage` 流经 `build_chunks` 转换后写入，用于前端内联展示 ExecutionTrace。
- `main_session_impact: Option<MainSessionImpact>`：记录此 subagent 对父 session 贡献的 token 合计（来自 parent session 内 Task tool_result 的 usage 聚合）；本结构体当前仅含 `total_tokens: u64` 一个字段，其余 breakdown 字段留给后续扩展。
- `is_ongoing: bool` MUST 由装载层在 `parse_file(path)` 得到 `Vec<ParsedMessage>` 后立即调用 `cdt_analyze::check_messages_ongoing(&msgs)` 计算，与主 session `get_session_detail.isOngoing` / `extract_session_metadata` 走同一套五信号活动栈算法（text / interrupt / ExitPlanMode / tool rejection / SendMessage shutdown_response）。**禁止**仅用 `end_ts.is_none()` 或"末行 timestamp > 首行 → done"等时间戳简化判定——subagent 中断后无 assistant 收尾（例如末尾 `user/tool_result` 但无后续 assistant response）时，时间戳简化判定会误判 done，导致 UI `SubagentCard` 错显 ✓。resolver 层 `compute_is_ongoing(cand) = cand.is_ongoing || cand.end_ts.is_none()` 的 OR 兜底保留——装载层判 `true` 时强制 ongoing；判 `false` 时仍允许 `end_ts=None` 兜底（parse 失败 / 空 session 等 edge case）。
- `duration_ms: Option<u64>`：`end_ts - spawn_ts` 的毫秒差；未结束时为 `None`。**注意**：`end_ts` 仍按 JSONL 末行 timestamp 填充，与 `is_ongoing` 判定独立——`duration_ms` 是"已流逝时长"，与"是否仍在跑"是两件事。
- `parent_task_id: Option<String>`：关联的 Task / Agent tool_use 的 `tool_use_id`，由 `resolve_subagents` 在匹配成功时回填。
- `description: Option<String>`：Task tool_use 的 `input.description` 字段（独立于 `root_task_description`，后者保留为 subagent session root prompt）。

#### Scenario: Team member spawned via TaskCreate
- **WHEN** 一个 subagent 通过携带 team 元数据的 TaskCreate 调用 spawn
- **THEN** `Process.team` SHALL 被填上 team name、member name、color

#### Scenario: subagent_type 从 Task input 抽取
- **WHEN** Task tool_use input 含 `subagent_type: "code-reviewer"`
- **THEN** 对应 Process SHALL 设 `subagent_type = Some("code-reviewer".into())`

#### Scenario: messages 字段填充 subagent session chunks
- **WHEN** resolver 成功匹配到 subagent session 且其 ParsedMessage 数 > 0
- **THEN** `Process.messages` SHALL 为 `build_chunks(&subagent_parsed_messages)` 的结果；空 session 时 messages SHALL 为空数组

#### Scenario: parent_task_id 回填
- **WHEN** resolver 通过任一 phase 匹配到 Process
- **THEN** `Process.parent_task_id` SHALL 被设置为触发匹配的 Task / Agent tool_use 的 `tool_use_id`

#### Scenario: duration_ms 计算
- **WHEN** subagent session 同时有 spawn_ts 与 end_ts
- **THEN** `Process.duration_ms` SHALL 等于 `(end_ts - spawn_ts).num_milliseconds() as u64`

#### Scenario: is_ongoing 判定走 check_messages_ongoing 算法
- **WHEN** subagent session 的 `Vec<ParsedMessage>` 经 `cdt_analyze::check_messages_ongoing` 判定仍在进行（末尾活动栈中最后一个 ending 信号之后仍有 AI 活动，或从未出现 ending 但有 AI 活动）
- **THEN** `SubagentCandidate.is_ongoing` MUST 为 `true`，对应 `Process.is_ongoing` 经 resolver OR 兜底后 MUST 为 `true`

#### Scenario: orphan tool_result without assistant reply → is_ongoing=true
- **WHEN** subagent JSONL 末尾是一条 `user` 消息含 `tool_result`，但其前驱 `assistant` 发出的 `tool_use` 之后无 assistant 收尾（即 subagent 被中断前 Claude 已执行 tool，但未在 result 之后继续发 text / ExitPlanMode / shutdown_response）
- **THEN** 即使 JSONL 末行 timestamp 晚于首行（按时间戳简化判定会判 done），`is_ongoing` MUST 为 `true`（对齐主 session `check_messages_ongoing` 的五信号算法）

#### Scenario: 装载层与主 session ongoing 判定一致
- **WHEN** 同一份 `Vec<ParsedMessage>` 被分别送入主 session 路径（`get_session_detail` → `check_messages_ongoing`）与 subagent 装载路径（`parse_subagent_candidate` → `check_messages_ongoing`）
- **THEN** 两路返回的 `is_ongoing` 值 MUST 相等（装载层与主 session 走同一算法）

#### Scenario: main_session_impact 聚合
- **WHEN** parent session 中与此 subagent 对应的 Task tool_result 携带 usage（`input_tokens` + `output_tokens` + `cache_*`）
- **THEN** `Process.main_session_impact.total_tokens` SHALL 等于该 usage 四项之和

### Requirement: Format readable summaries for team coordination tools

系统 SHALL 为每个团队协作工具（`TeamCreate`、`TaskCreate`、`TaskUpdate`、`TaskList`、`TaskGet`、`SendMessage`、`TeamDelete`）产出一条简短可读的 summary 字符串，捕捉最显著的参数。

#### Scenario: SendMessage with recipient and body
- **WHEN** 一次 `SendMessage` 工具调用含 `to` 与 `message` 参数
- **THEN** summary SHALL 同时含 recipient 与截断后的 message 预览

### Requirement: Detect teammate-spawned tool results

`tool_linking::pair` 在配对 `tool_use` 与对应 user 消息的 `tool_result` 时 MUST 检查 user 消息顶层 `toolUseResult.status` 字段。当 `status == "teammate_spawned"` 时，从 `toolUseResult` 抽出 `name` 与 `color` 字段封装为 `cdt_core::TeammateSpawnInfo` 并赋给 `ToolExecution.teammate_spawn`；其它情况 `teammate_spawn` SHALL 保持 `None`。

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
- **WHEN** `tool_use_result.status` 为其它值（例如 `"ok"`、缺失或非字符串）
- **THEN** `ToolExecution.teammate_spawn` SHALL 为 `None`

#### Scenario: No tool_use_result leaves teammate_spawn None
- **WHEN** user 消息无顶层 `toolUseResult` 字段
- **THEN** 配对产出的 `ToolExecution.teammate_spawn` SHALL 为 `None`

#### Scenario: Empty teammate_spawn omitted from IPC payload
- **WHEN** `ToolExecution.teammate_spawn = None`
- **THEN** 序列化 JSON SHALL 不含 `teammateSpawn` 键（由 `skip_serializing_if = "Option::is_none"` 控制）
