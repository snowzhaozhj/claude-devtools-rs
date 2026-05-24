## ADDED Requirements

### Requirement: Unified invalidator 作为 `LocalDataApi.file_tx` 唯一生产者

`LocalDataApi::new_with_watcher(...)` 启动路径 SHALL 把 `spawn_unified_cache_invalidator` 升级为 `LocalDataApi.file_tx` 的**唯一**生产者，**不**再 spawn 任何独立的 `bridge_task` 把 `watcher.subscribe_files()` 直接转发到 `file_tx`。invalidator 内部 sync 跑完三档判定后 SHALL 把 enriched `FileChangeEvent`（带 `session_list_changed` 字段）通过 `file_tx.send(enriched)` 广播给下游消费者（Tauri host emit / HTTP `spawn_file_bridge` / 其它 `subscribe_file_changes` 调用方）。

SSH 路径 SHALL 通过 `cdt-watch::FileWatcher::attach_remote(sftp, projects_dir, cancel_token)`（扩展签名后的 `watcher.rs:101` API）接入 watcher broadcast，`LocalDataApi::attach_remote_watcher`（`local.rs:1760`）SHALL NOT 再走 `RemotePollingWatcher::spawn(..., self.file_tx.clone(), ...)` 路径——SSH event 必须经过同一 unified invalidator enrichment gateway，与 Local event 行为一致。`FileWatcher::attach_remote` 签名 SHALL 接受调用方注入的 `CancelToken`（替代原内部 `CancelToken::new()`），保留 `RemoteWatcherHandle` 返回值不变；调用方持有 token clone 用于 dead-signal monitor 路径，外部 disconnect 时仍能 cancel SSH polling。

**Emit 时机契约（unified invalidator loop 顺序）**：

1. `rx.recv().await` 收 raw event
2. sync 调 `apply_file_event_to_project_scan_cache(event)` 拿三档判定结果（返回 `bool` 表示 structural）；该函数内部锁在 sync block 末尾自动释放
3. 构造 `enriched_event = FileChangeEvent { session_list_changed: <step 2 返回值>, ..raw_event }`
4. 调 `file_tx.send(enriched_event)` broadcast emit（**锁已释放**，emit 永不在持锁路径）
5. async 调 `apply_file_event_to_parsed_cache(event).await`（**不**阻塞 emit）

emit MUST 在 step 4 完成（即 sync invalidate 之后，async parsed invalidate 之前）。这保证：(a) 前端拿到 file-change 时 `ProjectScanCache` 状态已是事件后的最新；(b) 前端无需等磁盘 stat I/O 完成；(c) `parsed_cache` 失效路径仍走 async 不阻塞 emit。

**反压**：`broadcast::Sender::send` 满时丢旧元素不阻塞，invalidator 自身永远不会被慢 subscriber 阻塞；slow subscriber 引发的 lag 走下游 bridge 的 `Lagged` 兜底（见 `Emit push events for file changes and notifications` Requirement 的 lag 兜底契约）。

**MUST NOT**：

- MUST NOT 与额外的 `bridge_task` 并存——unified invalidator 是 `file_tx` 唯一生产者，避免双 producer 引发的事件顺序与重复问题
- MUST NOT 让 SSH `polling_watcher` 直接生产到 `LocalDataApi.file_tx` ——必须经过 `FileWatcher::attach_remote` → watcher broadcast → unified invalidator 的统一路径
- MUST NOT 在 `RecvError::Lagged(_)` 路径调 `file_tx.send` 合成 enriched event（lag 期间事件已丢失，不应在前端制造不对应任何 raw event 的合成事件）

#### Scenario: unified invalidator 是 `file_tx` 唯一生产者

- **WHEN** `LocalDataApi::new_with_watcher` 构造完成，启动 watcher 桥任务
- **THEN** 启动路径 SHALL NOT spawn 任何独立的 `bridge_task` 把 `watcher.subscribe_files()` 直接转发到 `file_tx`
- **AND** `file_tx` 的所有事件 SHALL 来自 unified invalidator 的 `file_tx.send(enriched_event)` 调用（即 enriched event 流，含 `session_list_changed` 字段）

#### Scenario: SSH 路径走 attach_remote 进入 unified invalidator

