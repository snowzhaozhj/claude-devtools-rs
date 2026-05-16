//! 基于 mtime 的 LRU 搜索文本缓存。
//!
//! 避免对未变更的 session 文件重复解析 JSONL + 提取可搜索文本。
//!
//! 双闸门：count cap（默认 500）+ byte cap（默认 50 MiB）。任一上限触发就
//! 从 LRU 端驱逐，防止极端场景下 1000 条大 session 把进程内存撑到几百 MB。

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use lru::LruCache;

use crate::search_extract::SearchableEntry;

/// 默认 entry 数量上限。原值 1000 偏大，缩到 500 与 byte cap 形成双闸门——
/// 单条 entry 平均几 KB 时仍够用 50+ project；entry 异常大时由 byte cap 兜底。
const DEFAULT_CAPACITY: usize = 500;

/// 默认 byte 上限：50 MiB。覆盖典型 27 project × 534 session 工作集（实测
/// 一条 `SearchableEntry` 平均 ~2 KB，500 条 = ~1 MB），同时防止超大会话
/// 把单条 entry 顶到几十 MB 时整个 cache 失控。
const DEFAULT_MAX_BYTES: usize = 50 * 1024 * 1024;

/// `String` 在堆上的 metadata 估算（capacity field 等），以及 `SearchableEntry`
/// 自身固定字段开销。准确估算意义不大——byte cap 是粗粒度上限，留 1/8 余量
/// 给 `Vec<SearchableEntry>` 自身分配 + path key 的 `PathBuf` 大小。
const ENTRY_FIXED_OVERHEAD: usize = std::mem::size_of::<SearchableEntry>();
const CACHE_ENTRY_OVERHEAD: usize = std::mem::size_of::<CacheEntry>() + 64;

/// 缓存条目。
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub entries: Vec<SearchableEntry>,
    pub session_title: String,
    pub mtime_ms: u64,
}

/// mtime-based LRU 搜索文本缓存。
pub struct SearchTextCache {
    inner: LruCache<PathBuf, CacheEntry>,
    current_bytes: usize,
    max_bytes: usize,
}

impl Default for SearchTextCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchTextCache {
    /// 创建默认配置（500 条 + 50 MiB）的缓存。
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity_and_max_bytes(DEFAULT_CAPACITY, DEFAULT_MAX_BYTES)
    }

    /// 创建指定 entry 数量上限的缓存（沿用默认 byte cap）。
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self::with_capacity_and_max_bytes(cap, DEFAULT_MAX_BYTES)
    }

    /// 创建指定 entry 数量上限 + byte 上限的缓存。
    #[must_use]
    pub fn with_capacity_and_max_bytes(cap: usize, max_bytes: usize) -> Self {
        let cap = NonZeroUsize::new(cap).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self {
            inner: LruCache::new(cap),
            current_bytes: 0,
            max_bytes,
        }
    }

    /// 当前 byte 使用量（仅计入估算的 entry payload，不计 lru 内部 node 开销）。
    #[must_use]
    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    /// mtime 匹配则返回缓存条目，否则移除过期条目并返回 `None`。
    pub fn get(&mut self, path: &Path, current_mtime_ms: u64) -> Option<&CacheEntry> {
        if let Some(entry) = self.inner.peek(path) {
            if entry.mtime_ms == current_mtime_ms {
                // promote 到 MRU
                return self.inner.get(path);
            }
            // mtime 不匹配，移除并扣减 byte 计数
            if let Some(stale) = self.inner.pop(path) {
                self.current_bytes = self
                    .current_bytes
                    .saturating_sub(estimate_entry_bytes(&stale));
            }
        }
        None
    }

    /// 写入缓存条目。可能触发 LRU 端驱逐——按 entry 数量或 byte 任一上限。
    pub fn put(&mut self, path: PathBuf, entry: CacheEntry) {
        let new_bytes = estimate_entry_bytes(&entry);

        // `push` 在 (a) 同 key 替换 或 (b) 容量超限驱逐 时返回旧条目；这里
        // 两种情况下都需要把旧 byte 计数从 current_bytes 扣掉。
        if let Some((_, old_entry)) = self.inner.push(path, entry) {
            self.current_bytes = self
                .current_bytes
                .saturating_sub(estimate_entry_bytes(&old_entry));
        }
        self.current_bytes = self.current_bytes.saturating_add(new_bytes);

        // 超 byte 上限则继续从 LRU 端驱逐，但保留至少 1 条（即"刚 put 的那条"）。
        // pop_lru 取的是 LRU 侧，新插入的在 MRU 侧，循环条件 `len > 1` 保证不会
        // 把刚插入的回退掉——即便单条就超过 max_bytes，至少 cache 里还留它一份。
        while self.current_bytes > self.max_bytes && self.inner.len() > 1 {
            if let Some((_, evicted)) = self.inner.pop_lru() {
                self.current_bytes = self
                    .current_bytes
                    .saturating_sub(estimate_entry_bytes(&evicted));
            } else {
                break;
            }
        }
    }
}

