## MODIFIED Requirements

### Requirement: `ProjectScanCache` 按事件语义分级失效

`LocalDataApi::new_with_watcher(...)` 构造路径 SHALL spawn 后台 task（"unified invalidator"），订阅 `FileWatcher::subscribe_files()` 广播。该 task 对每条 `FileChangeEvent` SHALL 仅根据 `FileChangeEvent` 字段（`project_id` / `session_id` / `deleted` / `project_list_changed`）+ `ProjectScanCache` snapshot lookup 决定**是否**失效 `ProjectScanCache` Local entry。三档判定结果 SHALL **仅**用于 invalidate 决策，**不**再用于填写 `FileChangeEvent.session_list_changed` 字段——后者由 watcher 层负责（详 `file-watching` Requirement `跟踪 session 首见性以填写 revalidation hint`）。本 Requirement 同时 SHALL 把 cache snapshot 视角的"unknown_session"判定结果作为**辅助 hint** 暴露给 unified invalidator emit 路径，与 watcher 字段做并集 OR（详 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement 的 emit 公式）。

**判定规则（三档，仅决定 invalidate）**：

1. `event.project_list_changed == true` **OR** `event.deleted == true` → 调 `ProjectScanCache::invalidate_local()`，inc counter `project_scan_cache.invalidate.structural`
2. `event.session_id` 非空 **AND** (`ProjectScanCache::has_entry(local_ctx) == true` **OR** `ProjectScanCache::has_in_flight_scan() == true`) **AND** `ProjectScanCache::contains_session_id(local_ctx, &event.project_id, &event.session_id) == false`（cache 已有该 ctx 的 entry 或当前有 in-flight scan 在跑，且 snapshot 不含此 session）→ 同规则 1：`invalidate_local()` + structural counter
3. 其他（普通 JSONL append + watcher 折叠的 subagent 修改 + 空 sid 事件 + cache 无 entry **且**无 in-flight scan 时的任意非 structural 事件）→ **不**调任何失效 API，保留现有 cache，inc counter `project_scan_cache.invalidate.content_append_skipped`

**为何需要规则 2**：`cdt-watch::FileWatcher` 在构造时**预填**当前已存在的 project 目录到 `known_projects` HashSet。已知 project 下新建 session 时 `mark_project_seen` 不会返回 true，watcher 输出 `plc=false, deleted=false`——与"已知 session JSONL 追加"在 `project_list_changed` / `deleted` 字段上**外观完全相同**。仅靠 (plc, deleted) 两 bool 判定会让新 session 最长 `LOCAL_CACHE_TTL = 300s` 不可见。规则 2 用 cache snapshot 反向查询补这个语义缺口决定是否清缓存。watcher 层填写的 `session_list_changed` 字段为 emit 路径直接提供前端 revalidate hint，不依赖 cache 状态，与本规则的 invalidate 决策独立。

**为何需要 `has_entry || has_in_flight_scan` 守护组合**：

- **`has_entry` 单条件不足以防风暴**：lag 路径调 `invalidate_local()` 后 cache 被清空，若不守护，后续普通 append 事件 `contains_session_id` 一律返 false → unknown_session 命中 → 又调 `invalidate_local()` 反复 bump `invalidation_generation` → 在重扫期间 `finish_scan_with_insert` 因 generation mismatch 一直丢弃 snapshot → cache 长期无法 repopulate（持续重扫风暴）。`has_entry` 守护让 cache 空时直接走规则 3 等待业务路径重扫填回。

- **仅 `has_entry` 又会漏掉 in-flight scan 期间结构事件**：cache 空 + 业务路径已经 `begin_scan` 在跑 scan 期间到达"已知 project 下新 session"事件被吞 → generation 不 bump → scan 完成 `finish_scan_with_insert` 旧 snapshot 因 generation 未变成功落地 → 新 session 最长等 TTL 5min 才能看到。

- **联合条件 `has_entry || has_in_flight_scan` 二者兼得**：cache 有 entry 或 scan 在途时走规则 2 判定 bump；cache 空且无 scan 在途时走规则 3 不 bump。**注意**：cache 空且无 scan 在途时本规则**不**清缓存，但此时 watcher 层已经在 `session_list_changed` 字段上承载了"first-seen" 信号，下游 emit 路径仍能让前端正确触发 revalidate（前端 revalidate 路径自然走 cache miss + 重 scan 兜底）。