- **WHEN** `LocalDataApi::attach_remote_watcher` 被调用（SSH 连接上时）
- **THEN** 实现 SHALL 调用 `FileWatcher::attach_remote(sftp, projects_dir, cancel_token)` 让 SSH polling 事件喂入 `watcher.file_tx`，且调用方 SHALL 注入自己持有的 `CancelToken`（用于 dead-signal monitor cancel 路径）
- **AND** SSH event SHALL 经过 unified invalidator 的三档判定（cache 无 SSH entry 时退化为只看 `project_list_changed || deleted`）
- **AND** enriched SSH event SHALL 通过 `file_tx.send` 广播给下游，与 Local event 形态一致
- **AND** 外部 disconnect 触发 `cancel_token.cancel()` 时 SHALL 让 SSH polling task 退出（dead-signal monitor 路径保持原行为）

#### Scenario: emit 顺序在 sync invalidate 之后、async parsed invalidate 之前

- **WHEN** unified invalidator loop 收到一条 raw `FileChangeEvent`
- **THEN** 实现 SHALL 先 sync 调 `apply_file_event_to_project_scan_cache` 拿 structural bool
- **AND** 然后 sync 调 `file_tx.send(enriched_event)` emit（锁已释放）
- **AND** 最后 async 调 `apply_file_event_to_parsed_cache(event).await`
- **AND** `file_tx.send` MUST NOT 在 cache lock 临界区内调用

#### Scenario: lag 路径 NOT emit synthetic event

- **WHEN** unified invalidator 的 `rx.recv().await` 返回 `Err(RecvError::Lagged(n))`
- **THEN** 实现 SHALL 调 `apply_lag_to_project_scan_cache`（保守 `invalidate_local()` + counter `lag_conservative`）
- **AND** MUST NOT 调 `file_tx.send` 合成 enriched event
- **AND** lag 兜底由下游 bridge 的 `RecvError::Lagged` 处理路径（`sse-lagged` 通知前端）承担

## MODIFIED Requirements

### Requirement: Emit push events for file changes and notifications

系统 SHALL 从 main 进程向 renderer 推送以下事件：session 文件变更、todo 文件变更、新通知、SSH 状态变化、context 切换、updater 进度。

桌面（Tauri）host SHALL 在 `setup` 阶段订阅 `LocalDataApi::subscribe_file_changes()` 广播（**enriched event 流**——见 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement），并向前端 webview `emit("file-change", payload)`。Payload SHALL 是 `FileChangeEvent` 的 camelCase 序列化结果，字段 SHALL 含 `projectId`、`sessionId`、`deleted`、`projectListChanged`、`sessionListChanged`，与其它 IPC payload 命名约定一致。

`sessionListChanged: bool` 字段语义：标记该 file-change 事件是否会改变某 group 内 session 集合（"已知 project 下首次见 session" / 删除 / 重命名等场景）。该字段值由 unified invalidator 的"三档判定"结果填充——`project_list_changed=true || deleted=true || (规则 2 命中)` → `sessionListChanged=true`；普通 JSONL append + watcher 折叠的 subagent 修改 → `sessionListChanged=false`。`#[serde(default, skip_serializing_if = "std::ops::Not::not")]` 让旧反序列化路径在缺字段时拿到 `false`，行为退化为"不触发 loadProjects 刷新"。

桌面 host SHALL 在 `setup` 阶段订阅 `LocalDataApi::subscribe_ssh_status()`，并向前端 webview `emit("ssh_status", payload)`。Payload SHALL 是 `SshStatusChange` 的 camelCase 序列化结果（字段 `contextId`、`status`、`error?`、`authChain?`）。同样订阅 `subscribe_context_changed()` emit `context_changed` 事件 payload `{ activeContextId, kind }`。

