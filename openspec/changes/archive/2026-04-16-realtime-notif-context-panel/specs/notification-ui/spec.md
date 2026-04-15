# notification-ui Specification (Delta — realtime-notif-context-panel)

> Delta spec：新增通知实时更新 Requirements。

## ADDED Requirements

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
