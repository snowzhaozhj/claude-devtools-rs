## MODIFIED Requirements

### Requirement: WorkflowCard 渲染

当 AIChunk 包含 `workflows` 字段时，前端 SHALL 为每个 WorkflowItem 渲染 WorkflowCard 组件。WorkflowCard SHALL 支持 6 种状态的差异化渲染。运行态（manifest 缺失降级）下 WorkflowItem 可携带由后端合成的匿名 agents，WorkflowCard SHALL 在运行态展示 agent 计数与匿名 `"Agent N"` chip，且 SHALL NOT 渲染假进度条或百分比指示器。

#### Scenario: 完成态 WorkflowCard
- **WHEN** WorkflowItem.status 为 "completed" 且 agents 非空
- **THEN** SHALL 渲染折叠态 header（icon · name · phase/agent 计数 · "Done" 状态 · tokens · duration）
- **AND** 展开后 SHALL 渲染 phase 纵向分组 + agent chip 横排
- **AND** agent chip 的 status dot SHALL 为绿色静态圆点

#### Scenario: 部分失败态 WorkflowCard
- **WHEN** WorkflowItem.status 为 "partial_failure"
- **THEN** header SHALL 显示 "N failed" 状态标签（红色）
- **AND** 失败 agent chip SHALL 使用红色 status dot 和红色边框

#### Scenario: Running 态 WorkflowCard（含合成 agents）
- **WHEN** WorkflowItem.status 为 "running" 且 agents 非空（后端 manifest 缺失降级合成）
- **THEN** header SHALL 显示旋转 spinner · name（或 "Workflow" 兜底）· `N agents (M done)` 计数（N = agents.len，M = state 为 completed 的 agent 数）
- **AND** 展开后 SHALL 渲染 agent chip 横排
- **AND** SHALL NOT 渲染假进度条或百分比指示器

#### Scenario: Running 态匿名 agent 显示 "Agent N"
- **WHEN** WorkflowItem.status 为 "running" 且某 agent 的 label 为空字符串
- **THEN** 该 agent chip SHALL 显示 `"Agent <序号>"`（1-based，按 agents 数组顺序）
- **AND** chip status dot SHALL 静态着色（completed 绿 / running 中性），不带动画

#### Scenario: Running 态 Tier 1 phases 静态列表
- **WHEN** WorkflowItem.status 为 "running" 且 phases 非空（Tier 1 解析 script meta 得到）
- **THEN** 展开后 SHALL 在 agent chips 之上显示 phase 静态列表
- **AND** SHALL NOT 高亮「当前第几 phase」（运行态无权威当前 phase 来源）

#### Scenario: Empty WorkflowCard
- **WHEN** WorkflowItem.agents 为空且 status 非 "running"
- **THEN** 展开后 SHALL 显示 "No subagents" 文字

#### Scenario: Launch error 不渲染 WorkflowCard
- **WHEN** Workflow tool 调用结果 is_error 为 true
- **THEN** SHALL 通过 BaseItem 错误渲染路径显示错误信息
- **AND** SHALL NOT 产出 WorkflowDisplayItem

#### Scenario: WorkflowCard 仅 header 有动画
- **WHEN** WorkflowCard 处于 running 态
- **THEN** 仅 header 的 spinner 元素 SHALL 有旋转动画
- **AND** 展开区域内所有 agent chip status dot SHALL 为静态着色（不带动画）

#### Scenario: Script disclosure 默认折叠
- **WHEN** WorkflowItem.scriptPreview 非空
- **THEN** SHALL 渲染 "View script" disclosure toggle，默认折叠
- **AND** 点击后 SHALL 展开显示 scriptPreview 内容的预格式化块
