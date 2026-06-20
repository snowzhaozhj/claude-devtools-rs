## ADDED Requirements

### Requirement: 骨架 subagent 渲染与懒拉状态纠正

当 `get_subagent_trace` 返回的子 transcript 内含由 `result_agent_id` 升级出的**骨架 subagent**(`messagesOmitted=true`、`messagesTotalCount=0`、`isOngoing=false`、`messages` 为空)时,前端 SHALL 沿用既有 `Subagent 内联展开 ExecutionTrace` 的递归渲染路径把它渲染为可展开的内联 `SubagentCard`,并复用既有 `rootSessionId` 向下传递与深度上限保护。骨架 subagent 与完整 resolve 的 subagent 在渲染路径上无差别——用户首次展开时 MUST 用骨架的 `sessionId` 调 `getSubagentTrace(rootSessionId, sessionId)` 懒拉下一层,使嵌套层可逐级深入。

骨架 subagent 的状态字段是**已知降级**:展开前其 header 依据 `isOngoing=false` 显示为已完成,即使该嵌套 subagent 实际仍在运行。首次展开懒拉到真实 trace 后,状态 SHALL 以懒拉结果为准。该降级是契约的一部分,SHALL 由测试固定,而非按 bug 处理。

为避免懒拉造成**始终可见的 card header 布局跳动**,SubagentCard header 的 model badge SHALL 只读后端预算的稳定字段 `Process.headerModel`,SHALL NOT 从懒拉后才填充的 `messages` 派生。骨架 `headerModel` 缺省 → header 不显示 model badge(展开前后一致,不会在展开瞬间突然冒出);真实 model 随展开 body 的 `Model` 详情行一并出现,属正常展开行为。完整 resolve 的(非骨架)subagent 在候选转换阶段已预算 `headerModel`,header model badge 不受影响。

#### Scenario: 骨架 subagent 首次展开触发懒拉

- **WHEN** 一个展开后的 SubagentCard 的 ExecutionTrace 内含一个骨架 subagent(`messagesOmitted=true`),用户点击展开它
- **THEN** 前端 SHALL 用该骨架的 `sessionId` 调 `getSubagentTrace(rootSessionId, sessionId)` 拉取其完整 trace 并缓存
- **AND** 展开前 SHALL NOT 因骨架 `messages` 为空而渲染成永久空白——懒拉结果填充后 SHALL 正常渲染其执行链

#### Scenario: 骨架状态以懒拉结果为准

- **WHEN** 一个真实仍在运行的嵌套 subagent 以骨架形式(`isOngoing=false`)出现在父执行链中
- **THEN** 展开前其 header MAY 显示为已完成(已知降级)
- **AND** 用户展开懒拉到完整 trace 后,SubagentCard SHALL 依据真实 trace 重新计算并展示其运行 / 完成状态

#### Scenario: 骨架 header 不因展开懒拉冒出 model badge

- **WHEN** 一个 `headerModel` 缺省的骨架 subagent 以折叠态渲染,用户点击展开触发懒拉
- **THEN** 其 card header 在展开前后 SHALL 始终不显示 model badge(header model badge 只读 `Process.headerModel`,不从懒拉 `messages` 派生)——不出现"展开瞬间 header 突然多冒出 model"的布局跳动
- **AND** 懒拉完成后真实 model SHALL 出现在展开 body 的 `Model` 详情行(随展开内容一并出现,非 header 跳变)
