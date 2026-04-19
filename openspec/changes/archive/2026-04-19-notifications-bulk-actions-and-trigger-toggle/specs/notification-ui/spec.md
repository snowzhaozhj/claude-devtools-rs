## ADDED Requirements

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
