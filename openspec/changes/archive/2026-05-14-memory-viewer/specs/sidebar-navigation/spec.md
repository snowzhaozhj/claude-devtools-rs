## ADDED Requirements

### Requirement: Sidebar Memory 入口

Sidebar SHALL 在当前选中项目存在 memory layers 时显示 `Memory (N)` 入口，其中 `N` 为可展示 memory layers 数量。点击入口 SHALL 调用 tab 系统打开该项目 Memory tab。若当前项目没有 memory layers，Sidebar SHALL NOT 显示 Memory 入口。

#### Scenario: 当前项目有 memory 时显示入口
- **WHEN** 当前选中项目的 memory discovery 返回 `hasMemory = true` 且 `count = 11`
- **THEN** Sidebar SHALL 在会话列表上方显示 `Memory (11)` 入口

#### Scenario: 点击 Memory 入口打开 tab
- **WHEN** 用户点击 Sidebar 的 `Memory (11)` 入口
- **THEN** 系统 SHALL 调用 tab 系统打开当前项目的 Memory tab

#### Scenario: 当前项目无 memory 时隐藏入口
- **WHEN** 当前选中项目的 memory discovery 返回 `hasMemory = false` 或 `count = 0`
- **THEN** Sidebar SHALL NOT 渲染 Memory 入口

#### Scenario: 切换项目刷新 Memory 入口
- **WHEN** 用户从有 memory 的 project A 切换到无 memory 的 project B
- **THEN** Sidebar SHALL 隐藏 Memory 入口，并继续显示 project B 的会话列表
