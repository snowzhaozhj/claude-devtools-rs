## Why

当前 Rust 版桌面应用没有 Settings 页面和通知系统。后端 `cdt-config` 已完整实现配置读写、通知持久化、trigger 管理，`cdt-api` 的 `DataApi` trait 也定义了 `get_config`/`update_config`/`get_notifications`/`mark_notification_read` 方法并已在 `LocalDataApi` 中实现。只缺 Tauri IPC commands 接线和前端 UI。这是第三批迁移工作，补全 Settings + 通知后即可进入高级交互阶段。

## What Changes

### 后端（Tauri commands 接线）
- 新增 `get_config` Tauri command — 透传 `DataApi::get_config()`
- 新增 `update_config` Tauri command — 透传 `DataApi::update_config()`，接收 section + data
- 新增 `get_notifications` Tauri command — 透传 `DataApi::get_notifications()`
- 新增 `mark_notification_read` Tauri command — 透传 `DataApi::mark_notification_read()`

### 前端（Settings MVP）
- 新增 `SettingsView.svelte` — Settings 页面主体，含 tab 导航
- General section：theme 选择（暂只展示当前值）
- Notifications section：enabled/sound toggles、trigger 列表（只读展示）
- Settings 通过新 tab 类型打开（TabBar 中点击齿轮图标或快捷键）

### 前端（Notifications MVP）
- 新增 `NotificationsView.svelte` — 通知列表页面
- `NotificationRow` 行：trigger 颜色点 + 消息摘要 + 时间 + 标记已读/删除按钮
- TabBar 通知 badge：未读数 > 0 时显示红色圆形数字
- 点击通知行导航到对应 session + 自动标记已读

### 不含（留给后续）
- Connection / Workspace / Advanced settings section
- Trigger 创建/编辑/删除 UI（只做只读展示）
- 通知筛选芯片条、虚拟滚动
- Snooze 操作
- 配置导入导出

## Capabilities

### New Capabilities

- `settings-ui`：Settings 页面行为契约——tab 导航、配置读取与展示、section 切换
- `notification-ui`：通知页面行为契约——列表展示、标记已读、删除、badge、导航到错误

### Modified Capabilities

（无——后端 API 不变，只是 Tauri 层透传接线）

## Impact

- **后端文件**：`src-tauri/src/lib.rs` 新增 4 个 Tauri commands
- **前端文件**：新增 `SettingsView.svelte`、`NotificationsView.svelte`；改造 `TabBar.svelte`（badge + 齿轮图标）、`tabStore.svelte.ts`（支持非 session tab 类型）、`App.svelte`（路由 settings/notifications tab）、`api.ts`（新增 API 函数）
- **依赖**：无新增依赖
