# notification-ui Specification

## Purpose

定义通知页面（NotificationsView）与 TabBar badge 的行为契约：未读计数的实时刷新策略（后端 `notification-update` / `notification-added` 事件驱动 + 30 秒兜底轮询）、通知列表分页与空态展示、标记已读（单条与全部）、点击通知导航到错误所在会话并自动标记已读、触发器颜色在卡片左侧圆点的呈现、前端状态与后端 `NotificationManager` 的一致性保证。

## Requirements

### Requirement: 打开 Notifications 页面

用户 SHALL 能通过 TabBar 的 bell 图标打开 Notifications 页面。Notifications tab SHALL 为单例。

#### Scenario: 点击 bell 图标打开 Notifications
- **WHEN** 用户点击 TabBar 的 bell 图标且无 Notifications tab
- **THEN** 系统 SHALL 创建 type 为 "notifications" 的 tab 并设为 active

#### Scenario: 重复点击 bell 图标
- **WHEN** 已有 Notifications tab 时用户再次点击 bell 图标
- **THEN** 系统 SHALL 切换焦点到已有 Notifications tab

### Requirement: 通知列表展示

Notifications 页面 SHALL 展示通知列表，按时间倒序排列。每行 SHALL 显示 trigger 颜色、trigger 名称、消息摘要和时间戳。

#### Scenario: 有通知时展示列表
- **WHEN** 通知列表非空
- **THEN** SHALL 按时间倒序渲染所有通知行

#### Scenario: 无通知时展示空状态
- **WHEN** 通知列表为空
- **THEN** SHALL 显示空状态提示

#### Scenario: 未读通知视觉区分
- **WHEN** 通知未被标记为已读
- **THEN** 该行 SHALL 有视觉区分（如加粗或背景色）

### Requirement: 标记通知已读

用户 SHALL 能标记单条通知为已读。

#### Scenario: 点击标记已读按钮
- **WHEN** 用户点击通知行的标记已读按钮
- **THEN** 系统 SHALL 调用 mark_notification_read API，成功后该通知 SHALL 变为已读状态

### Requirement: 导航到错误会话

用户点击通知行 SHALL 导航到对应的 session。

#### Scenario: 点击通知行导航
- **WHEN** 用户点击通知行
- **THEN** 系统 SHALL 打开或切换到对应 sessionId 的 tab，并自动标记该通知为已读

### Requirement: 通知 Badge

TabBar 的 bell 图标旁 SHALL 显示未读通知数 badge。无未读时 badge SHALL 隐藏。

#### Scenario: 有未读通知时显示 badge
- **WHEN** 未读通知数 > 0
- **THEN** bell 图标旁 SHALL 显示红色圆形 badge，内容为未读数（超过 99 显示 "99+"）

#### Scenario: 无未读时隐藏 badge
- **WHEN** 未读通知数为 0
- **THEN** badge SHALL 不显示

### Requirement: 通知数据加载

Notifications 页面打开时 SHALL 从后端加载通知列表。

#### Scenario: 加载成功
- **WHEN** Notifications 页面打开
- **THEN** SHALL 调用 get_notifications API，显示 loading 状态，成功后渲染列表

#### Scenario: 加载失败
- **WHEN** API 调用失败
- **THEN** SHALL 显示错误提示

### Requirement: 通知 badge 自动刷新

TabBar 的通知 badge SHALL 自动更新未读数量，无需用户手动刷新。

#### Scenario: 定时轮询
- **WHEN** 应用运行中
- **THEN** TabBar SHALL 每 30 秒自动查询 unreadCount 并更新 badge 显示

#### Scenario: 标记已读后立即刷新
- **WHEN** 用户标记一条通知为已读
- **THEN** 后端 SHALL 通过 Tauri event 通知前端，badge SHALL 立即更新

### Requirement: Tauri event 监听

前端 SHALL 监听后端推送的 `notification-update` 事件以实现实时响应。

#### Scenario: 事件触发刷新
- **WHEN** 前端收到 `notification-update` 事件
- **THEN** SHALL 刷新 unreadCount 并更新 TabBar badge

#### Scenario: 应用销毁时清理
- **WHEN** App 组件销毁
- **THEN** SHALL 取消事件监听，避免内存泄漏

### Requirement: 批量标记全部已读

