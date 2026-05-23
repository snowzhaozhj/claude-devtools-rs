## ADDED Requirements

### Requirement: `ProjectScanCache` 按事件语义分级失效

`LocalDataApi::new_with_watcher(...)` 构造路径 SHALL spawn 后台 task，订阅 `FileWatcher::subscribe_files()` 广播。该 task 对每条 `FileChangeEvent` SHALL 仅根据 `FileChangeEvent` 四字段（`project_id` / `session_id` / `deleted` / `project_list_changed`）+ `ProjectScanCache` snapshot lookup 决定是否失效 `ProjectScanCache` Local entry：

**判定规则（三档）**：

1. `event.project_list_changed == true` **OR** `event.deleted == true` → 调 `ProjectScanCache::invalidate_local()`，inc counter `project_scan_cache.invalidate.structural`
2. `event.session_id` 非空 **AND** `ProjectScanCache::has_entry(local_ctx) == true` **AND** `ProjectScanCache::contains_session_id(local_ctx, &event.project_id, &event.session_id) == false`（cache 已有该 ctx 的 entry 且 snapshot 不含此 session）→ 同规则 1：`invalidate_local()` + structural counter
3. 其他（普通 JSONL append + watcher 折叠的 subagent 修改 + 空 sid 事件 + cache 无 entry 时的任意非 structural 事件）→ **不**调任何失效 API，保留现有 cache，inc counter `project_scan_cache.invalidate.content_append_skipped`

**为何需要规则 2**：`cdt-watch::FileWatcher` 在构造时**预填**当前已存在的 project 目录到 `known_projects` HashSet（`crates/cdt-watch/src/watcher.rs:30-41,79`）。已知 project 下新建 session 时 `mark_project_seen` 不会返回 true，watcher 输出 `plc=false, deleted=false`——与"已知 session JSONL 追加"在事件字段上**外观完全相同**。仅靠 (plc, deleted) 两 bool 判定会让新 session 最长 `LOCAL_CACHE_TTL = 300s` 不可见。规则 2 用 cache snapshot 反向查询补这个语义缺口。

**为何需要 `has_entry` 守护**：lag 路径调 `invalidate_local()` 后 cache 被清空，若不守护，后续普通 append 事件 `contains_session_id` 一律返 false → unknown_session 命中 → 又调 `invalidate_local()` 反复 bump `invalidation_generation` → 在重扫期间 `try_insert` 因 generation mismatch 一直丢弃 snapshot → cache 长期无法 repopulate（持续重扫风暴）。`has_entry` 守护让 cache 空时直接走规则 3 等待业务路径重扫填回。

**对各类真实 fs 事件的语义覆盖**（对应 `cdt-watch::FileWatcher::parse_project_event` 的输出）：

- 新 project 目录创建（`<projects_root>/<pid>` dir-create）→ watcher 输出 `plc=true, sid=""` → 走规则 1
- 启动后第一次见某 pid（典型场景：watcher 重启）→ watcher 输出 `plc=true` → 走规则 1
- **已知 project 下新 session 首次出现** → watcher 输出 `plc=false, deleted=false` 且 `contains_session_id == false` → 走规则 2
- 已知 project 已知 session JSONL 追加（普通 hot path）→ watcher 输出 `plc=false, deleted=false` 且 `contains_session_id == true` → 走规则 3
- watcher 折叠的 subagent JSONL **修改**（事件 `(pid, sid=父, deleted=false, plc=false)` + `contains_session_id(父 sid) == true`）→ 走规则 3
- 主 session JSONL 删除 → watcher 输出 `deleted=true` → 走规则 1
- watcher 折叠的 subagent JSONL **删除**（事件 `(pid, sid=父, deleted=true, plc=false)`）→ 走规则 1（**false-positive**：事件无法区分主 vs subagent 删除；触发一次重扫即结束，无正确性问题，详 design R6）

**MUST NOT**：

- MUST NOT 扩展或读取 `cdt-core::FileChangeEvent` 中除 `project_id` / `session_id` / `deleted` / `project_list_changed` 之外的其他字段
- MUST NOT 在事件回调路径内调任何 fs 操作（`fs::stat` / `fs::metadata` 等）—— 完全基于事件字段 + cache snapshot lookup 判定
- MUST NOT 引入 per-project 失效粒度（`ProjectScanCache.entries: HashMap<ContextId, Arc<Vec<Project>>>` 当前数据结构无 per-project entry 概念，per-project 重构超本 Requirement scope）

