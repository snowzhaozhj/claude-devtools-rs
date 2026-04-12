//! Session 搜索器——owns **session-search** capability。
//!
//! 支持三级搜索 scope（单 session、单 project、全局），
//! 基于 mtime 的 LRU 缓存避免重复解析，SSH 模式支持分阶段限制。
//!
//! Spec：`openspec/specs/session-search/spec.md`。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use cdt_core::{SearchHit, SearchSessionsResult, SessionSearchResult};

use crate::error::DiscoverError;
use crate::fs_provider::{FileSystemProvider, FsKind};
use crate::path_decoder::get_projects_base_path;
use crate::search_cache::{CacheEntry, SearchTextCache};
use crate::search_extract::{SearchableEntry, extract_searchable_entries};

/// SSH 分阶段搜索配置。
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// 是否 SSH 模式（启用 stage-limit）。
    pub is_ssh: bool,
    /// 每阶段的文件数上限。
    pub stage_limits: Vec<usize>,
    /// 总时间预算。
    pub time_budget: Duration,
    /// 达到此结果数后可提前返回。
    pub min_results: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            is_ssh: false,
            stage_limits: vec![40, 140, 320],
            time_budget: Duration::from_millis(4500),
            min_results: 8,
        }
    }
}

impl SearchConfig {
    /// 根据 `FileSystemProvider` 的 kind 自动推断配置。
    pub fn from_fs_kind(kind: FsKind) -> Self {
        Self {
            is_ssh: kind == FsKind::Ssh,
            ..Self::default()
        }
    }
}

/// Session 搜索器。
pub struct SessionSearcher<F: FileSystemProvider> {
    fs: Arc<F>,
    cache: Arc<Mutex<SearchTextCache>>,
}

impl<F: FileSystemProvider> SessionSearcher<F> {
    pub fn new(fs: Arc<F>, cache: Arc<Mutex<SearchTextCache>>) -> Self {
        Self { fs, cache }
    }

    /// 搜索单个 session 文件。
    pub async fn search_session_file(
        &self,
        project_id: &str,
        session_id: &str,
        file_path: &Path,
        query: &str,
        max_results: usize,
    ) -> Result<SessionSearchResult, DiscoverError> {
        let (entries, session_title) = self.get_or_extract(file_path).await?;
        let query_lower = query.to_lowercase();
        let hits = find_matches(&entries, &query_lower, max_results);

        let total_matches = if hits.len() < max_results {
            hits.len()
        } else {
            count_all_matches(&entries, &query_lower)
        };

        Ok(SessionSearchResult {
            session_id: session_id.to_owned(),
            project_id: project_id.to_owned(),
            session_title,
            hits,
            total_matches,
        })
    }

    /// 搜索 project 下的所有 session。
    pub async fn search_sessions(
        &self,
        project_id: &str,
        query: &str,
        max_results: usize,
        config: &SearchConfig,
    ) -> Result<SearchSessionsResult, DiscoverError> {
        let base = get_projects_base_path();
        let project_dir = base.join(project_id);

        let mut files = self.list_session_files(&project_dir).await?;
        // 按 mtime 降序
        files.sort_by(|a, b| b.1.cmp(&a.1));

        let mut results = Vec::new();
        let mut total_matches = 0usize;
        let mut sessions_searched = 0usize;
        let mut is_partial = false;

        let start = Instant::now();

        let file_count = files.len();
        let mut processed = 0usize;

        for (path, _mtime, session_id) in &files {
            // SSH stage-limit 检查
            if config.is_ssh {
                let elapsed = start.elapsed();
                if elapsed >= config.time_budget && !results.is_empty() {
                    is_partial = true;
                    break;
                }
                if should_stop_at_stage(
                    processed,
                    &config.stage_limits,
                    results.len(),
                    config.min_results,
                ) {
                    is_partial = true;
                    break;
                }
            }

            sessions_searched += 1;
            processed += 1;

            let result = self
                .search_session_file(project_id, session_id, path, query, max_results)
                .await;

            match result {
                Ok(r) if !r.hits.is_empty() => {
                    total_matches += r.total_matches;
                    results.push(r);
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping session file");
                }
                _ => {}
            }

            if results.len() >= max_results && !config.is_ssh {
                break;
            }
        }

        // 非 SSH 且未搜完所有文件
        if !config.is_ssh && processed < file_count && !results.is_empty() {
            is_partial = true;
        }

        Ok(SearchSessionsResult {
            results,
            total_matches,
            sessions_searched,
            query: query.to_owned(),
            is_partial,
        })
    }

