## MODIFIED Requirements

### Requirement: 会话选择与 Tab 联动

点击会话项 SHALL 通过 Tab 系统在 focused pane 内打开或切换到对应 session。Sidebar 高亮 SHALL 跟随 focused pane 的 activeTab 的 sessionId。

#### Scenario: 点击会话打开 tab
- **WHEN** 用户点击一个会话项
- **THEN** 系统 SHALL 调用 Tab 系统的 openTab，该 openTab 的作用域 SHALL 为 `focusedPaneId` 对应 pane（具体 tab 生命周期行为由 tab-management spec 定义）

#### Scenario: 高亮跟随 focused pane 的 activeTab
- **WHEN** focused pane 的 activeTabId 变化（无论通过 Sidebar 点击、TabBar 点击、跨 pane focus 切换还是快捷键）
- **THEN** Sidebar 中对应 sessionId 的会话项 SHALL 高亮，之前的高亮 SHALL 移除

#### Scenario: 无 active tab 时无高亮
- **WHEN** focused pane 的 activeTabId 为 null
- **THEN** Sidebar 中 SHALL 无会话项高亮

### Requirement: 右键菜单

Sidebar 会话项 SHALL 支持右键打开上下文菜单，提供快捷操作。菜单 SHALL 包含 "Open in New Pane"（当 `paneLayout.panes.length < 4` 时可用）操作，用于在当前 focused pane 右侧创建新 pane 打开该 session。

#### Scenario: 打开右键菜单
- **WHEN** 用户在会话项上右键点击
- **THEN** SHALL 在点击位置显示浮层菜单，菜单包含：在新标签页打开、Open in New Pane、置顶/取消置顶、隐藏/取消隐藏、复制 Session ID、复制恢复命令

#### Scenario: Open in New Pane
- **WHEN** 用户在会话项右键菜单选择 "Open in New Pane"
- **AND** `paneLayout.panes.length < 4`
- **THEN** 系统 SHALL 在 focused pane 右侧创建一个新 pane，在该 pane 中打开目标 session 的 tab，新 pane SHALL 成为 focused

#### Scenario: Open in New Pane 达到上限时禁用
- **WHEN** `paneLayout.panes.length === 4`
- **THEN** 右键菜单的 "Open in New Pane" 项 SHALL 显示为禁用状态（灰色 + 不可点击）

#### Scenario: 菜单定位 clamping
- **WHEN** 右键位置靠近视口右边界或下边界
- **THEN** 菜单位置 SHALL clamp 到视口内（8px 内边距）

#### Scenario: 菜单关闭
- **WHEN** 用户点击菜单外区域或按 Escape
- **THEN** 菜单 SHALL 关闭

#### Scenario: 复制操作反馈
- **WHEN** 用户选择复制 Session ID 或复制恢复命令
- **THEN** SHALL 复制到剪贴板，菜单项文本 SHALL 变为 "已复制!" 并在 600ms 后自动关闭菜单
