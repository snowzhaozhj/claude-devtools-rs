## ADDED Requirements

### Requirement: 骨架 subagent 渲染与懒拉状态纠正

当 `get_subagent_trace` 返回的子 transcript 内含由 `result_agent_id` 升级出的**骨架 subagent**(`messagesOmitted=true`、`messagesTotalCount=0`、`isOngoing=false`、`messages` 为空)时,前端 SHALL 沿用既有 `Subagent 内联展开 ExecutionTrace` 的递归渲染路径把它渲染为可展开的内联 `SubagentCard`,并复用既有 `rootSessionId` 向下传递与深度上限保护。骨架 subagent 与完整 resolve 的 subagent 在渲染路径上无差别——用户首次展开时 MUST 用骨架的 `sessionId` 调 `getSubagentTrace(rootSessionId, sessionId)` 懒拉下一层,使嵌套层可逐级深入。

骨架 subagent 的状态字段是**已知降级**:展开前其 header 依据 `isOngoing=false` 显示为已完成,即使该嵌套 subagent 实际仍在运行。首次展开懒拉到真实 trace 后,状态 SHALL 以懒拉结果为准。该降级是契约的一部分,SHALL 由测试固定,而非按 bug 处理。

#### Scenario: 骨架 subagent 首次展开触发懒拉

- **WHEN** 一个展开后的 SubagentCard 的 ExecutionTrace 内含一个骨架 subagent(`messagesOmitted=true`),用户点击展开它
- **THEN** 前端 SHALL 用该骨架的 `sessionId` 调 `getSubagentTrace(rootSessionId, sessionId)` 拉取其完整 trace 并缓存
- **AND** 展开前 SHALL NOT 因骨架 `messages` 为空而渲染成永久空白——懒拉结果填充后 SHALL 正常渲染其执行链

#### Scenario: 骨架状态以懒拉结果为准

- **WHEN** 一个真实仍在运行的嵌套 subagent 以骨架形式(`isOngoing=false`)出现在父执行链中
- **THEN** 展开前其 header MAY 显示为已完成(已知降级)
- **AND** 用户展开懒拉到完整 trace 后,SubagentCard SHALL 依据真实 trace 重新计算并展示其运行 / 完成状态
