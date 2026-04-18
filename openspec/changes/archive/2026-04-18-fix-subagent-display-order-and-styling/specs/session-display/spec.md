## ADDED Requirements

### Requirement: Subagent 卡片与 Task tool 就地交错渲染

SessionDetail 渲染 `AIChunk` 时 MUST 按 `semantic_steps` 顺序依次输出 DisplayItem，使 subagent 卡片与其对应 Task 调用**时序相邻**；同时 UI SHALL 跳过与 subagent 已关联的 Task `tool_execution`，避免同一个逻辑调用同时以"Task tool 调用行"和"Subagent 卡片"两种形式重复显示。

#### Scenario: Task 调用后紧随 Subagent 卡片
- **WHEN** AIChunk 含 `Read` → `Task(t_task)` → `Grep` 三个 tool_execution，且 `Task(t_task)` 已解析出 subagent A
- **THEN** UI SHALL 依序渲染：Read tool item → Subagent 卡片（A）→ Grep tool item；SHALL NOT 在 Grep 之后再输出 Subagent

#### Scenario: Task 去重
- **WHEN** `chunk.subagents` 中某 subagent 的 `parentTaskId = t_task`，且 `chunk.toolExecutions` 也含 `toolUseId = t_task, toolName = "Task"`
- **THEN** `displayItemBuilder` SHALL NOT 为该 `tool_execution` 步骤输出 `DisplayItem.type === "tool"`；subagent 卡片 SHALL 是该 Task 的唯一可见代表

#### Scenario: Orphan Task 保留显示
- **WHEN** 某 Task `tool_use` 未匹配任何 subagent（`Resolution::Orphan`）
- **THEN** 对应的 `tool_execution` SHALL 照常渲染为 Tool item（Default viewer），不受去重影响

### Requirement: Subagent 卡片 Header badge 固定 TASK 文案

非 team 成员的 subagent 卡片 Header badge MUST 显示固定文本 `TASK`（由 CSS uppercase 样式控制），`process.subagentType` 仅用于决定 badge 颜色（通过 `getSubagentTypeColorSet`），不再作为 badge 文字。Team 成员保持显示 `process.team.memberName`。展开视图的 Meta 行 Type 字段仍显示 `subagentType ?? (team ? "Team" : "Task")` 原值。

#### Scenario: 已知 subagent_type
- **WHEN** subagent `subagentType = "code-reviewer"`、无 team
- **THEN** Header badge 文字 MUST 为 `TASK`；badge 颜色 MUST 使用 agent config 或 djb2 hash 解析出的调色板颜色；展开 Meta 行 Type 字段 MUST 显示 `code-reviewer`

#### Scenario: 无类型信息
- **WHEN** `subagentType = None` 且 `team = None`
- **THEN** Header SHALL 渲染 Bot 图标 + 中性 badge "TASK"（已有中性样式路径）；展开 Meta 行 Type 字段 SHALL 显示 `Task`

#### Scenario: Team 成员保留成员名
- **WHEN** subagent `team.memberName = "reviewer"`
- **THEN** Header badge 文字 MUST 为 `reviewer`，不变为 `TASK`

### Requirement: Subagent 模型名对齐 parseModelString

Subagent 卡片 Header 与展开 Meta 行显示的模型名 MUST 通过 `parseModelString(rawModel)` 产出的 `name` 字段渲染：去 `claude-` 前缀、去 `-YYYYMMDD` 日期 suffix 后，把 `-` 分隔的 family/版本段以 `.` 连接（`haiku-4-5` → `haiku4.5`、`sonnet-4-6` → `sonnet4.6`、`opus-4-7` → `opus4.7`）。模型为 `<synthetic>` 或缺失时 SHALL 不渲染模型名。

#### Scenario: 版本号压缩
- **WHEN** rawModel = `claude-haiku-4-5-20251001`
- **THEN** Header 显示文本 MUST 为 `haiku4.5`

#### Scenario: synthetic 模型隐藏
- **WHEN** rawModel = `<synthetic>`
- **THEN** Header SHALL NOT 渲染模型名元素
