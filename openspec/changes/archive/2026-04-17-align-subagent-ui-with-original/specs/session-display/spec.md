## ADDED Requirements

### Requirement: Subagent 内联展开 ExecutionTrace

每个 `AIChunk` 中的 subagent（`SemanticStep.kind === "subagent_spawn"` 或 `DisplayItem.type === "subagent"`）SHALL 以内联卡片形式渲染；用户 SHALL 能在**当前 tab 内**展开查看其 Dashboard 与完整执行链，SHALL NOT 自动跳转到新 tab。

#### Scenario: Subagent 默认折叠
- **WHEN** 一条 AI 组首次渲染，其中包含一个 subagent
- **THEN** subagent 卡片 SHALL 以单行 Header 形式展示，Dashboard 与 ExecutionTrace 均不可见

#### Scenario: 点击 Header 展开 Dashboard
- **WHEN** 用户点击 subagent 卡片的 Header 区域
- **THEN** SHALL 展开显示 Dashboard（meta 行 + Context Usage 列表）与 Execution Trace 折叠头；chevron SHALL 旋转 90°

#### Scenario: Execution Trace 内独立展开
- **WHEN** 用户点击已展开卡片中的 "Execution Trace" 折叠头
- **THEN** SHALL 显示该 subagent 完整的 DisplayItem 流（thinking / tool / output / 嵌套 subagent），与父卡片展开状态独立保存

#### Scenario: 嵌套 subagent 递归渲染
- **WHEN** subagent 的 ExecutionTrace 中包含另一个 subagent（`process.messages` 内再有 Task tool 调用结果）
- **THEN** 内层 subagent SHALL 作为可独立展开的 SubagentCard 渲染；渲染深度 SHALL 不超过 8 层，超过时内层只显示 Header 不再递归

#### Scenario: 不产生"打开新 tab"副作用
- **WHEN** 用户点击 subagent 卡片的任意区域
- **THEN** 应用 SHALL NOT 创建新 tab，也 SHALL NOT 调用 `openTab(subagent.sessionId, ...)`

### Requirement: Subagent 彩色标识体系

每个 subagent 卡片 SHALL 根据所属类别选用颜色：

1. **Team 成员**：`Process.team.member_color` → 通过 `getTeamColorSet` 映射到 border/badge/text 三色
2. **已知 subagentType 且有 agent config**：`AgentConfig.color` 对应调色板 → 通过 `getSubagentTypeColorSet` 返回
3. **已知 subagentType 无 config**：对 `subagent_type` 做 djb2 hash，映射到 14 色调色板任一槽位（确定性映射）
4. **未知类型（`subagent_type = None` 且非 team）**：使用中性 muted 色 + Bot 图标，不显示彩色圆点与 badge

#### Scenario: Team 成员使用 team 颜色
- **WHEN** subagent `process.team.member_color = "#8b5cf6"`
- **THEN** Header 圆点背景色 SHALL 为 `#8b5cf6`；badge SHALL 显示 `process.team.member_name` 文本且使用同色系 background/border

#### Scenario: agent config 匹配
- **WHEN** `subagent_type = "code-reviewer"` 且 agentConfigs 中存在同名条目 `color = "purple"`
- **THEN** Header 圆点与 badge SHALL 使用调色板 `purple` 槽位颜色；badge 文本 SHALL 为 `"code-reviewer"`（uppercase 样式由 CSS 控制）

#### Scenario: agent config 未命中走 hash
- **WHEN** `subagent_type = "unknown-type-xyz"` 且 agentConfigs 无对应条目
- **THEN** Header 圆点与 badge SHALL 使用 `djb2("unknown-type-xyz") % 14` 对应的调色板槽位颜色

#### Scenario: 完全无类型信息
- **WHEN** `subagent_type = None` 且 `team = None`
- **THEN** SHALL 使用中性 `--color-text-muted` Bot 图标，不渲染彩色圆点与 badge

### Requirement: Subagent MetricsPill 多维度展示

Subagent 卡片 Header SHALL 显示 MetricsPill，根据数据可用性展示以下维度：

- **Main Context**：`process.main_session_impact.total_tokens` 格式化；仅非 team 成员显示
- **Subagent Context**：最后一条 assistant 消息 `usage` 的 `input + output + cache_read + cache_creation` 之和
- **Duration**：`process.duration_ms` 使用 `formatDuration` 格式化（秒/分钟）

若某维度数据缺失（`None` 或零值），对应槽位 SHALL 不渲染。

#### Scenario: 非 team subagent 显示两维
- **WHEN** subagent 有 `main_session_impact.total_tokens = 5000` 与最新 usage 合计 12000
- **THEN** MetricsPill SHALL 显示 `Main: 5.0k` 与 `Ctx: 12.0k` 两个槽位

#### Scenario: Team 成员隐藏 Main Context
- **WHEN** subagent 是 team 成员（`team != None`）
- **THEN** MetricsPill SHALL NOT 显示 Main Context 槽位，仅显示 Context Window（最新 usage 合计）

#### Scenario: 数据全缺失
- **WHEN** 两个维度均为 `None` 或 0
- **THEN** MetricsPill SHALL 整体不渲染，但 Duration 显示逻辑不受影响
