## Context

后端已完备：`cdt-config` 的 `ConfigManager` 支持分 section 更新（general/display/notifications/httpServer），`NotificationManager` 支持增/查/标记已读/清空，`TriggerManager` 支持 CRUD。`cdt-api` 的 `LocalDataApi` 已实现 `get_config`/`update_config`/`get_notifications`/`mark_notification_read`。

缺口仅在：Tauri IPC commands 未接线（当前只有 4 个 session/project commands），前端无 Settings 和 Notifications UI。

Tab 系统已支持 session tab，需要扩展支持 settings/notifications 两种新 tab 类型。

## Goals / Non-Goals

**Goals:**
- 4 个 Tauri commands 接线
- Settings 页面：General + Notifications section（只读展示 + 简单 toggle）
- Notifications 页面：列表 + 标记已读 + 删除 + badge
- 点击通知行导航到 session tab

**Non-Goals:**
- Trigger 创建/编辑/删除 UI（只做只读列表）
- Connection / Workspace / Advanced section
- 通知筛选芯片条、虚拟滚动
- Snooze、配置导入导出

## Decisions

### 1. Tab 类型扩展

**选择**：给 Tab 接口加 `type: 'session' | 'settings' | 'notifications'` 字段。openTab 函数针对 settings/notifications 做单例化（只允许同时存在一个 settings tab 和一个 notifications tab）。

**理由**：与原版一致。settings/notifications 不需要 per-tab session 缓存，只需要全局状态。

### 2. Settings/Notifications 状态管理

**选择**：在 `tabStore.svelte.ts` 中增加模块级 `$state` 管理 config 和 notifications 数据。不为它们创建独立 store 文件。

**替代方案**：创建 `configStore.svelte.ts` 和 `notificationStore.svelte.ts`。

**理由**：MVP 阶段数据量小，集中在一个 store 中易于理解。后续复杂化时再拆。但为避免 tabStore 过大，Settings/Notifications 状态改为放到各自组件内用 `$state` 管理，API 调用在 `api.ts` 中封装。

### 3. Tauri commands 接线模式

**选择**：沿用现有模式——Tauri command 返回 `serde_json::Value`，调用 `DataApi` trait 方法后直接透传 JSON。

**理由**：与现有 `list_projects`/`list_sessions`/`get_session_detail` 一致，前端类型在 `api.ts` 中定义。

### 4. Notifications badge 位置

**选择**：在 TabBar 右侧添加一个 bell 图标按钮，附带红色 badge 显示未读数。点击打开 notifications tab。

**理由**：与原版一致。齿轮图标同理放在 TabBar 右侧打开 settings。

### 5. 导航到错误

**选择**：通知行点击时调用 `openTab(sessionId, projectId, label)` 打开 session tab，然后标记已读。MVP 不做精确定位到具体工具（需要 TabNavigationRequest 机制，后续加）。

**理由**：精确定位涉及展开 AI group + 滚动到 tool，复杂度高。MVP 先导航到 session 级别。

## Risks / Trade-offs

- **[Settings 只读]** MVP 只做展示，toggle 写回需要 update_config 调用，需确保乐观更新 + 错误回滚。缓解：先做 toggle，简单的 section update。
- **[通知实时性]** 没有文件监听事件推送，通知列表只在打开时加载。缓解：MVP 可接受手动刷新，后续加 Tauri event。
- **[Badge 轮询]** 未读数需要定期获取。缓解：MVP 在打开 notifications tab 时获取，badge 不做实时轮询。