    /// 从缓存获取或解析提取可搜索文本。
    async fn get_or_extract(
        &self,
        file_path: &Path,
    ) -> Result<(Vec<SearchableEntry>, String), DiscoverError> {
        let stat = self.fs.stat(file_path).await.map_err(DiscoverError::Fs)?;
        let mtime_ms = u64::try_from(stat.mtime_ms()).unwrap_or(0);

        {
            let mut cache = self.cache.lock().await;
            if let Some(entry) = cache.get(file_path, mtime_ms) {
                return Ok((entry.entries.clone(), entry.session_title.clone()));
            }
        }

        // 缓存未命中，解析文件
        let content = self
            .fs
            .read_to_string(file_path)
            .await
            .map_err(DiscoverError::Fs)?;
        let messages: Vec<_> = content
            .lines()
            .enumerate()
            .filter_map(|(n, line)| cdt_parse::parse_entry_at(line, n).ok().flatten())
            .collect();
        let deduped = cdt_parse::dedupe_by_request_id(messages);
        let (entries, title) = extract_searchable_entries(&deduped);

        // 写入缓存
        {
            let mut cache = self.cache.lock().await;
            cache.put(
                file_path.to_path_buf(),
                CacheEntry {
                    entries: entries.clone(),
                    session_title: title.clone(),
                    mtime_ms,
                },
            );
        }

        Ok((entries, title))
    }

    /// 列出目录下所有 `.jsonl` 文件及其 mtime 和 `session_id`。
    async fn list_session_files(
        &self,
        dir: &Path,
    ) -> Result<Vec<(PathBuf, i64, String)>, DiscoverError> {
        let entries = self.fs.read_dir(dir).await.map_err(DiscoverError::Fs)?;
        let mut result = Vec::new();
        for entry in entries {
            if let Some(id) = entry.name.strip_suffix(".jsonl") {
                let path = dir.join(&entry.name);
                let mtime = self.fs.stat(&path).await.map(|m| m.mtime_ms()).unwrap_or(0);
                result.push((path, mtime, id.to_owned()));
            }
        }
        Ok(result)
    }
}

/// 大小写不敏感匹配，返回 hits（限制数量）。
fn find_matches(entries: &[SearchableEntry], query_lower: &str, max: usize) -> Vec<SearchHit> {
    let mut hits = Vec::new();
    for entry in entries {
        let text_lower = entry.text.to_lowercase();
        let mut start = 0;
        while let Some(pos) = text_lower[start..].find(query_lower) {
            let offset = start + pos;
            hits.push(SearchHit {
                message_uuid: entry.uuid.clone(),
                offset,
                preview: extract_preview(&entry.text, offset, query_lower.len()),
                message_type: entry.message_type.clone(),
            });
            if hits.len() >= max {
                return hits;
            }
            start = offset + query_lower.len();
        }
    }
    hits
}

/// 统计所有匹配数（不限制）。
fn count_all_matches(entries: &[SearchableEntry], query_lower: &str) -> usize {
    let mut count = 0;
    for entry in entries {
        let text_lower = entry.text.to_lowercase();
        let mut start = 0;
        while let Some(pos) = text_lower[start..].find(query_lower) {
            count += 1;
            start += pos + query_lower.len();
        }
    }
    count
}

/// 提取匹配位置前后各 50 char 的预览。
fn extract_preview(text: &str, byte_offset: usize, match_len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    // 找到 byte_offset 对应的 char index
    let mut byte_acc = 0;
    let mut char_offset = 0;
    for (i, c) in chars.iter().enumerate() {
        if byte_acc >= byte_offset {
            char_offset = i;
            break;
        }
        byte_acc += c.len_utf8();
        char_offset = i + 1;
    }

    let context = 50;
    let start = char_offset.saturating_sub(context);
    // match_len 是 lowercase bytes，近似 char 数
    let match_chars = match_len.min(chars.len().saturating_sub(char_offset));
    let end = (char_offset + match_chars + context).min(chars.len());

    chars[start..end].iter().collect()
}

/// SSH 阶段检查：是否应在当前阶段停下。
fn should_stop_at_stage(
    processed: usize,
    stage_limits: &[usize],
    result_count: usize,
    min_results: usize,
) -> bool {
    for &limit in stage_limits {
        if processed == limit && result_count >= min_results {
            return true;
        }
    }
    false
}
