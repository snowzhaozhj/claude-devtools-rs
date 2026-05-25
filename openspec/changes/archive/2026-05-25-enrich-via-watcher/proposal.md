## Why

PR #291（archived change `enrich-file-change-with-session-list-changed`）把 `FileChangeEvent.session_list_changed` 的判定放在 `cdt-api::project_scan_cache::apply_file_event_to_project_scan_cache` 三档 enrich，依赖 `cache.has_entry(local_ctx) || cache.has_in_flight_scan()` 守护——cache 空 / context 切换 / `reconfigure_claude_root` 后第一条 jsonl 写入会让 `unknown_session=false` → `session_list_changed=false`，前端 Sidebar / DashboardView / ProjectSwitcher 因 PR #291 引入的"仅 structural 守护"全部不刷新（用户实测：在已知项目 aiUltron 下新建会话 sessionId=3f159cf1-... 后项目列表 + 项目下拉均未更新）。同时 `spawn_unified_cache_invalidator` 的 `RecvError::Lagged` 分支只 invalidate scan cache 不 emit 任何信号到 `channels.files`，src-tauri 兜底的 `sse-lagged` event 监听的是 `channels.files` 而非上游 watcher——watcher broadcast 滞后时前端连兜底 silent refresh 都收不到。

## What Changes

- **后端 watcher 视角 enrich**：`cdt-watch::FileWatcher` 新加 `known_sessions: Mutex<HashSet<(PathBuf, String)>>` 跟踪 `(project_id, session_id)` 首见性；`parse_project_event` 在写事件首次插入时填 `session_list_changed=true`、append 时填 `false`、删除事件**无条件** `true`。子 agent jsonl（`<project>/<session>/subagents/agent-*.jsonl`）SHALL NOT 进入 known_sessions。known_sessions 启动时不预填（lazy）——接受冷启窗口期内每个活跃 session 多触发一次 revalidate 的 false positive（实测约占 PR #291 优化收益的 < 1%），换"零漏失"鲁棒性。
- **SSH polling watcher 对称语义**：`cdt-ssh::polling_watcher::build_change_event` 当前直接构造 `session_list_changed=false`，改为基于 `RemotePollingWatcher` 已有的 `baseline: BTreeMap<PathBuf, FileFingerprint>` 判定首见。第一次 poll 静默建 baseline（spec 已规定）→ baseline 内的 session 自然算"已见"；后续 poll 新增 / 删除 path 触发 `session_list_changed=true`，size/mtime 变化触发 `false`。让 SSH context 切回 local 行为对称。
- **后端 cache 层退化为只管 invalidate**：`apply_file_event_to_project_scan_cache` 不再 enrich `session_list_changed` 字段。原三档判定只用于决定是否 `cache.invalidate_local()`，保留 `has_entry || has_in_flight_scan` 守护防 lag-after-empty 风暴。`spawn_unified_cache_invalidator` emit 时取 `event.session_list_changed || cache_unknown_hint` OR——watcher 已知（first-seen）+ cache 已知（snapshot 视角下也算 unknown）取并集，最大兜底。
- **修 lag 丢信号**：`spawn_unified_cache_invalidator` 的 `RecvError::Lagged` 分支显式 emit synthetic `FileChangeEvent { project_id: "", session_id: "", deleted: false, project_list_changed: true, session_list_changed: true }` 到 `channels.files`，让前端三档守护命中触发兜底全量 revalidate。前端 Sidebar / DashboardView 已有 `if (!currentGroupId || !payload.sessionId) return` 类守护，新加单测确认 synthetic 不让 sidebar 误尝试 `loadSessions("")`。
- **前端守护逻辑保持不变**：Sidebar L754-762 / DashboardView L124-130 / 三档守护条件不动——watcher 现在填得对了，前端守护语义自然正确。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `file-watching`：新增 Requirement 锁 `known_sessions` 首见性跟踪 + `session_list_changed` 字段填写规则；SSH polling watcher 对称 first-seen 判定。
- `ipc-data-api`：修改 §"`ProjectScanCache` 按事件语义分级失效"——emit 决策 SHALL 与 invalidate 决策独立（不再共用 structural bool）；watcher broadcast lag SHALL emit synthetic structural event 触发前端兜底。

## Impact

- **代码**：`cdt-watch/src/watcher.rs`（~50 LOC + HashSet 维护 + first-seen 单测）、`cdt-ssh/src/polling_watcher.rs`（~30 LOC + baseline 复用）、`cdt-api/src/ipc/project_scan_cache.rs`（~20 LOC，签名返 `EnrichDecision` 拆 emit/invalidate）、`cdt-api/src/ipc/local.rs::spawn_unified_cache_invalidator`（~15 LOC OR + lag synthetic）
- **测试**：cdt-watch 单测、cdt-ssh polling 单测、`cdt-api/tests/sse_event_bridge.rs` cold-cache 集成测试 + lag synthetic 集成测试、反转 fix 红绿验证
- **IPC 字段**：`FileChangeEvent` 字段集合 / 序列化形态**不变**——`projectListChanged` / `sessionListChanged` / `deleted` 仍是同名 camelCase 字段。**不**触发前端 api.ts 字段同步。
- **性能**：稳态 IPC 频率与 PR #291 等价（109 次/天 structural / 1328 次 append 仍跳过 emit）；冷启 / reconfigure 窗口期 +5-10 次/天 false positive（< 1% 噪声）
- **依赖**：无新依赖
