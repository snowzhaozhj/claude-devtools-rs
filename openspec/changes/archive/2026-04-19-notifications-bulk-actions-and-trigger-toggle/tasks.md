## 1. 修复 `update_notifications` 吞掉 triggers 字段的 bug（cdt-config）

- [x] 1.1 在 `crates/cdt-config/src/manager.rs::ConfigManager::update_notifications` 的 match 中新增 `"triggers"` 分支：反序列化成 `Vec<NotificationTrigger>`，对每条调用 `validate_trigger`，任一非法时返回 `ConfigError::validation` 携带 `id` + 错误列表；校验全通过后整体替换 `self.config.notifications.triggers` 并调用 `self.trigger_manager.set_triggers(list)` 同步
- [x] 1.2 `_ => {}` 分支改为 `_ => tracing::warn!(key = %k, "unknown notifications update key ignored")`
- [x] 1.3 `cdt-config::manager::tests` 新增 `update_notifications_persists_triggers`（写入后 `get_enabled_triggers` 反映新状态 + 磁盘文件含新 trigger）
- [x] 1.4 `cdt-config::manager::tests` 新增 `update_notifications_rejects_invalid_trigger`（含非法条目时返回 Err，且 `config` + `trigger_manager` 保持修改前状态）
- [x] 1.5 `cdt-config::manager::tests` 新增 `update_notifications_warn_on_unknown_key`（未知键不报错）
- [x] 1.6 `cargo clippy -p cdt-config --all-targets -- -D warnings` + `cargo test -p cdt-config`

## 2. `NotificationManager::delete_one` + 批量方法暴露（cdt-config）

- [x] 2.1 在 `crates/cdt-config/src/notification_manager.rs` 新增 `pub async fn delete_one(&mut self, notification_id: &str) -> Result<bool, ConfigError>`：`retain` 移除匹配 id、比较 len 变化、变则 `save()` 返回 `Ok(true)`，未变返回 `Ok(false)`；同时新增 `pub async fn clear_by_trigger_id(&mut self, trigger_id: &str) -> Result<usize, ConfigError>`（给 Group 3.2 使用，提前到 Group 2 一并落地）
- [x] 2.2 `cdt-config::notification_manager::tests` 新增 `delete_one_removes_single`（删除存在 id 返回 true + 存储减少 + unread_count 减少）
- [x] 2.3 `cdt-config::notification_manager::tests` 新增 `delete_one_missing_returns_false`（删不存在 id 返回 false + 不落盘，mtime 校验）
- [x] 2.4 `cdt-config::notification_manager::tests` 新增 `clear_by_trigger_id_removes_matching_only`（按 trigger_id 精准删除，不误伤其它 trigger）
- [x] 2.5 `cargo clippy -p cdt-config --all-targets -- -D warnings` + `cargo test -p cdt-config`

## 3. `DataApi` 三个新方法 + `LocalDataApi` 实现（cdt-api）

- [x] 3.1 `crates/cdt-api/src/ipc/traits.rs::DataApi` trait 添加：`async fn delete_notification(&self, id: &str) -> Result<bool, ApiError>`、`async fn mark_all_notifications_read(&self) -> Result<(), ApiError>`、`async fn clear_notifications(&self, trigger_id: Option<&str>) -> Result<usize, ApiError>`
- [x] 3.2 `crates/cdt-api/src/ipc/local.rs::LocalDataApi` 实现三个方法：`delete_notification` 调 `notif_mgr.delete_one`；`mark_all_notifications_read` 调 `notif_mgr.mark_all_as_read`；`clear_notifications(None)` 先 `get_notifications(usize::MAX, 0).total` 读 before 再 `clear_all`；`clear_notifications(Some(id))` 调 `clear_by_trigger_id(id)`
- [x] 3.3 Group 2 已落地 `clear_by_trigger_id` + 对应测试
- [x] 3.4 HTTP 路由同步：`crates/cdt-api/src/http/routes.rs` 注册 `DELETE /api/notifications/{id}`、`POST /api/notifications/mark-all-read`、`POST /api/notifications/clear`（后者支持 `{ "triggerId": "..." }` 可选 body）
- [x] 3.5 `crates/cdt-api/tests/notification_ops.rs` 新增 5 个集成测试覆盖 delete / mark-all / clear-all / clear-by-trigger 行为
- [x] 3.6 同文件含 `update_config_persists_triggers_and_get_enabled_reflects` 验证 `update_config("notifications", { triggers: [...] })` 落盘
- [x] 3.7 `cargo clippy -p cdt-api --all-targets -- -D warnings` + `cargo test -p cdt-api` 全绿（5 integration tests + 27 unit tests）

## 4. Tauri commands（src-tauri）

