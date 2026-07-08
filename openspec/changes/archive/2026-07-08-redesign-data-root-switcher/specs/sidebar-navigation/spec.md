## ADDED Requirements

### Requirement: 数据根切换后的导航数据刷新

系统 SHALL 在数据根目录切换成功后丢弃旧数据根下的项目 / 会话导航状态，并重新加载当前数据根的 project / group 数据。刷新过程 SHALL 避免展示旧数据根的 project、session list 或 memory 入口作为新数据根的内容；若新数据根无项目，Sidebar 与 Dashboard SHALL 显示无项目状态而非旧项目列表。

#### Scenario: 切换成功后不展示旧项目列表
- **WHEN** 用户切换数据根目录且配置保存成功
- **THEN** Sidebar、项目选择器与 Dashboard SHALL 重新加载当前数据根的 project / group 数据
- **AND** 在当前数据根加载完成前，系统 SHALL NOT 把旧数据根的项目列表作为新数据根结果展示

#### Scenario: 新数据根无项目
- **WHEN** 用户切换到一个没有 projects 的数据根目录
- **THEN** Sidebar 与 Dashboard SHALL 显示无项目状态
- **AND** SHALL NOT 保留旧数据根的 selected project 或 session list

#### Scenario: 会话列表缓存不跨数据根复用
- **WHEN** 用户切换数据根目录成功
- **AND** 新数据根中存在与旧数据根相同的 project 或 group 标识
- **THEN** Sidebar SHALL NOT 用旧数据根的会话列表缓存 hydrate 新数据根列表
- **AND** 当前 group 的会话列表 SHALL 从当前数据根重新加载或显示加载状态

#### Scenario: Memory 入口不跨数据根复用
- **WHEN** 用户切换数据根目录成功
- **AND** 新数据根中存在与旧数据根相同的 project 或 group 标识
- **THEN** Sidebar SHALL NOT 用旧数据根的 Memory 入口状态作为新数据根的显示依据
- **AND** Memory 入口 SHALL 依据当前数据根重新判断是否显示

#### Scenario: 切换成功只触发当前数据根刷新
- **WHEN** 用户切换数据根目录成功
- **THEN** 系统 SHALL 只刷新当前数据根的 project / group 数据
- **AND** SHALL NOT 为 `recentRoots` 中的其它历史数据根扫描 project 或 session 数据