**对各类真实 fs 事件的语义覆盖**（对应 `cdt-watch::FileWatcher::parse_project_event` 的输出）：

- 新 project 目录创建（`<projects_root>/<pid>` dir-create）→ watcher 输出 `plc=true, sid=""` → 走规则 1（invalidate_local）
- 启动后第一次见某 pid（典型场景：watcher 重启）→ watcher 输出 `plc=true` → 走规则 1（invalidate_local）
- **已知 project 下新 session 首次出现** → watcher 输出 `plc=false, deleted=false` 且 `contains_session_id == false` → 走规则 2（invalidate_local，仅当 has_entry||has_in_flight 时）；watcher 同时填 `session_list_changed=true` 给 emit 路径
- 已知 project 已知 session JSONL 追加（普通 hot path）→ watcher 输出 `plc=false, deleted=false` 且 `contains_session_id == true` → 走规则 3（不清缓存）
- watcher 折叠的 subagent JSONL **修改**（事件 `(pid, sid=父, deleted=false, plc=false)` + `contains_session_id(父 sid) == true`）→ 走规则 3（不清缓存）
- 主 session JSONL 删除 → watcher 输出 `deleted=true` → 走规则 1（invalidate_local）；watcher 同时填 `session_list_changed=true` 给 emit 路径
- watcher 折叠的 subagent JSONL **删除**（事件 `(pid, sid=父, deleted=true, plc=false)`）→ 走规则 1（**false-positive**：事件无法区分主 vs subagent 删除；触发一次重扫即结束，无正确性问题，详 design R6）

**MUST NOT**：

- MUST NOT 扩展或读取 `cdt-core::FileChangeEvent` 中除 `project_id` / `session_id` / `deleted` / `project_list_changed` 之外的其他字段做**判定**输入（`session_list_changed` 字段由 watcher 层填，本规则**仅消费 `event.session_id` 等输入字段**，不依赖 emit 字段做判定输入）
- MUST NOT 在事件回调路径内调任何 fs 操作（`fs::stat` / `fs::metadata` 等）—— 完全基于事件字段 + cache snapshot lookup 判定
- MUST NOT 引入 per-project 失效粒度（`ProjectScanCache.entries: HashMap<ContextId, Arc<Vec<Project>>>` 当前数据结构无 per-project entry 概念，per-project 重构超本 Requirement scope）
- MUST NOT 让 invalidate 决策影响 `FileChangeEvent.session_list_changed` 字段填写——该字段由 watcher 层独立决定，本规则只产出"是否 invalidate" + "供 emit 路径 OR 兜底的 cache_unknown_hint"

**`ProjectScanCache::contains_session_id` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `contains_session_id(&self, ctx: &ContextId, project_id: &str, session_id: &str) -> bool`，遍历指定 ctx 对应 entry 的 `Arc<Vec<Project>>`，定位 `Project.id == project_id` 后检查 `Project.sessions: Vec<String>` 是否含 `session_id`；ctx 无 entry 或 project 不存在时返回 `false`。复杂度 O(N project × N session_per_project)，对 30 project × 538 session corpus 单次 ~10µs，可在 hot 路径调用。

**`ProjectScanCache::has_entry` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `has_entry(&self, ctx: &ContextId) -> bool`，返回 `entries` 是否含此 ctx 的 entry。invalidator 在规则 2 判定前 SHALL 先用本方法守护——cache 空时跳过 unknown_session 判定，避免 lag 后被普通 append 事件持续触发 invalidate 导致重扫风暴。

**`ProjectScanCache::has_in_flight_scan` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `has_in_flight_scan(&self) -> bool`，返回当前 `in_flight_scans > 0`。invalidator 在规则 2 判定前 SHALL 与 `has_entry` 共同 OR 守护——cache 空但有 scan 在途时仍 bump generation，让 in-flight scan 完成回写时识别 race 丢弃 stale snapshot。