**HTTP/SSE 路径字段同步契约**：`crates/cdt-api/src/ipc/events.rs::PushEvent::FileChange` 变体 SHALL 含 `session_list_changed: bool` 字段（`#[serde(default)]`）。该 enum 既有 attribute 是 `#[serde(tag = "type", rename_all = "snake_case")]`——variant tag 转 `snake_case`（`FileChange` → `file_change`），但 struct variant 内部字段保留 Rust 字段名 `snake_case`（与既有 `project_id` / `project_list_changed` 风格一致）。SHALL NOT 在 enum 上加 `rename_all_fields = "camelCase"`——会破坏既有 SSE payload 与 `transport.ts::normalizePushPayload` 当前 snake_case 字段读取路径（`transport.ts:456-462`）。`crates/cdt-api/src/http/bridge.rs::spawn_file_bridge` SHALL 把 enriched `FileChangeEvent.session_list_changed` 透传到 `PushEvent::FileChange.session_list_changed`；前端 `transport.ts::normalizePushPayload` 在 `case "file_change"` 分支 SHALL 把 `payload.session_list_changed` 映射到归一化后的 `sessionListChanged`，与 IPC `FileChangeEvent`（`#[serde(rename_all = "camelCase")]`，原生输出 `sessionListChanged`）形态对齐。

**lag 兜底契约（三处来源同形态）**：

- **Tauri host file-change bridge** 在 `RecvError::Lagged(n)` 路径 SHALL 通过 `app.emit("sse-lagged", { source: "file-change", missed: n })` 通知前端 webview；`RecvError::Closed` 时退出 loop
- **HTTP `spawn_file_bridge`** 在 `file_rx.recv()` 返回 `RecvError::Lagged(n)` 路径 SHALL 调 `events_tx.send(PushEvent::SseLagged { source: "file-change", missed: n })`；MUST NOT 静默吞掉（codex round 2 阻塞 3：当前 `bridge.rs:56` `Err(Lagged) => {}` 是错误的兜底）
- **`PushEvent::SseLagged` variant**（`crates/cdt-api/src/ipc/events.rs`）：携带 `source: String` + `missed: u64`，序列化为 `{"type":"sse_lagged","source":"...","missed":...}`（variant tag 由 enum 既有 `rename_all = "snake_case"` 自动转换；字段 `source` / `missed` 单词无下划线，snake_case 形态即字面，不需要额外 rename attribute），与 `crates/cdt-api/src/http/sse.rs:23` 既有的 `SSE_LAGGED_SENTINEL = r#"{"type":"sse_lagged"}"#` 形态向后兼容（消费者解析 `type === "sse_lagged"` 走同一 handler；旧 sentinel 缺 source/missed 字段 → 前端读 undefined 不报错）
- 前端 `Sidebar` 收到 `sse-lagged` SHALL 按 `sidebar-navigation` spec `Sidebar SHALL 订阅 sse-recovered / sse-lagged 触发 silent refresh` Requirement 走保守 silent refresh，覆盖 lag 期间错过的 structural 信号

#### Scenario: New notification while renderer is subscribed

- **WHEN** renderer 已订阅通知事件，期间产出一条新通知
- **THEN** renderer SHALL 在 debounce 窗口内收到一条 push 事件，携带通知 payload

#### Scenario: Tauri 转发 file-change 事件（含 sessionListChanged 字段）

- **WHEN** `cdt-watch::FileWatcher` 在 100 ms debounce 后产出 `FileChangeEvent { project_id: "p", session_id: "s", deleted: false, project_list_changed: false, session_list_changed: false }`
- **AND** Tauri host 在 `setup` 已 spawn 桥任务订阅 `LocalDataApi::subscribe_file_changes()`
- **AND** unified invalidator 已完成判定（普通 append → `session_list_changed: false`）
- **THEN** webview SHALL 通过 `listen("file-change", ...)` 收到 payload `{ projectId: "p", sessionId: "s", deleted: false, projectListChanged: false, sessionListChanged: false }`

#### Scenario: file-change payload 是 camelCase

- **WHEN** Tauri 桥任务 emit 一条 `file-change` 事件
- **THEN** 序列化后的 JSON SHALL 使用 camelCase 字段名（`projectId` / `sessionId` / `deleted` / `projectListChanged` / `sessionListChanged`），与既有 IPC 类型约定一致

#### Scenario: 已知 project 下新 session 首次出现 sessionListChanged 为 true

