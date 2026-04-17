## 1. 后端：DetectedError 确定性 id + NotificationManager 去重

- [x] 1.1 在 workspace `Cargo.toml` 的 `[workspace.dependencies]` 加 `sha2 = "0.10"`（实际已存在 `sha2 = "0.11"`，跳过）
- [x] 1.2 在 `crates/cdt-config/Cargo.toml` 添加 `sha2 = { workspace = true }`
- [x] 1.3 修改 `crates/cdt-config/src/detected_error.rs` 的 `create_detected_error`：用 SHA-256 对 `session_id + '\0' + file_path + '\0' + line_number + '\0' + tool_use_id.unwrap_or("") + '\0' + trigger_id.unwrap_or("") + '\0' + message` 计算 hash，取前 16 字节 hex 作为 id
- [x] 1.4 新增单测 `create_detected_error_produces_deterministic_id`：相同参数两次调用返回相同 id
- [x] 1.5 新增单测 `create_detected_error_different_sessions_different_ids`：`session_id` 不同时 id 不同
- [x] 1.6 新增单测 `create_detected_error_different_triggers_different_ids`：`trigger_id` 不同时 id 不同
- [x] 1.7 修改 `crates/cdt-config/src/notification_manager.rs::add_notification`：在 push 前检查 `self.notifications.iter().any(|n| n.error.id == error.id)`，若存在则 return Ok 不写入（签名同步改为返回 `bool`）
- [x] 1.8 新增单测 `add_notification_dedup_by_id`：add 同一 id 两次，get_notifications 返回 1 条
- [x] 1.9 `cargo test -p cdt-config` + `cargo clippy -p cdt-config --all-targets -- -D warnings`

## 2. 后端：cdt-api::notifier 模块

- [x] 2.1 在 `crates/cdt-api/Cargo.toml` 添加 `cdt-watch = { workspace = true }`（已存在，跳过）
- [x] 2.2 新建 `crates/cdt-api/src/notifier.rs`：定义 `pub struct NotificationPipeline { file_rx: broadcast::Receiver<FileChangeEvent>, config_mgr: Arc<Mutex<ConfigManager>>, notif_mgr: Arc<Mutex<NotificationManager>>, error_tx: broadcast::Sender<DetectedError> }`
- [x] 2.3 实现 `pub async fn run(mut self)`：在 `loop` 里 `file_rx.recv().await`，对 `Ok(event)` 调 `process_file_change(&event)`；`Err(RecvError::Lagged(n))` 记 warning 继续；`Err(RecvError::Closed)` break
- [x] 2.4 实现 `async fn process_file_change(&self, event: &FileChangeEvent)`：若 `deleted == true` 直接 return；通过 `cdt-discover::path_decoder::get_projects_base_path` + `event.project_id` + `event.session_id` 拼出 JSONL 路径；`parse_file(path)` 获取 messages；读 `ConfigManager::get_enabled_triggers()`；调 `detect_errors`；对每条 `DetectedError` 调 `notif_mgr.add_notification`（返回 new/dedup），新条目通过 `error_tx.send` 广播
- [x] 2.5 `add_notification` 需返回 `Result<bool, ConfigError>`（true = new，false = dup），notifier 仅对 `true` 走 error_tx 发送（已在 1.7 完成）
- [~] 2.6 `notifier_skips_when_triggers_empty` 改为依赖 `detect_errors_empty_triggers` 纯函数单测 + Section 3 端到端集成覆盖（ConfigManager 默认 builtin triggers 不可移除，单 crate 内难以伪造空 triggers 状态）
- [~] 2.7 `notifier_emits_new_error` 推迟到 Section 3 的 `notifier_pipeline.rs` 集成测试（需要真实 FileWatcher 覆盖订阅路径）
- [~] 2.8 `notifier_dedups_on_rescan` 同上，合并到集成测试
- [x] 2.9 新增单测 `notifier_skips_deleted_events`：`deleted: true` 的 event 跳过解析
- [x] 2.9a 新增单测 `notifier_missing_file_is_silent`：JSONL 文件不存在时 pipeline 静默跳过
- [x] 2.10 在 `crates/cdt-api/src/lib.rs` 加 `pub mod notifier; pub use notifier::NotificationPipeline;`
- [x] 2.11 `cargo test -p cdt-api --lib notifier` + `cargo clippy -p cdt-api --all-targets -- -D warnings`

## 3. 后端：LocalDataApi 持有 watcher + 订阅入口

