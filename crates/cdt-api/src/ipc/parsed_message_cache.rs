//! parsed-message LRU 缓存：按 `((ContextId, jsonl_path), FileSignature)` 缓存
//! `cdt_parse::parse_file` 结果，让 `get_tool_output` / `get_image_asset`
//! hot path 避免重复 line-by-line 解析整个 JSONL。
//!
//! 行为契约见 `openspec/specs/ipc-data-api/spec.md` §"`get_tool_output` 与
//! `get_image_asset` 走 parsed-message LRU 缓存"。形态与
//! `crates/cdt-api/src/ipc/session_metadata.rs::MetadataCache` 完全对齐
//! （change `metadata-cache-context-prefix` PR-B → 本 change PR-C 同型搬运）：
//! - 同款 `(mtime, size, identity)` 签名 + LRU + 命中 bump 到队首
//! - key 加 `ContextId` 前缀，跨 Local / SSH host 不串扰（详 change
//!   `parsed-message-cache-context-prefix` design D1）
//! - stat 路径走 `FileSystemProvider::stat`（design D7），SSH callsite
//!   暂不接入 cache（design D6，留 PR-D）
//!
//! 容量上限 50（vs metadata 2000）—— 单 entry 是 `Arc<Vec<ParsedMessage>>`，
//! 量级千倍以上；详 change `parsed-message-cache-context-prefix` design D3。

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use cdt_core::ParsedMessage;
use cdt_fs::{ContextId, FileSystemProvider};
use cdt_parse::parse_file_via_fs;

use crate::cache_signature::FileSignature;

/// 缓存容量上限。详 change `parsed-message-cache-context-prefix` design D3
/// （单 entry 大、count 少；保持原 50 不动避免触碰内存峰值）。
pub const PARSED_MESSAGE_CACHE_CAPACITY: usize = 50;

#[derive(Debug, Clone)]
struct ParsedMessageEntry {
    signature: FileSignature,
    messages: Arc<Vec<ParsedMessage>>,
}

/// cache key 形态：`(ContextId, PathBuf)` tuple —— 与 `MetadataCache` 同形
/// （PR-A spec `fs-abstraction::ContextId 三元组作为 cache key 前缀` SHALL 句 +
/// change `parsed-message-cache-context-prefix` design D1）。Local vs SSH 或
/// 不同 SSH host 间天然由 `ContextId` 的 `Hash`/`Eq` 隔离，不串扰。
type ParsedMessageCacheKey = (ContextId, PathBuf);

/// `LocalDataApi` 持有的 parsed-message LRU 缓存（**不**用全局单例，详 change
/// `parsed-message-lru-cache` design D3）。key 加 `ContextId` 前缀
/// （change `parsed-message-cache-context-prefix` PR-C）。
#[derive(Debug)]
pub struct ParsedMessageCache {
    map: HashMap<ParsedMessageCacheKey, ParsedMessageEntry>,
    order: VecDeque<ParsedMessageCacheKey>,
    capacity: usize,
}

impl Default for ParsedMessageCache {
    fn default() -> Self {
        Self::new(PARSED_MESSAGE_CACHE_CAPACITY)
    }
}

