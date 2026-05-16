//! parsed-message LRU 缓存：按 `(jsonl_path, FileSignature)` 缓存
//! `cdt_parse::parse_file` 结果，让 `get_tool_output` / `get_image_asset`
//! hot path 避免重复 line-by-line 解析整个 JSONL。
//!
//! 行为契约见 `openspec/specs/ipc-data-api/spec.md` §"`get_tool_output` 与
//! `get_image_asset` 走 parsed-message LRU 缓存"。模式与
//! `crates/cdt-api/src/ipc/session_metadata.rs::MetadataCache` 完全对齐：
//! 同款 `(mtime, size, identity)` 签名 + LRU + 命中 bump 到队首。
//!
//! 容量上限调低为 50（vs metadata 200）——单条 entry 是
//! `Arc<Vec<ParsedMessage>>`，量级千倍以上；详 change
//! `parsed-message-lru-cache` design D3。

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use cdt_core::ParsedMessage;
use cdt_parse::parse_file;

use crate::cache_signature::FileSignature;

/// 缓存容量上限。详 change `parsed-message-lru-cache` design D3。
pub const PARSED_MESSAGE_CACHE_CAPACITY: usize = 50;

#[derive(Debug, Clone)]
struct ParsedMessageEntry {
    signature: FileSignature,
    messages: Arc<Vec<ParsedMessage>>,
}

/// `LocalDataApi` 持有的 parsed-message LRU 缓存（**不**用全局单例，详 design D3）。
#[derive(Debug)]
pub struct ParsedMessageCache {
    map: HashMap<PathBuf, ParsedMessageEntry>,
    order: VecDeque<PathBuf>,
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

    fn lookup(&mut self, path: &Path) -> Option<ParsedMessageEntry> {
        let entry = self.map.get(path)?.clone();
        if let Some(pos) = self.order.iter().position(|p| p == path) {
            let p = self.order.remove(pos).expect("position 已校验");
            self.order.push_front(p);
        }
        Some(entry)
    }

    fn insert(&mut self, path: PathBuf, entry: ParsedMessageEntry) {
        if self.map.contains_key(&path) {
            self.map.insert(path.clone(), entry);
            if let Some(pos) = self.order.iter().position(|p| p == &path) {
                let p = self.order.remove(pos).expect("position 已校验");
                self.order.push_front(p);
            }
            return;
        }

        if self.map.len() >= self.capacity {
            if let Some(evicted) = self.order.pop_back() {
                self.map.remove(&evicted);
            }
        }

        self.map.insert(path.clone(), entry);
        self.order.push_front(path);
    }

    /// 主动从缓存移除 `path` 条目（由 file-watcher 广播 invalidate 路径调用）。
    /// 不在 cache 中时 no-op。
    pub fn remove(&mut self, path: &Path) {
        if self.map.remove(path).is_some() {
            if let Some(pos) = self.order.iter().position(|p| p == path) {
                let _ = self.order.remove(pos);
            }
        }
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
/// - `tokio::fs::metadata(path)` 失败：SHALL NOT 写入 cache
/// - `parse_file(path)` 返回 `Err`：SHALL NOT 写入 cache（避免 negative cache
///   引入新失效边界，详 design D6）
///
/// 行为契约：`openspec/specs/ipc-data-api/spec.md` §"`get_tool_output` 与
/// `get_image_asset` 走 parsed-message LRU 缓存"。
pub(crate) async fn extract_parsed_messages_cached(
    cache: &StdMutex<ParsedMessageCache>,
    path: &Path,
) -> Option<Arc<Vec<ParsedMessage>>> {
    let sig = FileSignature::from_metadata(&tokio::fs::metadata(path).await.ok()?);

    {
        let cached = cache
            .lock()
            .expect("parsed message cache mutex poisoned")
            .lookup(path);
        if let Some(entry) = cached {
            if entry.signature == sig {
                return Some(entry.messages);
            }
        }
    }

    let messages = match parse_file(path).await {
        Ok(m) => Arc::new(m),
        Err(e) => {
            tracing::warn!(
                target: "cdt_api::parsed_message_cache",
                path = %path.display(),
                error = %e,
                "parse_file failed; SHALL NOT write to cache"
            );
            return None;
        }
    };

    cache
        .lock()
        .expect("parsed message cache mutex poisoned")
        .insert(
            path.to_path_buf(),
            ParsedMessageEntry {
                signature: sig,
                messages: messages.clone(),
            },
        );

    Some(messages)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::cache_signature::{FileIdentity, FileSignature};

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
        let mut cache = ParsedMessageCache::new(2);
        cache.insert(PathBuf::from("/a"), dummy_entry(1));
        cache.insert(PathBuf::from("/b"), dummy_entry(2));
        cache.insert(PathBuf::from("/c"), dummy_entry(3));
        assert!(cache.lookup(Path::new("/a")).is_none(), "/a 应被淘汰");
        assert!(cache.lookup(Path::new("/b")).is_some());
        assert!(cache.lookup(Path::new("/c")).is_some());
        assert!(cache.len() <= 2);
    }

    #[test]
    fn parsed_cache_lookup_bumps_hit_to_front() {
        let mut cache = ParsedMessageCache::new(2);
        cache.insert(PathBuf::from("/a"), dummy_entry(1));
        cache.insert(PathBuf::from("/b"), dummy_entry(2));
        assert!(cache.lookup(Path::new("/a")).is_some());
        cache.insert(PathBuf::from("/c"), dummy_entry(3));
        assert!(
            cache.lookup(Path::new("/a")).is_some(),
            "命中后 bump 队首，不应被淘汰"
        );
        assert!(cache.lookup(Path::new("/b")).is_none(), "/b 应被淘汰");
    }

    #[test]
    fn parsed_cache_remove_drops_entry() {
        let mut cache = ParsedMessageCache::new(2);
        cache.insert(PathBuf::from("/a"), dummy_entry(1));
        cache.insert(PathBuf::from("/b"), dummy_entry(2));
        cache.remove(Path::new("/a"));
        assert!(cache.lookup(Path::new("/a")).is_none());
        assert!(cache.lookup(Path::new("/b")).is_some());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn parsed_cache_remove_noop_when_absent() {
        let mut cache = ParsedMessageCache::new(2);
        cache.insert(PathBuf::from("/a"), dummy_entry(1));
        cache.remove(Path::new("/nonexistent"));
        assert_eq!(cache.len(), 1);
    }

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

        let a = extract_parsed_messages_cached(&cache, &path)
            .await
            .expect("first parse should succeed");
        assert_eq!(a.len(), 2);
        assert_eq!(cache.lock().unwrap().len(), 1);

        let b = extract_parsed_messages_cached(&cache, &path)
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
        let m1 = extract_parsed_messages_cached(&cache, &path).await.unwrap();
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

        let m2 = extract_parsed_messages_cached(&cache, &path).await.unwrap();
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
        let m1 = extract_parsed_messages_cached(&cache, &path).await.unwrap();
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

        let m2 = extract_parsed_messages_cached(&cache, &path).await.unwrap();
        assert_eq!(
            m2.len(),
            2,
            "rename 替换（inode 变化）SHALL 走 cache miss + 重 parse"
        );
    }

    #[tokio::test]
    async fn cached_stat_failure_returns_none_no_write() {
        let cache = make_cache();
        let result =
            extract_parsed_messages_cached(&cache, Path::new("/nonexistent/path.jsonl")).await;
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
        let result = extract_parsed_messages_cached(&cache, &path).await.unwrap();
        assert!(result.is_empty());
        assert_eq!(cache.lock().unwrap().len(), 1);
    }
}
