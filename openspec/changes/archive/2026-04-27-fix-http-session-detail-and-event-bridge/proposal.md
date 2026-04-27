## Why

Codex 项目审计（2026-04-27）抓出 HTTP API 两条 P1 缺陷：(1) `GET /api/sessions/{session_id}` 永远 404——`crates/cdt-api/src/http/routes.rs` handler 把 `project_id` 写死为空串，下沉到 `LocalDataApi::get_session_detail("", sid)` 后路径与 scanner 都拿不到任何东西，`get_sessions_by_ids` 也是同根因；(2) `GET /api/events` 的 SSE 永远空——`AppState.events_tx` 只有订阅者没有 producer，`cdt-cli/main.rs` 既没构造 `FileWatcher` 也没把任何事件桥到 `events_tx`。spec `http-data-api` 的 `Serve projects and sessions over HTTP under /api prefix` 与 `Push events via Server-Sent Events` 两条 Requirement 实际未满足——非 Tauri 客户端（headless / 浏览器）当前完全用不了 HTTP API。

## What Changes

- HTTP `GET /api/sessions/:id` 改为：先调新增 trait 方法 `DataApi::find_session_project(session_id)` 反查所属 project_id，再走 `get_session_detail(project_id, session_id)`；找不到走既有 `Return safe defaults on lookup failures` 路径返 `404 not_found`。
- `LocalDataApi::get_sessions_by_ids` 同步切换到 `find_session_project + get_session_detail` 的复合路径；不再调 `get_session_detail("", sid)`。
- `DataApi` trait 新增 `find_session_project(&self, session_id: &str) -> Result<Option<String>, ApiError>`，默认实现遍历 `list_projects` + `list_sessions_sync`；`LocalDataApi` 直扫 `scanner.projects_dir()`（匹配 `<sid>.jsonl` 主会话或 `find_subagent_jsonl` 三种结构）覆盖。
- `LocalDataApi::get_session_detail` 内部从 `path_decoder::get_projects_base_path()` 改为 `scanner.projects_dir()`，让集成测试能在 tmpdir 下覆盖端到端路径（其余 3 处硬编码读 home 的位置作为 followup 不动）。
- 新增 `cdt_api::http::spawn_event_bridge(events_tx, file_rx, todo_rx, error_rx)`：spawn 三个 task 把 `FileChangeEvent` / `TodoChangeEvent` / `DetectedError` 转发为对应 `PushEvent` 变体写入 `events_tx`；接受 `RecvError::Lagged(_)` 时跳过当条继续 loop（与现有 src-tauri host 桥模式一致），`Closed` 时退出。
- `cdt-cli/main.rs` 改为：`FileWatcher::with_paths(<projects_dir>, <todos_dir>)` 用 `cdt_discover::home_dir()` 解析的 home 构造、`LocalDataApi::new_with_watcher(... watcher, projects_dir)`、spawn `watcher.start()`、调 `spawn_event_bridge` 喂 `AppState.events_tx`。
- 不在本 change 范围（明确点名以免 reviewer 误以为遗漏）：`PushEvent::SshStatusChange` 与 updater 事件源——`cdt-ssh` 当前没有 broadcast 源，`cdt-api` 的 updater 事件目前只走 Tauri emit；这两条作为 followup 单列。

## Capabilities

### New Capabilities

无。本 change 不引入新 capability。

### Modified Capabilities

- `http-data-api`：(a) `Serve projects and sessions over HTTP under /api prefix` 增补 Scenario 描述 `GET /api/sessions/:id` 的全局反查语义；(b) `Push events via Server-Sent Events` 明确 SHALL 在启动 HTTP server 时同时启动 file/todo/notification 三个 producer 把事件桥到 SSE 通道。
- `ipc-data-api`：新增 Requirement `Resolve project id from session id alone`（描述 `find_session_project` 行为契约：`Ok(Some)` 命中、`Ok(None)` 未命中、不依赖 caller 传 project_id）。

## Impact

- 代码：`crates/cdt-api/src/ipc/{traits.rs, local.rs}`、`crates/cdt-api/src/http/{routes.rs, mod.rs, bridge.rs(新)}`、`crates/cdt-api/src/lib.rs`、`crates/cdt-cli/src/main.rs`。
- 测试：`crates/cdt-api/tests/http_session_detail_global_lookup.rs`（新）、`crates/cdt-api/tests/sse_event_bridge.rs`（新）；`tests/ipc_contract.rs::get_session_detail_missing_session_returns_error` 继续守护错误路径。
- 公开 API：`cdt_api::http::spawn_event_bridge` 新增 lib-level 公开符号；`DataApi::find_session_project` 新增 trait 方法（带默认实现，兼容现有实现）。
- 依赖 / 配置 / IPC 字段：无变化。`EXPECTED_TAURI_COMMANDS` 与 invoke_handler 不动（Tauri host 不暴露 `find_session_project` 给前端，仅 HTTP / 后端复用）。
- followups.md 增一条：「SSE 增量补全 ssh-status / updater 事件源」（依赖 cdt-ssh 加 broadcast）。
