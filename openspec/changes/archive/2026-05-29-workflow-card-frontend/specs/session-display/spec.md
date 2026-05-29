# session-display Specification (delta)

## ADDED Requirements

### Requirement: WorkflowCard 渲染

当 AIChunk 包含 `workflows` 字段时，前端 SHALL 为每个 WorkflowItem 渲染 WorkflowCard 组件。WorkflowCard SHALL 支持 6 种状态的差异化渲染。

#### Scenario: 完成态 WorkflowCard
- **WHEN** WorkflowItem.status 为 "completed" 且 agents 非空
- **THEN** SHALL 渲染折叠态 header（icon · name · phase/agent 计数 · "Done" 状态 · tokens · duration）
- **AND** 展开后 SHALL 渲染 phase 纵向分组 + agent chip 横排
- **AND** agent chip 的 status dot SHALL 为绿色静态圆点

#### Scenario: 部分失败态 WorkflowCard
- **WHEN** WorkflowItem.status 为 "partial_failure"
- **THEN** header SHALL 显示 "N failed" 状态标签（红色）
- **AND** 失败 agent chip SHALL 使用红色 status dot 和红色边框

#### Scenario: Running 最小态 WorkflowCard
- **WHEN** WorkflowItem.status 为 "running" 且 phases 为空
- **THEN** header SHALL 显示旋转 spinner
- **AND** 展开后 SHALL 仅显示 "Details available after completion" 文字
- **AND** SHALL NOT 渲染假进度条或百分比指示器

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