- **WHEN** unified invalidator 收到 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false }` 且 `ProjectScanCache::contains_session_id` 返回 `false`（cache 已有 entry 但不含 sa_new）
- **THEN** invalidator SHALL 在 enrich 阶段把 `session_list_changed` 置为 `true`
- **AND** webview 通过 `listen("file-change", ...)` 收到的 payload SHALL 含 `sessionListChanged: true`

#### Scenario: 普通 JSONL append sessionListChanged 为 false

- **WHEN** unified invalidator 收到 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false }` 且 `ProjectScanCache::contains_session_id` 返回 `true`（cache 含 sa）
- **THEN** invalidator SHALL 在 enrich 阶段保持 `session_list_changed: false`
- **AND** webview 收到的 payload SHALL 含 `sessionListChanged: false`

#### Scenario: HTTP/SSE PushEvent::FileChange 携带 session_list_changed 字段

- **WHEN** 用户通过 `?http=1` 浏览器调试入口连接到 cdt-cli HTTP server
- **AND** 后端 unified invalidator 检测到"已知 project 下新 session"，enriched event `session_list_changed: true`
- **THEN** `PushEvent::FileChange` SHALL 序列化为含 `"session_list_changed": true` 的 SSE payload（snake_case，与既有 `project_id` / `project_list_changed` 字段风格一致；该 enum 既有 `#[serde(tag = "type", rename_all = "snake_case")]` attribute 不变）
- **AND** 前端 `transport.ts::normalizePushPayload` 在 `case "file_change"` 分支 SHALL 把 `payload.session_list_changed` 映射到 `sessionListChanged`（与既有 `projectListChanged` 映射一致），让 Sidebar handler 拿到与 Tauri 路径同形的 camelCase payload

#### Scenario: file_tx 满时 Tauri bridge 通知前端 sse-lagged

- **WHEN** Tauri host file-change bridge 的 `recv()` 返回 `Err(RecvError::Lagged(n))`（broadcast capacity 满 + slow subscriber）
- **THEN** bridge SHALL 调 `app.emit("sse-lagged", { source: "file-change", missed: n })` 通知前端
- **AND** bridge SHALL NOT 退出 loop，继续处理后续 event
- **AND** 前端 Sidebar 收到 `sse-lagged` 时按 `sidebar-navigation` spec 已有 Requirement 走 silent refresh 兜底

#### Scenario: HTTP spawn_file_bridge file_rx Lagged 走 PushEvent::SseLagged

- **WHEN** HTTP `spawn_file_bridge` 内 `file_rx.recv()` 返回 `Err(RecvError::Lagged(n))`（`LocalDataApi.file_tx → events_tx` 一跳的 lag）
- **THEN** bridge SHALL 调 `events_tx.send(PushEvent::SseLagged { source: "file-change", missed: n })`
- **AND** SHALL NOT 静默吞掉该 lag 信号
- **AND** SSE 客户端 SHALL 通过 `convert_broadcast_result` 把该 PushEvent 序列化为 `{"type":"sse_lagged","source":"file-change","missed":n}` 与既有 `SSE_LAGGED_SENTINEL` 形态对齐
- **AND** 前端浏览器 transport 解析 `type === "sse_lagged"` 走 `sse-lagged` handler

#### Scenario: PushEvent::SseLagged 序列化形态与 sentinel 兼容

- **WHEN** `PushEvent::SseLagged { source: "file-change".into(), missed: 7 }` 通过 serde_json 序列化
- **THEN** 输出 SHALL 是 `{"type":"sse_lagged","source":"file-change","missed":7}`（variant tag 由 enum `rename_all = "snake_case"` 自动转 `sse_lagged`；字段 `source` / `missed` 单词无下划线无需 rename）
- **AND** 该形态 SHALL 与 `crates/cdt-api/src/http/sse.rs::SSE_LAGGED_SENTINEL = r#"{"type":"sse_lagged"}"#` 向后兼容——前端解析 `type === "sse_lagged"` 走同一 handler；旧 sentinel 缺 source / missed 字段时前端读 undefined 不报错

#### Scenario: file-change 桥与通知管线并存

- **WHEN** Tauri host 同时持有 `LocalDataApi::subscribe_file_changes()`（emit `file-change`）与 `subscribe_detected_errors()`（emit `notification-added`）两个订阅
- **THEN** 两个桥 SHALL 独立运行，文件变更不会因通知 pipeline 的 lag 被丢弃，反之亦然

#### Scenario: ssh_status event broadcast on connect