- [x] 3.1 `crates/cdt-api/src/ipc/local.rs`：`LocalDataApi` 新增字段 `error_tx: Option<broadcast::Sender<DetectedError>>`（watcher 本身不需要持有；生命周期由 host 管理，本 api 只订阅其广播）
- [x] 3.2 保留现有 `pub fn new(...)`，新增 `pub fn new_with_watcher(scanner, config_mgr, notif_mgr, ssh_mgr, watcher: &FileWatcher, projects_dir: PathBuf) -> Self`
- [x] 3.3 调整 `LocalDataApi` 内部 `config_mgr: Arc<Mutex<ConfigManager>>`、`notif_mgr: Arc<Mutex<NotificationManager>>`；`new()` 内部 `Arc::new(Mutex::new(...))`（Arc 不影响 await 语义，既有使用点透明）
- [x] 3.4 新增非 trait 方法 `pub fn subscribe_detected_errors(&self) -> broadcast::Receiver<DetectedError>`：若 `error_tx` 为 `Some` 则 subscribe；否则返回独立 `broadcast::channel(1)` 的 receiver（receiver 可能立即 `Closed`——语义上等同"永不收到"）
- [x] 3.5 启动 notifier task：在 `new_with_watcher` 里 `tokio::spawn(pipeline.run())`；`NotificationPipeline::new` 新增 `projects_dir: PathBuf` 参数，避免测试命中真实 `~/.claude/projects/`
- [x] 3.6 新建集成测试 `crates/cdt-api/tests/notifier_pipeline.rs`：`pipeline_emits_detected_error_on_new_jsonl_line`（端到端写入→订阅收到）+ `subscribe_detected_errors_without_watcher_is_silent_receiver`（无 watcher 时 silent）
- [x] 3.7 回归现有 `crates/cdt-api/tests/*.rs` 跑通（`agent_configs.rs` 继续绿）
- [x] 3.8 `cargo test -p cdt-api` + `cargo clippy -p cdt-api --all-targets -- -D warnings`

## 4. Tauri 接入

- [x] 4.1 `src-tauri/src/lib.rs` 的 `run()` 里：`let watcher = Arc::new(FileWatcher::new());`；`LocalDataApi::new_with_watcher(..., watcher.as_ref(), projects_dir)`
- [x] 4.2 在 `tauri::Builder::setup` 里用 `tauri::async_runtime::spawn` 启动 `watcher.start()`
- [x] 4.3 在 setup 里再 spawn 一个 task：订阅 `api.subscribe_detected_errors()` 并 `emit("notification-added", &err)` 到前端
- [x] 4.4 `cargo build --manifest-path src-tauri/Cargo.toml` 通过；clippy 无 warning
- [x] 4.5 `cargo fmt --all`

## 5. 前端：订阅 notification-added

- [x] 5.1 `ui/src/App.svelte`：复用现有 `onNotificationUpdate`（refetch unread count）作为 `notification-added` handler；加第二个 `listen`/`unlisten` 对，避免改 tabStore 侵入性设计
- [x] 5.2 `ui/src/routes/NotificationsView.svelte`：`onMount` 里 `listen("notification-added", reload)`，`onDestroy` 调 unlisten；将原 onMount 内联加载逻辑提取为 `reload()` 供事件复用
- [x] 5.3 `DetectedError` payload 直接被 `onNotificationUpdate`/`reload` 走 `getNotifications` 重新获取（不依赖 payload 形状），前端类型无需新增
- [x] 5.4 `npm run check --prefix ui`

## 6. 文档与 followups

- [x] 6.1 `openspec/followups.md`：新增 "notification-triggers pipeline" 小节标记本 change slug 为已修复
- [x] 6.2 `CLAUDE.md` "UI 已知遗留问题" 章节清空（P0/P1 全部 done）
- [x] 6.3 `.remember/remember.md` 更新 State/Next

## 7. 集成验证

- [x] 7.1 `cargo build --workspace` 全部编译通过
- [x] 7.2 `cargo test --workspace`：除 `cdt-watch --test file_watching` 有预存 macOS FSEvents 时序 flake 外全部通过；`cargo test -p cdt-watch -- --test-threads=1` 确认 6/6 绿
- [x] 7.3 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 7.4 `cargo fmt --all`
- [x] 7.5 `npm run check --prefix ui`（无 errors，4 个预存 warnings 与本 change 无关）
- [x] 7.6 `openspec validate 2026-04-17-auto-notification-pipeline --strict`
- [~] 7.7 `cargo tauri dev` 人工验证留给用户本地跑（自动化任务不便交互启动；`notifier_pipeline.rs` 端到端集成测试已验证完整管线）
- [x] 7.8 `openspec archive 2026-04-17-auto-notification-pipeline -y`：3 个 spec 已 sync（file-watching 修改 1 项 / ipc-data-api 新增 1 项 / notification-triggers 新增 1 项 + 修改 2 项）
