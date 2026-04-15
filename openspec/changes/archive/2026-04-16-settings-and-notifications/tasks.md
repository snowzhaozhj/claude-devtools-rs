## 1. Tauri Commands 接线

- [x] 1.1 在 `src-tauri/src/lib.rs` 新增 `get_config` command：调用 `data.api.get_config()`，注册到 invoke_handler
- [x] 1.2 新增 `update_config` command：接收 `section: String` + `data: serde_json::Value`，调用 `data.api.update_config()`
- [x] 1.3 新增 `get_notifications` command：接收 `limit`/`offset`，调用 `data.api.get_notifications()`
- [x] 1.4 新增 `mark_notification_read` command：接收 `notification_id: String`，调用 `data.api.mark_notification_read()`
- [x] 1.5 `cargo clippy -p claude-devtools-tauri` + `cargo build -p claude-devtools-tauri` 编译通过

## 2. 前端 API 封装

- [x] 2.1 在 `ui/src/lib/api.ts` 新增类型定义：`AppConfig`（general/notifications/display 子结构）、`StoredNotification`（error + is_read + created_at）、`GetNotificationsResult`
- [x] 2.2 新增 API 函数：`getConfig()`、`updateConfig(section, data)`、`getNotifications(limit, offset)`、`markNotificationRead(id)`

## 3. Tab 类型扩展

- [x] 3.1 `tabStore.svelte.ts`：Tab 接口增加 `type: 'session' | 'settings' | 'notifications'` 字段，openTab 默认 type='session'
- [x] 3.2 新增 `openSettingsTab()` 和 `openNotificationsTab()` 函数（单例化：已有则切换焦点）
- [x] 3.3 新增通知状态：模块级 `notificationUnreadCount: number`，导出 `getUnreadCount()`/`setUnreadCount()` 函数

## 4. TabBar 扩展

- [x] 4.1 TabBar 右侧新增 bell 图标按钮（点击调用 `openNotificationsTab()`）+ 齿轮图标按钮（点击调用 `openSettingsTab()`）
- [x] 4.2 Bell 图标旁 badge：未读数 > 0 时显示红色圆形数字（>99 显示 "99+"）

## 5. App 路由扩展

- [x] 5.1 `App.svelte` 根据 `activeTab.type` 路由：session → SessionDetail，settings → SettingsView，notifications → NotificationsView

## 6. Settings 页面

- [x] 6.1 创建 `ui/src/routes/SettingsView.svelte`：section tab 导航（General / Notifications），加载 config 数据
- [x] 6.2 General section：展示 theme 值
- [x] 6.3 Notifications section：enabled/soundEnabled toggle（调用 updateConfig 写回）+ trigger 只读列表（名称、颜色点、启用状态）

## 7. Notifications 页面

- [x] 7.1 创建 `ui/src/routes/NotificationsView.svelte`：加载通知列表，按时间倒序渲染
- [x] 7.2 NotificationRow：trigger 颜色点 + 名称 + 消息摘要(截断100字) + 时间 + 标记已读按钮
- [x] 7.3 点击通知行：调用 `openTab` 导航到 session + `markNotificationRead` 标记已读
- [x] 7.4 空状态和 loading 状态

## 8. 验证

- [x] 8.1 `npm run check --prefix ui` 类型检查通过
- [x] 8.2 `cargo tauri dev` 启动验证：Settings 页面打开/切换 section/toggle 写回；Notifications 打开/标记已读/导航到 session；badge 显示
