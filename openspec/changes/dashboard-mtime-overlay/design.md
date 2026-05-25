## Context

Dashboard 项目卡片"最近活动"时间在用户看 sidebar 同 session 显示"刚刚"时显示"28 分钟前"——可以稳定复现。两边数据源都是 jsonl 文件 mtime，但 freshness 通路不同：

- **Dashboard 卡片**：`group.mostRecentSession ← max(worktree.mostRecentSession) ← max(Project.most_recent_session) ← max(record.mtime_ms)`，通过 `LocalDataApi::list_repository_groups` / `list_projects` 走 `project_scan_cache`（Local TTL 5 分钟、SSH 10 秒）
- **Sidebar 列表**：`SessionSummary.timestamp ← Session.last_modified ← record.mtime_ms`，通过 `list_sessions_skeleton` 走 `ProjectScanner::list_sessions` 直接扫描，**不**经 `project_scan_cache`

`apply_file_event_to_project_scan_cache`（`crates/cdt-api/src/ipc/project_scan_cache.rs:431-470`）的三档判定 `projectListChanged || deleted || unknown_session` 是 spec `ipc-data-api::ProjectScanCache 按事件语义分级失效` 显式定义的，目的是把每秒 N 次普通 jsonl append 事件从"重扫风暴"压成只对结构性事件（新建项目 / 删 session / 新 session 首次出现）失效——实测把 1437 次 IPC 降到 109 次 structural。

副作用是：`Project.most_recent_session = max(record.mtime_ms)` 是会随每次 append 推进的字段，被 cache 当成"项目拓扑快照"锁住，最长 5 分钟才有 TTL 兜底。但在 dashboard 静止视图场景（用户停留 + 当前 session 持续 append），前端不主动 fetch、TTL 也无机会触发——staleness 可远超 5 分钟（用户实测 28 分钟）。

TS 原版 `../claude-devtools` 同样有 stale 问题：`ProjectScanner.scan()` 无进程级 cache，但前端 file-change handler 完全不调 `fetchProjects`/`fetchRepositoryGroups`——单层 staleness，靠"任意用户操作触发 fetch 时裸扫拿新值"掩盖。Rust port 引入的 `project_scan_cache` 是 spec 级 perf 优化（30 项目 × 538 session 冷扫 95ms → cache hit µs 级，~1000× 提速），不能去掉，但叠加了第二层 staleness 让问题更严重。

## Goals / Non-Goals

**Goals:**

- Local 用户：dashboard 项目卡片"最近活动"时间在用户感知时长内（< 1 秒，受 watcher debounce 100ms + 合成开销约束）反映最新 jsonl mtime
- SSH 用户：dashboard"最近活动"时间在远端 polling 节拍上界内（默认 3 秒 / catch-up 30 秒，详 D6）反映最新 jsonl mtime——即一轮 poll 发现 append 后立即追上，不再被 stale snapshot 回滚
- 卡片按"最近活动"排序也跟随真实 mtime 更新
- 不破坏 `project_scan_cache` 的风暴抑制收益（1437 → 109 IPC 不退化）
- 不新增 hot-path fs op / 不破 perf 预算 / 不引入新 syscall
- 修得**比 TS 原版彻底**——在用户停留 dashboard 不切窗口时也能实时刷新

**Non-Goals:**

- 不改三档判定规则（普通 append 仍不动 cache snapshot 主体）
- 不引入 per-project 失效粒度（保留现有 `ContextId` 级 entry）
- 不在前端引入定时刷新 / visibilitychange 监听等"靠用户操作触发"的弱兜底
- 不解决"跨 SSH/local 同名 project 的延迟事件可能误 patch 同名条目"——这是 Risk 4 accepted limitation，根治需 watcher 注入 ContextId（独立 cap 改动）
- 不改 `Project.most_recent_session` / `Project.created_at` 等 cache snapshot 主体字段语义

## Decisions

### D1: `FileChangeEvent` 新增可选 `mtime_ms: Option<i64>` 字段

**选**：在 `cdt-core::FileChangeEvent` 加 `#[serde(default, skip_serializing_if = "Option::is_none")] pub mtime_ms: Option<i64>`，camelCase wire `mtimeMs`。

**理由**：

- 向后兼容——旧 client / 序列化版本缺字段时 deserialize 退化为 `None`，下游消费方按"无 hint"处理
- 可选语义对齐既有 `project_list_changed` / `session_list_changed` 字段的 default-skip-serializing 模式
- IPC 字段类型 `i64` 与 `Project.most_recent_session: Option<i64>` 完全对齐，下游合成路径直接 max 不需类型转换

