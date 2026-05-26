## Why

Dashboard 项目卡片"最近活动"时间长期 stale（用户实测同 session 在 sidebar 显示"刚刚"、在 dashboard 卡片显示"28 分钟前"），且按"最近活动"排序也跟着错。根因：`Project.most_recent_session = max(record.mtime_ms)` 是会随每次 jsonl append 推进的字段，但被 `project_scan_cache` 当作"项目拓扑快照"锁住——三档 invalidate 规则（`projectListChanged || deleted || unknown_session`）为了抑制 cache 风暴（实测 1437 次 IPC 降到 109 次 structural）显式跳过普通 append，副作用让 mtime 字段陷入 cache TTL 内的旧值；TTL 兜底也因前端 dashboard 在普通 append 不主动 fetch 而无机会触发。

## What Changes

- `cdt-core::FileChangeEvent` 新增可选 `mtimeMs: Option<i64>` 字段（向后兼容，缺字段时退化为现状行为）
- `cdt-watch` 本地 watcher：把现有 `path.exists()` 一换一替成 `metadata()`，同时产出 `deleted` + `mtime_ms`——**不**新增 hot-path fs op
- `cdt-ssh::polling_watcher`：从既有 `FileFingerprint.mtime` 透传到 `build_change_event`，零额外 SFTP stat
- `ProjectScanCache` 新增 per-`(ContextId, project_id)` 的 monotonic mtime overlay（`AtomicI64`）：watcher 收到带 `mtime_ms` 的 append 时 CAS 单调更新；`list_repository_groups` / `list_projects` cache hit 后返回前合成 `max(snapshot.most_recent_session, overlay)`
- ProjectScanCache 三档 invalidate 逻辑**不变**——mtime advance 不参与 structural 判定，保留风暴抑制收益
- cache 重新 populate 时合并 overlay：`> snapshot.mtime` 的值保留，防止 scan 期间 append 被覆盖回退
- 前端无改动——后端返回的就是合成后的最新值

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ipc-data-api`: 新增 Requirement"`ProjectScanCache` 维护 per-project mtime overlay 让 cache 命中路径返回新鲜 mtime"，明确 overlay 与三档 invalidate 解耦、cache repopulate 时 overlay 与 fresh snapshot 的合并规则
- `file-watching`: `FileChangeEvent` 新增可选 `mtime_ms` 字段语义；本地 watcher SHALL 在既有 deleted 判定路径产出 mtime hint、**不**新增额外 fs op；SSH watcher SHALL 透传既有 fingerprint mtime、零额外 SFTP stat
- `push-events`: `file-change` payload 字段契约新增 optional `mtimeMs` / `mtime_ms`（IPC / SSE 双形态），定义缺失时向后兼容退化为"不携带 hint"
- `project-discovery`: 补 Scenario 说明"已知 session 普通追加 SHALL 推进 display mtime 但不改变 sessions / cwd / topology"——明确 `most_recent_session` 字段在 cache 命中路径下的 freshness 契约

## Impact

- **代码**：
  - `crates/cdt-core/src/watch_event.rs`（加字段）
  - `crates/cdt-watch/src/watcher.rs`（exists → metadata）
  - `crates/cdt-ssh/src/polling_watcher.rs`（fingerprint mtime 透传）
  - `crates/cdt-api/src/ipc/project_scan_cache.rs`（overlay AtomicI64 + 合成路径 + populate 合并）
  - `crates/cdt-api/src/ipc/local.rs`（`list_repository_groups` / `list_projects` 返回前合成 + watcher 事件路由更新 overlay）
- **IPC 契约**：`file-change` Tauri event / HTTP SSE event payload 新增 optional `mtimeMs` —— 旧客户端缺字段不影响
- **跨 capability spec**：`ipc-data-api`、`file-watching`、`push-events`、`project-discovery` 四处 delta（详 design.md / specs/）
- **依赖**：无新增依赖
- **性能**：本地 + SSH 两条 watcher 路径 0 syscall 增量；cache hit 路径合成 overlay 30 项目 < 10 µs；watcher append CAS < µs/event；不破 perf.md 任何反模式 / 200 ms 预算
- **TS 偏差**：TS 原版同样有 stale 问题（无进程级 cache，但前端 file-change 不调 fetchProjects），单层 staleness；本 change 修得**比原版彻底** —— 在保留 cache 性能优势的同时让 dashboard mtime 实时跟进。该背离不引入 TS bug，无需进 `TS_BASELINE_DEVIATIONS.md`，但在 design.md 决策记录中点明
