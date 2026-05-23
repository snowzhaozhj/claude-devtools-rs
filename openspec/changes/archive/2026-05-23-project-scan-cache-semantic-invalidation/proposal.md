## Why

桌面后端 PID 26766 在用户长时间使用过程中经历**事件触发型 CPU spike**：活动监视器观测到 52.3% CPU / 305 线程 / 795 idle wakeups/s；ps lifetime 平均 65.7%；运行 2h 累积 7,713 context-switch/s。诊断（含 codex 两轮二审）确认根因是 **fsevents → ProjectScanCache 全失效 → 下次 IPC 全扫**的自激震荡链：

1. 活跃 claude-code 会话每秒多次追加写 JSONL → macOS fsevents
2. `cdt-watch::FileWatcher` 100ms debounce 后 broadcast `FileChangeEvent`
3. `LocalDataApi::spawn_watcher_runtime` 内的 invalidator 对**任意**事件调 `ProjectScanCache::invalidate_local()`，清光所有 Local entry（`crates/cdt-api/src/ipc/project_scan_cache.rs:177-185`）
4. 前端 `Sidebar.svelte::file-change` handler 走 `loadProjects(true)` → `listRepositoryGroups` IPC
5. 后端 cache miss → `ProjectScanner::scan` 全扫 30 project × 538 session ≈ 16k+ fs::open / read_dir / canonicalize syscall
6. 每个 fs op 走 spawn_blocking 进 blocking pool → keep_alive=10s 内累积 211 idle worker
7. 下一波 fsevents 又来一次 → 重复

`ProjectScanCache` 已通过 PR #198 (`FU-4 ProjectScanner memoize`) 实现跨 IPC 复用，但当时为简化实现选择了"任何 file-change 全失效"——**JSONL 内容追加根本不改变 project / worktree 拓扑**，却被无差别失效。这是本次 change 修复的核心。

## What Changes

`cdt-watch::FileWatcher::parse_project_event` 已经把 fs path 形态压缩到 `cdt-core::FileChangeEvent` 的 4 个字段语义中：

| 真实 fs 事件 | watcher 输出 |
|---|---|
| 新 project 顶层 dir-create | `project_list_changed=true, session_id=""` |
| `mark_project_seen` 第一次见某 pid（含新 session 首次出现） | `project_list_changed=true` |
| 已知 session JSONL 追加 | `project_list_changed=false, deleted=false` |
| watcher 折叠的 subagent JSONL 改 | `project_list_changed=false, deleted=false`（事件 ids 是父 session） |
| session JSONL 删除 | `deleted=true` |

本 change 利用 watcher 这层语义压缩，把 invalidator 从"任何事件全失效"改为**三档判定**：

- **`event.project_list_changed == true` OR `event.deleted == true`** → 调 `invalidate_local()`（拓扑变 / session 删）
- **`event.session_id` 非空 AND `ProjectScanCache::contains_session_id(...)` 返回 false** → 调 `invalidate_local()`（已知 project 下新 session 首次出现）
- **其他**（普通 JSONL append + 折叠的 subagent 修改）→ no-op，cache 保留

> 第二档不能省：codex 第三轮二审实证 `cdt-watch::FileWatcher` 在构造时**预填**当前已存在的 project 目录到 `known_projects` HashSet（`crates/cdt-watch/src/watcher.rs:30-41,79`）。已知 project 下新建 session 时 `mark_project_seen` 不返回 true，watcher 输出 `plc=false, deleted=false`——与"已知 session 追加"事件外观完全相同。仅靠 (plc, deleted) 两 bool 判定会让新 session 最长 `LOCAL_CACHE_TTL = 300s` 不可见。第二档用 cache snapshot 反向查询 (`O(N session) ~10µs`) 补这个语义缺口，无须改 watcher / 不引入 path 字段。

辅助变更：

- 新增 3 个 telemetry counter（`project_scan_cache.invalidate.{structural, content_append_skipped, lag_conservative}`）以便观察失效行为分布
- 给 `ProjectScanCache` 加 `pub fn contains_session_id(&self, ctx, project_id, session_id) -> bool` API，供第二档判定使用
- broadcast `Lagged(_)` 走保守 `invalidate_local()`（与 `parsed-message 缓存按 file-change 广播主动失效` 的"静默继续 + 被动 signature 兜底"行为有意不一致——ProjectScanCache 无 path-level 被动校验机制）
- 把"`extract_session_cwd` 仅读首行"从隐含行为升级为 `project-discovery` capability 的契约不变量，由测试守护（用 `cdt-fs::with_fs_counter` 返回 `FsOpCounts.read_to_string == 0` 断言，scanner.fs 用 `InstrumentedFs` 包装才能计数）