- **WHEN** 后端 SSH 连接状态从 `connecting` 切到 `connected`
- **AND** Tauri host 在 setup 已 spawn 桥任务订阅 `subscribe_ssh_status()`
- **THEN** webview SHALL 通过 `listen("ssh_status", ...)` 收到 payload `{ contextId: "ssh-host-A", status: "connected" }`
- **AND** payload `error` 与 `authChain` 字段在 success 路径 SHALL 为 `null` 或省略

#### Scenario: ssh_status event carries error detail on failure

- **WHEN** SSH 连接失败导致状态切到 `error`
- **THEN** webview 收到的 `ssh_status` payload SHALL 含 `error: { code: "ssh_auth_exhausted", attempts: [...] }`

### Requirement: `ProjectScanCache` 按事件语义分级失效

`LocalDataApi::new_with_watcher(...)` 构造路径 SHALL spawn 后台 task（"unified invalidator"），订阅 `FileWatcher::subscribe_files()` 广播。该 task 对每条 `FileChangeEvent` SHALL 仅根据 `FileChangeEvent` 字段（`project_id` / `session_id` / `deleted` / `project_list_changed`）+ `ProjectScanCache` snapshot lookup 决定是否失效 `ProjectScanCache` Local entry。三档判定结果 SHALL 同步用于 enrich `session_list_changed` 字段后通过 `file_tx.send` 转发（详见 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement 的 emit 时机契约）。

**判定规则（三档）**：

1. `event.project_list_changed == true` **OR** `event.deleted == true` → 调 `ProjectScanCache::invalidate_local()`，inc counter `project_scan_cache.invalidate.structural`，enrich 时 `session_list_changed: true`
2. `event.session_id` 非空 **AND** (`ProjectScanCache::has_entry(local_ctx) == true` **OR** `ProjectScanCache::has_in_flight_scan() == true`) **AND** `ProjectScanCache::contains_session_id(local_ctx, &event.project_id, &event.session_id) == false`（cache 已有该 ctx 的 entry 或当前有 in-flight scan 在跑，且 snapshot 不含此 session）→ 同规则 1：`invalidate_local()` + structural counter + enrich `session_list_changed: true`
3. 其他（普通 JSONL append + watcher 折叠的 subagent 修改 + 空 sid 事件 + cache 无 entry **且**无 in-flight scan 时的任意非 structural 事件）→ **不**调任何失效 API，保留现有 cache，inc counter `project_scan_cache.invalidate.content_append_skipped`，enrich `session_list_changed: false`

**为何需要规则 2**：`cdt-watch::FileWatcher` 在构造时**预填**当前已存在的 project 目录到 `known_projects` HashSet（`crates/cdt-watch/src/watcher.rs:30-41,79`）。已知 project 下新建 session 时 `mark_project_seen` 不会返回 true，watcher 输出 `plc=false, deleted=false`——与"已知 session JSONL 追加"在事件字段上**外观完全相同**。仅靠 (plc, deleted) 两 bool 判定会让新 session 最长 `LOCAL_CACHE_TTL = 300s` 不可见。规则 2 用 cache snapshot 反向查询补这个语义缺口。

**为何需要 `has_entry || has_in_flight_scan` 守护组合**：

- **`has_entry` 单条件不足以防风暴**：lag 路径调 `invalidate_local()` 后 cache 被清空，若不守护，后续普通 append 事件 `contains_session_id` 一律返 false → unknown_session 命中 → 又调 `invalidate_local()` 反复 bump `invalidation_generation` → 在重扫期间 `finish_scan_with_insert` 因 generation mismatch 一直丢弃 snapshot → cache 长期无法 repopulate（持续重扫风暴）。`has_entry` 守护让 cache 空时直接走规则 3 等待业务路径重扫填回。

- **仅 `has_entry` 又会漏掉 in-flight scan 期间结构事件**：cache 空 + 业务路径已经 `begin_scan` 在跑 scan 期间到达"已知 project 下新 session"事件被吞 → generation 不 bump → scan 完成 `finish_scan_with_insert` 旧 snapshot 因 generation 未变成功落地 → 新 session 最长等 TTL 5min 才能看到。

- **联合条件 `has_entry || has_in_flight_scan` 二者兼得**：cache 有 entry 或 scan 在途时走规则 2 判定 bump；cache 空且无 scan 在途时走规则 3 不 bump。

