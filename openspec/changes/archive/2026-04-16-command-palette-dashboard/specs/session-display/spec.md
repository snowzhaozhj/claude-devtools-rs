# session-display Specification (Delta — command-palette-dashboard)

> Delta spec：新增 Dashboard 空状态替换 Requirement。

## ADDED Requirements

### Requirement: Dashboard 项目概览

当无 active tab 时，主区域 SHALL 显示 Dashboard 项目概览页替代空状态。

#### Scenario: 无 tab 时显示 Dashboard
- **WHEN** 无 active tab
- **THEN** 主区域 SHALL 显示项目卡片网格，每张卡片包含项目名、路径缩写、会话数量

#### Scenario: 卡片点击选择项目
- **WHEN** 用户点击项目卡片
- **THEN** SHALL 在 Sidebar 中选中该项目并加载其会话列表

#### Scenario: Dashboard 本地搜索
- **WHEN** 用户在 Dashboard 搜索框中输入文本
- **THEN** 项目卡片 SHALL 按 displayName 或 path 过滤（大小写不敏感）

#### Scenario: 无项目
- **WHEN** 无可用项目
- **THEN** Dashboard SHALL 显示空状态提示
