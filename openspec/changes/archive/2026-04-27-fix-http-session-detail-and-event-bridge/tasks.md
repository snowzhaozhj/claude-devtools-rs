## 1. cdt-api：trait 加 `find_session_project`（capability `ipc-data-api`）

- [x] 1.1 `crates/cdt-api/src/ipc/traits.rs` 在 `DataApi` trait 加 `find_session_project(&self, session_id: &str) -> Result<Option<String>, ApiError>`，默认实现遍历 `list_projects` + `list_sessions_sync(project_id, { page_size: usize::MAX, cursor: None })`，命中即返；遍历完返 `Ok(None)`
- [x] 1.2 doc-comment 描述行为契约（命中口径、复杂度、`LocalDataApi` 覆盖能力差异），按 clippy `doc_markdown` 规则处理标识符 backtick

## 2. cdt-api：`LocalDataApi` 直扫覆盖与 `get_session_detail` 解耦（capability `ipc-data-api`）

- [x] 2.1 `crates/cdt-api/src/ipc/local.rs` 实现 `LocalDataApi::find_session_project`：用 `scanner.lock().await.projects_dir().to_path_buf()` 拿 projects_dir，`tokio::fs::read_dir` 遍历每个子目录，依次匹配主会话快路径 `<dir>/<sid>.jsonl` 与 `find_subagent_jsonl(<dir>, sid)` 慢路径；命中返 `Ok(Some(<dir filename as encoded project_id>))`，未命中返 `Ok(None)`
- [x] 2.2 把 `LocalDataApi::get_session_detail` 内 `let projects_dir = path_decoder::get_projects_base_path();` 改为 `let projects_dir = scanner.projects_dir().to_path_buf();`（在 `scanner.lock().await` 内取，避免重复加锁）；行为不变（默认 `ProjectScanner::new` 仍传 `path_decoder::get_projects_base_path()`），生产路径零回归
- [x] 2.3 把 `LocalDataApi::get_sessions_by_ids` 改为先 `find_session_project(sid)` 反查、再走 `get_session_detail(project_id, sid)`；反查 `Ok(None)` / `Err(_)` 时 push `metadata.status="not_found"` 占位条目，与既有兜底分支保持兼容

## 3. cdt-api：HTTP `get_session_detail` handler 改造（capability `http-data-api`）

- [x] 3.1 `crates/cdt-api/src/http/routes.rs::get_session_detail` 改为：先 `s.api.find_session_project(&session_id).await?`；`None` 返 `ApiError::not_found(format!("session {session_id}"))`；命中后调 `s.api.get_session_detail(&project_id, &session_id).await?`
- [x] 3.2 删除原 `get_session_detail("", &session_id)` 调用与"简化为空字符串"注释；用新注释说明 spec `Return safe defaults on lookup failures` 走 404 路径

## 4. cdt-api：SSE event bridge（capability `http-data-api`）

- [x] 4.1 新建 `crates/cdt-api/src/http/bridge.rs`，实现 `pub fn spawn_event_bridge(events_tx: tokio::sync::broadcast::Sender<PushEvent>, file_rx: tokio::sync::broadcast::Receiver<FileChangeEvent>, todo_rx: tokio::sync::broadcast::Receiver<TodoChangeEvent>, error_rx: tokio::sync::broadcast::Receiver<DetectedError>)`：spawn 三个 task（file / todo / detected_error），各自 loop `recv().await`，`Ok(event)` 转换为对应 `PushEvent` 变体后 `events_tx.send(...)`（忽略 `send` 返回的 `SendError`，无订阅者属正常），`Err(Lagged(_))` `continue`，`Err(Closed)` `break`
- [x] 4.2 todo bridge 的 `PushEvent::TodoChange.project_id` 字段填空字符串占位（`TodoChangeEvent` 仅含 `session_id`，spec delta 已记录约定）
- [x] 4.3 detected-error bridge 把 `DetectedError` 通过 `serde_json::to_value` 序列化为 `serde_json::Value` 后嵌入 `PushEvent::NewNotification { notification: <value> }`；序列化失败时 `tracing::warn!` 跳过当条
- [x] 4.4 `crates/cdt-api/src/http/mod.rs` `pub mod bridge;` + `pub use bridge::spawn_event_bridge;`
- [x] 4.5 `crates/cdt-api/src/lib.rs` 把 `spawn_event_bridge` 加到 `pub use http::{...}` 行

## 5. cdt-cli：启动路径补 producer（capability `http-data-api`）

