//! `ProjectScanner::scan()` 结果在 `LocalDataApi` 内的进程级缓存。
//!
//! 行为契约（性能优化，不改 IPC 返回字段）：
//! - key = `ContextId`，让 Local / SSH host 间天然隔离；与
//!   `MetadataCache` / `ParsedMessageCache` 同形（fs-abstraction PR-A
//!   `ContextId 三元组作为 cache key 前缀`）。
//! - value = `Arc<Vec<Project>>` + `root_generation` + `context_generation` + `inserted_at`。
//!   命中时直接返回 `Arc`，调用方零分配 iter 即可生成 `ProjectInfo` / `RepositoryGroup` 输入。
//! - 失效层级：
//!   1. **主动 watcher**（Local 唯一）：`spawn_project_scan_cache_invalidator`
//!      订阅 `FileWatcher` 广播，按事件语义**三档判定**调 `invalidate_local()`：
//!      `project_list_changed` / `deleted` / `contains_session_id` 反查未命中
//!      （已知 project 下新 session 首次出现）任一条件触发，普通 JSONL append
//!      与 watcher 折叠的 subagent 修改放行。详 `openspec/specs/ipc-data-api/spec.md`
//!      §`ProjectScanCache 按事件语义分级失效`。
//!   2. **被动 generation 校验**：cache hit 时若 `root_generation` 或 `context_generation` 与 entry 写入时不符 → miss。
//!   3. **TTL 兜底**：Local 5 分钟（watcher 已主动）、SSH 10 秒（无 watcher，靠 TTL 保证用户操作"切了 SSH context 后新建 session 几秒后能看到"）。
//!
//! Local entry 在 SSH 路径无需关心；SSH entry 不被 watcher 触发，仅靠 TTL
//! + generation 校验回收，单 SSH context 一份缓存。
//!
//! 不在本模块的失效路径：
//! - `root_generation` / `context_generation` 递增本身由 caller
//!   （`reconfigure_claude_root` / `switch_context` / `ssh_connect` /
//!   `ssh_disconnect`）保证；本 cache 仅消费 atomic 值。
//! - SSH disconnect 后旧 `ContextId` 对应 entry 走 TTL 自然过期；命中也
//!   不会有读者，因为 `active_fs_and_context_strict()` 已经报错挡掉。
//!
//! ## In-flight scan 与 watcher invalidation 的 race（codex 二审 #2）
//!
//! 普通文件变化（新建 session / 删 session）只 bump cache 内部
//! `invalidation_generation`，**不** bump `root_generation` /
//! `context_generation`。因此 `scan_projects_cached()` miss 后 `await`
//! 期间 watcher 收到事件 → `invalidate_local()` 清空 entry +
//! `invalidation_generation += 1`。在 scan 完成回写前若直接 insert，
//! 旧 snapshot 会盖掉 watcher 的清空信号，最长 Local TTL（5min）内
//! 一直返回旧数据。
//!
//! 解法：`scan_projects_cached()` 在 miss 路径 scan 前先记下当前
//! `invalidation_generation`，scan 完成后 insert 时比较；mismatch
//! → 丢弃本次 snapshot，让下次 lookup 走真实 miss 重 scan。
//! `try_insert` 内部完成校验，hot path 单 lock 临界区。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use cdt_core::Project;
use cdt_fs::{ContextId, FsKind};
#[cfg(any(test, feature = "test-utils"))]
use tokio::sync::broadcast;

/// Local cache entry TTL。watcher 已经主动 invalidate，本 TTL 只在 watcher
/// 关停 / lagged 时兜底，给一个很宽的窗口避免误命中过期数据。
pub const LOCAL_CACHE_TTL: Duration = Duration::from_secs(300);

/// SSH cache entry TTL。SSH context 没有 file watcher 主动 invalidate；
/// 10 秒让"切 group" / "新建 session 后立刻看 sidebar" 等典型操作能命中，
/// 同时避免远端 fs 状态长时间过期。
pub const SSH_CACHE_TTL: Duration = Duration::from_secs(10);

/// 单次 scan 结果在内存中的缓存条目。
#[derive(Clone)]
struct CacheEntry {
    snapshot: Arc<Vec<Project>>,
    root_generation: u64,
    context_generation: u64,
    inserted_at: Instant,
    /// entry 写入时的 `FsKind`，让 lookup 用对应 TTL（不再传 `&fs` 进 cache）。
    fs_kind: FsKind,
}

/// `LocalDataApi` 持有的 scan 结果缓存。**不**走全局单例；多实例隔离便于
/// 测试与未来多 root 拓展（同 `MetadataCache` / `ParsedMessageCache`）。
#[derive(Default)]
pub struct ProjectScanCache {
    entries: HashMap<ContextId, CacheEntry>,
    /// 单调递增的内部失效计数器。`invalidate_local` / `invalidate` 都会
    /// `+= 1`，让 in-flight scan 完成回写前能识别"期间 cache 被清过"
    /// 从而丢弃旧 snapshot（codex 二审 #2 race）。
    invalidation_generation: u64,
    /// 当前在途的 scan 数（`begin_scan()` += 1 / `finish_scan_with_insert()`
    /// / `abort_scan()` -= 1）。invalidator 在三档判定 `unknown_session` 时
    /// `has_entry || has_in_flight_scan` 共同决定是否 bump generation——
    /// cache 空但 scan 在途时 SHALL 仍 bump，让 in-flight scan 完成回写时
    /// 通过 `try_insert` 识别 race 丢弃 stale snapshot（codex PR 二审第二轮
    /// BLOCK 修复：原 `has_entry` 单条件守护漏掉"启动后第一次扫描"期间
    /// 新 session 事件 → snapshot 落地后等 TTL 5min 才能看到的问题）。
    in_flight_scans: u32,
    /// per-(`ContextId`, `project_id`) 单调推进的 mtime hint。带 mtime 的非删除
    /// file-change event SHALL 用 `advance_mtime` fetch-max 进入；
    /// `list_repository_groups` / `list_projects` 在合成路径取
    /// `max(snapshot.most_recent_session, overlay)` 让 dashboard 在 cache hit
    /// 命中路径也能反映新鲜 mtime。
    ///
    /// 解耦规则（spec `ipc-data-api/spec.md::ProjectScanCache 维护 per-project
    /// mtime overlay`）：
    /// - 三档 invalidate（[`Self::invalidate_local`]）SHALL **不**清 overlay——
    ///   overlay 是 watcher 单调观测的中间结果，丢失无法重建；snapshot 是
    ///   fs 真相的快照，可重新 scan
    /// - 显式 context 切换（[`Self::invalidate_all`] / [`Self::invalidate`]）
    ///   SHALL 清 overlay——上下文已切换，旧 hint 不再适用
    ///
    /// `i64` 而非 `AtomicI64`：外层已 `Mutex<ProjectScanCache>` 包裹，写路径
    /// 临界区单线程，不需要再叠 atomic。
    mtime_overlay: HashMap<(ContextId, String), i64>,
    /// 累计命中次数 / 累计 lookup 次数，调试 / perf bench 用。
    hits: u64,
    lookups: u64,
}

