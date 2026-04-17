## MODIFIED Requirements

### Requirement: Enrich subagent processes with team metadata

The system SHALL enrich Process records with `team` metadata (teamName, memberName, memberColor) when the spawning Task input or the `teammate_spawned` tool result carries team information. 此外，SHALL 填充以下 UI 必需字段，用于对齐原版 `SubagentItem` 视觉：

- `subagent_type: Option<String>`：从 Task tool_use `input.subagent_type` 字段读取（例如 `"code-reviewer"`）；未声明时 `None`
- `messages: Vec<Chunk>`：将 subagent session 的 `ParsedMessage` 流通过 `build_chunks` 转换后写入，用于前端内联展示 ExecutionTrace
- `main_session_impact: Option<MainSessionImpact>`：记录此 subagent 对父 session 贡献的 token 合计（来自 parent session 内 Task tool_result 的 usage 聚合）；新结构体字段为 `total_tokens: u64`（本次实现仅此一字段，为未来 `breakdown` 预留）
- `is_ongoing: bool`：若 subagent session 最后一条 assistant 消息尚无配对 tool_result 且无 `end_ts`，则 `true`；否则 `false`
- `duration_ms: Option<u64>`：`end_ts - spawn_ts` 的毫秒差；未结束时 `None`
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

#### Scenario: is_ongoing 判定
- **WHEN** subagent session 的最后一条 assistant 消息不含终结标记且 `end_ts` 为 `None`
- **THEN** Process.is_ongoing SHALL 为 `true`

#### Scenario: main_session_impact 聚合
- **WHEN** parent session 中与此 subagent 对应的 Task tool_result 携带 usage（`input_tokens` + `output_tokens` + `cache_*`）
- **THEN** Process.main_session_impact.total_tokens SHALL 为该 usage 四项之和