**`ProjectScanCache::begin_scan` / `finish_scan_with_insert` / `abort_scan` API 契约**：业务路径 `scan_projects_cached_with` SHALL 用 `begin_scan` 替代裸 `invalidation_generation()` 拿 recorded_generation 同时 `in_flight_scans += 1`；scan 成功时 SHALL 用 `finish_scan_with_insert` 替代 `try_insert`（内部 `in_flight_scans -= 1` + race 校验）；scan 失败时 SHALL 调 `abort_scan` 配对 `begin_scan` 不漏减。这三 API 联合保护 in-flight scan 与 invalidator 之间的 race 协议。

**SSH context entry 不受 file-change 影响**：watcher 是 Tauri 本地 fs 的硬不变量。invalidator 推算 `ContextId::local(projects_dir)` 决定失效作用域；`ProjectScanCache::invalidate_local()` 实现仅对 `FsKind::Local` entry 生效，SSH entry 仍按既有 TTL 自然过期。SSH `polling_watcher` 通过 `FileWatcher::attach_remote` 喂入同一 watcher broadcast 的事件，进入 unified invalidator 后 invalidate 决策同样由本规则三档判定（cache 为空 / 不含 SSH session 场景下规则 2 自然不命中，退化为仅看 `project_list_changed || deleted`，与 Local 行为一致）；`session_list_changed` 字段已由 SSH polling watcher 在远端事件上填好，与 Local watcher 行为对称（详 `file-watching` Requirement `Watch SSH remote project directory via SFTP polling`）。

**`new()` 构造路径不启动该订阅**：`LocalDataApi::new()`（无 watcher 参数）SHALL NOT spawn 此 task；该场景仅依赖被动 generation 校验路径兜底，与 `MetadataCache` / `ParsedMessageCache` 在 `new()` 路径的行为对齐。

**broadcast lag 走保守全失效**：`broadcast::Receiver::recv` 返回 `Err(RecvError::Lagged(_))` 时 SHALL 调 `invalidate_local()` 并 inc counter `project_scan_cache.invalidate.lag_conservative`，因为 lag 期间可能错过 `plc=true` / `deleted=true` 事件且 `ProjectScanCache` 没有 path-level 被动校验机制可兜底。lag 路径下 file_tx emit 行为契约由 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement 单独承担（synthetic structural event 兜底）。返回 `Err(RecvError::Closed)` 时 SHALL 退出 loop。

> 该 lag 行为与 `parsed-message 缓存按 file-change 广播主动失效` Requirement 的 lag 静默继续策略**有意不一致**：parsed-message cache 在 lookup 时 stat 比对 `FileSignature` 兜底 lag 错过的事件；ProjectScanCache 无类似被动校验，lag 时必须保守清空。

**telemetry counter 注册**：实现 SHALL 在 `cdt-telemetry` 静态白名单中注册以下 3 个 counter：

- `project_scan_cache.invalidate.structural`
- `project_scan_cache.invalidate.content_append_skipped`
- `project_scan_cache.invalidate.lag_conservative`

每条事件 SHALL 按规则结果 inc 对应 counter 各 1 次。

**性能契约**：长时间使用场景（活跃 claude-code 会话每秒多次追加 JSONL）下，`content_append_skipped` 计数 SHALL 远超 `structural`（典型预期 ≥ 95% 走 skipped 分支）；偏离此预期是判定逻辑或 watcher 字段填充偏差的信号。

#### Scenario: 已知 session JSONL 追加 SHALL NOT 失效 cache

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，且 `ProjectScanCache` 已经因前一次 `list_repository_groups` 写入了某 ctx 的 entry，含 project `pa` 和 session `sa`
- **AND** `<projects_root>/pa/sa.jsonl` 被 claude-code 追加新行
- **AND** `FileWatcher` 广播一条对应 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false, session_list_changed: false }`（watcher 跟踪集合已含 `(pa, sa)`）
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa")` 得到 `true`（规则 2 不命中）
- **AND** MUST NOT 调 `invalidate_local`
- **AND** `ProjectScanCache::lookup` 后续仍 SHALL 命中既有 entry（同一 `root_generation` / `context_generation` 下）
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1

