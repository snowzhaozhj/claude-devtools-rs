## ADDED Requirements

### Requirement: Promote nested Agent calls to skeleton subagents

系统 SHALL 提供纯函数 `promote_result_agent_tasks(chunks)`，在一段**已构建**的 chunks 上,把每个携带 `result_agent_id` 的 `Agent` / `Task` `ToolExecution` 就地升级成一个**骨架 subagent** `Process`,attach 进其所属 `AIChunk.subagents`,并在 `semantic_steps` 中插入对应 `SubagentSpawn`。该后处理用于 `build_chunks_with_subagents` 不可用的路径(典型:`get_subagent_trace` 对单个 subagent transcript 调 `build_chunks` 之后),让嵌套 `Agent` 调用暴露为可展开 subagent 而非普通工具。workflow agent trace(`get_workflow_agent_trace`)**不**在本后处理范围——其嵌套子文件落 `subagents/workflows/`,递归懒拉的 `getSubagentTrace` 定位不到,接入会产"可展开但展开为空"的假骨架(理由见 design D8)。

骨架 `Process` 的字段 MUST 按以下策略合成,缺一不可:

- `session_id` = `ToolExecution.result_agent_id`
- `parent_task_id` = `Some(ToolExecution.tool_use_id)` —— 关键:消费方据此对原始 `Agent`/`Task` 工具 item 去重,缺失会导致同一调用既渲染骨架卡片又渲染原始工具
- `subagent_type` 取自该 `ToolExecution`(沿用现有 subagent_type 来源);`description` 取自调用 input 的 description,并 SHALL 截断到固定字符上限以防一层 fan-out 大量子节点时 payload 膨胀
- `spawn_ts` = `ToolExecution.start_ts`;`metrics` = 零值
- `messages` = 空;`messages_omitted` = `true`;`messages_total_count` = 0 / 缺省
- `is_ongoing` = `false`(已知降级:真实状态以消费方首次展开懒拉结果为准)

插入的 `SubagentSpawn` MUST 满足:`placeholder_id` 等于骨架 `Process.session_id`;且 MUST 按对应 `ToolExecution` 的 `tool_use_id` 在 `semantic_steps` 中查找同 id 的 `ToolExecution` step,被 insert 在该 step 之后(相邻位置),SHALL NOT 一律 append 到末尾——与既有 `build_chunks_with_subagents` 的 SubagentSpawn 插入顺序契约一致。

该函数 SHALL 是纯同步函数,不读文件、不发起 IO,只消费 chunks 内已解析的 `result_agent_id`。不带 `result_agent_id` 的 `ToolExecution`(普通工具或未 spawn 子 agent 的调用)SHALL 原样保留,不产骨架。

#### Scenario: result_agent_id 升级为骨架 subagent

- **WHEN** 一个 `AIChunk` 含一个 `Agent` `ToolExecution`,其 `result_agent_id = "sub-x"`,`tool_use_id = "toolu_1"`
- **THEN** `promote_result_agent_tasks` 后该 `AIChunk.subagents` SHALL 含一个骨架 `Process`,`session_id="sub-x"`、`parent_task_id=Some("toolu_1")`、`messages_omitted=true`、`is_ongoing=false`
- **AND** `semantic_steps` 中 SHALL 在该 `ToolExecution` step 之后相邻插入一个 `SubagentSpawn`,其 `placeholder_id="sub-x"`

#### Scenario: 无 result_agent_id 的工具不升级

- **WHEN** 一个 `AIChunk` 含一个普通 `Bash` `ToolExecution`(无 `result_agent_id`)
- **THEN** `promote_result_agent_tasks` 后该 `ToolExecution` SHALL 原样保留,`AIChunk.subagents` SHALL NOT 因它新增条目

#### Scenario: 骨架不与原始工具重复渲染

- **WHEN** 升级产生骨架 subagent(`parent_task_id=Some("toolu_1")`),消费方按 `parent_task_id` 命中去重原始 `Agent`/`Task` 工具
- **THEN** 同一 `toolu_1` 调用 SHALL 只表现为一个可展开 subagent,SHALL NOT 同时再渲染一个普通 Agent 工具块

#### Scenario: description 超长被截断

- **WHEN** 升级的 `Agent` 调用 description 长度超过字符上限
- **THEN** 骨架 `Process.description` SHALL 被截断到上限内,不携带完整长文本
