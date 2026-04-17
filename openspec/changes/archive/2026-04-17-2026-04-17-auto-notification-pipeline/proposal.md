## Why

`cdt-watch::FileWatcher`、`cdt-config::detect_errors`、`cdt-config::NotificationManager` 三个零件分别到位，但从未被任何一段代码串起来。用户在设置里配置好 trigger、并产生新的 session 错误时，Rust 端不会主动扫描、不会写入 `claude-devtools-notifications.json`、前端 TabBar badge 永远是 0。`NotificationsView` 里能看到的只有靠现有（未接入的）旧流程写入的数据或空列表。这是 `CLAUDE.md` "UI 已知遗留问题" 里遗留的最后一项 P0。

原版 `claude-devtools/src/main/services/NotificationOrchestrator.ts`（约 400 行）描述了完整管线：**FSWatcher → 增量扫描 → ErrorDetector → NotificationManager.add → IPC emit** 到 renderer。Rust 端把这条链落地是把"数据层 + UI 单点 CRUD"变成"真正的后台服务"。

目前 `create_detected_error` 使用 `Uuid::new_v4()` 生成非确定性 id，使得每次重新扫描同一行都会产生新 id，无法天然去重——必须在本次 change 里一并修掉，否则管线跑起来会被重复通知淹没。

## What Changes

- **BREAKING** `cdt-config::DetectedError::id` 改为**确定性哈希**（基于 `session_id + file_path + line_number + tool_use_id + trigger_id + message` 的 SHA-256 前 16 字节 hex），替换 `Uuid::new_v4()`；`NotificationManager::add_notification` 在写入前按 id 去重，已存在则 no-op
- 新增 crate 内模块 `cdt-api::notifier`：持有 `FileWatcher`/`ConfigManager`/`NotificationManager` 共享引用，订阅 `subscribe_files()` 广播，收到 `FileChangeEvent` 后按 session 解析 JSONL → `detect_errors` → `add_notification`，新产生的 `DetectedError` 通过新增的 `broadcast::Sender<DetectedError>` 向外广播
- `LocalDataApi` 暴露 `fn subscribe_detected_errors(&self) -> broadcast::Receiver<DetectedError>` 非 trait 方法，供 Tauri 侧订阅后 `emit("notification-added", ...)` 推到前端
- `LocalDataApi` 持有可选的 `FileWatcher` 句柄（通过 `new_with_watcher` 构造），避免改现有构造签名破坏既有 4 个集成测试
- `src-tauri` 在 `tauri::Builder::setup` 里 spawn 三个 task：`FileWatcher::start()`、notifier 主循环、`DetectedError` → `AppHandle::emit` 的 bridge
- 前端 `tabStore.svelte.ts` 除现有 30 秒轮询之外，订阅 `listen("notification-added")` 事件立即刷新 `notificationUnreadCount`；`NotificationsView` 也订阅同事件在前台时 reload 列表
- `openspec/followups.md` 标记本 change 修复了"FileWatcher 通知管道"遗留；`CLAUDE.md` 移除 "UI 已知遗留问题" 里的第一条

## Capabilities

### Modified Capabilities
- `notification-triggers`: 新增 "Automatic background pipeline" Requirement 与 4 个 Scenarios；修改 "Persist and expose notifications" Requirement 增加去重语义
- `file-watching`: 在 "Broadcast events to multiple subscribers" 下新增 Scenario 覆盖"notifier 作为订阅者"
- `ipc-data-api`: 新增 "Stream detected errors to subscribers" Requirement（非 trait 方法契约）

## Impact

- 代码：`crates/cdt-config/src/detected_error.rs`（id 生成改确定性）、`crates/cdt-config/src/notification_manager.rs`（add 去重）、`crates/cdt-api/src/notifier.rs`（新文件）、`crates/cdt-api/src/lib.rs`（pub use）、`crates/cdt-api/src/ipc/local.rs`（持有 watcher + 订阅方法）、`src-tauri/src/lib.rs`（setup 启动后台任务）、`ui/src/lib/tabStore.svelte.ts`（订阅 notification-added）、`ui/src/routes/NotificationsView.svelte`（同上）
- 依赖：新增 `sha2` workspace dep（确定性 id 哈希）；`cdt-api` 新增 `cdt-watch` path dep（之前不依赖）
- 测试：`cdt-config` 扩 detected_error id determinism + notification_manager dedup 测试；`cdt-api` 新集成测试 `notifier_pipeline.rs` 覆盖"写 JSONL → 收到 DetectedError"端到端（tmp 目录 + 真实 FileWatcher）；既有 `notifications.rs` 集成测试不动
- 数据迁移：既有 notifications.json 里历史 uuid 型 id 在重启后仍能 load（DetectedError 结构不变），只是新产生的 id 会是确定性短 hash，不影响前端 `mark_as_read(notification_id)` 路径
- 向后兼容：`create_detected_error` 签名不变，仅内部实现换算法；外部调用点全部透明