#### Scenario: 已知 project 下新 session 首次出现 SHALL 失效 cache

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，`ProjectScanCache` 已写入某 ctx 的 entry，含已知 project `pa` 与已知 sessions `{sa1, sa2}`（`sa_new` 不在此列表）
- **AND** claude-code 在已知 project `pa` 下创建新 session `sa_new`，写入 `<projects_root>/pa/sa_new.jsonl`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false, session_list_changed: true }`（watcher 跟踪集合此前不含 `(pa, sa_new)` → first-seen → `session_list_changed=true`）
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa_new")` 得到 `false`
- **AND** MUST 调 `ProjectScanCache::invalidate_local()`（规则 2 触发）
- **AND** 下一次 `list_repository_groups` SHALL 走 cache miss 并把 `sa_new` 纳入返回值
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: cache 空 + 新 session 事件 SHALL NOT invalidate（emit 不受影响）

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，`ProjectScanCache::entries` 为空（冷启 / `reconfigure_claude_root` 后 / SSH context 切换让 Local entry 被驱逐），`has_in_flight_scan() == false`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false, session_list_changed: true }`（watcher 跟踪集合此前不含 `(pa, sa_new)`）
- **THEN** 后台 invalidator MUST NOT 调 `invalidate_local`（规则 2 守护命中：`has_entry == false && has_in_flight_scan == false`）
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1
- **AND** invalidate 决策的"未触发"SHALL NOT 影响 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement 定义的 emit 行为——前端仍 SHALL 收到 `session_list_changed=true` 触发兜底 revalidate

#### Scenario: 顶层 dir-create 标 plc=true 时直接走规则 1

- **WHEN** `ProjectScanCache` 已存若干 ctx entry
- **AND** claude-code 创建新 project 顶层目录 `<projects_root>/p_new`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "p_new", session_id: "", deleted: false, project_list_changed: true, session_list_changed: false }`
- **THEN** 后台 invalidator MUST 仅基于 `event.project_list_changed == true` 走规则 1，调 `invalidate_local()`
- **AND** SHALL NOT 调 `contains_session_id`（事件 `session_id == ""` 触发规则 2 的 `!session_id.is_empty()` 守护跳过）
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: 删除已知 session JSONL SHALL 失效 cache

- **WHEN** `ProjectScanCache` 已存某 ctx entry 且内含 project `pa` / session `sa`
- **AND** 用户或外部工具删除 `<projects_root>/pa/sa.jsonl`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: true, project_list_changed: false, session_list_changed: true }`
- **THEN** 后台 invalidator MUST 仅基于 `event.deleted == true` 走规则 1，调 `ProjectScanCache::invalidate_local()`
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: subagent JSONL 修改 SHALL NOT 失效 cache

- **WHEN** `ProjectScanCache` 已存某 ctx entry，含 project `pa` / 父 session `s_parent`
- **AND** claude-code 写入 `<projects_root>/pa/s_parent/subagents/agent-xyz.jsonl`
- **AND** watcher 折叠到父 session 后广播 `FileChangeEvent { project_id: "pa", session_id: "s_parent", deleted: false, project_list_changed: false, session_list_changed: false }`（subagent 路径 SHALL NOT 进入跟踪集合 → `session_list_changed=false`）
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "s_parent")` 得到 `true`
- **AND** MUST NOT 调任何失效 API
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1
- **AND** `ProjectScanCache::lookup` 后续仍 SHALL 命中既有 entry

#### Scenario: subagent JSONL 删除触发 false-positive invalidate（接受）

- **WHEN** `ProjectScanCache` 已存某 ctx entry，含 project `pa` / 父 session `s_parent`
- **AND** subagent 文件 `<projects_root>/pa/s_parent/subagents/agent-xyz.jsonl` 被删除
- **AND** watcher 折叠到父 session 后广播 `FileChangeEvent { project_id: "pa", session_id: "s_parent", deleted: true, project_list_changed: false, session_list_changed: true }`
- **THEN** 后台 invalidator MUST 基于 `event.deleted == true` 走规则 1，调 `ProjectScanCache::invalidate_local()`
- **AND** 这是已知的 **false-positive 行为**：事件字段无 path，无法区分主 session 删除 vs subagent 删除；本 spec 显式接受此 false-positive，触发一次 ProjectScanner 重扫的成本可接受
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: SSH context entry 不受 file-change 影响

- **WHEN** `ProjectScanCache` 已存 SSH ctx entry（由 SSH `polling_watcher` 间接触发或通过其它路径写入）
- **AND** 本地 `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false, session_list_changed: false }`
- **THEN** unified invalidator MUST 调 `ProjectScanCache::invalidate_local()`，仅对 `FsKind::Local` entry 生效
- **AND** SSH ctx entry SHALL NOT 被失效，按既有 TTL 自然过期

