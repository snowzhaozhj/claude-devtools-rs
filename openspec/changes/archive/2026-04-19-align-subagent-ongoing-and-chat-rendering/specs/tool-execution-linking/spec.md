# tool-execution-linking Spec Delta

## MODIFIED Requirements

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