impl ParsedMessageCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    fn lookup(&mut self, ctx: &ContextId, path: &Path) -> Option<ParsedMessageEntry> {
        // HashMap key 是 owned `(ContextId, PathBuf)` tuple，无法用 `(&ContextId, &Path)`
        // 直接 get；克隆 key 用于 lookup 是常规模式（ContextId ~300 bytes + PathBuf
        // ~120 bytes 短暂分配，相对 cache hit 后 Arc::clone 完整 messages 的几 µs
        // 量级可忽略）。
        let key = (ctx.clone(), path.to_path_buf());
        let entry = self.map.get(&key)?.clone();
        if let Some(pos) = self.order.iter().position(|k| k == &key) {
            let k = self.order.remove(pos).expect("position 已校验");
            self.order.push_front(k);
        }
        Some(entry)
    }

    /// 用调用方提供的 `FileSignature` 直接查 cache —— 跳过内部 stat。
    ///
    /// 用于 list 后台 batch 校验路径：调用方先 `fs.read_dir_with_metadata(parent)`
    /// 一次拿全 dir 内 entry 的 metadata，再批量 lookup，避免 N 次串行 stat
    /// （详 change `unify-fs-direct-calls` design D3）。
    #[allow(dead_code)]
    pub(crate) fn lookup_with_known_signature(
        &mut self,
        ctx: &ContextId,
        path: &Path,
        signature: &FileSignature,
    ) -> Option<Arc<Vec<ParsedMessage>>> {
        let key = (ctx.clone(), path.to_path_buf());
        let entry = self.map.get(&key)?.clone();
        if entry.signature != *signature {
            return None;
        }
        if let Some(pos) = self.order.iter().position(|k| k == &key) {
            let k = self.order.remove(pos).expect("position 已校验");
            self.order.push_front(k);
        }
        Some(entry.messages)
    }

    /// hot path cache hit trust —— 不校验 signature 直接返当前 entry。
    /// signature 校验由后台 batch task 异步跑（详 change `unify-fs-direct-calls` design D3）。
    #[allow(dead_code)]
    pub(crate) fn lookup_trust_cached(
        &mut self,
        ctx: &ContextId,
        path: &Path,
    ) -> Option<Arc<Vec<ParsedMessage>>> {
        let key = (ctx.clone(), path.to_path_buf());
        let entry = self.map.get(&key)?.clone();
        if let Some(pos) = self.order.iter().position(|k| k == &key) {
            let k = self.order.remove(pos).expect("position 已校验");
            self.order.push_front(k);
        }
        Some(entry.messages)
    }

    fn insert(&mut self, key: ParsedMessageCacheKey, entry: ParsedMessageEntry) {
        if self.map.contains_key(&key) {
            self.map.insert(key.clone(), entry);
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                let k = self.order.remove(pos).expect("position 已校验");
                self.order.push_front(k);
            }
            return;
        }

        if self.map.len() >= self.capacity {
            if let Some(evicted) = self.order.pop_back() {
                self.map.remove(&evicted);
            }
        }

        self.map.insert(key.clone(), entry);
        self.order.push_front(key);
    }

    /// 主动从缓存移除 `(ctx, path)` 条目。不在 cache 中时 no-op。
    pub fn remove(&mut self, ctx: &ContextId, path: &Path) {
        let key = (ctx.clone(), path.to_path_buf());
        if self.map.remove(&key).is_some() {
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                let _ = self.order.remove(pos);
            }
        }
    }

    /// 仅在 cache 中 `(ctx, path)` 条目的 `FileSignature` 与 `current_sig`
    /// 不一致时才 remove。用于 file-watcher 广播 invalidate 路径——避免 spurious
    /// 事件（如 CI 上 inotify 启动期对刚创建的 watch dir 偶发的"无内容变化"事件、
    /// metadata-only touch 等）错杀有效 cache。返回是否真的发生了 remove。
    pub fn remove_if_signature_mismatch(
        &mut self,
        ctx: &ContextId,
        path: &Path,
        current_sig: &FileSignature,
    ) -> bool {
        let key = (ctx.clone(), path.to_path_buf());
        let Some(entry) = self.map.get(&key) else {
            return false;
        };
        if entry.signature == *current_sig {
            return false;
        }
        self.remove(ctx, path);
        true
    }

    /// 当前缓存条目数。`LocalDataApi::parsed_msg_cache_len`（仅
    /// `test-utils` feature 下编译）会用到；默认构建下没人调，加 `allow`。
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// `len() == 0`，clippy `len_zero` 要求 `len` 配对暴露。
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// 缓存 wrapper：先查 cache + 校验 `FileSignature`；命中返回 `Some(Arc)`，miss
/// 调 `parse_file(path)` 写入缓存。
///
/// 返回 `None` 的两类场景（caller 走原有错误兜底）：
/// - `fs.stat(path)` 失败：SHALL NOT 写入 cache
/// - `parse_file(path)` 返回 `Err`：SHALL NOT 写入 cache（避免 negative cache
///   引入新失效边界，详 change `parsed-message-lru-cache` design D6）
///
/// stat 路径走 `FileSystemProvider::stat`（详 change
/// `parsed-message-cache-context-prefix` design D6 / D7）；cache miss 后的扫描
/// 路径仍是 `cdt_parse::parse_file`（内部 `tokio::fs::File::open`），本 change
/// scope 边界——SSH callsite 当前不经过此 wrapper（design D6），完整 SSH 接入 +
/// `parse_file` 切 `fs.open_read` 留 PR-D。
///
/// 行为契约：`openspec/specs/ipc-data-api/spec.md` §"`get_tool_output` 与
/// `get_image_asset` 走 parsed-message LRU 缓存"。
pub(crate) async fn extract_parsed_messages_cached(
    cache: &StdMutex<ParsedMessageCache>,
    fs: &dyn FileSystemProvider,
    context_id: &ContextId,
    path: &Path,
) -> Option<Arc<Vec<ParsedMessage>>> {
    let meta = fs.stat(path).await.ok()?;
    let sig = FileSignature::from_fs_metadata(&meta);

    {
        let cached = cache
            .lock()
            .expect("parsed message cache mutex poisoned")
            .lookup(context_id, path);
        if let Some(entry) = cached {
            if entry.signature == sig {
                return Some(entry.messages);
            }
        }
    }

    let messages = match parse_file_via_fs(fs, path).await {
        Ok(m) => Arc::new(m),
        Err(e) => {
            tracing::warn!(
                target: "cdt_api::parsed_message_cache",
                path = %path.display(),
                error = %e,
                "parse_file_via_fs failed; SHALL NOT write to cache"
            );
            return None;
        }
    };

    cache
        .lock()
        .expect("parsed message cache mutex poisoned")
        .insert(
            (context_id.clone(), path.to_path_buf()),
            ParsedMessageEntry {
                signature: sig,
                messages: messages.clone(),
            },
        );

    Some(messages)
}