**对各类真实 fs 事件的语义覆盖**（对应 `cdt-watch::FileWatcher::parse_project_event` 的输出）：

- 新 project 目录创建（`<projects_root>/<pid>` dir-create）→ watcher 输出 `plc=true, sid=""` → 走规则 1 → `session_list_changed: true`
- 启动后第一次见某 pid（典型场景：watcher 重启）→ watcher 输出 `plc=true` → 走规则 1 → `session_list_changed: true`
- **已知 project 下新 session 首次出现** → watcher 输出 `plc=false, deleted=false` 且 `contains_session_id == false` → 走规则 2 → `session_list_changed: true`
- 已知 project 已知 session JSONL 追加（普通 hot path）→ watcher 输出 `plc=false, deleted=false` 且 `contains_session_id == true` → 走规则 3 → `session_list_changed: false`
- watcher 折叠的 subagent JSONL **修改**（事件 `(pid, sid=父, deleted=false, plc=false)` + `contains_session_id(父 sid) == true`）→ 走规则 3 → `session_list_changed: false`
- 主 session JSONL 删除 → watcher 输出 `deleted=true` → 走规则 1 → `session_list_changed: true`
- watcher 折叠的 subagent JSONL **删除**（事件 `(pid, sid=父, deleted=true, plc=false)`）→ 走规则 1（**false-positive**：事件无法区分主 vs subagent 删除；触发一次重扫即结束，无正确性问题，详 design R6）→ `session_list_changed: true`

**MUST NOT**：

- MUST NOT 扩展或读取 `cdt-core::FileChangeEvent` 中除 `project_id` / `session_id` / `deleted` / `project_list_changed` 之外的其他字段做**判定**输入（`session_list_changed` 是判定**输出**写入 enriched event 的字段，不参与判定输入）
- MUST NOT 在事件回调路径内调任何 fs 操作（`fs::stat` / `fs::metadata` 等）—— 完全基于事件字段 + cache snapshot lookup 判定
- MUST NOT 引入 per-project 失效粒度（`ProjectScanCache.entries: HashMap<ContextId, Arc<Vec<Project>>>` 当前数据结构无 per-project entry 概念，per-project 重构超本 Requirement scope）

**`ProjectScanCache::contains_session_id` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `contains_session_id(&self, ctx: &ContextId, project_id: &str, session_id: &str) -> bool`，遍历指定 ctx 对应 entry 的 `Arc<Vec<Project>>`，定位 `Project.id == project_id` 后检查 `Project.sessions: Vec<String>` 是否含 `session_id`；ctx 无 entry 或 project 不存在时返回 `false`。复杂度 O(N project × N session_per_project)，对 30 project × 538 session corpus 单次 ~10µs，可在 hot 路径调用。

**`ProjectScanCache::has_entry` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `has_entry(&self, ctx: &ContextId) -> bool`，返回 `entries` 是否含此 ctx 的 entry。invalidator 在规则 2 判定前 SHALL 先用本方法守护——cache 空时跳过 unknown_session 判定，避免 lag 后被普通 append 事件持续触发 invalidate 导致重扫风暴。

**`ProjectScanCache::has_in_flight_scan` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `has_in_flight_scan(&self) -> bool`，返回当前 `in_flight_scans > 0`。invalidator 在规则 2 判定前 SHALL 与 `has_entry` 共同 OR 守护——cache 空但有 scan 在途时仍 bump generation，让 in-flight scan 完成回写时识别 race 丢弃 stale snapshot。

**`ProjectScanCache::begin_scan` / `finish_scan_with_insert` / `abort_scan` API 契约**：业务路径 `scan_projects_cached_with` SHALL 用 `begin_scan` 替代裸 `invalidation_generation()` 拿 recorded_generation 同时 `in_flight_scans += 1`；scan 成功时 SHALL 用 `finish_scan_with_insert` 替代 `try_insert`（内部 `in_flight_scans -= 1` + race 校验）；scan 失败时 SHALL 调 `abort_scan` 配对 `begin_scan` 不漏减。这三 API 联合保护 in-flight scan 与 invalidator 之间的 race 协议。

