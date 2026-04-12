//! 基于 mtime 的 LRU 搜索文本缓存。
//!
//! 避免对未变更的 session 文件重复解析 JSONL + 提取可搜索文本。

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use lru::LruCache;

use crate::search_extract::SearchableEntry;

const DEFAULT_CAPACITY: usize = 1000;

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
}

impl Default for SearchTextCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchTextCache {
    /// 创建默认容量（1000）的缓存。
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// 创建指定容量的缓存。
    pub fn with_capacity(cap: usize) -> Self {
        let cap = NonZeroUsize::new(cap).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self {
            inner: LruCache::new(cap),
        }
    }

    /// mtime 匹配则返回缓存条目，否则移除过期条目并返回 `None`。
    pub fn get(&mut self, path: &Path, current_mtime_ms: u64) -> Option<&CacheEntry> {
        if let Some(entry) = self.inner.peek(path) {
            if entry.mtime_ms == current_mtime_ms {
                // promote 到 MRU
                return self.inner.get(path);
            }
            // mtime 不匹配，移除
            self.inner.pop(path);
        }
        None
    }

    /// 写入缓存条目。
    pub fn put(&mut self, path: PathBuf, entry: CacheEntry) {
        self.inner.put(path, entry);
    }
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
}