- [x] 4.1 `src-tauri/src/lib.rs` 新增 `#[tauri::command] async fn delete_notification(app, data, notification_id: String) -> Result<bool, String>`：调 `data.api.delete_notification(&id)`，成功 `app.emit("notification-update", ())`
- [x] 4.2 同上新增 `mark_all_notifications_read(app, data) -> Result<(), String>` 和 `clear_notifications(app, data, trigger_id: Option<String>) -> Result<usize, String>`，成功后均 `app.emit("notification-update", ())`
- [x] 4.3 在 `invoke_handler!` 宏中注册三个新 command
- [x] 4.4 `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 绿

## 5. 前端 API 封装（ui/src/lib/api.ts）

- [x] 5.1 新增 `deleteNotification(id: string): Promise<boolean>`
- [x] 5.2 新增 `markAllNotificationsRead(): Promise<void>`
- [x] 5.3 新增 `clearNotifications(triggerId?: string): Promise<number>`（undefined → 清空全部；参数用 `triggerId ?? null` 兼容 Tauri `Option<String>` 反序列化）
- [x] 5.4 `npm run check --prefix ui` 0 errors

## 6. `SettingsToggle.svelte` 组件

- [x] 6.1 新建 `ui/src/lib/components/SettingsToggle.svelte`：Props `{ enabled; onChange; disabled?; ariaLabel? }`；`<button role="switch" aria-checked>` + `<span class="toggle-thumb">`；focus-visible 用 switch-on 色
- [x] 6.2 `ui/src/app.css` 加 `--color-switch-on: #6366f1` + `--color-switch-off-track` + `--color-switch-thumb`，两套主题同步
- [x] 6.3 `npm run check --prefix ui` 0 errors

## 7. `NotificationsView.svelte` header 批量 + row 单条删除

- [x] 7.1 `lib/icons.ts` 新增 `CHECK_CHECK_SVG` / `CHECK_SVG` / `TRASH2_SVG` / `X_SVG` 常量；`NotificationsView.svelte` 导入三个新 api
- [x] 7.2 `$state clearPending` + `setTimeout` 3 秒自动复原；二次点击前清 timer 再调 `clearNotifications(undefined)`
- [x] 7.3 `onDestroy` 里清 `clearTimer` + 取消两个 `listen` 订阅（`notification-added` + `notification-update`）
- [x] 7.4 header 加 `.header-actions`：列表非空即显示 "全部已读"（无未读时 disabled 态）+ "清空"（clearPending 态切换红色 `.header-action-danger`）
- [x] 7.5 row 右端加 `.notif-row-btn-mark`（未读行 hover 显示 ✓，调 `markNotificationRead`）+ `.notif-row-btn-delete`（hover 显示 ×，调 `deleteNotification`）；两个按钮都 `e.stopPropagation()` 阻止冒泡
- [x] 7.6 `.notif-row-btn` 公共 hover 显现 + focus-visible 保持可见；mark 按钮 hover 灰调，delete 按钮 hover 红调
- [x] 7.7 `npm run check --prefix ui` 0 errors（手测移至 Group 9）

## 8. `SettingsView.svelte` 4 处 toggle 替换

- [x] 8.1 `import SettingsToggle from "../lib/components/SettingsToggle.svelte"`
- [x] 8.2 "自动展开 AI 组" 替换；注意：前端原代码传的是 `autoExpandAiGroups`（小 i），与后端 manager.rs 的 `autoExpandAIGroups`（大 AI）键名不一致，是**既存 bug 不在本 scope**，toggle 替换保持原键名不改，待后续 followup 处理
- [x] 8.3 "启用通知" + "提示音" 两个 toggle 替换
- [x] 8.4 trigger 列表的 `.trigger-toggle` 替换为 `<SettingsToggle>`
- [x] 8.5 移除不再使用的 `.toggle-btn` / `.toggle-on` / `.trigger-toggle` / `.trigger-toggle-on` CSS
- [x] 8.6 `npm run check --prefix ui` 0 errors

## 9. 端到端验证（手测，由用户执行）

- [ ] 9.1 `just dev` 启动；手动场景 A（核心 bug）：设置 → 通知 → 新建一个 custom trigger → 触发匹配使其产生通知 → 把该 trigger 的 toggle 切到禁用 → **通知应立即停止产生新条目**（修复前的 bug 场景）
- [ ] 9.2 手动场景 B（删除 trigger）：删除该 trigger → 后续不再产生通知 → 通知面板原有历史通知保留（符合本轮非 goal）
- [ ] 9.3 手动场景 C（批量操作）：有未读通知时 header 可见两个按钮；清空首次点击进红色确认态，3 秒后自动复原；确认态内再次点击清空列表；全部已读按钮按上之后 badge 归零
- [ ] 9.4 手动场景 D（toggle 样式）：4 处 toggle 渲染为 Linear 滑块；启用态紫色 thumb 靠右，禁用态灰色 thumb 靠左
- [ ] 9.5 重启 app（`pkill -f claude-devtools-tauri && just dev`）验证 toggle / 删除 trigger 均已持久化

## 10. 最终收束

- [x] 10.1 `just preflight`（fmt + lint + test + spec-validate）一把梭绿：workspace 全 crate test 0 failed、svelte-check 0 errors、openspec validate 21/21 pass
- [x] 10.2 `openspec validate notifications-bulk-actions-and-trigger-toggle --strict` 通过
- [ ] 10.3 生成 commit，准备 `opsx:archive`（等用户完成 §9 手测后执行）