### Requirement: Unified invalidator 作为 `LocalDataApi.file_tx` 唯一生产者

`LocalDataApi::new_with_watcher(...)` 启动路径 SHALL 把 `spawn_unified_cache_invalidator` 升级为 `LocalDataApi.file_tx` 的**唯一**生产者，**不**再 spawn 任何独立的 `bridge_task` 把 `watcher.subscribe_files()` 直接转发到 `file_tx`。invalidator 内部 sync 跑完三档判定（详 `ProjectScanCache 按事件语义分级失效` Requirement）后 SHALL 把 enriched `FileChangeEvent` 通过 `file_tx.send(enriched)` 广播给下游消费者（Tauri host emit / HTTP `spawn_file_bridge` / 其它 `subscribe_file_changes` 调用方）。

SSH 路径 SHALL 通过 `cdt-watch::FileWatcher::attach_remote(sftp, projects_dir, cancel_token)` 接入 watcher broadcast，`LocalDataApi::attach_remote_watcher` SHALL NOT 再走 `RemotePollingWatcher::spawn(..., self.file_tx.clone(), ...)` 路径——SSH event 必须经过同一 unified invalidator enrichment gateway，与 Local event 行为一致。`FileWatcher::attach_remote` 签名 SHALL 接受调用方注入的 `CancelToken`（替代原内部 `CancelToken::new()`），保留 `RemoteWatcherHandle` 返回值不变；调用方持有 token clone 用于 dead-signal monitor 路径，外部 disconnect 时仍能 cancel SSH polling。

**Emit 时机契约（unified invalidator loop 顺序）**：

1. `rx.recv().await` 收 raw event
2. sync 调 `apply_file_event_to_project_scan_cache(event)` 拿判定结果（返回 `EnrichDecision { invalidated: bool, emit_session_list_changed_hint: bool }`）；该函数内部锁在 sync block 末尾自动释放
3. 构造 `enriched_event = FileChangeEvent { session_list_changed: event.session_list_changed || decision.emit_session_list_changed_hint, ..raw_event }`——OR 公式让 watcher 视角 + cache 视角并集决定字段，最大兜底
4. 调 `file_tx.send(enriched_event)` broadcast emit（**锁已释放**，emit 永不在持锁路径）
5. async 调 `apply_file_event_to_parsed_cache(event).await`（**不**阻塞 emit）

emit MUST 在 step 4 完成（即 sync invalidate 之后，async parsed invalidate 之前）。这保证：(a) 前端拿到 file-change 时 `ProjectScanCache` 状态已是事件后的最新；(b) 前端无需等磁盘 stat I/O 完成；(c) `parsed_cache` 失效路径仍走 async 不阻塞 emit。

**emit 字段 OR 公式语义**：watcher 层填的 `event.session_list_changed` 是判定**主源**（基于 watcher 跟踪集合首见性，详 `file-watching` Requirement `跟踪 session 首见性以填写 revalidation hint`）；`decision.emit_session_list_changed_hint` 是 cache 视角的辅助 hint（值 = "本 event 命中 `ProjectScanCache 按事件语义分级失效` Requirement 规则 2 的 unknown_session 判定条件"）。两源并集 OR 兜底 watcher 重启 / `reconfigure_claude_root` 等让 watcher 跟踪集合重置但 cache 仍有有效 snapshot 的窗口。

**反压**：`broadcast::Sender::send` 满时丢旧元素不阻塞，invalidator 自身永远不会被慢 subscriber 阻塞；slow subscriber 引发的 lag 走下游 bridge 的 `Lagged` 兜底（见 `Emit push events for file changes and notifications` Requirement 的 lag 兜底契约）。

