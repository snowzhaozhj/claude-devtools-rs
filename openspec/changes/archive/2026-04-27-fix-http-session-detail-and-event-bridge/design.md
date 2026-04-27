## Context

`cdt-api` 暴露两条对外能力：`ipc-data-api`（Tauri host 通过 `LocalDataApi` 直接调用）与 `http-data-api`（headless / 浏览器 / 第三方客户端通过 axum 走 HTTP/SSE）。Tauri host 在 `src-tauri/src/lib.rs` 已经把 `FileChangeEvent` / `SessionMetadataUpdate` / `DetectedError` 桥到前端 emit 通道，UI 实时刷新工作良好；但 HTTP 路径长期处于"骨架可用、关键操作不可用"状态：

- HTTP `GET /api/sessions/:id` handler 把 `project_id` 写死成 `""`，下沉后路径与 scanner 都退化为不可用值，导致 client 永远拿到 `404 not_found`。`get_sessions_by_ids` 是同根因。
- `AppState.events_tx` 只有订阅端（`sse_handler`）没有发送端：`cdt-cli/main.rs` 走 `LocalDataApi::new(...)` 不带 watcher，没人把后端事件转发到这个 channel，SSE 客户端连上后永远空。

`PushEvent`、`LocalDataApi::new_with_watcher`、`subscribe_detected_errors`、`FileWatcher::subscribe_files`/`subscribe_todos` 都已存在；缺的是把它们粘合起来的胶水代码。本 change 即修这两条 P1 + 抽出公共 bridge API 让未来 host 复用。

历史上 `LocalDataApi::get_session_detail` 内部硬编码 `path_decoder::get_projects_base_path()`（多处共用），导致集成测试无法 cover 这条 IPC 路径——既有 `tests/get_session_detail_with_teammate.rs` 文件头注释明确点了这件事。本 change 顺手把 `get_session_detail` 切到 `scanner.projects_dir()`，留出测试入口；其余 3 处（`get_subagent_trace`、`get_image_asset`、`get_tool_output`）暂不动，避免 PR 失焦。

## Goals / Non-Goals

**Goals:**

- HTTP `GET /api/sessions/:id` 真正能取到 detail；不存在时 `404` + `code=not_found`，符合 spec `Return safe defaults on lookup failures`。
- `get_sessions_by_ids` 不再产生空字符串 project_id 的 detail。
- HTTP SSE 客户端连接后能收到 `file-change` / `todo-change` / `new-notification` 三类 `PushEvent`。
- 集成测试覆盖：`find_session_project` 反查、HTTP detail 路径端到端、event bridge 三类事件转发。
- 把 bridge 抽成 `cdt_api::http::spawn_event_bridge` 公共 API，方便未来 src-tauri host 或其他 runtime 复用。

**Non-Goals:**

- 不补 `PushEvent::SshStatusChange` / updater 事件源（`cdt-ssh` / updater 现无 broadcast 源，跨 capability 改动单列 followup）。
- 不重构其余 3 处硬编码 `path_decoder::get_projects_base_path()`（`get_subagent_trace` / `get_image_asset` / `get_tool_output`）。这些方法 scanner 锁内不需要 projects_dir，改动收益小风险大，留 followup。
- 不做反查结果缓存或索引（首次实现 FS 直扫即可，未来量大再做）。
- 不改 IPC 字段 / Tauri command 列表 / 前端代码（contract test / EXPECTED_TAURI_COMMANDS 不动）。

## Decisions

### D1 — `GET /api/sessions/:id` 用全局反查，**不**加 query param

**候选方案：**
- A：handler signature 改成 `Query(projectId)`，强制 client 传 `?projectId=...`。
- B：handler 不变（path 仅 session_id），后端反查 project_id。
- C：URL 重构为 `/api/projects/:projectId/sessions/:sessionId`。

**选 B**。理由：
1. spec `http-data-api` Scenario `GET session detail` 明文 URL 是 `GET /api/sessions/:id`，没有 project_id；选 A / C 是修 spec 而非修 bug。
2. 客户端拿到 session_id 时常常没有 project_id 上下文（搜索结果只携带 session_id；`get_sessions_by_ids` 入参也只有 ids）。后端反查是更自然的语义。
3. 性能：`LocalDataApi` 直扫 `read_dir(projects_dir)` 是 O(项目数)，每个目录 `metadata(<sid>.jsonl)` O(1)；命中一般在前几个目录。fallback 慢路径（subagent）每个目录再做一次 read_dir + metadata，整体开销可接受。未来若量大可加 `(session_id → project_id)` 缓存。

### D2 — `find_session_project` 放 `DataApi` trait 还是仅 HTTP 层 hack

**候选方案：**
- A：仅 HTTP layer downcast 到 `LocalDataApi` 私有方法。
- B：加 `DataApi` trait 方法（默认实现 + `LocalDataApi` 覆盖）。

**选 B**。理由：
1. `get_sessions_by_ids` 在 trait 层就存在同样需求（缺 project_id），HTTP 层 hack 解决不了它。
2. 默认 `list_projects + list_sessions_sync` fallback 让远端 / mock 实现也能跑（慢但正确），保留 trait 抽象的"transport-agnostic"性质。
3. 加方法是兼容变更（带默认实现），现有实现不被打断；无 IPC 字段动静，contract test 不受影响。