**SSH context entry 不受 file-change 影响**：watcher 是 Tauri 本地 fs 的硬不变量。invalidator 推算 `ContextId::local(projects_dir)` 决定失效作用域；`ProjectScanCache::invalidate_local()` 实现仅对 `FsKind::Local` entry 生效，SSH entry 仍按既有 TTL 自然过期。SSH `polling_watcher` 通过 `FileWatcher::attach_remote` 喂入同一 watcher broadcast 的事件，进入 unified invalidator 后 enrich `session_list_changed` 字段同样由三档判定统一计算（cache 为空 / 不含 SSH session 场景下规则 2 自然不命中，退化为仅看 `project_list_changed || deleted`，与 Local 行为一致）。

**`new()` 构造路径不启动该订阅**：`LocalDataApi::new()`（无 watcher 参数）SHALL NOT spawn 此 task；该场景仅依赖被动 generation 校验路径兜底，与 `MetadataCache` / `ParsedMessageCache` 在 `new()` 路径的行为对齐。

**broadcast lag 走保守全失效**：`broadcast::Receiver::recv` 返回 `Err(RecvError::Lagged(_))` 时 SHALL 调 `invalidate_local()` 并 inc counter `project_scan_cache.invalidate.lag_conservative`，因为 lag 期间可能错过 `plc=true` / `deleted=true` 事件且 `ProjectScanCache` 没有 path-level 被动校验机制可兜底。lag 路径 SHALL NOT emit 任何 enriched event 到 `file_tx`（broadcast lag 期间事件已经丢失，不应在前端制造一条不对应任何 raw event 的合成事件；lag 兜底由下游 bridge 的 `Lagged` 处理路径 emit `sse-lagged` pseudo-event 承担，详 `Emit push events for file changes and notifications` Requirement）。返回 `Err(RecvError::Closed)` 时 SHALL 退出 loop。

> 该 lag 行为与 `parsed-message 缓存按 file-change 广播主动失效` Requirement 的 lag 静默继续策略**有意不一致**：parsed-message cache 在 lookup 时 stat 比对 `FileSignature` 兜底 lag 错过的事件；ProjectScanCache 无类似被动校验，lag 时必须保守清空。

**telemetry counter 注册**：实现 SHALL 在 `crates/cdt-telemetry/src/registry.rs::COUNTER_NAMES` 静态白名单中注册以下 3 个 counter：

- `project_scan_cache.invalidate.structural`
- `project_scan_cache.invalidate.content_append_skipped`
- `project_scan_cache.invalidate.lag_conservative`

每条事件 SHALL 按规则结果 inc 对应 counter 各 1 次（`AtomicU64::fetch_add(1, Relaxed)`）。

**性能契约**：长时间使用场景（活跃 claude-code 会话每秒多次追加 JSONL）下，`content_append_skipped` 计数 SHALL 远超 `structural`（典型预期 ≥ 95% 走 skipped 分支）；偏离此预期是判定逻辑或 watcher 字段填充偏差的信号。

#### Scenario: 已知 session JSONL 追加 SHALL NOT 失效 cache

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，且 `ProjectScanCache` 已经因前一次 `list_repository_groups` 写入了某 ctx 的 entry，含 project `pa` 和 session `sa`
- **AND** `<projects_root>/pa/sa.jsonl` 被 claude-code 追加新行
- **AND** `FileWatcher` 广播一条对应 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false }`
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa")` 得到 `true`（规则 2 不命中）
- **AND** MUST NOT 调 `invalidate_local`
- **AND** `ProjectScanCache::lookup` 后续仍 SHALL 命中既有 entry（同一 `root_generation` / `context_generation` 下）
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1
- **AND** 通过 `file_tx` emit 的 enriched event SHALL 含 `session_list_changed: false`

#### Scenario: 已知 project 下新 session 首次出现 SHALL 失效 cache

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，`ProjectScanCache` 已写入某 ctx 的 entry，含已知 project `pa` 与已知 sessions `{sa1, sa2}`（`sa_new` 不在此列表）
- **AND** claude-code 在已知 project `pa` 下创建新 session `sa_new`，写入 `<projects_root>/pa/sa_new.jsonl`
- **AND** `FileWatcher::parse_project_event` 因 `mark_project_seen` 已在构造期将 `pa` 预填入 `known_projects`（参照 `crates/cdt-watch/src/watcher.rs:30-41,79`），返回 `false` → 广播 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false }`
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa_new")` 得到 `false`
- **AND** MUST 调 `ProjectScanCache::invalidate_local()`（规则 2 触发）
- **AND** 下一次 `list_repository_groups` SHALL 走 cache miss 并把 `sa_new` 纳入返回值
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1
- **AND** 通过 `file_tx` emit 的 enriched event SHALL 含 `session_list_changed: true`