**broadcast lag 路径 SHALL emit synthetic structural event**：`rx.recv().await` 返回 `Err(RecvError::Lagged(n))` 时除调 `apply_lag_to_project_scan_cache`（保守 `invalidate_local()` + counter `lag_conservative`）外，SHALL 显式 send 一条 synthetic `FileChangeEvent { project_id: "", session_id: "", deleted: false, project_list_changed: true, session_list_changed: true }` 到 `file_tx`。理由：本路径的 lag 在 `watcher.subscribe_files()` 上游 receiver 上，下游 `LocalDataApi.file_tx` 的下游 bridge（src-tauri Tauri host emit / HTTP SSE bridge）的 `RecvError::Lagged` 兜底监听的是 `file_tx`——上游 lag 不会让下游 receiver 同步 lag，下游 bridge 的 sse-lagged 通知路径不会触发，前端连兜底 silent refresh 都收不到。synthetic event 让前端三档守护命中并触发兜底全量 revalidate。返回 `Err(RecvError::Closed)` 时 SHALL 退出 loop。

**Synthetic event 在所有下游 bridge 路径的传播契约**：synthetic event 经 `LocalDataApi.file_tx` broadcast 后，下游两路 bridge SHALL 按既有 forward 路径处理，**不**对 synthetic event 做特殊识别 / 过滤：

- **Tauri host bridge**（src-tauri）：SHALL `app.emit("file-change", &payload)` 转发 synthetic event 到 webview，与 real event 行为一致
- **HTTP SSE bridge**（HTTP 客户端 / 浏览器 transport 路径）：SHALL 把 synthetic event 序列化为 `PushEvent::FileChange { ... }` 推到 `/api/events` SSE stream，与 real event 形态一致

**Synthetic event 在前端消费侧的副作用守护**：所有前端 surface（Tauri webview / 浏览器 transport `?http=1`）SHALL 在收到 `payload.projectId === ""` **且** `payload.sessionId === ""` 时跳过 per-session 操作（如 `loadSessions("")` / per-session DOM patch），仅触发"项目列表 / dashboard 全量 revalidate"等顶层兜底刷新。Tauri webview 与浏览器 transport 共用同一前端 handler 链（`fileChangeStore.svelte.ts`），守护实现 SHALL 集中在该 handler 链的入口处或各 surface 自身的回调内，避免跨 transport 漂移。

**MUST NOT**：

- MUST NOT 与额外的 `bridge_task` 并存——unified invalidator 是 `file_tx` 唯一生产者，避免双 producer 引发的事件顺序与重复问题
- MUST NOT 让 SSH `polling_watcher` 直接生产到 `LocalDataApi.file_tx` ——必须经过 `FileWatcher::attach_remote` → watcher broadcast → unified invalidator 的统一路径
- MUST NOT 在 emit 路径覆盖 `event.session_list_changed` 字段——OR 公式 SHALL 保留 watcher 已填值，cache hint 仅做 OR 提升

#### Scenario: unified invalidator 是 `file_tx` 唯一生产者

- **WHEN** `LocalDataApi::new_with_watcher` 构造完成，启动 watcher 桥任务
- **THEN** 启动路径 SHALL NOT spawn 任何独立的 `bridge_task` 把 `watcher.subscribe_files()` 直接转发到 `file_tx`
- **AND** `file_tx` 的所有事件 SHALL 来自 unified invalidator 的 `file_tx.send(enriched_event)` 调用

#### Scenario: SSH 路径走 attach_remote 进入 unified invalidator

- **WHEN** `LocalDataApi::attach_remote_watcher` 被调用（SSH 连接上时）
- **THEN** 实现 SHALL 调用 `FileWatcher::attach_remote(sftp, projects_dir, cancel_token)` 让 SSH polling 事件喂入 `watcher.file_tx`，且调用方 SHALL 注入自己持有的 `CancelToken`（用于 dead-signal monitor cancel 路径）
- **AND** SSH event SHALL 经过 unified invalidator 的判定（cache 无 SSH entry 时 invalidate 决策退化为只看 `project_list_changed || deleted`）
- **AND** enriched SSH event SHALL 通过 `file_tx.send` 广播给下游，与 Local event 形态一致；`session_list_changed` 字段已由 SSH polling watcher 在远端事件上填好（详 `file-watching` Requirement `Watch SSH remote project directory via SFTP polling`）
- **AND** 外部 disconnect 触发 `cancel_token.cancel()` 时 SHALL 让 SSH polling task 退出（dead-signal monitor 路径保持原行为）

#### Scenario: emit 顺序在 sync invalidate 之后、async parsed invalidate 之前