**`ProjectScanCache::contains_session_id` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `contains_session_id(&self, ctx: &ContextId, project_id: &str, session_id: &str) -> bool`，遍历指定 ctx 对应 entry 的 `Arc<Vec<Project>>`，定位 `Project.id == project_id` 后检查 `Project.sessions: Vec<String>` 是否含 `session_id`；ctx 无 entry 或 project 不存在时返回 `false`。复杂度 O(N project × N session_per_project)，对 30 project × 538 session corpus 单次 ~10µs，可在 hot 路径调用。

**`ProjectScanCache::has_entry` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `has_entry(&self, ctx: &ContextId) -> bool`，返回 `entries` 是否含此 ctx 的 entry。invalidator 在规则 2 判定前 SHALL 先用本方法守护——cache 空时跳过 unknown_session 判定，避免 lag 后被普通 append 事件持续触发 invalidate 导致重扫风暴。

**SSH context entry 不受 file-change 影响**：watcher 是 Tauri 本地 fs 的硬不变量。invalidator 推算 `ContextId::local(projects_dir)` 决定失效作用域；`ProjectScanCache::invalidate_local()` 实现仅对 `FsKind::Local` entry 生效，SSH entry 仍按既有 TTL 自然过期。

**`new()` 构造路径不启动该订阅**：`LocalDataApi::new()`（无 watcher 参数）SHALL NOT spawn 此 task；该场景仅依赖被动 generation 校验路径兜底，与 `MetadataCache` / `ParsedMessageCache` 在 `new()` 路径的行为对齐。

**broadcast lag 走保守全失效**：`broadcast::Receiver::recv` 返回 `Err(RecvError::Lagged(_))` 时 SHALL 调 `invalidate_local()` 并 inc counter `project_scan_cache.invalidate.lag_conservative`，因为 lag 期间可能错过 `plc=true` / `deleted=true` 事件且 `ProjectScanCache` 没有 path-level 被动校验机制可兜底。返回 `Err(RecvError::Closed)` 时 SHALL 退出 loop。

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

#### Scenario: 已知 project 下新 session 首次出现 SHALL 失效 cache

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，`ProjectScanCache` 已写入某 ctx 的 entry，含已知 project `pa` 与已知 sessions `{sa1, sa2}`（`sa_new` 不在此列表）
- **AND** claude-code 在已知 project `pa` 下创建新 session `sa_new`，写入 `<projects_root>/pa/sa_new.jsonl`
- **AND** `FileWatcher::parse_project_event` 因 `mark_project_seen` 已在构造期将 `pa` 预填入 `known_projects`（参照 `crates/cdt-watch/src/watcher.rs:30-41,79`），返回 `false` → 广播 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false }`
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa_new")` 得到 `false`
- **AND** MUST 调 `ProjectScanCache::invalidate_local()`（规则 2 触发）
- **AND** 下一次 `list_repository_groups` SHALL 走 cache miss 并把 `sa_new` 纳入返回值
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: 顶层 dir-create 标 plc=true 时直接走规则 1

- **WHEN** `ProjectScanCache` 已存若干 ctx entry
- **AND** claude-code 创建新 project 顶层目录 `<projects_root>/p_new`
- **AND** `FileWatcher::parse_project_event` 检测到顶层 dir-create 分支，广播 `FileChangeEvent { project_id: "p_new", session_id: "", deleted: false, project_list_changed: true }`
- **THEN** 后台 invalidator MUST 仅基于 `event.project_list_changed == true` 走规则 1，调 `invalidate_local()`
- **AND** SHALL NOT 调 `contains_session_id`（事件 `session_id == ""` 触发规则 2 的 `!session_id.is_empty()` 守护跳过）
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: 删除已知 session JSONL SHALL 失效 cache

- **WHEN** `ProjectScanCache` 已存某 ctx entry 且内含 project `pa` / session `sa`
- **AND** 用户或外部工具删除 `<projects_root>/pa/sa.jsonl`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: true, project_list_changed: false }`
- **THEN** 后台 invalidator MUST 仅基于 `event.deleted == true` 走规则 1，调 `ProjectScanCache::invalidate_local()`
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: subagent JSONL 修改 SHALL NOT 失效 cache

- **WHEN** `ProjectScanCache` 已存某 ctx entry，含 project `pa` / 父 session `s_parent`
- **AND** claude-code 写入 `<projects_root>/pa/s_parent/subagents/agent-xyz.jsonl`
- **AND** `FileWatcher::parse_project_event` 识别为嵌套 subagent 形态（`components.len() == 4 AND components[2] == "subagents" AND filename starts_with "agent-"`），折叠到父 session，广播 `FileChangeEvent { project_id: "pa", session_id: "s_parent", deleted: false, project_list_changed: false }`
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "s_parent")` 得到 `true`
- **AND** MUST NOT 调任何失效 API
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1
- **AND** `ProjectScanCache::lookup` 后续仍 SHALL 命中既有 entry

