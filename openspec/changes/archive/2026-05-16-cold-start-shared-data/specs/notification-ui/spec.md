## ADDED Requirements

### Requirement: 通知未读数冷启去重

通知 badge 的冷启 unread count 初始化 SHALL 通过单一前端共享请求入口完成。多个组件在同一启动窗口需要 unread count 时 MUST 复用 in-flight 请求或已缓存结果，避免重复调用 `getNotifications(1, 0)`。

#### Scenario: App 与 TabBar 冷启共享 unread count 请求

- **WHEN** App 初始化通知事件监听且 TabBar 同时初始化 badge
- **THEN** 前端 SHALL 至多发起一次 `getNotifications(1, 0)` 请求获取 unread count
- **AND** App 与 TabBar SHALL 复用同一结果更新 badge 状态

#### Scenario: 后续事件仍可刷新 unread count

- **WHEN** 前端收到 `notification-update` 事件
- **THEN** 前端 SHALL 通过同一共享请求入口刷新 unread count
- **AND** 若没有同类请求正在进行，刷新 SHALL 发起新的 `getNotifications(1, 0)` 请求

#### Scenario: 定时轮询复用共享入口

- **WHEN** TabBar 的 30 秒兜底轮询触发
- **THEN** 轮询 SHALL 调用同一共享 unread count 刷新入口
- **AND** 若事件刷新请求仍在进行中，轮询 SHALL 复用该 in-flight 请求而不是并行发起重复请求