#### Scenario: 顶层 dir-create 标 plc=true 时直接走规则 1

- **WHEN** `ProjectScanCache` 已存若干 ctx entry
- **AND** claude-code 创建新 project 顶层目录 `<projects_root>/p_new`
- **AND** `FileWatcher::parse_project_event` 检测到顶层 dir-create 分支，广播 `FileChangeEvent { project_id: "p_new", session_id: "", deleted: false, project_list_changed: true }`
- **THEN** 后台 invalidator MUST 仅基于 `event.project_list_changed == true` 走规则 1，调 `invalidate_local()`
- **AND** SHALL NOT 调 `contains_session_id`（事件 `session_id == ""` 触发规则 2 的 `!session_id.is_empty()` 守护跳过）
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1
- **AND** 通过 `file_tx` emit 的 enriched event SHALL 含 `session_list_changed: true`

#### Scenario: 删除已知 session JSONL SHALL 失效 cache

- **WHEN** `ProjectScanCache` 已存某 ctx entry 且内含 project `pa` / session `sa`
- **AND** 用户或外部工具删除 `<projects_root>/pa/sa.jsonl`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: true, project_list_changed: false }`
- **THEN** 后台 invalidator MUST 仅基于 `event.deleted == true` 走规则 1，调 `ProjectScanCache::invalidate_local()`
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1
- **AND** 通过 `file_tx` emit 的 enriched event SHALL 含 `session_list_changed: true`

#### Scenario: subagent JSONL 修改 SHALL NOT 失效 cache

- **WHEN** `ProjectScanCache` 已存某 ctx entry，含 project `pa` / 父 session `s_parent`
- **AND** claude-code 写入 `<projects_root>/pa/s_parent/subagents/agent-xyz.jsonl`
- **AND** `FileWatcher::parse_project_event` 识别为嵌套 subagent 形态（`components.len() == 4 AND components[2] == "subagents" AND filename starts_with "agent-"`），折叠到父 session，广播 `FileChangeEvent { project_id: "pa", session_id: "s_parent", deleted: false, project_list_changed: false }`
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "s_parent")` 得到 `true`
- **AND** MUST NOT 调任何失效 API
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1
- **AND** `ProjectScanCache::lookup` 后续仍 SHALL 命中既有 entry
- **AND** 通过 `file_tx` emit 的 enriched event SHALL 含 `session_list_changed: false`

#### Scenario: subagent JSONL 删除触发 false-positive invalidate（接受）

- **WHEN** `ProjectScanCache` 已存某 ctx entry，含 project `pa` / 父 session `s_parent`
- **AND** subagent 文件 `<projects_root>/pa/s_parent/subagents/agent-xyz.jsonl` 被删除
- **AND** `FileWatcher::parse_project_event` 折叠到父 session，广播 `FileChangeEvent { project_id: "pa", session_id: "s_parent", deleted: true, project_list_changed: false }`
- **THEN** 后台 invalidator MUST 基于 `event.deleted == true` 走规则 1，调 `ProjectScanCache::invalidate_local()`
- **AND** 这是已知的 **false-positive 行为**：事件字段无 path，无法区分主 session 删除 vs subagent 删除；本 spec 显式接受此 false-positive，触发一次 ProjectScanner 重扫的成本可接受
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1
- **AND** 通过 `file_tx` emit 的 enriched event SHALL 含 `session_list_changed: true`

#### Scenario: SSH context entry 不受 file-change 影响

- **WHEN** `ProjectScanCache` 已存 SSH ctx entry（由 SSH `polling_watcher` 间接触发或通过其它路径写入）
- **AND** 本地 `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false }`
- **THEN** unified invalidator MUST 调 `ProjectScanCache::invalidate_local()`，仅对 `FsKind::Local` entry 生效
- **AND** SSH ctx entry SHALL NOT 被失效，按既有 TTL 自然过期