#### Scenario: subagent JSONL 删除触发 false-positive invalidate（接受）

- **WHEN** `ProjectScanCache` 已存某 ctx entry，含 project `pa` / 父 session `s_parent`
- **AND** subagent 文件 `<projects_root>/pa/s_parent/subagents/agent-xyz.jsonl` 被删除
- **AND** `FileWatcher::parse_project_event` 折叠到父 session，广播 `FileChangeEvent { project_id: "pa", session_id: "s_parent", deleted: true, project_list_changed: false }`
- **THEN** 后台 invalidator MUST 基于 `event.deleted == true` 走规则 1，调 `ProjectScanCache::invalidate_local()`
- **AND** 这是已知的 **false-positive 行为**：事件字段无 path，无法区分主 session 删除 vs subagent 删除；本 spec 显式接受此 false-positive，触发一次 ProjectScanner 重扫的成本可接受
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: SSH context entry 不受 file-change 影响

- **WHEN** `ProjectScanCache` 同时存在 Local 与 SSH ctx 的 entry
- **AND** `FileWatcher` 广播任意 `FileChangeEvent`
- **THEN** 后台 invalidator MUST NOT 移除任何 `FsKind::Ssh` entry
- **AND** SSH entry 仍 SHALL 按 `SSH_CACHE_TTL = 10s` 自然过期

#### Scenario: broadcast lag 走保守全失效并 inc 单独 counter

- **WHEN** `broadcast::Receiver::recv` 返回 `Err(RecvError::Lagged(_))`
- **THEN** 后台 invalidator MUST 调 `invalidate_local()`
- **AND** counter `project_scan_cache.invalidate.lag_conservative` MUST inc 1（与 `structural` counter 区分以诊断广播背压）
- **AND** counter `project_scan_cache.invalidate.structural` MUST NOT inc
- **AND** loop SHALL 继续等待下一条事件

#### Scenario: lag 后 cache 空时后续普通 append SHALL NOT 引发 structural storm

- **WHEN** `broadcast::Receiver::recv` 返回 `Err(RecvError::Lagged(_))` 触发 `invalidate_local()` 清空 Local entry
- **AND** 紧接着收到若干普通 append 事件 `FileChangeEvent { ..., deleted: false, project_list_changed: false, session_id != "" }`
- **THEN** 后台 invalidator MUST 用 `has_entry(local_ctx)` 守护规则 2，发现 cache 已空时 SHALL 跳过 `contains_session_id` 反查
- **AND** 这些 append 事件 MUST 全部走规则 3（content_append_skipped）
- **AND** counter `project_scan_cache.invalidate.structural` MUST NOT 因后续 append 事件而递增
- **AND** `ProjectScanCache::invalidation_generation` MUST NOT 因后续 append 事件而递增——避免 in-flight scan 完成回写时 `try_insert` 反复 mismatch 让 cache 长期无法 repopulate
- **AND** 业务路径下次调 `list_repository_groups` 走 cache miss 重扫并通过 `try_insert` 成功填回 snapshot

#### Scenario: broadcast close 退出 loop

- **WHEN** `broadcast::Receiver::recv` 返回 `Err(RecvError::Closed)`
- **THEN** 后台 invalidator task SHALL 退出 loop（task 终止）
- **AND** SHALL NOT 调任何失效 API

#### Scenario: `new()` 构造不启动失效订阅

- **WHEN** `LocalDataApi` 由 `new(scanner, config_mgr, notif_mgr, ssh_mgr)` 构造（无 watcher 参数）
- **THEN** SHALL NOT spawn 任何订阅 `FileWatcher::subscribe_files()` 的 ProjectScanCache invalidator task
- **AND** `ProjectScanCache` 仅依赖被动 generation 校验路径兜底（与 `MetadataCache` / `ParsedMessageCache` 在 `new()` 路径行为对齐）

#### Scenario: 跨 IPC cache 复用在普通 append 场景不退化

- **WHEN** 调 `list_repository_groups` 一次完成首次扫描，写入 `ProjectScanCache` 某 ctx entry
- **AND** 期间发生 N 条普通 `FileChangeEvent { ..., deleted: false, project_list_changed: false }`（已知 session 追加），N ≥ 1
- **AND** 再调 `list_repository_groups` 第二次
- **THEN** 第二次调用 MUST 走 cache hit 分支（不调 `ProjectScanner::scan`）
- **AND** `ProjectScanCache::hits` 计数器 SHALL inc（用于回归测试断言"FU-4 跨 IPC 复用语义不退化"）
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` SHALL == N
