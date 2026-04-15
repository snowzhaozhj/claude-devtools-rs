# notification-ui Specification

## Purpose

定义通知页面的行为契约：通知列表展示、标记已读、导航到错误会话、TabBar badge。

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