- [x] 5.1 `crates/cdt-cli/src/main.rs` 顶部加 `use cdt_api::http::spawn_event_bridge;` 与 `use cdt_watch::FileWatcher;`
- [x] 5.2 home 解析改用 `cdt_discover::home_dir()`（不要 `dirs::home_dir()`，遵循 CLAUDE.md 跨平台路径硬约束），构造 `projects_dir = home.join(".claude").join("projects")` 与 `todos_dir = home.join(".claude").join("todos")`
- [x] 5.3 用 `FileWatcher::with_paths(projects_dir.clone(), todos_dir)` 创建 watcher；保留 scanner 仍用 `path_decoder::get_projects_base_path()` 入参（与现状一致）
- [x] 5.4 `LocalDataApi` 切换到 `LocalDataApi::new_with_watcher(scanner, config_mgr, notif_mgr, ssh_mgr, &watcher, projects_dir)`
- [x] 5.5 在 `start_server` 之前 `tokio::spawn(async move { if let Err(e) = watcher_clone.start().await { tracing::warn!(error=%e, "FileWatcher terminated"); } })`；并 `spawn_event_bridge(state.events_tx.clone(), watcher.subscribe_files(), watcher.subscribe_todos(), api.subscribe_detected_errors())`
- [x] 5.6 注意 watcher 与 api 所有权：`api` 在 `Arc::new(api)` 之前抽出 `subscribe_detected_errors()` receiver；watcher 走 `Arc<FileWatcher>` 或 `clone` `subscribe_*` 后再 spawn `start()`

## 6. 集成测试（capability `http-data-api` + `ipc-data-api`）

- [x] 6.1 新建 `crates/cdt-api/tests/http_session_detail_global_lookup.rs`：用 tmpdir 起 `LocalDataApi`，写两个 project（每个一条 fixture session jsonl），断言：(a) `find_session_project("sid-A")` 返回 project A 的 encoded id；(b) `find_session_project("sid-ghost")` 返 `Ok(None)`；(c) `get_session_detail(project_id, "sid-A")` 端到端跑通（reuse `session_metadata_stream::write_fixture_session` fixture helper 或本地复刻）
- [x] 6.2 加测试覆盖 subagent 反查：在 project 目录下写 `<parent>/subagents/agent-<sub_sid>.jsonl`，`find_session_project("<sub_sid>")` 返回该 project_id
- [x] 6.3 加测试覆盖 `get_sessions_by_ids` 混合存在性：传 `["sid-existing", "sid-ghost"]`，断言长度 2、第 1 条 `projectId` 非空、第 2 条 `metadata.status == "not_found"`
- [x] 6.4 新建 `crates/cdt-api/tests/sse_event_bridge.rs`：构造 `broadcast::channel<PushEvent>(64)` 拿 `events_tx`；分别构造 `broadcast::channel<FileChangeEvent>` / `<TodoChangeEvent>` / `<DetectedError>` 拿 `*_tx`；调 `spawn_event_bridge(events_tx.clone(), file_tx.subscribe(), todo_tx.subscribe(), error_tx.subscribe())`；订阅 `events_tx.subscribe()`；分别 `*_tx.send(...)` 各类事件，断言 events_rx 收到对应 `PushEvent` 变体且字段一致
- [x] 6.5 测试覆盖 `Lagged` 跳过：发送量超过 channel capacity 之后再发一条事件，断言 events_rx 仍能收到尾条事件（producer 没退出 loop）

## 7. 工程检查与归档

- [x] 7.1 `cargo clippy --workspace --all-targets -- -D warnings` 全绿
- [x] 7.2 `cargo fmt --all`
- [x] 7.3 `cargo test -p cdt-api`（含本 change 新增的两份集成测试）
- [x] 7.4 `cargo test --workspace --exclude cdt-watch`（cdt-watch macOS FSEvents flake，按 CLAUDE.md 例外条款单独跑）
- [x] 7.5 `cargo test -p cdt-watch`（macOS 上单 case 跑，flake 视环境）
- [x] 7.6 `npm run check --prefix ui`（无前端改动，跑过即可）
- [x] 7.7 `openspec validate fix-http-session-detail-and-event-bridge --strict`
- [x] 7.8 `openspec/followups.md` 加一条「SSE 增量补全 ssh-status / updater 事件源」（依赖 cdt-ssh / updater 加 broadcast 源；本 change 不解决）
- [x] 7.9 commit + push（PR title `fix(cdt-api): http session detail global lookup + SSE event bridge`）
- [x] 7.10 push 后调 `Agent({ subagent_type: "codex:codex-rescue", ... })` 跑 codex 异构二审（行为契约 + 并发桥接同时改，命中"必跑 codex"判据）；按反馈修完再 archive