**替代方案 A（rejected）**：必填 `mtime_ms: i64` —— 缺字段时会破 client 反序列化，且某些 SSH 透传路径上当前数据结构里 mtime 可能 `None`（远端 SFTP 偶发返 mtime 缺失），无法保证总是填得上。

**替代方案 B（rejected）**：在 cache 侧调 fs.metadata 重算 mtime —— 30 项目 × stat 几 ms hot path，破 perf 200ms 预算 + 违 cache 初衷。

### D2: 本地 watcher 用 `metadata()` 一换一替代 `path.exists()`

**选**：`crates/cdt-watch/src/watcher.rs:431-455` 把 `path.exists()` 调用替换成 `tokio::fs::metadata(path).await`，从结果同时产出：

- `deleted = matches!(err.kind(), io::ErrorKind::NotFound)`
- `mtime_ms = metadata.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64)`

**理由**：

- watcher debounce flush 路径**已经**在调 `path.exists()` 决定 `deleted` 字段——`metadata()` 是同一类 syscall（一换一），**不**是新增 fs op
- 这条是 perf 硬约束：codex 二审 BLOCK 要求"本地 watcher 不能在 exists 之后再多 stat 一次"——本设计严格遵守

**替代方案 A（rejected）**：`exists()` 后再 `metadata()` —— 两次 syscall，违 perf.md "hot loop 禁新增 fs op"。

**替代方案 B（rejected）**：watcher 不填 mtime，让 unified invalidator 收到 event 后自己 `fs.metadata` —— 同样多一次 syscall，且把 fs op 从 watcher 路径搬到 invalidator 路径不解决问题。

### D3: SSH watcher 从既有 `FileFingerprint.mtime` 透传

**选**：`crates/cdt-ssh/src/polling_watcher.rs::build_change_event` 入参增加 `mtime: Option<SystemTime>`，从 `FileFingerprint.mtime`（已存在字段，`crates/cdt-ssh/src/polling_watcher.rs:37-40`）透传到 `FileChangeEvent.mtime_ms`。

**理由**：

- SSH polling watcher 在 diff fingerprint 时已经持有远端 mtime，无需任何额外 SFTP stat
- SSH 路径与 Local 路径在字段填法上对称（spec `file-watching::Watch SSH remote project directory via SFTP polling` 已要求"SSH 与 Local 字段语义对称"）

**替代方案（rejected）**：SSH 路径不填 `mtime_ms` 留 `None` —— SSH 用户的 dashboard 仍卡 stale，背离 D5 的目的。

### D4: ProjectScanCache 三档 invalidate 逻辑不变

**选**：`apply_file_event_to_project_scan_cache` 的三档判定 `projectListChanged || deleted || unknown_session` **完全保留**，mtime advance 不参与 structural 判定——普通 append 不 invalidate cache snapshot 主体。

**理由**：

- 保留 PR `enrich-file-change-with-session-list-changed` 实测的 1437 → 109 IPC 风暴抑制收益
- mtime 字段的 freshness 通过 D5 的 overlay 机制独立解决，不需要把它升级到结构性事件级别
- spec `ipc-data-api::ProjectScanCache 按事件语义分级失效` 主体不动，只在它之外新增 overlay sub-Requirement

**替代方案 A（rejected）**：把 mtime advance 纳入 structural invalidate 让普通 append 也清空 cache snapshot 主体——会回退 1437 → 109 IPC 风暴抑制收益，重扫 30 项目 × 538 session 在每秒数次的 append 频率下吃满 cache miss 路径，破 perf 预算。

**替代方案 B（rejected）**：把 mtime advance 升级为 enrich `sessionListChanged=true` 让前端 reload list_repository_groups——同样回退风暴抑制收益（前端会触发 IPC 重 fetch），且 IPC 链路费事件费时间费序列化，远不如后端 overlay 直接合成。

### D5: 后端维护 per-project monotonic mtime overlay（codex BLOCK 修复）

**选**：在 `ProjectScanCache` 内新增 `mtime_overlay: HashMap<(ContextId, ProjectId), AtomicI64>`，行为契约：

- watcher 收到带 `mtime_ms` 的 event → 单调更新 overlay：`overlay[(ctx, project_id)].fetch_max(mtime_ms, Ordering::Relaxed)`
- `list_repository_groups` / `list_projects` cache hit 后**返回前合成**：`max(snapshot.most_recent_session.unwrap_or(0), overlay.get((ctx, project_id)).map(load).unwrap_or(0))`
- cache 重新 populate（`finish_scan_with_insert` 路径）：合并 overlay 到新 snapshot——`overlay > new_snapshot.most_recent_session` 的值保留为 overlay；`<=` 的值清除（防止 stale overlay 被 fresh scan 替代后还残留）
- `invalidate_local()` SHALL **不**清空 overlay——overlay 是 mtime 单调推进，invalidate 只作用于 snapshot 主体

