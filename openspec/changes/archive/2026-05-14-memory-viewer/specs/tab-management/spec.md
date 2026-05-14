## ADDED Requirements

### Requirement: 打开 project-scoped Memory tab

用户从 Sidebar 点击 Memory 入口时，系统 SHALL 在当前 focused pane 内打开该项目的 Memory tab。Memory tab SHALL 绑定 `projectId`，同一项目重复打开时 SHALL 复用已有 Memory tab；不同项目的 Memory tab SHALL 可以同时存在。

#### Scenario: 首次打开 Memory tab
- **WHEN** 用户点击当前项目 Sidebar 中的 Memory 入口
- **THEN** 系统 SHALL 在 `focusedPaneId` 对应 pane 中创建 `type = "memory"` 的 tab，并设为该 pane 的 activeTabId

#### Scenario: 重复打开同一项目 Memory tab
- **WHEN** 用户再次点击同一项目的 Memory 入口
- **THEN** 系统 SHALL 激活已有 Memory tab，而不是创建重复 tab

#### Scenario: 不同项目 Memory tab 独立
- **WHEN** 用户先打开 project A 的 Memory tab，再切换到 project B 并打开 Memory tab
- **THEN** 系统 SHALL 创建另一个绑定 project B 的 Memory tab，不替换 project A 的 Memory tab