- **WHEN** unified invalidator loop 收到一条 raw `FileChangeEvent`
- **THEN** 实现 SHALL 先 sync 调 `apply_file_event_to_project_scan_cache` 拿 `EnrichDecision`
- **AND** 然后 sync 调 `file_tx.send(enriched_event)` emit（锁已释放）
- **AND** 最后 async 调 `apply_file_event_to_parsed_cache(event).await`
- **AND** `file_tx.send` MUST NOT 在 cache lock 临界区内调用

#### Scenario: emit 字段 OR 公式 watcher 主源 + cache hint 兜底

- **WHEN** unified invalidator 收到 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false, session_list_changed: true }`（watcher 已填 first-seen=true），且 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa_new")` 返 `false` 让 `decision.emit_session_list_changed_hint = true`
- **THEN** enriched event 的 `session_list_changed` 字段 SHALL 是 `true || true == true`
- **AND** 通过 `file_tx` emit 给下游

#### Scenario: emit 字段 OR 公式 watcher 已填 false 且 cache hit 也 false

- **WHEN** unified invalidator 收到 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false, session_list_changed: false }`（watcher 跟踪集合已含 `(pa, sa)` → first-seen=false），且 `contains_session_id(&local_ctx, "pa", "sa")` 返 `true` 让 `decision.emit_session_list_changed_hint = false`
- **THEN** enriched event 的 `session_list_changed` 字段 SHALL 是 `false || false == false`
- **AND** 该事件 SHALL NOT 触发前端三档守护 revalidate

#### Scenario: emit 字段 OR 公式 watcher 重启窗口期 cache 兜底

- **WHEN** watcher 已重启（`reconfigure_claude_root` 触发）让跟踪集合清空，但 `ProjectScanCache` 仍持有旧 entry（含 project `pa` 与 `sa`）；用户在 `pa` 下追加已知 session `sa.jsonl`
- **AND** watcher 视为 first-seen 填 `session_list_changed=true`（lazy false-positive）
- **THEN** enriched event 的 `session_list_changed` 字段 SHALL 是 `true || (cache contains_session_id 返 true → hint=false) == true`
- **AND** 前端 revalidate 一次（false-positive，cache 视角下其实是已知 session 追加，但 watcher 视角是 first-seen，OR 取并集偏向 emit）

#### Scenario: lag 路径 SHALL emit synthetic structural event

- **WHEN** unified invalidator 的 `rx.recv().await` 返回 `Err(RecvError::Lagged(n))`
- **THEN** 实现 SHALL 调 `apply_lag_to_project_scan_cache`（保守 `invalidate_local()` + counter `lag_conservative`）
- **AND** SHALL 显式 send 一条 synthetic `FileChangeEvent { project_id: "", session_id: "", deleted: false, project_list_changed: true, session_list_changed: true }` 到 `file_tx`

#### Scenario: synthetic event 经 Tauri host bridge 转发到 webview

- **WHEN** synthetic event 进入 `file_tx` broadcast 后被 Tauri host bridge 接收
- **THEN** bridge SHALL `app.emit("file-change", &payload)` 把 synthetic event 转发给 webview，与 real event 行为一致
- **AND** SHALL NOT 对 synthetic event 做特殊识别 / 过滤
- **AND** webview 前端 handler 收到该 payload 后 SHALL 触发兜底全量 revalidate
- **AND** webview 前端 SHALL 按 `payload.projectId === "" && payload.sessionId === ""` 守护跳过 per-session 操作（`loadSessions("")` / per-session DOM patch），不引发副作用

#### Scenario: synthetic event 经 HTTP SSE bridge 推到浏览器客户端

- **WHEN** synthetic event 进入 `file_tx` broadcast 后被 HTTP SSE bridge 接收
- **THEN** bridge SHALL 把 synthetic event 序列化为 `PushEvent::FileChange` 推到 `/api/events` SSE stream，与 real event 形态一致
- **AND** SHALL NOT 对 synthetic event 做特殊识别 / 过滤
- **AND** 浏览器 transport 收到该 SSE 消息后 SHALL 走与 webview 同一 file-change handler 链
- **AND** 浏览器 transport 路径前端 SHALL 按 `payload.projectId === "" && payload.sessionId === ""` 同款守护跳过 per-session 操作，仅触发顶层 revalidate