### D3 — SSE producer 放 `cdt-api::http` library 还是 `cdt-cli` inline

**候选方案：**
- A：`spawn_event_bridge(events_tx, file_rx, todo_rx, error_rx)` 公开在 `cdt-api::http`，cdt-cli 调用。
- B：`cdt-cli/main.rs` inline 三个 `tokio::spawn` 转发循环。

**选 A**。理由：
1. 桥接逻辑（`recv` 循环 + `Lagged` 跳过 + `Closed` 退出）是 SSE / 后续多路客户端复用模式，集中在 lib 层有利于维护一致性。
2. 集成测试只能 unit-test lib 层公开符号；放 cdt-cli 里就只能起进程黑盒测，回归成本飞涨。
3. 接口签名 `(events_tx, file_rx, todo_rx, error_rx)` 让上层选订阅源——src-tauri host 未来如要透出 HTTP SSE 给浏览器调试时可直接复用同一 bridge。

### D4 — `LocalDataApi::get_session_detail` 改用 `scanner.projects_dir()`

**候选方案：**
- A：保留 `path_decoder::get_projects_base_path()`，集成测试通过 mock home env 跑。
- B：改用 `scanner.projects_dir()`（scanner 已经持有正确目录）。
- C：在 `LocalDataApi` 里加显式 `projects_dir: PathBuf` 字段，构造器传入。

**选 B**。理由：
1. `ProjectScanner` 构造时已经接受 `projects_dir: PathBuf`（`cdt-cli/main.rs` 默认传 `path_decoder::get_projects_base_path()`，scanner 与 detail 路径统一）。
2. 选 A 需要 mock `HOME` env，跨平台 / 并行测试 flake 风险大（CLAUDE.md 已经提过 `dirs::home_dir()` 的坑）。
3. 选 C 引入冗余字段（scanner 已经有一份），违反"single source of truth"。
4. 副作用零：默认构造路径不变，生产 home 解析逻辑保留。

### D5 — `find_session_project` 默认实现 vs LocalDataApi 覆盖的查找口径

**口径必须一致**：默认实现走 `list_projects` + `list_sessions_sync`，仅命中**主会话**；`LocalDataApi` 覆盖额外命中 subagent jsonl（`agent-<sid>.jsonl` legacy + `<parent>/subagents/agent-<sid>.jsonl` new）。

理由：`get_session_detail` 自己就接受 subagent session_id（`find_subagent_jsonl` fallback 路径），如果 `find_session_project` 不返回 subagent 所在 project，HTTP `GET /api/sessions/<subagent-sid>` 会不一致地 404。所以 `LocalDataApi::find_session_project` 必须复用 `find_subagent_jsonl`。默认实现因为只能走 `list_sessions_sync`（不含 subagent），覆盖率会差，但在远端 trait 实现里这是已知 trade-off——本仓只用 LocalDataApi，影响为零。

### D6 — `RecvError::Lagged(_)` 处理

**选 `continue`**（与 src-tauri host 现有桥模式一致，参见 `src-tauri/src/lib.rs:566-568`）。理由：file-change / todo-change 事件本质 hint，下次同 session 文件再变会重新触发；丢一两条不影响最终一致性。`Closed` 才退出 loop。

## Risks / Trade-offs

- **[Risk] 反查在大仓库慢** → Mitigation：`LocalDataApi` 用 `read_dir(projects_dir)` + `metadata(<sid>.jsonl)` 直扫，命中常见 case ≤ 几 ms；上千个项目时最坏 O(项目数)，可接受。未来发现瓶颈再加缓存 / 索引。
- **[Risk] 默认 trait 实现 fallback 不覆盖 subagent** → Mitigation：spec delta 明确点出"主会话覆盖 + subagent 视实现而定"；本仓 `LocalDataApi` 覆盖含 subagent 路径，远端实现按需自己加。
- **[Risk] `get_session_detail` 切 `scanner.projects_dir()` 引发回归** → Mitigation：默认 `ProjectScanner::new` 仍传 `path_decoder::get_projects_base_path()`，生产路径完全一致；新加 `http_session_detail_global_lookup` 测试在 tmpdir 跑端到端。
- **[Risk] `spawn_event_bridge` 漏退出** → Mitigation：每个 task 内 `match recv()` 显式覆盖 `Lagged` / `Closed`，`Closed` 时 break；caller 持有的 `events_tx` drop 会自然关闭订阅，task 退出。无 leak。
- **[Trade-off] SSE 当前 only 三类事件**：`PushEvent::SshStatusChange` / updater 暂缺 producer。spec delta 明确"本 change 仅承诺 file/todo/notification"，SSH / updater 列 followup；SSE 客户端不会收到这两类，但也不破坏现有契约（spec 的 SSE Requirement 列表里它们仍存在，只是实现待补——followups.md 记录）。

## Migration Plan

无运行时迁移：
- 默认 `ProjectScanner` 构造仍指向 `~/.claude/projects/`，老用户行为不变。
- HTTP `/api/sessions/:id` 之前是 100% 404，现在能拿到 detail——纯 bug fix，无回退需求。
- `find_session_project` 是新方法，老 `LocalDataApi` 实例（如有外部嵌入）走默认 fallback 仍正确。

回滚：本 change 全部走代码 PR；如发现回归，`git revert` 即可，无持久状态变化。

## Open Questions

无。所有决策点已敲定。