impl ProjectScanCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 尝试命中 cache。命中条件：
    /// 1. 同 `ContextId` 有 entry
    /// 2. entry 的 `root_generation` / `context_generation` 与当前一致
    /// 3. entry 未过 TTL（按写入时记录的 `FsKind` 选 Local / SSH TTL）
    ///
    /// 命中递增 `hits`；总 lookups 计数器无条件递增，便于外部计算命中率。
    pub fn lookup(
        &mut self,
        ctx: &ContextId,
        current_root_generation: u64,
        current_context_generation: u64,
    ) -> Option<Arc<Vec<Project>>> {
        self.lookups += 1;
        let entry = self.entries.get(ctx)?;
        if entry.root_generation != current_root_generation
            || entry.context_generation != current_context_generation
        {
            return None;
        }
        let ttl = match entry.fs_kind {
            FsKind::Local => LOCAL_CACHE_TTL,
            FsKind::Ssh => SSH_CACHE_TTL,
        };
        if entry.inserted_at.elapsed() > ttl {
            return None;
        }
        self.hits += 1;
        Some(entry.snapshot.clone())
    }

    /// 当前 `invalidation_generation` 的 read-only snapshot。
    /// 仅用于测试断言；生产路径走 [`Self::begin_scan`] /
    /// [`Self::finish_scan_with_insert`] 自动管理。
    #[cfg(any(test, feature = "test-utils"))]
    #[allow(dead_code)]
    pub fn invalidation_generation(&self) -> u64 {
        self.invalidation_generation
    }

    /// 无条件写入 / 覆盖 entry（**不**带 race 校验）。仅用于直接构造
    /// cache 的测试场景；生产路径用 [`Self::try_insert`] 防 in-flight
    /// scan race。
    ///
    /// `cfg(test)` + `feature = "test-utils"` 双门——同 crate 单测可用，
    /// 集成测试（`crates/cdt-api/tests/`）通过 `dev-deps cdt-api = { features = ["test-utils"] }`
    /// 可见。
    /// Spec：`openspec/specs/ipc-data-api/spec.md` §`ProjectScanCache 按事件语义分级失效`。
    #[cfg(any(test, feature = "test-utils"))]
    #[allow(dead_code)] // 集成测试通过 test-utils feature 调用；本 crate lib 内部不调
    pub fn insert(
        &mut self,
        ctx: ContextId,
        snapshot: Arc<Vec<Project>>,
        root_generation: u64,
        context_generation: u64,
        fs_kind: FsKind,
    ) {
        self.entries.insert(
            ctx,
            CacheEntry {
                snapshot,
                root_generation,
                context_generation,
                inserted_at: Instant::now(),
                fs_kind,
            },
        );
    }

    /// 反向查询：指定 ctx 的 entry snapshot 是否含 `(project_id, session_id)`
    /// 这一 session。`spawn_project_scan_cache_invalidator` 用于"已知 session
    /// 追加" vs "已知 project 下新 session 首次出现"的区分（D2 第三档判定）。
    ///
    /// 复杂度 O(N project × N `session_per_project`)；30 project × 538 session
    /// corpus 单次 ~10µs，可在 hot 路径调用。ctx 无 entry 或 project 不存在
    /// 时返回 `false`。
    ///
    /// **注意**：调用方 SHALL 先用 [`Self::has_entry`] 守护，避免 ctx 无 entry
    /// 时把"cache 空"误判为"unknown session"——后者会让 invalidator 在 lag 后
    /// 持续 bump `invalidation_generation`，导致 in-flight scan `try_insert`
    /// 一直 mismatch，cache 长期无法 repopulate（codex PR 二审 WARN 1）。
    /// Spec：`openspec/specs/ipc-data-api/spec.md` §`ProjectScanCache 按事件语义分级失效`。
    #[must_use]
    pub fn contains_session_id(&self, ctx: &ContextId, project_id: &str, session_id: &str) -> bool {
        let Some(entry) = self.entries.get(ctx) else {
            return false;
        };
        entry
            .snapshot
            .iter()
            .find(|p| p.id == project_id)
            .is_some_and(|p| p.sessions.iter().any(|s| s == session_id))
    }

    /// 指定 ctx 是否有 cache entry。`spawn_project_scan_cache_invalidator`
    /// 在三档判定的"unknown session"档前先用本方法守护——cache 空时跳过
    /// `invalidate_local()` 与 generation bump，避免 lag 后清空状态被持续
    /// 普通 append 事件引发的"在重扫期间反复 bump → `try_insert` 一直 mismatch
    /// → cache 长期无法 repopulate"风暴（codex PR 二审 WARN 1）。
    #[must_use]
    pub fn has_entry(&self, ctx: &ContextId) -> bool {
        self.entries.contains_key(ctx)
    }

    /// 当前是否有 in-flight scan 在跑。invalidator 在 cache 空但 scan 在途
    /// 时 SHALL 仍走 `unknown_session` 判定 bump generation，让 in-flight scan
    /// 完成回写时 `try_insert` 识别 race（codex PR 二审第二轮 BLOCK 修复）。
    #[must_use]
    pub fn has_in_flight_scan(&self) -> bool {
        self.in_flight_scans > 0
    }

    /// 标记一次 scan 开始：`in_flight_scans` += 1，返回当前
    /// `invalidation_generation` 给调用方记录。配对 [`Self::finish_scan_with_insert`]
    /// （成功路径）或 [`Self::abort_scan`]（错误路径）使用。
    pub fn begin_scan(&mut self) -> u64 {
        self.in_flight_scans = self.in_flight_scans.saturating_add(1);
        self.invalidation_generation
    }

    /// scan 失败时调，`in_flight_scans` -= 1。不动 entries。
    pub fn abort_scan(&mut self) {
        self.in_flight_scans = self.in_flight_scans.saturating_sub(1);
    }

    /// scan 成功时调：`in_flight_scans` -= 1 + 校验 generation 未变并写入 entry。
    /// `recorded_generation` 是 [`Self::begin_scan`] 时拿的 snapshot；若期间
    /// `invalidation_generation` 被 invalidator bump → mismatch → 丢弃 snapshot
    /// 返 `false`，下次 lookup 走真实 miss 重 scan。
    ///
    /// 写入 entry 后 SHALL 调 [`Self::merge_overlay_with_fresh_snapshot`] 完成
    /// hint 合并（详 spec `ipc-data-api::ProjectScanCache 维护 per-project mtime
    /// overlay`）：
    /// - fresh snapshot 已反映或超过 hint → 移除 hint
    /// - hint 仍大于 snapshot → 保留 hint（不被回退）
    /// - fresh snapshot 不再含某 project → 移除该 project 的 hint（避免已删除
    ///   project 的 hint 永久驻留）
    pub fn finish_scan_with_insert(
        &mut self,
        ctx: ContextId,
        snapshot: Arc<Vec<Project>>,
        root_generation: u64,
        context_generation: u64,
        fs_kind: FsKind,
        recorded_generation: u64,
    ) -> bool {
        self.in_flight_scans = self.in_flight_scans.saturating_sub(1);
        if recorded_generation != self.invalidation_generation {
            return false;
        }
        self.merge_overlay_with_fresh_snapshot(&ctx, &snapshot);
        self.entries.insert(
            ctx,
            CacheEntry {
                snapshot,
                root_generation,
                context_generation,
                inserted_at: Instant::now(),
                fs_kind,
            },
        );
        true
    }

    /// fetch-max 单调推进 `(ctx, project_id)` 对应的 mtime hint。带 mtime 的
    /// 非删除 file-change event SHALL 经此入口；删除事件 / 缺 mtime 的事件
    /// **不**调本方法（`apply_mtime_advance_to_project_scan_cache` 在 wrapper
    /// 处守护）。
    ///
    /// 单调性：仅当传入值大于已记录值时更新；写入只追加 / 提升，不会回退。
    /// Spec：`ipc-data-api/spec.md::ProjectScanCache 维护 per-project mtime
    /// overlay::已知 session 普通 append 推进 hint`。
    pub fn advance_mtime(&mut self, ctx: &ContextId, project_id: &str, mtime_ms: i64) {
        let key = (ctx.clone(), project_id.to_owned());
        match self.mtime_overlay.get_mut(&key) {
            Some(slot) if *slot < mtime_ms => *slot = mtime_ms,
            None => {
                self.mtime_overlay.insert(key, mtime_ms);
            }
            _ => {}
        }
    }

    /// 查 `(ctx, project_id)` 的 hint。`None` 表示无记录；调用方在合成路径取
    /// `max(snapshot.most_recent_session_ms, overlay)` 即可——空缺时合成结果
    /// 落到 snapshot 原值。
    #[must_use]
    pub fn lookup_mtime_overlay(&self, ctx: &ContextId, project_id: &str) -> Option<i64> {
        let key = (ctx.clone(), project_id.to_owned());
        self.mtime_overlay.get(&key).copied()
    }

    /// 重扫合并：fresh snapshot 落库前调一次。规则按 spec：
    ///
    /// - snapshot 已反映或超过 hint → 移除 hint（snapshot 已是新真相）
    /// - hint 仍大于 snapshot → 保留 hint（scan 期间 append 不被回退）
    /// - fresh snapshot 不再含某 project → 移除该 project 的 hint（已删除
    ///   project 的 hint 永久驻留无意义）
    ///
    /// 仅作用于本次 scan 涉及的 `ctx`；其他 context 下 hint 不动。
    fn merge_overlay_with_fresh_snapshot(&mut self, ctx: &ContextId, snapshot: &[Project]) {
        let live_projects: HashMap<&str, Option<i64>> = snapshot
            .iter()
            .map(|p| (p.id.as_str(), p.most_recent_session))
            .collect();
        self.mtime_overlay.retain(|(ckey, pid), hint| {
            if ckey != ctx {
                return true;
            }
            match live_projects.get(pid.as_str()) {
                Some(Some(snap)) if *snap >= *hint => false,
                Some(Some(_) | None) => true,
                None => false,
            }
        });
    }

    /// 条件写入：仅在 `recorded_generation == 当前 invalidation_generation`
    /// 时落 entry；mismatch → 丢弃本次 snapshot，返回 `false`。让
    /// in-flight scan 不覆盖 watcher 在 scan 期间发出的 invalidate 信号
    /// （codex 二审 #2）。
    ///
    /// 仅用于测试场景；生产路径走 [`Self::finish_scan_with_insert`]，
    /// 内部含 `in_flight_scans` -= 1 + race 校验。本方法不动
    /// `in_flight_scans`，便于测试单独构造 race 场景。
    #[cfg(any(test, feature = "test-utils"))]
    #[allow(dead_code)]
    pub fn try_insert(
        &mut self,
        ctx: ContextId,
        snapshot: Arc<Vec<Project>>,
        root_generation: u64,
        context_generation: u64,
        fs_kind: FsKind,
        recorded_generation: u64,
    ) -> bool {
        if recorded_generation != self.invalidation_generation {
            return false;
        }
        self.entries.insert(
            ctx,
            CacheEntry {
                snapshot,
                root_generation,
                context_generation,
                inserted_at: Instant::now(),
                fs_kind,
            },
        );
        true
    }

    /// 清除 Local `ContextId` 对应 entry。watcher invalidator 用。SSH entry
    /// 由 TTL 自然过期，本接口不动 SSH entry（避免 Local 文件变化误清远端）。
    /// 同步 bump `invalidation_generation` —— 让 in-flight scan 完成回写时
    /// 通过 `try_insert` 自检并丢弃旧 snapshot。
    ///
    /// **不**清 mtime overlay：overlay 是 watcher 单调观测的中间结果，丢失
    /// 无法重建；snapshot 是 fs 真相的快照，可重新 scan（spec
    /// `ipc-data-api/spec.md::ProjectScanCache 维护 per-project mtime overlay::
    /// 三档 invalidate 不清 hint`）。
    pub fn invalidate_local(&mut self) {
        self.entries
            .retain(|_, entry| !matches!(entry.fs_kind, FsKind::Local));
        self.invalidation_generation = self.invalidation_generation.wrapping_add(1);
    }

    /// 清除所有 entry（Local + SSH）+ 所有 mtime overlay。
    /// `reconfigure_claude_root` / SSH context 切换等显式 hook 用；同步 bump
    /// `invalidation_generation`。测试也可用本入口让 SSH 路径测试用例之间
    /// 不串扰。
    ///
    /// 同时清 overlay 因为上下文已切换，旧 hint 不再适用（spec
    /// `ipc-data-api/spec.md::ProjectScanCache 维护 per-project mtime overlay::
    /// 显式 invalidate 总清同时清 hint`）。
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
        self.mtime_overlay.clear();
        self.invalidation_generation = self.invalidation_generation.wrapping_add(1);
    }

    /// 单 entry 删除（test / `reconfigure_claude_root` 等显式 hook 用）。
    /// 同步 bump `invalidation_generation`，且清空该 ctx 下所有 project 的
    /// mtime overlay（context 切换语义）。
    #[allow(dead_code)]
    pub fn invalidate(&mut self, ctx: &ContextId) {
        if self.entries.remove(ctx).is_some() {
            self.invalidation_generation = self.invalidation_generation.wrapping_add(1);
        }
        self.mtime_overlay.retain(|(ckey, _), _| ckey != ctx);
    }

    /// 当前缓存条目数。perf bench / 调试用。
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空。与 `len` 配对（clippy `len_zero`）。
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 累计命中数 / 累计 lookup 数。perf bench / 调试用。
    #[allow(dead_code)]
    pub fn stats(&self) -> ProjectScanCacheStats {
        ProjectScanCacheStats {
            hits: self.hits,
            lookups: self.lookups,
        }
    }
}

