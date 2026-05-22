//! `ProjectScanner::scan()` 结果在 `LocalDataApi` 内的进程级缓存。
//!
//! 行为契约（性能优化，不改 IPC 返回字段）：
//! - key = `ContextId`，让 Local / SSH host 间天然隔离；与
//!   `MetadataCache` / `ParsedMessageCache` 同形（fs-abstraction PR-A
//!   `ContextId 三元组作为 cache key 前缀`）。
//! - value = `Arc<Vec<Project>>` + `root_generation` + `context_generation` + `inserted_at`。
//!   命中时直接返回 `Arc`，调用方零分配 iter 即可生成 `ProjectInfo` / `RepositoryGroup` 输入。
//! - 失效层级：
//!   1. **主动 watcher**（Local 唯一）：`spawn_project_scan_cache_invalidator` 订阅 `FileWatcher` 广播，任何 `FileChangeEvent` 都清掉 Local entry — `scan()` 结果是 immutable Arc，partial invalidation 复杂度不划算。
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

    /// `scan_projects_cached()` miss 前 snapshot 当前
    /// `invalidation_generation` 的辅助。`try_insert` 据此判断 scan 期间
    /// 是否被 invalidate 过。
    pub fn invalidation_generation(&self) -> u64 {
        self.invalidation_generation
    }

    /// 无条件写入 / 覆盖 entry（**不**带 race 校验）。仅用于直接构造
    /// cache 的测试场景；生产路径用 [`Self::try_insert`] 防 in-flight
    /// scan race。
    #[cfg(test)]
    fn insert(
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

    /// 条件写入：仅在 `recorded_generation == 当前 invalidation_generation`
    /// 时落 entry；mismatch → 丢弃本次 snapshot，返回 `false`。让
    /// in-flight scan 不覆盖 watcher 在 scan 期间发出的 invalidate 信号
    /// （codex 二审 #2）。
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
    pub fn invalidate_local(&mut self) {
        self.entries
            .retain(|_, entry| !matches!(entry.fs_kind, FsKind::Local));
        self.invalidation_generation = self.invalidation_generation.wrapping_add(1);
    }

    /// 清除所有 entry（Local + SSH）。`reconfigure_claude_root` / SSH
    /// context 切换等显式 hook 用；同步 bump `invalidation_generation`。
    /// 测试也可用本入口让 SSH 路径测试用例之间不串扰。
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
        self.invalidation_generation = self.invalidation_generation.wrapping_add(1);
    }

    /// 单 entry 删除（test / `reconfigure_claude_root` 等显式 hook 用）。
    /// 同步 bump `invalidation_generation`。
    #[allow(dead_code)]
    pub fn invalidate(&mut self, ctx: &ContextId) {
        if self.entries.remove(ctx).is_some() {
            self.invalidation_generation = self.invalidation_generation.wrapping_add(1);
        }
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

/// 启动后台 invalidator —— 订阅 file-watcher 广播，任何 `FileChangeEvent`
/// 触发 `invalidate_local()`。watcher 自身已 debounce（详 `cdt-watch`），
/// 高频写入也只会让 Local entry 在下次 IPC 调用时重扫一次，可接受。
///
/// `Lagged` **同样**触发 `invalidate_local()`——表示已丢失至少一条事件，
/// 不能假设丢的那条不重要（典型：新建 project / 删除 session）。Local TTL
/// 5min 太长不能等，必须主动清掉 entry 让下次 IPC 真扫一次（codex 二审 #1）。
/// `Closed` 时退出 loop。
pub fn spawn_project_scan_cache_invalidator(
    cache: Arc<std::sync::Mutex<ProjectScanCache>>,
    mut rx: broadcast::Receiver<cdt_core::FileChangeEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) = rx.recv().await {
            if let Ok(mut cache) = cache.lock() {
                cache.invalidate_local();
            }
        }
    })
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
        let inserted =
            c.try_insert(local_ctx(), snapshot(), 1, 2, FsKind::Local, recorded);
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
}
