## 1. 依赖准备

- [ ] 1.1 在 workspace `Cargo.toml` `[workspace.dependencies]` 中添加 `notify-debouncer-mini`（版本 `0.4`，适配 notify v6 生态）
- [ ] 1.2 在 `crates/cdt-watch/Cargo.toml` 中引用 `notify-debouncer-mini = { workspace = true }`

## 2. cdt-core：共享事件类型

- [ ] 2.1 在 `crates/cdt-core/src/` 新增 `watch_event.rs`，定义 `FileChangeEvent`（含 `project_id: String`、`session_id: String`、`deleted: bool`）和 `TodoChangeEvent`（含 `session_id: String`）；两者均 `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]`
- [ ] 2.2 在 `crates/cdt-core/src/lib.rs` `pub mod watch_event; pub use watch_event::{FileChangeEvent, TodoChangeEvent};` 导出

## 3. cdt-watch：核心实现

- [ ] 3.1 新增 `crates/cdt-watch/src/watcher.rs`：定义 `FileWatcher` 结构体，持有 `broadcast::Sender<FileChangeEvent>` 和 `broadcast::Sender<TodoChangeEvent>`；暴露 `FileWatcher::new() -> Self`、`subscribe_files() -> broadcast::Receiver<FileChangeEvent>`、`subscribe_todos() -> broadcast::Receiver<TodoChangeEvent>`、`async fn start(self) -> Result<(), WatchError>`
- [ ] 3.2 在 `start()` 内，使用 `notify_debouncer_mini::new_debouncer(Duration::from_millis(100), callback)` 同时监听 `~/.claude/projects/`（递归）和 `~/.claude/todos/`（非递归）
- [ ] 3.3 实现事件路由：projects 目录下 `.jsonl` 文件事件 → 解析 `project_id` / `session_id` → 广播 `FileChangeEvent`；todos 目录下 `.json` 事件 → 解析 `session_id` → 广播 `TodoChangeEvent`
- [ ] 3.4 实现 deleted 标记：`notify::EventKind::Remove` 时 `FileChangeEvent.deleted = true`
- [ ] 3.5 新增 `crates/cdt-watch/src/error.rs`：`#[derive(thiserror::Error, Debug)] pub enum WatchError { #[error("watcher init failed: {0}")] Init(#[from] notify::Error) }`
- [ ] 3.6 在 `crates/cdt-watch/src/lib.rs` 替换 `stub()` 占位符，`pub mod watcher; pub mod error; pub use watcher::FileWatcher; pub use error::WatchError;`
- [ ] 3.7 瞬时错误处理：在事件回调中，遇到 `notify::Error`（非致命）时 `tracing::warn!` 并 continue，不向外传播

## 4. 测试

- [ ] 4.1 在 `crates/cdt-watch/tests/` 新增 `file_watching.rs`；每个 `#[tokio::test]` 使用 `tempfile::TempDir` 创建隔离目录，通过 `FileWatcher::new()` 指定自定义路径（需在 `FileWatcher::new` 接受可配置路径以支持测试）
- [ ] 4.2 测试 Scenario "New session file created"：写入 `.jsonl` 文件后接收到 `FileChangeEvent`，`deleted == false`，`project_id` / `session_id` 正确
- [ ] 4.3 测试 Scenario "Existing session file appended"：对已存在 `.jsonl` 写入字节后接收到 `FileChangeEvent`
- [ ] 4.4 测试 Scenario "Session file deleted"：删除 `.jsonl` 后接收到 `FileChangeEvent` 且 `deleted == true`
- [ ] 4.5 测试 Scenario "Todo file updated"：写入 `<sessionId>.json` 后接收到 `TodoChangeEvent`，`session_id` 正确
- [ ] 4.6 测试 Scenario "Burst of writes"（去抖）：使用 `tokio::time::pause()` + `advance()` 在 30ms 内注入 5 次事件，确认订阅者仅收到 1 个事件
- [ ] 4.7 测试 Scenario "Two subscribers present"：两个 `subscribe_files()` 各接收到事件恰好一次

## 5. 质量校验

- [ ] 5.1 `cargo clippy -p cdt-watch -p cdt-core --all-targets -- -D warnings` 零警告
- [ ] 5.2 `cargo fmt --all` 无变更
- [ ] 5.3 `cargo test -p cdt-watch -p cdt-core` 全绿
- [ ] 5.4 `openspec validate port-file-watching --strict` 通过
- [ ] 5.5 更新根 `CLAUDE.md` Capability→crate 表中 `file-watching` 行为 `done ✓`