/// `ProjectScanCache` 累计命中统计。
#[derive(Debug, Clone, Copy)]
pub struct ProjectScanCacheStats {
    pub hits: u64,
    pub lookups: u64,
}

/// 启动后台 invalidator —— 订阅 file-watcher 广播，按事件语义**三档判定**
/// 是否调 `invalidate_local()`。watcher 自身已 debounce（详 `cdt-watch`）。
///
/// **判定规则**（详 `openspec/specs/ipc-data-api/spec.md` §`ProjectScanCache 按事件语义分级失效`）：
///
/// 1. `event.project_list_changed == true` **OR** `event.deleted == true` →
///    `invalidate_local()` + counter `project_scan_cache.invalidate.structural`
/// 2. `event.session_id` 非空 **AND** `contains_session_id(local_ctx, pid, sid) == false`
///    → 同规则 1（已知 project 下新 session 首次出现；watcher
///    `mark_project_seen` 构造时预填 `known_projects`，输出 `plc=false` 与
///    "已知 session 追加"外观相同，需 cache snapshot 反查）
/// 3. 其他（普通 JSONL append + watcher 折叠的 subagent 修改）→ no-op +
///    counter `project_scan_cache.invalidate.content_append_skipped`
///
/// `Err(Lagged)` 走保守 `invalidate_local()` + counter
/// `project_scan_cache.invalidate.lag_conservative`——`ProjectScanCache`
/// 无 path-level 被动校验机制，lag 期间错过的结构性事件没有兜底兑现，
/// 必须保守清空（与 `parsed-message 缓存按 file-change 广播主动失效`
/// 的 lag 静默继续策略**有意不一致**，详 design D7）。
/// `Err(Closed)` 时退出 loop。
///
/// `projects_dir` 用于构造作用域 `ContextId::local`；invalidator 只动
/// 该 ctx 的 entry，SSH entry 由 `invalidate_local()` 自身按 `FsKind::Local`
/// 隔离保留（详 design D5）。
/// 单一 cache 失效 task（仅集成测试与 test-utils feature 复用）。
///
/// 生产路径已切到 `local.rs::spawn_unified_cache_invalidator`（issue #261，
/// 一个 task 同时派发给 `ProjectScanCache` + `ParsedMessageCache`）。本函数保留
/// 作为 `tests/project_scan_cache_invalidation.rs` 的薄 wrapper，让 600+ 行
/// 三档判定 / lag conservative 行为契约测试零改动继续生效；prod 路径不再 spawn。
#[cfg(any(test, feature = "test-utils"))]
pub fn spawn_project_scan_cache_invalidator(
    cache: Arc<std::sync::Mutex<ProjectScanCache>>,
    mut rx: broadcast::Receiver<cdt_core::FileChangeEvent>,
    projects_dir: std::path::PathBuf,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let local_ctx = ContextId::local(projects_dir);
        // hot-path counter 引用一次性缓存：原 `registry().counter(name)` 每个 file
        // event 走一次 `&'static str` key 的 hashmap lookup（含 hash + equality）。
        // 改用 `counter!` 宏 → 每个 callsite 内部 `OnceLock<CounterRef>` 在首次调用
        // 后退化为 atomic load 的纯 atomic 路径（issue #255：v0.5.6 → v0.5.8 idle
        // CPU 回归直接相关）。
        loop {
            match rx.recv().await {
                Ok(event) => {
                    // test-utils fallback 路径只用 invalidated 判定，不 emit
                    let _decision =
                        apply_file_event_to_project_scan_cache(&cache, &local_ctx, &event);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    apply_lag_to_project_scan_cache(&cache);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

/// cache 层拆分后的 emit/invalidate 决策结果（change `enrich-via-watcher` D4）。
///
/// - `invalidated`：是否调用了 `invalidate_local()`（三档判定任一命中）
/// - `emit_session_list_changed_hint`：cache snapshot 视角下本 event 是否命中
///   规则 2 "`unknown_session`" 判定条件——给 `spawn_unified_cache_invalidator`
///   emit 路径 OR 公式用：`event.session_list_changed || hint`
pub(crate) struct EnrichDecision {
    /// 三档判定命中时为 true。当前 unified invalidator 不直接消费此字段
    /// （invalidate 已在函数内部完成），保留给测试断言 + 未来扩展。
    #[allow(dead_code)]
    pub invalidated: bool,
    pub emit_session_list_changed_hint: bool,
}

/// 单条 `FileChangeEvent` 应用到 `ProjectScanCache` 的逻辑：三档判定 +
/// counter 记录。**无 fs op**（纯 cache snapshot 反查），适合做合并 invalidator
/// 的 sync 快路径（issue #261，scan-first 顺序，避免被 parsed-cache 的
/// `fs.stat().await` 拖慢结构判定）。
///
/// 返回 [`EnrichDecision`]：`invalidated` 表示是否命中三档之一并调了
/// `invalidate_local()`；`emit_session_list_changed_hint` 给 emit 路径
/// OR 公式用（watcher 已填字段 + cache hint 取并集）。
///
/// 行为契约：spec `ipc-data-api/spec.md` §"`ProjectScanCache` 按事件语义分级失效"
/// + §"Unified invalidator 作为 `LocalDataApi.file_tx` 唯一生产者"。
pub(crate) fn apply_file_event_to_project_scan_cache(
    cache: &Arc<std::sync::Mutex<ProjectScanCache>>,
    local_ctx: &ContextId,
    event: &cdt_core::FileChangeEvent,
) -> EnrichDecision {
    let (invalidated, emit_session_list_changed_hint) = {
        // sync mutex（poison 走 into_inner 兜底，参照 cdt-api 既有模式）。
        // counter inc 在 drop guard 之后避免持锁期间走 atomic 路径加大临界区。
        let mut cache = match cache.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        // **守护组合**（codex PR 二审两轮迭代）：
        // - `has_entry`（WARN 1）：ctx 无 entry 时跳过 unknown_session
        //   判定，避免 lag 后续普通 append 反复 bump 引发风暴
        // - `has_in_flight_scan`（BLOCK 修复）：cache 空但有 scan
        //   在途时仍走 unknown_session 判定 bump generation，让
        //   in-flight scan 完成回写时 `finish_scan_with_insert`
        //   识别 race 丢弃可能 stale 的 snapshot（不漏新 session
        //   first-appearance 等结构事件）
        let track_unknown = cache.has_entry(local_ctx) || cache.has_in_flight_scan();
        let unknown_session = !event.session_id.is_empty()
            && track_unknown
            && !cache.contains_session_id(local_ctx, &event.project_id, &event.session_id);
        let structural = event.project_list_changed || event.deleted || unknown_session;
        if structural {
            cache.invalidate_local();
        }
        (structural, unknown_session)
    };
    if invalidated {
        cdt_telemetry::counter!("project_scan_cache.invalidate.structural").inc();
    } else {
        cdt_telemetry::counter!("project_scan_cache.invalidate.content_append_skipped").inc();
    }
    EnrichDecision {
        invalidated,
        emit_session_list_changed_hint,
    }
}

/// 单条 `FileChangeEvent` 把 `mtime_ms` 推进到指定 ctx 下对应 project 的
/// overlay。带 mtime 的非删除事件 SHALL 调本入口；删除事件 / 缺 mtime 事件 /
/// `project_id` 空（lag synthetic 事件）SHALL 早 return 不写。
///
/// `ctx` 由 invalidator 决定——Local event 写 Local context，SSH event 写
/// SSH active context。跨 context 隔离不变量：本入口仅作用于传入的 ctx；
/// 不同 ctx 下同名 project 互不影响（spec `ipc-data-api/spec.md::
/// ProjectScanCache 维护 per-project mtime overlay::SSH event 推进对应 SSH
/// context hint 但不影响 Local invalidate`）。
pub(crate) fn apply_mtime_advance_to_project_scan_cache(
    cache: &Arc<std::sync::Mutex<ProjectScanCache>>,
    ctx: &ContextId,
    event: &cdt_core::FileChangeEvent,
) {
    if event.deleted || event.project_id.is_empty() {
        return;
    }
    let Some(mtime_ms) = event.mtime_ms else {
        return;
    };
    let mut cache = match cache.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.advance_mtime(ctx, &event.project_id, mtime_ms);
}

/// `broadcast::Receiver::recv` 返回 `Err(Lagged)` 时的保守清空逻辑——
/// `ProjectScanCache` 无 path-level 被动校验机制，lag 期间错过的结构性事件
/// 没有兜底兑现，必须保守 `invalidate_local()` + `lag_conservative` counter。
///
/// 与 `parsed-message 缓存按 file-change 广播主动失效` 的 lag 静默继续策略
/// **有意不一致**——详 design D7 / spec ipc-data-api Requirement
/// "`ProjectScanCache` 按事件语义分级失效"。
pub(crate) fn apply_lag_to_project_scan_cache(cache: &Arc<std::sync::Mutex<ProjectScanCache>>) {
    {
        let mut cache = match cache.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        cache.invalidate_local();
    }
    cdt_telemetry::counter!("project_scan_cache.invalidate.lag_conservative").inc();
}

/// `list_repository_groups` / `list_projects` 在 cache hit 与 miss 路径
/// 返回前 SHALL 经此 helper 把 overlay 的 mtime hint 合成进对外 snapshot
/// 的 `Project.most_recent_session`。
///
/// 行为：
/// - 至少一个 project 有 `overlay > snapshot.most_recent_session` → clone 一份
///   `Vec<Project>`，注入合成值后包成新 Arc 返回；
/// - 无 hint 或所有 hint 都 ≤ snapshot 值 → 直接返原 Arc 零分配。
///
/// 合成只改返回数据，**不**改 cache 内部 snapshot 主体——overlay 仍由
/// `merge_overlay_with_fresh_snapshot` 在重扫时合并。Spec：
/// `ipc-data-api/spec.md::ProjectScanCache 维护 per-project mtime overlay::
/// cache hit 路径合成 hint 让用户看到最新 mtime`。
#[must_use]
pub(crate) fn synthesize_projects_with_overlay(
    cache: &ProjectScanCache,
    ctx: &ContextId,
    snapshot: &Arc<Vec<Project>>,
) -> Arc<Vec<Project>> {
    let needs_synth = snapshot.iter().any(|p| {
        cache.lookup_mtime_overlay(ctx, &p.id).is_some_and(|hint| {
            hint > p.most_recent_session.unwrap_or(i64::MIN)
        })
    });
    if !needs_synth {
        return snapshot.clone();
    }
    let mut new_vec = (**snapshot).clone();
    for p in &mut new_vec {
        if let Some(hint) = cache.lookup_mtime_overlay(ctx, &p.id) {
            let merged = std::cmp::max(p.most_recent_session.unwrap_or(i64::MIN), hint);
            p.most_recent_session = Some(merged);
        }
    }
    Arc::new(new_vec)
}

/// 给外部读 / 调试 cache 内部状态的 atomic 句柄包装。让 `LocalDataApi`
/// 字段 doc 显式记录 `Arc<AtomicU64>` 的来源，并避免 `local.rs` 内部
/// `std::sync::atomic::Ordering` 重复 import 散落。
#[derive(Default)]
#[allow(dead_code)]
pub struct ScanCacheGeneration(pub Arc<AtomicU64>);

impl ScanCacheGeneration {
    #[allow(dead_code)]
    pub fn load(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_fs::{HostSignature, SshConfigDigestInput};
    use std::path::PathBuf;

    fn local_ctx() -> ContextId {
        ContextId::local(PathBuf::from("/home/u/.claude/projects"))
    }

    fn ssh_ctx() -> ContextId {
        let sig = HostSignature::from_ssh_config_fields(&SshConfigDigestInput {
            hostname: "example.com".into(),
            port: 22,
            user: "alice".into(),
            identity_files: vec![],
            proxyjump: None,
            proxycommand: None,
            hostkeyalias: None,
        });
        ContextId::ssh(sig, PathBuf::from("/home/u/.claude/projects"))
    }

    fn snapshot() -> Arc<Vec<Project>> {
        Arc::new(Vec::new())
    }

    #[test]
    fn miss_when_empty() {
        let mut c = ProjectScanCache::new();
        assert!(c.lookup(&local_ctx(), 0, 0).is_none());
        assert_eq!(c.stats().hits, 0);
        assert_eq!(c.stats().lookups, 1);
    }

    #[test]
    fn hit_after_insert_same_generation() {
        let mut c = ProjectScanCache::new();
        c.insert(local_ctx(), snapshot(), 1, 2, FsKind::Local);
        assert!(c.lookup(&local_ctx(), 1, 2).is_some());
        assert_eq!(c.stats().hits, 1);
    }

    #[test]
    fn miss_when_root_generation_changes() {
        let mut c = ProjectScanCache::new();
        c.insert(local_ctx(), snapshot(), 1, 2, FsKind::Local);
        assert!(c.lookup(&local_ctx(), 2, 2).is_none());
    }

    #[test]
    fn miss_when_context_generation_changes() {
        let mut c = ProjectScanCache::new();
        c.insert(local_ctx(), snapshot(), 1, 2, FsKind::Local);
        assert!(c.lookup(&local_ctx(), 1, 3).is_none());
    }

    #[test]
    fn miss_for_different_context_id() {
        let mut c = ProjectScanCache::new();
        c.insert(local_ctx(), snapshot(), 1, 2, FsKind::Local);
        assert!(c.lookup(&ssh_ctx(), 1, 2).is_none());
    }

    #[test]
    fn invalidate_local_clears_local_only() {
        let mut c = ProjectScanCache::new();
        c.insert(local_ctx(), snapshot(), 1, 2, FsKind::Local);
        c.insert(ssh_ctx(), snapshot(), 1, 2, FsKind::Ssh);
        c.invalidate_local();
        assert!(c.lookup(&local_ctx(), 1, 2).is_none());
        assert!(c.lookup(&ssh_ctx(), 1, 2).is_some());
    }

    #[test]
    fn invalidate_specific_context_removes_one_entry() {
        let mut c = ProjectScanCache::new();
        c.insert(local_ctx(), snapshot(), 1, 2, FsKind::Local);
        c.insert(ssh_ctx(), snapshot(), 1, 2, FsKind::Ssh);
        c.invalidate(&ssh_ctx());
        assert!(c.lookup(&local_ctx(), 1, 2).is_some());
        assert!(c.lookup(&ssh_ctx(), 1, 2).is_none());
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn try_insert_succeeds_when_generation_unchanged() {
        let mut c = ProjectScanCache::new();
        let gen_snapshot = c.invalidation_generation();
        let ok = c.try_insert(local_ctx(), snapshot(), 1, 2, FsKind::Local, gen_snapshot);
        assert!(ok);
        assert!(c.lookup(&local_ctx(), 1, 2).is_some());
    }

    #[test]
    fn try_insert_drops_snapshot_when_invalidate_local_ran_during_scan() {
        // 模拟：scan 开始前 snapshot 当前 invalidation_generation
        // → 期间 watcher 触发 invalidate_local（bump generation）
        // → scan 完成 try_insert 时 mismatch 应该丢弃
        let mut c = ProjectScanCache::new();
        let recorded = c.invalidation_generation();
        c.invalidate_local(); // 期间 watcher 事件
        let inserted = c.try_insert(local_ctx(), snapshot(), 1, 2, FsKind::Local, recorded);
        assert!(
            !inserted,
            "watcher 在 scan 期间 invalidate 后 SHALL NOT 让旧 snapshot 写入"
        );
        assert!(c.lookup(&local_ctx(), 1, 2).is_none());
    }

    #[test]
    fn invalidate_all_clears_local_and_ssh() {
        let mut c = ProjectScanCache::new();
        c.insert(local_ctx(), snapshot(), 1, 2, FsKind::Local);
        c.insert(ssh_ctx(), snapshot(), 1, 2, FsKind::Ssh);
        c.invalidate_all();
        assert!(c.lookup(&local_ctx(), 1, 2).is_none());
        assert!(c.lookup(&ssh_ctx(), 1, 2).is_none());
        assert!(c.is_empty());
    }

    #[test]
    fn invalidate_local_bumps_invalidation_generation() {
        let mut c = ProjectScanCache::new();
        let g0 = c.invalidation_generation();
        c.invalidate_local();
        let g1 = c.invalidation_generation();
        assert_ne!(g0, g1);
    }

    fn snapshot_with(projects: Vec<Project>) -> Arc<Vec<Project>> {
        Arc::new(projects)
    }

    fn proj(id: &str, sessions: &[&str]) -> Project {
        Project {
            id: id.into(),
            name: id.into(),
            path: PathBuf::new(),
            sessions: sessions.iter().map(|s| (*s).to_string()).collect(),
            most_recent_session: None,
            created_at: None,
            distinct_cwds: Vec::new(),
        }
    }

    #[test]
    fn contains_session_id_returns_false_when_no_entry() {
        let c = ProjectScanCache::new();
        assert!(!c.contains_session_id(&local_ctx(), "pa", "sa"));
    }

    #[test]
    fn contains_session_id_returns_false_when_project_absent() {
        let mut c = ProjectScanCache::new();
        c.insert(
            local_ctx(),
            snapshot_with(vec![proj("pb", &["sb"])]),
            1,
            2,
            FsKind::Local,
        );
        assert!(!c.contains_session_id(&local_ctx(), "pa", "sb"));
    }

    #[test]
    fn contains_session_id_returns_false_when_session_absent() {
        let mut c = ProjectScanCache::new();
        c.insert(
            local_ctx(),
            snapshot_with(vec![proj("pa", &["sa1", "sa2"])]),
            1,
            2,
            FsKind::Local,
        );
        assert!(!c.contains_session_id(&local_ctx(), "pa", "sa3"));
    }

    #[test]
    fn contains_session_id_returns_true_on_hit() {
        let mut c = ProjectScanCache::new();
        c.insert(
            local_ctx(),
            snapshot_with(vec![proj("pa", &["sa1", "sa2"])]),
            1,
            2,
            FsKind::Local,
        );
        assert!(c.contains_session_id(&local_ctx(), "pa", "sa2"));
    }

    #[test]
    fn contains_session_id_isolates_across_contexts() {
        let mut c = ProjectScanCache::new();
        c.insert(
            local_ctx(),
            snapshot_with(vec![proj("pa", &["sa"])]),
            1,
            2,
            FsKind::Local,
        );
        c.insert(
            ssh_ctx(),
            snapshot_with(vec![proj("pb", &["sb"])]),
            1,
            2,
            FsKind::Ssh,
        );
        // Local ctx 命中 sa 不会让 SSH ctx 见到 sa
        assert!(c.contains_session_id(&local_ctx(), "pa", "sa"));
        assert!(!c.contains_session_id(&ssh_ctx(), "pa", "sa"));
        // SSH ctx 命中 sb 不会让 Local ctx 见到 sb
        assert!(c.contains_session_id(&ssh_ctx(), "pb", "sb"));
        assert!(!c.contains_session_id(&local_ctx(), "pb", "sb"));
    }

    // -- mtime overlay scenarios (spec ipc-data-api/spec.md::ProjectScanCache 维护
    // per-project mtime overlay) --

    fn proj_with_mtime(id: &str, sessions: &[&str], most_recent_session: Option<i64>) -> Project {
        Project {
            id: id.into(),
            name: id.into(),
            path: PathBuf::new(),
            sessions: sessions.iter().map(|s| (*s).to_string()).collect(),
            most_recent_session,
            created_at: None,
            distinct_cwds: Vec::new(),
        }
    }

    fn fc(
        project_id: &str,
        session_id: &str,
        deleted: bool,
        plc: bool,
        slc: bool,
        mtime_ms: Option<i64>,
    ) -> cdt_core::FileChangeEvent {
        cdt_core::FileChangeEvent {
            project_id: project_id.into(),
            session_id: session_id.into(),
            deleted,
            project_list_changed: plc,
            session_list_changed: slc,
            mtime_ms,
        }
    }

    /// Scenario `已知 session 普通 append 推进 hint 但不 invalidate`：
    /// 已知 session 的 append（plc=false / deleted=false / slc=false）
    /// SHALL 推进 hint，且 SHALL NOT 触发三档 invalidate。
    #[test]
    fn known_session_append_advances_hint_without_invalidate() {
        let cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));
        let ctx = local_ctx();
        // 预填 entry：含 (pa, sa)，hint 当前为 t0=100
        {
            let mut c = cache.lock().unwrap();
            c.insert(
                ctx.clone(),
                snapshot_with(vec![proj_with_mtime("pa", &["sa"], Some(100))]),
                1,
                2,
                FsKind::Local,
            );
            c.advance_mtime(&ctx, "pa", 100);
        }
        let event = fc("pa", "sa", false, false, false, Some(500));
        let decision = apply_file_event_to_project_scan_cache(&cache, &ctx, &event);
        apply_mtime_advance_to_project_scan_cache(&cache, &ctx, &event);
        assert!(
            !decision.invalidated,
            "已知 session append SHALL NOT 触发三档 invalidate"
        );
        let c = cache.lock().unwrap();
        assert_eq!(
            c.lookup_mtime_overlay(&ctx, "pa"),
            Some(500),
            "hint SHALL 单调推进到 t1"
        );
        // entry 仍在
        assert!(c.entries.contains_key(&ctx));
    }

    /// Scenario `删除事件不推进 hint`：deleted=true 的 event SHALL NOT
    /// 写 hint，但仍按既有规则触发 invalidate。
    #[test]
    fn deletion_event_does_not_advance_hint_but_still_invalidates() {
        let cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));
        let ctx = local_ctx();
        {
            let mut c = cache.lock().unwrap();
            c.insert(
                ctx.clone(),
                snapshot_with(vec![proj_with_mtime("pa", &["sa"], Some(100))]),
                1,
                2,
                FsKind::Local,
            );
            c.advance_mtime(&ctx, "pa", 100);
        }
        let event = fc("pa", "sa", true, false, true, None);
        let decision = apply_file_event_to_project_scan_cache(&cache, &ctx, &event);
        apply_mtime_advance_to_project_scan_cache(&cache, &ctx, &event);
        assert!(
            decision.invalidated,
            "deleted=true SHALL 命中三档第一档 invalidate"
        );
        let c = cache.lock().unwrap();
        assert_eq!(
            c.lookup_mtime_overlay(&ctx, "pa"),
            Some(100),
            "删除事件 SHALL NOT 改写 hint"
        );
    }

    /// Scenario `cache hit 路径合成 hint 让用户看到最新 mtime`：
    /// 多次 append 推进 hint 后，外部读取 hint SHALL 返回最大值；hint 不修改
    /// snapshot 主体（lookup 仍返回原 Arc，调用方在合成路径取 max）。
    #[test]
    fn cache_hit_path_synthesizes_via_overlay() {
        let mut c = ProjectScanCache::new();
        let ctx = local_ctx();
        let snap = snapshot_with(vec![proj_with_mtime("pa", &["sa"], Some(100))]);
        c.insert(ctx.clone(), snap.clone(), 1, 2, FsKind::Local);
        c.advance_mtime(&ctx, "pa", 250);
        c.advance_mtime(&ctx, "pa", 800);
        c.advance_mtime(&ctx, "pa", 300);
        assert_eq!(c.lookup_mtime_overlay(&ctx, "pa"), Some(800));
        let hit = c.lookup(&ctx, 1, 2).expect("entry 仍命中");
        assert_eq!(
            hit[0].most_recent_session,
            Some(100),
            "snapshot 主体 SHALL NOT 被改写——合成发生在合成路径，不是 cache 内"
        );
    }

    /// Scenario `cache 重扫合并保留较大 hint`：snapshot < hint 时 SHALL 保留
    /// hint，让 scan 期间 append 不被回退。
    #[test]
    fn rescan_keeps_larger_overlay() {
        let mut c = ProjectScanCache::new();
        let ctx = local_ctx();
        c.advance_mtime(&ctx, "pa", 999);
        let recorded = c.begin_scan();
        let new_snap = snapshot_with(vec![proj_with_mtime("pa", &["sa"], Some(500))]);
        let inserted =
            c.finish_scan_with_insert(ctx.clone(), new_snap, 1, 2, FsKind::Local, recorded);
        assert!(inserted);
        assert_eq!(
            c.lookup_mtime_overlay(&ctx, "pa"),
            Some(999),
            "snapshot=500 < hint=999 → 保留 hint"
        );
    }

    /// Scenario `cache 重扫清除已被覆盖的旧 hint`：snapshot ≥ hint 时
    /// SHALL 移除该 hint，避免冗余读取。
    #[test]
    fn rescan_drops_overlay_when_snapshot_caught_up() {
        let mut c = ProjectScanCache::new();
        let ctx = local_ctx();
        c.advance_mtime(&ctx, "pa", 500);
        let recorded = c.begin_scan();
        let new_snap = snapshot_with(vec![proj_with_mtime("pa", &["sa"], Some(800))]);
        let inserted =
            c.finish_scan_with_insert(ctx.clone(), new_snap, 1, 2, FsKind::Local, recorded);
        assert!(inserted);
        assert_eq!(
            c.lookup_mtime_overlay(&ctx, "pa"),
            None,
            "snapshot=800 ≥ hint=500 → 清掉 hint"
        );
    }

    /// Scenario `三档 invalidate 不清 hint`：watcher 触发的三档 invalidate
    /// SHALL 清 entries 但 SHALL NOT 清 overlay。
    #[test]
    fn invalidate_local_keeps_overlay() {
        let mut c = ProjectScanCache::new();
        let ctx = local_ctx();
        c.insert(
            ctx.clone(),
            snapshot_with(vec![proj_with_mtime("pa", &["sa"], Some(100))]),
            1,
            2,
            FsKind::Local,
        );
        c.advance_mtime(&ctx, "pa", 999);
        c.invalidate_local();
        assert!(
            c.lookup(&ctx, 1, 2).is_none(),
            "三档 invalidate SHALL 清 Local entry"
        );
        assert_eq!(
            c.lookup_mtime_overlay(&ctx, "pa"),
            Some(999),
            "三档 invalidate SHALL NOT 清 hint"
        );
    }

    /// Scenario `显式 invalidate 总清同时清 hint`：`invalidate_all()` 公开入口
    /// SHALL 同时清空 entries + overlay（覆盖所有 backend kind 与所有 context）。
    #[test]
    fn invalidate_all_clears_overlay_too() {
        let mut c = ProjectScanCache::new();
        c.insert(
            local_ctx(),
            snapshot_with(vec![proj_with_mtime("pa", &["sa"], Some(100))]),
            1,
            2,
            FsKind::Local,
        );
        c.insert(
            ssh_ctx(),
            snapshot_with(vec![proj_with_mtime("pb", &["sb"], Some(200))]),
            1,
            2,
            FsKind::Ssh,
        );
        c.advance_mtime(&local_ctx(), "pa", 500);
        c.advance_mtime(&ssh_ctx(), "pb", 600);
        c.invalidate_all();
        assert!(c.is_empty());
        assert_eq!(c.lookup_mtime_overlay(&local_ctx(), "pa"), None);
        assert_eq!(c.lookup_mtime_overlay(&ssh_ctx(), "pb"), None);
    }

    /// Scenario `SSH event 推进对应 SSH context hint 但不影响 Local invalidate`：
    /// SSH event 写 SSH context overlay，SHALL NOT 推进 Local context overlay，
    /// SHALL NOT 触发 Local cache 三档 invalidate（invalidator 上层守护）。
    #[test]
    fn ssh_event_advances_ssh_overlay_only() {
        let cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));
        // Local entry 存在用来证明 SSH event 不动 Local hint
        {
            let mut c = cache.lock().unwrap();
            c.insert(
                local_ctx(),
                snapshot_with(vec![proj_with_mtime("pa", &["sa"], Some(100))]),
                1,
                2,
                FsKind::Local,
            );
            c.advance_mtime(&local_ctx(), "pa", 100);
        }
        let event = fc("pa", "sa", false, false, false, Some(700));
        // 模拟 invalidator 路径：SSH event 走 SSH ctx，跳过 Local 三档判定
        apply_mtime_advance_to_project_scan_cache(&cache, &ssh_ctx(), &event);
        let c = cache.lock().unwrap();
        assert_eq!(
            c.lookup_mtime_overlay(&ssh_ctx(), "pa"),
            Some(700),
            "SSH context 下 hint SHALL 被推进"
        );
        assert_eq!(
            c.lookup_mtime_overlay(&local_ctx(), "pa"),
            Some(100),
            "Local context 下 hint SHALL NOT 被 SSH event 推进"
        );
    }

    /// Scenario `缺 mtimeMs 字段的 file-change event 不推进 hint`：`mtime_ms=None`
    /// 事件 SHALL NOT 写 hint，但仍按其他字段走三档判定。
    #[test]
    fn event_without_mtime_does_not_advance_hint() {
        let cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));
        let ctx = local_ctx();
        {
            let mut c = cache.lock().unwrap();
            c.insert(
                ctx.clone(),
                snapshot_with(vec![proj_with_mtime("pa", &["sa"], Some(100))]),
                1,
                2,
                FsKind::Local,
            );
        }
        let event = fc("pa", "sa", false, false, false, None);
        let decision = apply_file_event_to_project_scan_cache(&cache, &ctx, &event);
        apply_mtime_advance_to_project_scan_cache(&cache, &ctx, &event);
        assert!(!decision.invalidated, "已知 session append 仍走 no-op 档");
        let c = cache.lock().unwrap();
        assert_eq!(
            c.lookup_mtime_overlay(&ctx, "pa"),
            None,
            "缺 mtime_ms 的 event SHALL NOT 写 hint"
        );
    }

    /// Scenario `cache 空时收到 mtime hint 仍写 hint`：冷启动 cache 空也允许
    /// 提前写入 hint，让后续 scan 完成 populate 时合并阶段保留可用值。
    #[test]
    fn cold_start_event_writes_hint_even_without_entry() {
        let cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));
        let ctx = local_ctx();
        let event = fc("pa", "sa", false, false, true, Some(123));
        apply_mtime_advance_to_project_scan_cache(&cache, &ctx, &event);
        let c = cache.lock().unwrap();
        assert_eq!(
            c.lookup_mtime_overlay(&ctx, "pa"),
            Some(123),
            "cache 空时 SHALL 仍把 hint 写入 overlay"
        );
    }

    /// `synthesize_projects_with_overlay` 无 hint 时 SHALL 直接返原 Arc 零分配。
    #[test]
    fn synthesize_returns_same_arc_when_no_overlay() {
        let c = ProjectScanCache::new();
        let snap = snapshot_with(vec![proj_with_mtime("pa", &["sa"], Some(100))]);
        let out = synthesize_projects_with_overlay(&c, &local_ctx(), &snap);
        assert!(
            Arc::ptr_eq(&out, &snap),
            "无 hint SHALL 复用原 Arc 不分配新 Vec"
        );
    }

    /// `synthesize_projects_with_overlay` overlay > snapshot 时 SHALL clone Vec
    /// 并把合成 max 注入返回值；原 Arc 不被改写。
    #[test]
    fn synthesize_injects_overlay_when_greater() {
        let mut c = ProjectScanCache::new();
        let ctx = local_ctx();
        let snap = snapshot_with(vec![
            proj_with_mtime("pa", &["sa"], Some(100)),
            proj_with_mtime("pb", &["sb"], Some(500)),
        ]);
        c.advance_mtime(&ctx, "pa", 999); // pa: snapshot=100, overlay=999 → 应合成 999
        c.advance_mtime(&ctx, "pb", 200); // pb: snapshot=500, overlay=200 → 仍 500
        let out = synthesize_projects_with_overlay(&c, &ctx, &snap);
        assert!(
            !Arc::ptr_eq(&out, &snap),
            "至少一个 hint > snapshot SHALL clone 新 Vec"
        );
        assert_eq!(out[0].id, "pa");
        assert_eq!(out[0].most_recent_session, Some(999));
        assert_eq!(out[1].id, "pb");
        assert_eq!(
            out[1].most_recent_session,
            Some(500),
            "overlay < snapshot 时取 snapshot"
        );
        // 原 Arc 主体不被改写
        assert_eq!(snap[0].most_recent_session, Some(100));
    }

    /// `synthesize_projects_with_overlay` 所有 hint 都 ≤ snapshot 时 SHALL
    /// 仍返原 Arc 零分配（即使 overlay 有条目）。
    #[test]
    fn synthesize_returns_same_arc_when_all_hints_le_snapshot() {
        let mut c = ProjectScanCache::new();
        let ctx = local_ctx();
        c.advance_mtime(&ctx, "pa", 50);
        let snap = snapshot_with(vec![proj_with_mtime("pa", &["sa"], Some(100))]);
        let out = synthesize_projects_with_overlay(&c, &ctx, &snap);
        assert!(
            Arc::ptr_eq(&out, &snap),
            "overlay 50 ≤ snapshot 100 → SHALL 不分配新 Vec"
        );
    }

    /// Scenario `cache 重扫不再含某 project 时清掉对应 hint`：fresh snapshot
    /// 不再含某 project（用户已删 encoded 目录）SHALL 移除该 project 的 hint，
    /// 同 ctx 下其他 project 的 hint 按合并规则处理。
    #[test]
    fn rescan_drops_hint_for_project_no_longer_in_snapshot() {
        let mut c = ProjectScanCache::new();
        let ctx = local_ctx();
        c.advance_mtime(&ctx, "pa", 500);
        c.advance_mtime(&ctx, "pb", 700);
        let recorded = c.begin_scan();
        // fresh snapshot 不含 pa（已被删），含 pb 但 mtime 还小于 hint
        let new_snap = snapshot_with(vec![proj_with_mtime("pb", &["sb"], Some(300))]);
        let inserted =
            c.finish_scan_with_insert(ctx.clone(), new_snap, 1, 2, FsKind::Local, recorded);
        assert!(inserted);
        assert_eq!(
            c.lookup_mtime_overlay(&ctx, "pa"),
            None,
            "fresh snapshot 不含 pa → SHALL 清 pa 的 hint"
        );
        assert_eq!(
            c.lookup_mtime_overlay(&ctx, "pb"),
            Some(700),
            "pb 仍存在且 snapshot < hint → 保留 hint"
        );
    }
}