Notifications 页面 header 右侧 SHALL 提供 "全部标记已读" 按钮（CheckCheck 图标），点击后 MUST 调用 `mark_all_notifications_read` IPC 将所有通知置为已读。按钮 SHALL 在通知列表非空时常驻显示；当未读数为 0 时 SHALL 进入 disabled 态（视觉透明度降低、光标 `not-allowed`、不响应点击），以便用户始终能看到入口、理解当前状态。

#### Scenario: 点击"全部标记已读"

- **WHEN** 用户在存在未读通知时点击 "全部标记已读"
- **THEN** 前端 SHALL 调用 `mark_all_notifications_read()` 并在成功后 reload 通知列表
- **AND** TabBar badge SHALL 刷新为 0

#### Scenario: 无未读时按钮 disabled

- **WHEN** 当前列表未读数为 0 但列表非空
- **THEN** "全部标记已读" 按钮 SHALL 仍渲染但处于 disabled 态（透明度降低、不响应点击）

#### Scenario: 空列表时按钮隐藏

- **WHEN** 通知列表为空
- **THEN** "全部标记已读" 按钮 SHALL 不渲染

### Requirement: 单条通知标记已读（行 hover 出现）

通知行 SHALL 在 hover 时于右侧显示"标记已读"按钮（Check ✓ 图标），点击后调用 `mark_notification_read(id)` 将该条通知置为已读。按钮 SHALL 仅在该条通知未读时渲染；已读条目 SHALL 不显示该按钮。点击事件 MUST 阻止冒泡，避免同时触发行点击的导航。

#### Scenario: 未读行 hover 显示标记按钮

- **WHEN** 鼠标移入未读通知行
- **THEN** 该行右侧 SHALL 显示 Check ✓ 按钮
- **AND** 已读通知行 SHALL NOT 显示该按钮

#### Scenario: 点击标记已读

- **WHEN** 用户在未读行 hover 态点击 Check ✓ 按钮
- **THEN** 前端 SHALL 调用 `mark_notification_read(id)` 并阻止事件冒泡
- **AND** 成功后该行 SHALL 切换到已读视觉态、Check 按钮 SHALL 消失、TabBar badge SHALL 相应减 1

### Requirement: 清空全部通知（二次确认）

Notifications 页面 header 右侧 SHALL 提供 "清空" 按钮（Trash2 图标），点击后进入 3 秒的二次确认态，再次点击才真正执行。按钮 SHALL 仅在列表非空时可见。

#### Scenario: 首次点击进入确认态

- **WHEN** 用户在列表非空时点击 "清空"
- **THEN** 按钮 SHALL 切换为红色背景 + 文案 "再次点击确认"
- **AND** 3 秒计时开始；若 3 秒内无第二次点击，按钮 SHALL 恢复原态

#### Scenario: 3 秒内再次点击执行清空

- **WHEN** 用户在确认态（3 秒内）再次点击该按钮
- **THEN** 前端 SHALL 调用 `clear_notifications(undefined)` 清空全部通知
- **AND** 成功后 reload 列表、TabBar badge SHALL 刷新为 0、空态提示 SHALL 显示

#### Scenario: 列表为空时按钮隐藏

- **WHEN** 通知列表为空
- **THEN** "清空" 按钮 SHALL 不渲染

#### Scenario: 组件销毁清理定时器

- **WHEN** Notifications tab 关闭或用户切走时清空按钮处于确认态
- **THEN** 前端 SHALL 清理 setTimeout 以防内存泄漏

### Requirement: 单条通知删除（行 hover 出现）

通知行 SHALL 在 hover 时于右侧显示删除按钮（X 图标），点击后调用 `delete_notification(id)` 删除该条，删除成功后该行 MUST 从列表消失。删除操作 MUST 阻止事件冒泡，避免同时触发行点击的导航。

#### Scenario: hover 显示删除按钮

- **WHEN** 鼠标移入某个通知行
- **THEN** 该行右侧 SHALL 显示 X 删除图标
- **AND** 鼠标离开时该图标 SHALL 隐藏

#### Scenario: 点击删除移除该条

- **WHEN** 用户在通知行 hover 态点击删除图标
- **THEN** 前端 SHALL 调用 `delete_notification(id)` 并阻止事件冒泡（不触发导航）
- **AND** 成功后该行 SHALL 从列表消失、TabBar badge SHALL 相应更新

#### Scenario: 删除失败显示错误

- **WHEN** `delete_notification(id)` 调用失败
- **THEN** 前端 SHALL 显示错误提示且列表 SHALL 保持原样不丢条目