非目标（本 change 不动）：

- **不**扩 `cdt-core::FileChangeEvent` 字段（codex 二审驳回 path 字段方案：subagent 折叠时 `(pid, sid, path)` 三元组语义不一致，是新引入的 confusing 设计）
- **不**改 `cdt-watch::FileWatcher::parse_project_event` 内的 path → 字段解析逻辑
- **不**动 `MetadataCache` / `ParsedMessageCache` 失效逻辑
- **不**动 `ParsedMessageCache invalidator` 内现有的 `(project_id, session_id) → path` 推算代码（codex 指出此处推算与新逻辑可统一，属于 cleanup 范畴，不在本 change scope）
- **不**动前端 `Sidebar.svelte::file-change` 250ms throttle（后续独立 PR）
- **不**动双 tokio runtime / `max_blocking_threads` 上限（后续独立 PR）
- **不**引入 per-project 失效粒度（`ProjectScanCache.entries: HashMap<ContextId, Arc<Vec<Project>>>` 当前数据结构无 per-project entry 概念，重构超本 change scope）

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ipc-data-api`: 新增 Requirement「`ProjectScanCache` 按事件语义分级失效」，描述基于 `project_list_changed` / `deleted` 字段的判定逻辑、3 个 telemetry counter 注册、broadcast lag 保守失效语义、SSH entry 不受影响等。既有的"任何事件失效"隐含语义被收紧为"结构性事件失效 / 普通追加不失效"。
- `project-discovery`: 新增 Requirement「`extract_session_cwd` 仅读首行的不变量」，把"cwd 在 JSONL 首行命中"从隐含行为升级为契约。`ipc-data-api::ProjectScanCache 按事件语义分级失效` 的 ContentAppend 路径不失效 cache 依赖此前提；测试用 `cdt-fs::FsOpCounter` 守护。

## Impact

**改动文件（预估）**：

- `crates/cdt-api/src/ipc/local.rs::spawn_watcher_runtime` 的 project-scan invalidator closure — 改写为三档判定（`project_list_changed` / `deleted` / `contains_session_id` 反查），每分支按 telemetry counter 注入；poison 走 `into_inner` 兜底
- `crates/cdt-api/src/ipc/project_scan_cache.rs` — 加 `pub fn contains_session_id(&self, ctx, project_id, session_id) -> bool`；把 `#[cfg(test)] fn insert` 升级为 `#[cfg(any(test, feature = "test-utils"))] pub fn insert`，让集成测试可注入 cache 状态
- `crates/cdt-telemetry/src/registry.rs::COUNTER_NAMES` — 加 3 个 counter 名
- `crates/cdt-discover/src/project_scanner.rs::tests` — 加 2 个 cwd 不变量测试
- `crates/cdt-api/tests/project_scan_cache_invalidation.rs` — 新建集成测试覆盖 9 个 Scenario + 跨 IPC 复用回归
- `openspec/specs/ipc-data-api/spec.md` — 主 spec 注解 ProjectScanCache 失效语义（archive 时 sync）
- `openspec/specs/project-discovery/spec.md` — 主 spec 注解 cwd 首行不变量（archive 时 sync）

**性能预算**：

- 目标：长时间使用桌面应用稳态 CPU < 10%（idle 期 < 2%）
- 验证：60s 长 sample 改前后对比 `File::open` / `read_dir` / `extract_session_cwd` 命中数下降 ≥ 80%；blocking pool worker 顶峰从 211 降到 ≤ 80

**风险面**：

- `ProjectScanCache` 失效粒度变细 → 可能漏失效场景出 stale 数据。codex 指出 hidden risk：`extract_session_cwd` 读前 20 行 + 兜底读全文件，session 内容变化**理论上**可能改 cwd 抽取结果。本 change 选择信任 first-line invariant，并在测试中用 `FsOpCounter` 断言 `read_to_string == 0` 升级为契约
- 兼容性：`ProjectScanCache` 是后端内部数据结构，不暴露给前端；新 telemetry counter 是纯增量

**依赖**：无新依赖。