/// 粗粒度估算单条 `CacheEntry` 占用——只计入 String/Vec 持有的堆上 byte，
/// `LruCache` 自身的 node + Path key 用 `CACHE_ENTRY_OVERHEAD` 作常量补足。
fn estimate_entry_bytes(entry: &CacheEntry) -> usize {
    let mut bytes = CACHE_ENTRY_OVERHEAD;
    bytes = bytes.saturating_add(entry.session_title.capacity());
    for e in &entry.entries {
        bytes = bytes
            .saturating_add(ENTRY_FIXED_OVERHEAD)
            .saturating_add(e.uuid.capacity())
            .saturating_add(e.text.capacity())
            .saturating_add(e.message_type.capacity());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_entry(mtime_ms: u64) -> CacheEntry {
        CacheEntry {
            entries: vec![],
            session_title: "test".into(),
            mtime_ms,
        }
    }

    fn entry_with_text(mtime_ms: u64, text_size: usize) -> CacheEntry {
        CacheEntry {
            entries: vec![SearchableEntry {
                uuid: "u".repeat(36),
                text: "x".repeat(text_size),
                message_type: "user".into(),
            }],
            session_title: "title".into(),
            mtime_ms,
        }
    }

    #[test]
    fn cache_hit_when_mtime_matches() {
        let mut cache = SearchTextCache::new();
        let path = PathBuf::from("/a/b.jsonl");
        cache.put(path.clone(), dummy_entry(100));

        assert!(cache.get(&path, 100).is_some());
    }

    #[test]
    fn cache_miss_when_mtime_changed() {
        let mut cache = SearchTextCache::new();
        let path = PathBuf::from("/a/b.jsonl");
        cache.put(path.clone(), dummy_entry(100));

        assert!(cache.get(&path, 200).is_none());
        // 过期条目已被移除
        assert!(cache.get(&path, 100).is_none());
    }

    #[test]
    fn lru_eviction_on_capacity() {
        let mut cache = SearchTextCache::with_capacity(2);
        let p1 = PathBuf::from("/1.jsonl");
        let p2 = PathBuf::from("/2.jsonl");
        let p3 = PathBuf::from("/3.jsonl");

        cache.put(p1.clone(), dummy_entry(1));
        cache.put(p2.clone(), dummy_entry(2));
        cache.put(p3.clone(), dummy_entry(3));

        // p1 应被驱逐
        assert!(cache.get(&p1, 1).is_none());
        assert!(cache.get(&p2, 2).is_some());
        assert!(cache.get(&p3, 3).is_some());
    }

    #[test]
    fn byte_cap_evicts_oldest_when_exceeded() {
        // 1 KiB byte cap，三条 ~700 byte entry 顺序插入——前两条会被挤掉
        let mut cache = SearchTextCache::with_capacity_and_max_bytes(100, 1_024);
        let p1 = PathBuf::from("/big1.jsonl");
        let p2 = PathBuf::from("/big2.jsonl");
        let p3 = PathBuf::from("/big3.jsonl");

        cache.put(p1.clone(), entry_with_text(1, 700));
        let after_first = cache.current_bytes();
        assert!(
            after_first > 700,
            "首条插入应 >700 byte (含 metadata)，实际 {after_first}"
        );

        cache.put(p2.clone(), entry_with_text(2, 700));
        cache.put(p3.clone(), entry_with_text(3, 700));

        // byte cap = 1024，单条 ~700+ byte，最多容纳 1 条 → 后插入的胜出，前两条挤掉
        assert!(
            cache.current_bytes() <= 1_024,
            "current_bytes 应 ≤ max_bytes，实际 {}",
            cache.current_bytes()
        );
        assert!(cache.get(&p3, 3).is_some(), "最新插入条目应保留");
    }

    #[test]
    fn byte_count_decreases_after_pop() {
        let mut cache = SearchTextCache::with_capacity_and_max_bytes(10, 1_000_000);
        let p1 = PathBuf::from("/x.jsonl");
        cache.put(p1.clone(), entry_with_text(1, 500));
        let before = cache.current_bytes();
        assert!(before > 500);

        // mtime mismatch → pop
        assert!(cache.get(&p1, 999).is_none());
        assert_eq!(cache.current_bytes(), 0, "pop 后字节计数应归零");
    }

    #[test]
    fn replacing_same_key_does_not_double_count_bytes() {
        let mut cache = SearchTextCache::with_capacity_and_max_bytes(10, 1_000_000);
        let p1 = PathBuf::from("/x.jsonl");
        cache.put(p1.clone(), entry_with_text(1, 100));
        let after_first = cache.current_bytes();

        cache.put(p1.clone(), entry_with_text(2, 100));
        let after_second = cache.current_bytes();

        // 两次同 key insert 字节使用量应相近（替换语义），不应叠加
        let diff = after_second.abs_diff(after_first);
        assert!(diff < 50, "替换后字节差异 {diff} 应 < 50 (常量级)");
    }
}
