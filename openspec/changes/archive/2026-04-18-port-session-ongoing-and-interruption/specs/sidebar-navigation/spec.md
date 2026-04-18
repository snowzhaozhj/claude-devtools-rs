## ADDED Requirements

### Requirement: Ongoing indicator on session item

Sidebar 的每一行 session item SHALL 在 `session.isOngoing === true`
时于标题前渲染一枚绿色脉冲圆点 `<OngoingIndicator size="sm" />`。
圆点 SHALL 出现在 pin 图标（如有）之前；`isOngoing` 为 false /
undefined 时 SHALL NOT 占位。该渲染规则 MUST 同时作用于 PINNED
分区与日期分组（TODAY / YESTERDAY / …）两处 session 列表。

#### Scenario: Ongoing session shows pulsing dot
- **WHEN** `SessionSummary.isOngoing === true` 且该 session 出现在
  日期分组内
- **THEN** sidebar 对应一行 SHALL 在标题文本前渲染一枚绿色圆点，
  圆点 SHALL 带脉冲动画（`animate-ping` 或等价 CSS）

#### Scenario: Finished session shows no dot
- **WHEN** `SessionSummary.isOngoing === false`
- **THEN** sidebar 行 SHALL NOT 渲染圆点，其他视觉元素（pin 图标、
  标题、元数据行）位置 SHALL 与当前表现一致

#### Scenario: Dot appears in PINNED section too
- **WHEN** 一个被 pin 的 session 同时 `isOngoing === true`
- **THEN** PINNED 分区内该条目 SHALL 同时显示绿色圆点与蓝色 pin 图标，
  圆点 SHALL 位于 pin 图标之前

#### Scenario: Indicator updates after auto refresh
- **WHEN** 某个 session 的 `isOngoing` 因 `listSessions` 自动刷新
  从 `true` 变为 `false`
- **THEN** 对应 sidebar 行的绿点 SHALL 在该次刷新的下一帧被移除
