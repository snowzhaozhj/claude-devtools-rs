## ADDED Requirements

### Requirement: 数据根切换后的 tab 上下文重置

系统 SHALL 在数据根目录切换成功后关闭当前 root-scoped tabs，并让主工作区回到 Dashboard / 工作台空 pane 状态。root-scoped tabs 包括 session tab 与 memory tab；它们绑定旧数据根下的 project / session 身份，切换成功后继续展示会误导用户。配置保存失败时，系统 SHALL 保留当前 tabs 与已加载内容，不执行上下文重置。

#### Scenario: 切换数据根成功后回到工作台
- **WHEN** 用户在 Settings 中切换数据根目录
- **AND** 配置保存成功
- **THEN** 系统 SHALL 关闭已打开的 session tabs 与 memory tabs
- **AND** 主工作区 SHALL 回到 Dashboard / 工作台状态
- **AND** Sidebar 中 SHALL 不再高亮旧 session tab

#### Scenario: 多 pane 中的 root-scoped tabs 全部关闭
- **WHEN** 用户打开了多个 pane，且其中多个 pane 含 session 或 memory tab
- **AND** 数据根目录切换成功
- **THEN** 所有 pane 中的 session 与 memory tabs SHALL 被关闭
- **AND** 工作区 SHALL 收敛到可显示 Dashboard 的状态

#### Scenario: 切换失败保留旧上下文
- **WHEN** 用户尝试切换数据根目录
- **AND** 配置保存失败
- **THEN** 系统 SHALL 保留当前已打开 tabs
- **AND** 已加载的 session / memory 内容 SHALL 继续可见
- **AND** 主工作区 SHALL NOT 自动回到 Dashboard

#### Scenario: 关闭 root-scoped tabs 时释放旧内容
- **WHEN** 数据根目录切换成功导致 session 或 memory tab 被关闭
- **THEN** 系统 SHALL 释放这些 tab 关联的已加载内容与 UI 状态
- **AND** 用户后续重新打开任意 session SHALL 从当前数据根加载内容
