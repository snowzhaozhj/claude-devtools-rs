## Why

当前通知子系统有三类问题：

1. **存储 bug（最严重）**：`ConfigManager::update_notifications` 只处理 `enabled / soundEnabled / includeSubagentErrors / snoozeMinutes` 四个键，**完全忽略 `triggers` 字段**（`crates/cdt-config/src/manager.rs` line 177-206 的 match 中 `_ => {}` 静默吞掉）。前端 `SettingsView.handleToggleTrigger` 通过 `updateConfig("notifications", { triggers: [...] })` 批量写 triggers 数组——这条路径被**整条吞掉**，磁盘不变、内存 `trigger_manager` 不同步。表面上 toggle 按钮能切换（因为前端乐观更新），但 notifier 始终读到旧的启用列表，继续产生通知；重启应用后 UI 也会回退到旧值。对齐原版 `ConfigManager.updateConfig` 的行为——它用 object-spread 整段替换再 `triggerManager.setTriggers(...)` 同步内存。
2. **通知面板缺批量操作**：只有逐条 ✓ 标记已读。原版 Linear Inbox 风格 `NotificationsView.tsx` 有 "Mark all read"（CheckCheck 图标）、"Clear all"（Trash2 图标，二次确认态）以及行 hover 出现的单条 Archive / Delete 图标，体验完整得多。Rust port 既无对应 IPC 也无 UI 入口。
3. **Settings 里 toggle 样式不可辨**：当前 "启用/禁用" 文字按钮靠 `toggle-on` 轻微底色区分，对比度低，用户反馈看不出开关状态。原版 `SettingsToggle.tsx` 是 Linear 风格 9×5 圆角 track + 16px 白 thumb + `#6366f1` 启用紫色，thumb 位置一眼可辨。

## What Changes

- **修复 `update_notifications` 丢弃 `triggers` 字段 bug**：match 分支新增 `"triggers"` 处理——反序列化成 `Vec<NotificationTrigger>`、逐条 `validate_trigger` 拒绝非法条目、替换 `self.config.notifications.triggers` 并调 `self.trigger_manager.set_triggers(...)` 同步内存，再 `save()`。其余分支加一条 `"triggerColorOverrides"` 之类未来可选键的占位（若无则保留 `_ => {}`，但打 `tracing::warn!` 提示未知键，防下次再静默丢失）。同时加一个 `update_notifications_rejects_invalid_trigger` / `update_notifications_persists_triggers_and_syncs_manager` 单元测试锁死行为。
- **后端新增通知批量/单条操作**：
  - `NotificationManager::delete_one(id) -> Result<bool>` —— 按 id 删除一条，落盘；
  - 复用已有 `mark_all_as_read()` / `clear_all()`；
  - `DataApi` trait 新增 `delete_notification`、`mark_all_notifications_read`、`clear_notifications` 三个方法；`LocalDataApi` 与 HTTP routes 同步实现；
  - Tauri 新增三个 command，调用成功后 `emit("notification-update")` 让前端 badge 与列表立即刷新。
- **前端通知面板批量 + 单条操作 UI（对齐原版 `NotificationsView.tsx`）**：
  - `NotificationsView` header 右侧新增 "全部标记已读"（CheckCheck 图标）、"清空"（Trash2 图标）按钮；清空按钮第一次点击进入 `pending` 态（背景变红、文案 "再次点击确认"），3 秒未点第二次自动复原；无通知时两个按钮隐藏；全部已读时只显示清空；
  - 每行 hover 时右侧出现单条 delete 按钮（X 图标），点击调 `delete_notification(id)`；
  - 操作完成后列表从后端重新拉取（不走乐观更新，避免和 `notification-update` emit 冲突）。
- **Settings toggle 对齐原版 Linear 风格滑块**：
  - 新增 `ui/src/lib/components/SettingsToggle.svelte`（9×5 圆角 track，紫色 `#6366f1` 启用态，白色 16px thumb，`translateX` 过渡动画），API 与原版一致：`enabled: boolean, onChange: (v: boolean) => void, disabled?: boolean`；
  - `SettingsView.svelte` 的 4 处 `.toggle-btn`（自动展开 AI 组 / 启用通知 / 提示音 / 每个 trigger 启用状态）替换为 `<SettingsToggle>`；
  - **trigger 启用态切换改走 `add_trigger` / `remove_trigger` 等单独 command 语义不适用**——改为继续走 `update_config("notifications", { triggers: [...] })`，但上面的 bug 修完之后这条路会真正落盘；同时保留原有乐观更新 + 失败回滚模式。

## Capabilities

### New Capabilities
- 无

### Modified Capabilities
- `configuration-management`：修复 `update_notifications` 对 `triggers` 字段的静默丢弃——新增 "通知 triggers 字段可通过 `update_notifications` 整体替换并同步 `TriggerManager` 内存" requirement
- `notification-ui`：新增 "批量标记已读"、"清空全部通知（二次确认）"、"单条通知删除（hover 出现）" 三条 requirement
- `ipc-data-api`：新增 "批量与单条通知操作 IPC"（delete / mark all read / clear all）
- `settings-ui`：MODIFIED "Notifications Section 展示"——toggle 控件规范改为 Linear 风格滑块

## Impact

- **后端**（影响范围集中在 `cdt-config` + `cdt-api` + `src-tauri`）：
  - `crates/cdt-config/src/manager.rs`：`update_notifications` 补 `triggers` 分支 + `TriggerManager::set_triggers` 同步；新增单元测试覆盖 triggers 替换 / 非法 trigger 拒绝 / 未知键 warn；
  - `crates/cdt-config/src/notification_manager.rs`：新增 `delete_one(id)`；
  - `crates/cdt-api/src/ipc/traits.rs`、`ipc/local.rs`、`http/routes.rs`：三个新 API；
  - `src-tauri/src/lib.rs`：三个新 command + invoke_handler 注册 + emit `notification-update`。
- **前端**：
  - `ui/src/lib/api.ts`：三个新封装；
  - `ui/src/lib/components/SettingsToggle.svelte`：新建组件（当前 `lib/components/` 目录不存在，顺便建出来）；
  - `ui/src/routes/NotificationsView.svelte`：header actions + row hover delete + 清空二次确认；
  - `ui/src/routes/SettingsView.svelte`：4 处 toggle 替换。
- **测试**：
  - `cdt-config::manager::tests` 新增 `update_notifications_persists_triggers` / `update_notifications_rejects_invalid_trigger`；
  - `cdt-config::notification_manager::tests` 新增 `delete_one_removes_single` / `delete_one_missing_returns_false`；
  - `cdt-api::tests` 新增集成测试覆盖 `update_config` 路径 triggers 写入可被 `get_enabled_triggers` 读出，以及三个新 IPC；
  - 前端仅 `svelte-check + tsc` 类型校验 + `just dev` 手测（toggle 持久化、批量操作、单条删除）。
- **依赖/兼容**：无新外部依赖；IPC 只增不减；配置文件 schema 不变。
- **回滚**：`update_notifications` 修改的行为是 bug fix，无需 feature flag。前端 UI 改动可独立 revert。