**理由**：

- **codex 二审 BLOCK**：纯前端 patch 会被任何后续 `loadProjectData({ refresh: true })` 拿到的 backend stale snapshot 整体替换回滚——dashboard 从"偶尔 stale"变成"显示刚更新后又倒退到 28 分钟"，UX 比现状更差。必须把 overlay 下沉到后端，让 cache hit / miss 两条路径都返回合成后的 freshness mtime
- AtomicI64 单调 fetch_max 无锁、零阻塞——不破 hot path
- 数据结构选 HashMap key=(ContextId, ProjectId)：与现有 `entries: HashMap<ContextId, ...>` 形态一致，跨 SSH/Local context 天然隔离
- cache populate 时合并：scan 是 fs 真相，但 scan 期间可能有 append 事件让 overlay 已经超过 scan 时刻——保留较大值是单调性的自然推论

**替代方案 A（rejected，纯前端 patch）**：codex 已 BLOCK，详上。

**替代方案 B（rejected，per-project entry 拆分）**：把 `entries: HashMap<ContextId, Arc<Vec<Project>>>` 重构成 per-project 失效粒度——超本 change scope，spec 主 Requirement `MUST NOT 引入 per-project 失效粒度` 显式禁止。

**替代方案 C（rejected，cache hit 后 fs.stat）**：30 项目 × stat ≈ 几 ms hot path，破 perf 预算。

### D6: SSH polling interval 内 mtime 短暂过期列为 accepted limitation

**选**：SSH dashboard mtime freshness 受远端 polling 节拍约束（spec `file-watching::事件投递时延、远端 polling 频率与停止时延` 定义为 3 秒；catch-up 30 秒）；两次 poll 之间发生的远端 jsonl append，直到下一轮 poll diff 产出 file-change event 之前，cache overlay 仍可能显示上一轮观测到的 mtime。

**接受理由**：

- 本 change 不提高 SSH polling 频率，不引入新 SFTP stat；D3 仅透传既有 fingerprint mtime
- 与 Local 不同，SSH 远端无 OS 通知机制可用，polling 节拍是远端事件投递的物理上界——再低 polling 间隔会按比例增加远端 I/O 与连接负载
- 一旦 poll 发现 append（最长 3 秒），D5 overlay 路径保证后续 cache hit / miss 都返回新鲜 mtime，不再被 stale snapshot 回滚

**替代方案 A（rejected）**：dashboard fetch 时主动发 SFTP stat 实时校准 mtime——30 项目 × stat 在每次 IPC hot path，远端 I/O 大幅增加，破 perf "0 额外 SFTP stat" 约束 + 增加 SSH 连接负载

**替代方案 B（rejected）**：缩短 polling 间隔到 1 秒以下——远端 I/O 与连接负载按 3× 上升，部分 SFTP server 的 `read_dir` RPS 限流会被打到，引入新的可靠性问题，远超本 change scope

**用户感知影响**：SSH 用户在 dashboard 静止视图下看到的"最近活动"最长落后真实 mtime 一个 polling 间隔（3 秒）；首次 poll 发现后立即追上。Local 用户因 OS 通知 watcher 路径无该上界，仅受 debounce window（100ms）影响。

## Risks / Trade-offs

### Risk 1: overlay 与 snapshot 不一致窗口

[Risk] cache hit 时合成 overlay：snapshot.most_recent_session 是冷数据，overlay 是热数据，UI 看到的 mostRecentSession 字段是合成值——但 snapshot 主体里其他字段（sessions count、worktrees 列表）仍是冷数据。如果用户在这个窗口期内同时关心 mtime 和会话数，可能看到"mtime 是新的但会话总数是旧的"短暂态。

[Mitigation] 普通 append 不改变会话数（已知 session id），所以不一致窗口不会涉及"刚出现的新 session 没显示"——只在新 session 首次出现时（unknown_session 三档命中）整体 invalidate cache 一次性拉新，不走 overlay 路径。这条由 spec scenario 兜住。

### Risk 2: overlay 在长 SSH 切换循环下泄漏

[Risk] overlay HashMap 按 ContextId 分组，SSH switch 多次后旧 ContextId 的 overlay 条目永久驻留，理论可累积。

