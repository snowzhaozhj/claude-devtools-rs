## Why

Subagent 可以在自身执行链里再 spawn 子 subagent(`Agent` / `Task` 工具),真实会话里嵌套深度可达三层(样本 7f59237e:170 个 subagent,63 个 level-1 / 100 个 level-2 / 7 个 level-3)。但展开一个 subagent 时,它内部 spawn 出的子 subagent 只显示成一个普通工具块,无法展开查看其执行链——用户看不到嵌套层的真实工作。数据其实已经在磁盘上(子 transcript 与父执行链里子 agent 的 session id 都已被解析),只差把它接起来。

## What Changes

- 新增纯函数 `promote_result_agent_tasks(chunks)`(cdt-analyze):在一段已 build 的 chunks 上,把带有 `result_agent_id` 的 `Agent` / `Task` `ToolExecution` **就地升级**成一个"骨架 subagent"——合成最小 `Process`(只有 session id / 类型 / 描述,`messages` 为空且 `messages_omitted=true`),attach 进对应 `AIChunk.subagents` 并插入 `SubagentSpawn` 语义步骤;同时复用现有"过滤已 resolve 的 Task `ToolExecution`"去重,避免同一调用既渲染骨架卡片又渲染原始工具。
- `get_subagent_trace` IPC 在 `build_chunks` 之后调用该后处理,使返回给前端的子 transcript 把嵌套 `Agent` 调用暴露为可展开的骨架 subagent。
- 前端无需新增渲染逻辑——`ExecutionTrace` / `SubagentCard` 已支持递归渲染、`rootSessionId` 一路向下传递、`messagesOmitted` 首展开懒拉、深度上限保护。骨架 subagent 落入既有 `DisplayItem.type === "subagent"` 路径,用户首次点开时按子 session id 再调一次 `get_subagent_trace` 拉下一层。
- **零新文件 IO**:升级步骤只复用已解析出的 `result_agent_id`,不读取子文件、不额外 parse。展开一层 = 调一次今天已有的 `get_subagent_trace`,payload 与当前展开 level-1 完全相同路径,不触碰 `get_session_detail` 主路径。
- **状态降级(已确认接受)**:骨架的 `is_ongoing=false` / `messages_total_count` 缺省;展开前一个真在运行的嵌套 subagent 会短暂显示为已完成,用户首次展开懒拉后即纠正。以此换取"零新 IO",不引入每层逐子读首尾行的方案。

## Capabilities

### New Capabilities

(无新 capability)

### Modified Capabilities

- `chunk-building`: 新增"在已构建 chunks 上由 `result_agent_id` 升级骨架 subagent"的后处理 Requirement——定义骨架 `Process` 字段策略(`parent_task_id = tool_use_id`、`SubagentSpawn.placeholder_id = session_id`、`messages` 空 + `messages_omitted=true`、`description` 字符上限、`is_ongoing=false` 降级)、SubagentSpawn 紧随对应 `ToolExecution` 的插入顺序、以及对原 `Agent`/`Task` `ToolExecution` 的去重过滤。
- `ipc-data-api`: 修改 `Lazy load subagent trace` Requirement——`get_subagent_trace` 返回前 SHALL 对 chunks 执行骨架升级,使嵌套 `Agent` 调用在返回 payload 中表现为带 `messagesOmitted=true` 的可懒拉 subagent。
- `session-display`: 在 `Subagent 内联展开 ExecutionTrace` Requirement 下补充骨架 subagent 的渲染与懒拉 Scenario——骨架 `Process`(`messagesTotalCount=0` / `isOngoing=false`)首次展开 MUST 触发 `getSubagentTrace`,状态以懒拉结果为准。

## Impact

- **代码**:`crates/cdt-analyze`(新纯函数 `promote_result_agent_tasks` + 骨架工厂)、`crates/cdt-api/src/ipc/local.rs::get_subagent_trace`(调用后处理)。前端 `ui/src/components/ExecutionTrace.svelte` / `SubagentCard.svelte` 预期零改(复用既有递归路径),仅补测试。
- **IPC 契约**:`get_subagent_trace` 返回的 chunks 内 `AIChunk.subagents` 可能新增骨架条目;字段形态用既有 `Process` camelCase 契约,无新字段。`cdt-api/tests/ipc_contract.rs` 新增嵌套 Agent 骨架 round-trip 测试。
- **性能**:升级步骤无新 IO;不影响冷启动 / `get_session_detail` 预算。已知非阻塞 info:无 trace 级缓存,反复折叠重开会重复 parse 同一文件,留作后续可选优化。
- **范围外**:`find_subagent_jsonl` 兜底在同一 `project_dir` 下多 session 目录命中同名 `agent-<id>.jsonl` 时按 `read_dir` 顺序取第一份的歧义(main 既有隐患,嵌套场景会更频繁触发)——单独记 GitHub issue / 独立 PR 处理,不在本 change。