/// 单条 `FileChangeEvent` 应用到 parsed-message cache 的逻辑：推算
/// `<projects_dir>/<project_id>/<session_id>.jsonl` 路径，stat 拿当前
/// `FileSignature`，与 cache 内记录比对；mismatch 才 remove。stat 失败走保守
/// remove。
///
/// 设计为可被统一合并 invalidator 复用（issue #261）。`cache.lock()` 走
/// `into_inner` 兜底以避免 poison panic 拖垮统一 task（codex 二审 #261 panic
/// 隔离 hardening）—— 与 `project_scan_cache.rs` 的 invalidator lock 风格对齐。
///
/// 行为契约：spec `ipc-data-api/spec.md` §"parsed-message 缓存按 file-change
/// 广播主动失效"。
pub(crate) async fn apply_file_event_to_parsed_cache(
    cache: &StdMutex<ParsedMessageCache>,
    fs: &dyn FileSystemProvider,
    ctx: &ContextId,
    projects_dir: &Path,
    event: &cdt_core::FileChangeEvent,
) {
    if event.session_id.is_empty() {
        return;
    }
    let path = projects_dir
        .join(&event.project_id)
        .join(format!("{}.jsonl", event.session_id));
    // SHALL NOT 持 sync mutex guard 跨 `await`（tokio async + std Mutex 反模式）。
    // 先 await stat 拿结果，再单独拿 lock 做最短临界区。
    let stat_result = fs.stat(&path).await;
    let mut guard = match cache.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Ok(meta) = stat_result {
        let current_sig = FileSignature::from_fs_metadata(&meta);
        guard.remove_if_signature_mismatch(ctx, &path, &current_sig);
    } else {
        // 文件已删 / 权限失败：保守 remove，下次 lookup 也会 stat fail，
        // 提前清掉不影响正确性（与原 spawn_parsed_msg_cache_invalidator 一致）。
        guard.remove(ctx, &path);
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::cache_signature::{FileIdentity, FileSignature};

    fn local_ctx() -> ContextId {
        ContextId::local(PathBuf::from("/test/local"))
    }

    fn ssh_ctx() -> ContextId {
        // 用一个稳定 mock host_signature；不同 ssh_ctx() 调用返回同 digest，
        // 单测内的"SSH ctx"语义稳定。
        use cdt_fs::{HostSignature, SshConfigDigestInput};
        let input = SshConfigDigestInput {
            hostname: "fake-host".to_string(),
            port: 22,
            user: "user".to_string(),
            identity_files: vec![],
            proxyjump: None,
            proxycommand: None,
            hostkeyalias: None,
        };
        let sig = HostSignature::from_ssh_config_fields(&input);
        ContextId::ssh(sig, PathBuf::from("/remote/home"))
    }

    fn dummy_entry(size: u64) -> ParsedMessageEntry {
        ParsedMessageEntry {
            signature: FileSignature {
                mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(size),
                size,
                #[cfg(unix)]
                identity: FileIdentity::Unix { dev: 1, ino: size },
                #[cfg(not(unix))]
                identity: FileIdentity::None,
            },
            messages: Arc::new(Vec::new()),
        }
    }

    #[test]
    fn parsed_cache_evicts_lru_when_over_capacity() {
        let ctx = local_ctx();
        let mut cache = ParsedMessageCache::new(2);
        cache.insert((ctx.clone(), PathBuf::from("/a")), dummy_entry(1));
        cache.insert((ctx.clone(), PathBuf::from("/b")), dummy_entry(2));
        cache.insert((ctx.clone(), PathBuf::from("/c")), dummy_entry(3));
        assert!(cache.lookup(&ctx, Path::new("/a")).is_none(), "/a 应被淘汰");
        assert!(cache.lookup(&ctx, Path::new("/b")).is_some());
        assert!(cache.lookup(&ctx, Path::new("/c")).is_some());
        assert!(cache.len() <= 2);
    }

    #[test]
    fn parsed_cache_lookup_bumps_hit_to_front() {
        let ctx = local_ctx();
        let mut cache = ParsedMessageCache::new(2);
        cache.insert((ctx.clone(), PathBuf::from("/a")), dummy_entry(1));
        cache.insert((ctx.clone(), PathBuf::from("/b")), dummy_entry(2));
        assert!(cache.lookup(&ctx, Path::new("/a")).is_some());
        cache.insert((ctx.clone(), PathBuf::from("/c")), dummy_entry(3));
        assert!(
            cache.lookup(&ctx, Path::new("/a")).is_some(),
            "命中后 bump 队首，不应被淘汰"
        );
        assert!(cache.lookup(&ctx, Path::new("/b")).is_none(), "/b 应被淘汰");
    }

    #[test]
    fn parsed_cache_remove_drops_entry() {
        let ctx = local_ctx();
        let mut cache = ParsedMessageCache::new(2);
        cache.insert((ctx.clone(), PathBuf::from("/a")), dummy_entry(1));
        cache.insert((ctx.clone(), PathBuf::from("/b")), dummy_entry(2));
        cache.remove(&ctx, Path::new("/a"));
        assert!(cache.lookup(&ctx, Path::new("/a")).is_none());
        assert!(cache.lookup(&ctx, Path::new("/b")).is_some());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn parsed_cache_remove_noop_when_absent() {
        let ctx = local_ctx();
        let mut cache = ParsedMessageCache::new(2);
        cache.insert((ctx.clone(), PathBuf::from("/a")), dummy_entry(1));
        cache.remove(&ctx, Path::new("/nonexistent"));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn remove_if_signature_mismatch_keeps_entry_when_sig_matches() {
        // spurious watcher event 场景：cache 里的 signature 与当前文件 stat
        // 完全一致 → 不应 remove。
        let ctx = local_ctx();
        let mut cache = ParsedMessageCache::new(2);
        let entry = dummy_entry(7);
        let same_sig = entry.signature;
        cache.insert((ctx.clone(), PathBuf::from("/x")), entry);
        let removed = cache.remove_if_signature_mismatch(&ctx, Path::new("/x"), &same_sig);
        assert!(!removed, "signature 一致时不应 remove");
        assert!(cache.lookup(&ctx, Path::new("/x")).is_some());
    }

    #[test]
    fn remove_if_signature_mismatch_removes_when_sig_changes() {
        let ctx = local_ctx();
        let mut cache = ParsedMessageCache::new(2);
        cache.insert((ctx.clone(), PathBuf::from("/x")), dummy_entry(1));
        let new_sig = FileSignature {
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(999),
            size: 999,
            #[cfg(unix)]
            identity: FileIdentity::Unix { dev: 2, ino: 999 },
            #[cfg(not(unix))]
            identity: FileIdentity::None,
        };
        let removed = cache.remove_if_signature_mismatch(&ctx, Path::new("/x"), &new_sig);
        assert!(removed, "signature 不一致时应 remove");
        assert!(cache.lookup(&ctx, Path::new("/x")).is_none());
    }

    #[test]
    fn remove_if_signature_mismatch_noop_when_absent() {
        let ctx = local_ctx();
        let mut cache = ParsedMessageCache::new(2);
        let any_sig = dummy_entry(5).signature;
        let removed = cache.remove_if_signature_mismatch(&ctx, Path::new("/missing"), &any_sig);
        assert!(!removed);
        assert_eq!(cache.len(), 0);
    }

    // ----- 新增 4 个 ContextId 隔离 / 混合 LRU / 切换不清 / per-ctx 失效 -----

    #[test]
    fn parsed_local_vs_ssh_keys_do_not_collide() {
        // 同字面 path 在 Local ctx 与 SSH ctx 下应有两个独立 entry，互不串扰。
        let mut cache = ParsedMessageCache::new(4);
        let local = local_ctx();
        let ssh = ssh_ctx();
        let path = PathBuf::from("/shared/path.jsonl");

        let local_entry = dummy_entry(1);
        let ssh_entry = dummy_entry(2);
        cache.insert((local.clone(), path.clone()), local_entry.clone());
        cache.insert((ssh.clone(), path.clone()), ssh_entry.clone());
        assert_eq!(cache.len(), 2);

        let got_local = cache.lookup(&local, &path).expect("Local entry 应命中");
        assert_eq!(got_local.signature, local_entry.signature);

        let got_ssh = cache.lookup(&ssh, &path).expect("SSH entry 应命中");
        assert_eq!(got_ssh.signature, ssh_entry.signature);
        assert_ne!(
            got_local.signature, got_ssh.signature,
            "Local 与 SSH 同字面 path 应是独立 entry"
        );
    }

    #[test]
    fn parsed_lru_evicts_with_mixed_context() {
        // 跨 Local + SSH 混合插入 51 个 entry，验证容量上限 50 + 最早 entry 被淘汰。
        let mut cache = ParsedMessageCache::new(50);
        let local = local_ctx();
        let ssh = ssh_ctx();

        // 0..25 用 Local，25..50 用 SSH，50 触发淘汰
        for i in 0..25_u64 {
            cache.insert(
                (local.clone(), PathBuf::from(format!("/l/{i}"))),
                dummy_entry(i),
            );
        }
        for i in 0..25_u64 {
            cache.insert(
                (ssh.clone(), PathBuf::from(format!("/s/{i}"))),
                dummy_entry(100 + i),
            );
        }
        assert_eq!(cache.len(), 50);

        // 插入第 51 个 → 最早的 (Local, /l/0) 应被淘汰
        cache.insert(
            (local.clone(), PathBuf::from("/l/extra")),
            dummy_entry(9999),
        );
        assert_eq!(cache.len(), 50, "总容量 SHALL ≤ 50");
        assert!(
            cache.lookup(&local, Path::new("/l/0")).is_none(),
            "最早插入的 (Local, /l/0) 应被淘汰"
        );
        assert!(
            cache.lookup(&local, Path::new("/l/extra")).is_some(),
            "新插入的 entry 应在 cache 内"
        );
    }

    #[test]
    fn parsed_switch_context_does_not_clear_cache() {
        // 模拟 Local → SSH context 切换（直接构造 SSH ctx 查询）—— Local entry SHALL
        // 保留在 cache 内。本 change scope 内 SSH callsite 不写入 cache，但
        // ParsedMessageCache 公开 API 应支持"两 ctx 共存"形态。
        let mut cache = ParsedMessageCache::new(4);
        let local = local_ctx();
        let ssh = ssh_ctx();
        cache.insert((local.clone(), PathBuf::from("/a")), dummy_entry(1));

        // 切到 SSH ctx 查询 —— Local entry 自然不命中（key 不等）
        assert!(
            cache.lookup(&ssh, Path::new("/a")).is_none(),
            "SSH ctx 查询 SHALL NOT 命中 Local entry"
        );
        // 切回 Local ctx 查询 —— Local entry 仍在
        assert!(
            cache.lookup(&local, Path::new("/a")).is_some(),
            "切回 Local 后原 entry 仍在 cache"
        );
        assert_eq!(cache.len(), 1, "未发生主动清空");
    }

    #[test]
    fn parsed_remove_if_signature_mismatch_per_ctx() {
        // 同 path 写入 Local + SSH 两个 entry，对 Local ctx 触发 signature mismatch
        // remove，SSH entry SHALL 保留不受影响。
        let mut cache = ParsedMessageCache::new(4);
        let local = local_ctx();
        let ssh = ssh_ctx();
        let path = PathBuf::from("/shared.jsonl");

        cache.insert((local.clone(), path.clone()), dummy_entry(1));
        cache.insert((ssh.clone(), path.clone()), dummy_entry(2));
        assert_eq!(cache.len(), 2);

        let new_sig = FileSignature {
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(9999),
            size: 9999,
            #[cfg(unix)]
            identity: FileIdentity::Unix { dev: 7, ino: 9999 },
            #[cfg(not(unix))]
            identity: FileIdentity::None,
        };
        let removed = cache.remove_if_signature_mismatch(&local, &path, &new_sig);
        assert!(removed, "Local ctx signature mismatch 应触发 remove");
        assert!(
            cache.lookup(&local, &path).is_none(),
            "Local entry 应被 remove"
        );
        assert!(
            cache.lookup(&ssh, &path).is_some(),
            "SSH entry SHALL NOT 受 Local ctx remove 影响"
        );
        assert_eq!(cache.len(), 1);
    }

    // ----- extract_parsed_messages_cached wrapper 集成测试 -----

    fn write_jsonl(dir: &Path, lines: &[&str]) -> PathBuf {
        let path = dir.join("session.jsonl");
        std::fs::write(&path, lines.join("\n")).unwrap();
        path
    }

    fn user_text_line(uuid: &str, ts: &str, text: &str) -> String {
        let escaped = text.replace('"', "\\\"");
        format!(
            r#"{{"type":"user","uuid":"{uuid}","timestamp":"{ts}","sessionId":"sid","cwd":"/tmp","message":{{"role":"user","content":"{escaped}"}}}}"#
        )
    }

    fn make_cache() -> StdMutex<ParsedMessageCache> {
        StdMutex::new(ParsedMessageCache::default())
    }

    /// 测试用 `ContextId` —— 以 tempdir 路径作 Local root，与真磁盘 fixture 对齐。
    fn ctx_for(tmp: &tempfile::TempDir) -> ContextId {
        ContextId::local(tmp.path().to_path_buf())
    }

    #[tokio::test]
    async fn cached_hit_returns_arc_without_rereading() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_text_line("u1", "2026-05-03T10:00:00.000Z", "hi"),
                &user_text_line("u2", "2026-05-03T10:00:01.000Z", "more"),
            ],
        );

        let cache = make_cache();
        let fs = cdt_fs::local_handle();
        let ctx = ctx_for(&tmp);

        let a = extract_parsed_messages_cached(&cache, &*fs, &ctx, &path)
            .await
            .expect("first parse should succeed");
        assert_eq!(a.len(), 2);
        assert_eq!(cache.lock().unwrap().len(), 1);

        let b = extract_parsed_messages_cached(&cache, &*fs, &ctx, &path)
            .await
            .expect("second lookup should hit cache");
        assert!(
            Arc::ptr_eq(&a, &b),
            "命中缓存时 SHALL 返回同一 Arc，引用计数 ≥ 2"
        );
        assert_eq!(Arc::strong_count(&a), 3, "a + b + cache 内部各持一份");
    }

    #[tokio::test]
    async fn cached_miss_when_file_size_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line("u1", "2026-05-03T10:00:00.000Z", "first")],
        );

        let cache = make_cache();
        let fs = cdt_fs::local_handle();
        let ctx = ctx_for(&tmp);
        let m1 = extract_parsed_messages_cached(&cache, &*fs, &ctx, &path)
            .await
            .unwrap();
        assert_eq!(m1.len(), 1);

        tokio::time::sleep(Duration::from_millis(1100)).await;
        std::fs::write(
            &path,
            format!(
                "{}\n{}\n",
                user_text_line("u1", "2026-05-03T10:00:00.000Z", "first"),
                user_text_line("u2", "2026-05-03T10:00:01.000Z", "second"),
            ),
        )
        .unwrap();

        let m2 = extract_parsed_messages_cached(&cache, &*fs, &ctx, &path)
            .await
            .unwrap();
        assert_eq!(m2.len(), 2, "size 变化后 SHALL 走 cache miss + 重 parse");
        assert!(!Arc::ptr_eq(&m1, &m2), "miss 时 SHALL 是新 Arc");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cached_miss_when_inode_changes_via_rename() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line("u1", "2026-05-03T10:00:00.000Z", "first")],
        );

        let cache = make_cache();
        let fs = cdt_fs::local_handle();
        let ctx = ctx_for(&tmp);
        let m1 = extract_parsed_messages_cached(&cache, &*fs, &ctx, &path)
            .await
            .unwrap();
        assert_eq!(m1.len(), 1);

        let replacement = tmp.path().join("replace.jsonl");
        std::fs::write(
            &replacement,
            format!(
                "{}\n{}\n",
                user_text_line("u9", "2026-05-03T10:00:00.000Z", "renamed"),
                user_text_line("u10", "2026-05-03T10:00:01.000Z", "again"),
            ),
        )
        .unwrap();
        std::fs::rename(&replacement, &path).unwrap();

        let m2 = extract_parsed_messages_cached(&cache, &*fs, &ctx, &path)
            .await
            .unwrap();
        assert_eq!(
            m2.len(),
            2,
            "rename 替换（inode 变化）SHALL 走 cache miss + 重 parse"
        );
    }

    #[tokio::test]
    async fn cached_stat_failure_returns_none_no_write() {
        let cache = make_cache();
        let fs = cdt_fs::local_handle();
        let ctx = ContextId::local(PathBuf::from("/nonexistent/root"));
        let result = extract_parsed_messages_cached(
            &cache,
            &*fs,
            &ctx,
            Path::new("/nonexistent/path.jsonl"),
        )
        .await;
        assert!(result.is_none(), "stat 失败 SHALL 返回 None");
        assert_eq!(cache.lock().unwrap().len(), 0, "SHALL NOT 写入缓存");
    }

    #[tokio::test]
    async fn empty_file_is_cached_as_valid_empty_result() {
        // parse_file 对空文件返回 Ok(Vec::new())（非 Err），属于合法结果 SHALL 写入缓存
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("empty.jsonl");
        std::fs::write(&path, "").unwrap();

        let cache = make_cache();
        let fs = cdt_fs::local_handle();
        let ctx = ctx_for(&tmp);
        let result = extract_parsed_messages_cached(&cache, &*fs, &ctx, &path)
            .await
            .unwrap();
        assert!(result.is_empty());
        assert_eq!(cache.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn cached_uses_fs_stat_not_tokio_fs_metadata() {
        // 用 InstrumentedFs 包装 LocalFileSystemProvider，验证 wrapper 走 fs.stat
        // 而非 tokio::fs::metadata —— stat 计数应在 miss + hit 路径各 +1。
        use cdt_fs::{InstrumentedFs, LocalFileSystemProvider, with_fs_counter};

        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line("u1", "2026-05-03T10:00:00.000Z", "ping")],
        );
        let cache = std::sync::Arc::new(make_cache());
        let ctx = ctx_for(&tmp);
        let fs = std::sync::Arc::new(InstrumentedFs::new(LocalFileSystemProvider::new()));

        let cache_for_miss = cache.clone();
        let fs_for_miss = fs.clone();
        let ctx_for_miss = ctx.clone();
        let path_for_miss = path.clone();
        let ((), counters_miss) = with_fs_counter(move || async move {
            let _ = extract_parsed_messages_cached(
                &cache_for_miss,
                &*fs_for_miss,
                &ctx_for_miss,
                &path_for_miss,
            )
            .await;
        })
        .await;
        assert!(
            counters_miss.stat >= 1,
            "miss 路径 SHALL 调 fs.stat 至少 1 次（实测 {} 次）",
            counters_miss.stat
        );

        let cache_for_hit = cache.clone();
        let fs_for_hit = fs.clone();
        let ctx_for_hit = ctx.clone();
        let path_for_hit = path.clone();
        let ((), counters_hit) = with_fs_counter(move || async move {
            let _ = extract_parsed_messages_cached(
                &cache_for_hit,
                &*fs_for_hit,
                &ctx_for_hit,
                &path_for_hit,
            )
            .await;
        })
        .await;
        assert_eq!(
            counters_hit.stat, 1,
            "hit 路径 SHALL 仅调 fs.stat 1 次（signature 校验）"
        );
        assert_eq!(
            counters_hit.open_read, 0,
            "hit 路径 SHALL NOT 调 fs.open_read"
        );
        assert_eq!(
            counters_hit.read_to_string, 0,
            "hit 路径 SHALL NOT 调 fs.read_to_string"
        );
    }

    #[tokio::test]
    async fn cached_local_vs_ssh_isolation() {
        // 同 path 在 Local + SSH 两个 ctx 下写入 → 互不串扰。
        // 注：本 change scope 内 SSH callsite 实际不调 wrapper（design D6）；本测试
        // 验证 wrapper 的 ctx 隔离能力本身，为 PR-D 接入做铺路。
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line("u1", "2026-05-03T10:00:00.000Z", "x")],
        );
        let cache = make_cache();
        let fs = cdt_fs::local_handle();
        let local = ctx_for(&tmp);
        let ssh = ssh_ctx();

        // 用 Local ctx 写入 cache
        let a = extract_parsed_messages_cached(&cache, &*fs, &local, &path)
            .await
            .expect("Local 写入");
        // 用 SSH ctx 再写一次 —— 不应命中 Local entry，应作为新 entry 写入
        let b = extract_parsed_messages_cached(&cache, &*fs, &ssh, &path)
            .await
            .expect("SSH 写入");
        assert!(
            !Arc::ptr_eq(&a, &b),
            "Local 与 SSH ctx 写入应是独立 entry（不同 Arc）"
        );
        assert_eq!(cache.lock().unwrap().len(), 2, "cache 应有 2 个独立 entry");
    }

    // ========================================================================
    // counter-based bench：wrapper stat 入口 + hit 路径不重读全文件
    //
    // 详 change `parsed-message-cache-context-prefix` design D6 + codex 二审 Q5：
    // 原计划"fake-SSH fs + miss→hit RTT 节省" bench 设计上行不通——`parse_file`
    // 内部走 `tokio::fs::File::open` **不**经 fs trait，fake_ssh_fs 的 `open_read`
    // 永远不会被 cache wrapper miss 路径调用；等 PR-D 把 `parse_file` 切到
    // `FileSystemProvider::open_read` 之后才能补 fake-SSH RTT 节省 bench。
    //
    // 本 bench 验证 wrapper 自身的 stat 入口已切 fs trait + hit 路径不重读：
    // - 第一轮 miss：counter.stat ≥ N
    // - 第二轮 hit：counter.stat == N（每个 path 仍 stat 校验 signature）+
    //   open_read / read_to_string 均为 0
    //
    // 标 `#[ignore]`——CI 不跑；本地 `cargo test -p cdt-api --lib
    // ipc::parsed_message_cache::tests::parsed_message_cache_stat_counter_hit_miss
    // -- --ignored --nocapture` 验收。
    // ========================================================================
    #[tokio::test]
    #[ignore = "counter-based perf bench；本地手动跑（详方法 doc）"]
    async fn parsed_message_cache_stat_counter_hit_miss() {
        use cdt_fs::{InstrumentedFs, LocalFileSystemProvider, with_fs_counter};

        const N: u32 = 3;
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut paths: Vec<PathBuf> = Vec::with_capacity(N as usize);
        for i in 0..N {
            let p = tmp.path().join(format!("session-{i}.jsonl"));
            let line = user_text_line(
                &format!("u{i}"),
                "2026-05-21T10:00:00.000Z",
                &format!("hi-{i}"),
            );
            std::fs::write(&p, format!("{line}\n")).expect("write fixture");
            paths.push(p);
        }

        let cache = Arc::new(make_cache());
        let ctx = ContextId::local(tmp.path().to_path_buf());
        let fs = Arc::new(InstrumentedFs::new(LocalFileSystemProvider::new()));

        // 第一轮 miss
        let cache_for_miss = cache.clone();
        let fs_for_miss = fs.clone();
        let ctx_for_miss = ctx.clone();
        let paths_for_miss = paths.clone();
        let ((), miss_counts) = with_fs_counter(move || async move {
            for p in &paths_for_miss {
                let messages = extract_parsed_messages_cached(
                    &cache_for_miss,
                    &*fs_for_miss,
                    &ctx_for_miss,
                    p,
                )
                .await
                .expect("miss parse succeeds");
                assert_eq!(messages.len(), 1, "fixture 每个文件 1 条消息");
            }
        })
        .await;
        assert!(
            miss_counts.stat >= N,
            "miss 路径每个 path SHALL 调 fs.stat 至少 1 次（实测 stat={}，N={N}）",
            miss_counts.stat
        );
        assert_eq!(
            cache.lock().unwrap().len(),
            N as usize,
            "miss 后 cache 应有 {N} 个 entry"
        );

        // 第二轮 hit
        let cache_for_hit = cache.clone();
        let fs_for_hit = fs.clone();
        let ctx_for_hit = ctx.clone();
        let paths_for_hit = paths.clone();
        let ((), hit_counts) = with_fs_counter(move || async move {
            for p in &paths_for_hit {
                let _ =
                    extract_parsed_messages_cached(&cache_for_hit, &*fs_for_hit, &ctx_for_hit, p)
                        .await;
            }
        })
        .await;

        eprintln!(
            "[perf_parsed_message_cache_stat_counter] miss={miss_counts:?} hit={hit_counts:?} \
             cache_size={}",
            cache.lock().unwrap().len()
        );

        assert_eq!(
            hit_counts.stat, N,
            "hit 路径每个 path SHALL 仅调 fs.stat 1 次（signature 校验）"
        );
        assert_eq!(
            hit_counts.open_read, 0,
            "hit 路径 SHALL NOT 调 fs.open_read（cache 命中后不重读全文件）"
        );
        assert_eq!(
            hit_counts.read_to_string, 0,
            "hit 路径 SHALL NOT 调 fs.read_to_string"
        );
    }
}