[Mitigation] 加 `invalidate_all()` 路径同步清空 overlay（与 entries 同步）；显式 `ssh_disconnect` / `reconfigure_claude_root` 路径走 `invalidate_all`。每个 ContextId 的 overlay 条目数 ≤ 项目数（30 量级），单条 16 byte，长期驻留也只 KB 级，不破 RSS 预算。

### Risk 3: SSH 远端 mtime 跨 clock domain

[Risk] SSH 远端 fingerprint 的 mtime 来自远端文件系统，与本机 `SystemTime::now()` 跨 clock domain；overlay 直接 max 比对会让远端 mtime 偏未来 / 偏过去都可能出现。

[Mitigation] overlay 仅作"显示用 mtime"，不参与 stale 判定（`is_session_stale` 已经在 metadata 路径有跨 clock domain 守护，本 overlay 不引入新跨 clock 比较）。spec 标注"SSH context 下 overlay 反映远端 mtime 字面量，与本机时钟不可比"。

### Risk 4: 跨 context 同名 project 的延迟事件可能误 patch overlay

[Risk] `FileChangeEvent` 不带 `ContextId` 字段；overlay 写入路径用 `is_local_project` 守护（与现有 `apply_file_event_to_project_scan_cache` 对齐）判定 event 来源 context。SSH switch 后旧 watcher 已 abort 但 broadcast 队列里残留的旧 event 进入 invalidator → 若该 project_id 在新 context 也存在同名 project（典型：远端与本地 home 下都有 `-Users-foo-bar` encoded 目录），可能误推进新 context 的 overlay。

[Mitigation] 误 patch 只让显示 mtime 偏新（"假新鲜"最多几秒）——不破数据结构、不引入崩溃。`is_local_project` 守护已是 accepted edge case（spec `ipc-data-api::ProjectScanCache 按事件语义分级失效` 末段已定义同款 limitation）；overlay 沿用同一守护边界。根治留 followup（开 GitHub Issue 跟踪"file-change event 携带 source ContextId"，让所有依赖 `is_local_project` 的路径都升级到精确 dispatch；超本 change scope）。

## 性能预算

四维评估（按 `.claude/rules/perf.md`）：

| 路径 | 当前基线 | 本 change 增量 | 是否破预算 |
|---|---|---|---|
| 本地 watcher 单 event | `path.exists()` 一次 syscall | 改成 `metadata()` 一次 syscall（替换不新增） | ✓ 不破 |
| SSH watcher 单 event | fingerprint mtime 已存在 | 透传到 event 字段，0 额外 SFTP | ✓ 不破 |
| `apply_file_event_to_project_scan_cache` | 三档判定 + counter | 加 1 次 atomic fetch_max（mtime advance 路径），普通 append 多 ~10ns | ✓ 不破 |
| `list_repository_groups` cache hit | 现 ~5µs | 加 30 项目 × atomic load + max → 总 < 10µs，wall 增量 ~3µs | ✓ 远低于 200ms 预算 |
| `list_repository_groups` cache miss | ~95ms 全扫 + populate | 加 30 项目 × overlay 合并写回 → < 50µs | ✓ 不破 |
| watcher → invalidator 链路 | broadcast capacity 256 | event 字段 +16 byte struct，SSE/IPC payload +20 byte JSON | ✓ 不破 |
| RSS | 现 ~80MB cold scan baseline | overlay HashMap 30 项目 × 16 byte = 480 byte | ✓ 不破 |
| user/real ratio | 0.13（cold scan baseline）| 不引入新并发，overlay 单点写无锁 | ✓ 不破 |

**反模式逐条核对**：无 hot loop spawn / 无串行 fs I/O / 无 hot path 大对象 clone / 无新 cache 缺 byte cap（overlay 自带 project 数 cap）/ 无 broadcast subscriber 增加。

## 与 TS 原版差异

TS 原版 `ProjectScanner.scan()` 无进程级 cache，每次 IPC 全扫——仍然有 dashboard stale 问题（前端 file-change handler 不调 `fetchProjects`/`fetchRepositoryGroups`），但是单层 staleness。Rust 端为 perf 引入的 `project_scan_cache` 让问题更严重；本 change 通过 overlay 路径让 Rust 端在保留 cache 性能优势的同时**比 TS 原版彻底**——用户停留 dashboard 不切窗口时也能实时刷新（TS 原版做不到）。

该背离不引入 TS bug，无需进 `TS_BASELINE_DEVIATIONS.md`，但本 design 显式记录。

## Migration Plan

不涉及。`FileChangeEvent.mtime_ms` 是新增 optional 字段，不影响既有持久化数据（broadcast event 不持久）。前端无改动，旧 client 继续工作。

## Open Questions

- 无（codex 二审 6 条 finding 已全部消化进 D1-D6）
