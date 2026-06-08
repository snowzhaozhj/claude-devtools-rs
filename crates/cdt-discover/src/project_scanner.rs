//! 扫描 `~/.claude/projects/` 并输出 `Vec<Project>`。
//!
//! 职责：
//! 1. 枚举合法编码目录。
//! 2. 列出 `.jsonl` session，按 mtime 降序，附 `cwd` 字段（head-read）。
//! 3. 每个编码目录产 **1 个** `Project`：同目录下不同 `cwd` 的 session 始终归
//!    属同一 `Project`，cwd 差异由 `Session.cwd` 字段暴露。
//! 4. 全量按 `most_recent_session` 降序返回。
//!
//! Spec：`openspec/specs/project-discovery/spec.md` 的
//! `Scan Claude projects directory`、`List sessions per project`、
//! `Expose session cwd for downstream display`、
//! `Resolve historical Claude worktree directories` Requirement。

use std::collections::BTreeSet;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lru::LruCache;

use cdt_core::{Project, Session};
use cdt_parse::parse_entry_at;
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::Semaphore;

use crate::error::DiscoverError;
use crate::fs_provider::{FileSystemProvider, FsKind};
use crate::path_decoder::{
    decode_path, extract_base_dir, extract_project_name, is_valid_encoded_path,
    split_worktree_encoded_path,
};
use crate::project_path_resolver::ProjectPathResolver;

/// 扫描 session 头时读取的最大行数。
const SESSION_HEAD_LINES: usize = 20;

/// 顶层 project 目录并发扫描的上限。
const PROJECT_SCAN_CONCURRENCY: usize = 8;

/// 共享的文件读取并发上限。所有 `extract_session_cwd` 调用——无论来自哪个
/// project——在真正调用底层 `read_lines_head` / `read_to_string` 之前必须先持
/// 一份 permit。这是真正防止"8 project × N session 叠乘超过 macOS 默认 256
/// fd 上限"的硬闸门。
const FILE_READ_CONCURRENCY: usize = 64;

pub type CwdCache = Arc<Mutex<LruCache<PathBuf, String>>>;

const CWD_CACHE_CAPACITY: usize = 2048;

pub fn new_cwd_cache() -> CwdCache {
    Arc::new(Mutex::new(LruCache::new(
        NonZero::new(CWD_CACHE_CAPACITY).expect("non-zero"),
    )))
}

pub struct ProjectScanner {
    fs: Arc<dyn FileSystemProvider>,
    projects_dir: PathBuf,
    path_resolver: ProjectPathResolver,
    read_semaphore: Arc<Semaphore>,
    cwd_cache: Option<CwdCache>,
}

impl ProjectScanner {
    /// 便利构造：内部新建独立 `Arc<Semaphore>`。仅适合**测试** / 单次性
    /// 扫描场景；生产代码（多个 IPC 并发创建 scanner 时）SHALL 用
    /// [`new_with_semaphore`] 注入 `LocalDataApi` 持有的共享 semaphore，避免
    /// "N 个 scanner × 64 fd 击穿"风险。
    /// change `simplify-repository-as-project::D4`。
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemProvider>, projects_dir: PathBuf) -> Self {
        let semaphore = Arc::new(Semaphore::new(FILE_READ_CONCURRENCY));
        Self::new_with_semaphore(fs, projects_dir, semaphore)
    }

    /// 接受外部注入的共享 `Arc<Semaphore>` 控制 head-read 并发。多个
    /// scanner 实例共享同一 semaphore 时，全局 in-flight `read_lines_head`
    /// 上限始终为 semaphore 容量（生产默认 64），不会随 scanner 数量线性
    /// 放大。`LocalDataApi` 在内部所有动态 scanner 构造点 SHALL 走此入口。
    /// change `simplify-repository-as-project::D4`。
    #[must_use]
    pub fn new_with_semaphore(
        fs: Arc<dyn FileSystemProvider>,
        projects_dir: PathBuf,
        read_semaphore: Arc<Semaphore>,
    ) -> Self {
        let path_resolver = ProjectPathResolver::new(fs.clone(), projects_dir.clone());
        Self {
            fs,
            projects_dir,
            path_resolver,
            read_semaphore,
            cwd_cache: None,
        }
    }

    /// 在 `new_with_semaphore` 基础上注入共享 cwd 缓存。生产路径 SHALL 使用
    /// 此构造器确保跨 IPC 调用复用 cwd 读取结果。
    /// change `sidebar-cpu-throttle-and-cwd-cache::D1`。
    #[must_use]
    pub fn new_with_cwd_cache(
        fs: Arc<dyn FileSystemProvider>,
        projects_dir: PathBuf,
        read_semaphore: Arc<Semaphore>,
        cwd_cache: CwdCache,
    ) -> Self {
        let path_resolver = ProjectPathResolver::new(fs.clone(), projects_dir.clone());
        Self {
            fs,
            projects_dir,
            path_resolver,
            read_semaphore,
            cwd_cache: Some(cwd_cache),
        }
    }

    /// 返回 scanner 持有的 projects 根目录路径。
    #[must_use]
    pub fn projects_dir(&self) -> &std::path::Path {
        &self.projects_dir
    }

    /// 扫描根目录并返回所有 project。根目录不存在 → 空列表 + warn，**不报错**。
    pub async fn scan(&mut self) -> Result<Vec<Project>, DiscoverError> {
        if !self.fs.exists(&self.projects_dir).await {
            tracing::warn!(path = %self.projects_dir.display(), "projects root does not exist");
            return Ok(Vec::new());
        }

        self.path_resolver.clear();

        let entries = self.fs.read_dir(&self.projects_dir).await?;
        let dirs: Vec<String> = entries
            .into_iter()
            .filter(|e| e.kind.is_dir() && is_valid_encoded_path(&e.name))
            .map(|e| e.name)
            .collect();

        let mut all_projects = Vec::new();

        // SSH 模式保持顺序遍历（远端 ssh provider 串行更稳）；本地走并发上限受
        // `PROJECT_SCAN_CONCURRENCY` 控制的 `FuturesUnordered`。`scan_project_dir`
        // 只读 fs（`&self`），无内部 mutation，可并发跑。
        //
        // SSH 错误分流（修 GitHub issue #231 #2）：单 project scan 错误按
        // `FsError::is_likely_channel_dead()` 元方法分流——
        // - channel-dead（`Disconnected` / `TransientExhausted` 含 transport-dead
        //   关键字 / `Io` source kind 是 BrokenPipe/ConnectionReset/ConnectionAborted）
        //   SHALL 立即 abort 整轮 scan + 返 Err，让上层 `list_repository_groups`
        //   拿到 hard error 触发自愈路径，避免凑半成品列表误导用户
        // - 其它（普通单文件 IO / NotFound / 单 project 临时不可读）保留 silent
        //   skip + warn，让其它 project 仍可见
        if self.fs.kind() == FsKind::Ssh {
            for dir_name in dirs {
                match self.scan_project_dir(&dir_name).await {
                    Ok(projects) => all_projects.extend(projects),
                    Err(err) => {
                        if let crate::error::DiscoverError::Fs(fs_err) = &err {
                            if fs_err.is_likely_channel_dead() {
                                tracing::error!(
                                    dir = %dir_name,
                                    error = %fs_err,
                                    "ssh channel appears dead; aborting full scan to surface error",
                                );
                                return Err(err);
                            }
                        }
                        tracing::warn!(dir = %dir_name, error = ?err, "skip unreadable project dir");
                    }
                }
            }
        } else {
            let mut futs = FuturesUnordered::new();
            let mut iter = dirs.into_iter();
            for _ in 0..PROJECT_SCAN_CONCURRENCY {
                if let Some(dir_name) = iter.next() {
                    futs.push(Self::scan_with_name(self, dir_name));
                } else {
                    break;
                }
            }
            while let Some((dir_name, result)) = futs.next().await {
                match result {
                    Ok(projects) => all_projects.extend(projects),
                    Err(err) => {
                        tracing::warn!(dir = %dir_name, error = ?err, "skip unreadable project dir");
                    }
                }
                if let Some(next_dir) = iter.next() {
                    futs.push(Self::scan_with_name(self, next_dir));
                }
            }
        }

        all_projects.sort_by(|a, b| {
            b.most_recent_session
                .unwrap_or(0)
                .cmp(&a.most_recent_session.unwrap_or(0))
        });
        Ok(all_projects)
    }

    async fn scan_with_name(
        &self,
        dir_name: String,
    ) -> (String, Result<Vec<Project>, DiscoverError>) {
        let result = self.scan_project_dir(&dir_name).await;
        (dir_name, result)
    }

    /// 列出某个 project 的所有 session（带 mtime / size / cwd）。
    ///
    /// 若 `pinned` 集合里命中某个 session id，返回条目的 `is_pinned = true`。
    /// `cwd` 通过对每个 session 调 `extract_session_cwd`（head-read）填充，
    /// 与 `scan_project_dir` 同口径，使 sidebar / session 列表行能展示 cwd badge。
    pub async fn list_sessions(
        &self,
        project_id: &str,
        pinned: &BTreeSet<String>,
    ) -> Result<Vec<Session>, DiscoverError> {
        let base_dir = extract_base_dir(project_id);
        let dir = self.projects_dir.join(base_dir);
        if self.fs.kind() != FsKind::Ssh && !self.fs.exists(&dir).await {
            return Ok(Vec::new());
        }
        let entries = match self.fs.read_dir_with_metadata(&dir).await {
            Ok(entries) => entries,
            Err(crate::error::FsError::NotFound(_)) if self.fs.kind() == FsKind::Ssh => {
                return Ok(Vec::new());
            }
            Err(err) => return Err(err.into()),
        };

        let mut records: Vec<SessionStat> = Vec::new();
        for entry in entries {
            if !entry.kind.is_file() {
                continue;
            }
            let Some(id) = entry.name.strip_suffix(".jsonl") else {
                continue;
            };
            let full = dir.join(&entry.name);
            let stat = match entry.metadata {
                Some(metadata) => metadata,
                None => self.fs.stat(&full).await?,
            };
            records.push(SessionStat {
                id: id.to_string(),
                path: full,
                mtime_ms: stat.mtime_ms(),
                created_ms: stat.created_ms(),
                size: stat.size,
            });
        }

        let cwds: Vec<Option<String>> = self.extract_cwds_for(&records).await;

        let mut sessions: Vec<Session> = records
            .into_iter()
            .zip(cwds)
            .map(|(rec, cwd)| Session {
                last_modified: rec.mtime_ms,
                created: rec.created_ms,
                size: rec.size,
                is_pinned: pinned.contains(&rec.id),
                id: rec.id,
                cwd,
            })
            .collect();
        // 同 mtime 时 sid 字典序升序——k-way merge cursor 续页正确性依赖
        // session 流稳定顺序，否则 read_dir 顺序非确定性会让 (mtime, sid)
        // 指针的"严格之后"判定漂移。spec ipc-data-api §"Expose group session
        // listing via k-way merge pagination" Scenario "同 mtime sid 稳序"。
        sessions.sort_by(|a, b| {
            b.last_modified
                .cmp(&a.last_modified)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(sessions)
    }

    pub fn path_resolver(&self) -> &ProjectPathResolver {
        &self.path_resolver
    }

    async fn scan_project_dir(&self, dir_name: &str) -> Result<Vec<Project>, DiscoverError> {
        let dir_path = self.projects_dir.join(dir_name);
        let entries = self.fs.read_dir_with_metadata(&dir_path).await?;
        let mut records: Vec<SessionStat> = Vec::new();
        for entry in entries {
            if !entry.kind.is_file() {
                continue;
            }
            let Some(id) = entry.name.strip_suffix(".jsonl") else {
                continue;
            };
            let full = dir_path.join(&entry.name);
            let stat = match entry.metadata {
                Some(metadata) => metadata,
                None => self.fs.stat(&full).await?,
            };
            records.push(SessionStat {
                id: id.to_string(),
                path: full,
                mtime_ms: stat.mtime_ms(),
                created_ms: stat.created_ms(),
                size: stat.size,
            });
        }
        if records.is_empty() {
            return Ok(Vec::new());
        }

        records.sort_by_key(|s| std::cmp::Reverse(s.mtime_ms));

        let cwds: Vec<Option<String>> = self.extract_cwds_for(&records).await;

        // 取最新 mtime 的 session 的 cwd 当 `Project.path` 代表；无非空 cwd 时
        // fallback 到历史 worktree 解码 / encoded 目录解码。新设计下不再按 cwd
        // 分桶——一个 encoded 目录恒产一个 `Project`，cwd 差异由各 `Session.cwd`
        // 暴露。Spec：`project-discovery::Expose session cwd for downstream display`。
        let latest_cwd: Option<String> = cwds.iter().find_map(Clone::clone);
        let project_path: PathBuf = if let Some(cwd) = latest_cwd.as_deref() {
            PathBuf::from(cwd)
        } else if let Some(decoded) = self.decode_historical_worktree_dir(dir_name).await {
            decoded
        } else {
            decode_path(dir_name)
        };

        let session_ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
        let most_recent_ms: i64 = records.iter().map(|r| r.mtime_ms).max().unwrap_or(0);
        let created_ms: i64 = records.iter().map(|r| r.created_ms).min().unwrap_or(i64::MAX);

        // 收集所有 session 的 cwd 去重集合，保留 mtime 倒序：
        // 让 `agent-configs` 等消费方覆盖所有 cwd 的 `.claude/agents/` 扫描，
        // 避免合并 composite 后丢失非代表 cwd 的配置。Spec：
        // `agent-configs::Scan agent config files from global and project scopes`。
        //
        // 去重 key 走 `normalize_path_string_for_compare`（Windows 上 ASCII
        // 小写归一；非 Windows 字节精确），与 `project-discovery::Compare paths
        // case-insensitively on Windows` 保持一致。展示值保留**首次出现**的
        // 原始 cwd（即最新 mtime session 的 cwd 字面量），避免 Windows 上
        // 同一目录因 `C:\Users\foo` vs `c:\users\foo` 写法被当成两个 cwd。
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut distinct_cwds: Vec<String> = Vec::new();
        for cwd in cwds.iter().flatten() {
            let key = crate::path_compare::normalize_path_string_for_compare(cwd).into_owned();
            if seen.insert(key) {
                distinct_cwds.push(cwd.clone());
            }
        }

        let name = extract_project_name(&project_path);
        let project = Project {
            id: dir_name.to_string(),
            name,
            path: project_path,
            sessions: session_ids,
            most_recent_session: Some(most_recent_ms),
            created_at: if created_ms == i64::MAX {
                None
            } else {
                Some(created_ms)
            },
            distinct_cwds,
        };
        Ok(vec![project])
    }

    /// 并发提取一批 session 的 cwd。SSH 走顺序、本地走 `join_all`。
    async fn extract_cwds_for(&self, records: &[SessionStat]) -> Vec<Option<String>> {
        if self.fs.kind() == FsKind::Ssh {
            let mut out = Vec::with_capacity(records.len());
            for rec in records {
                out.push(self.extract_session_cwd(&rec.path).await);
            }
            out
        } else {
            futures::future::join_all(records.iter().map(|r| self.extract_session_cwd(&r.path)))
                .await
        }
    }

    async fn extract_session_cwd(&self, path: &Path) -> Option<String> {
        // Safety: no stat validation needed — relies on `extract_session_cwd_uses_first_line_only`
        // + `jsonl_append_after_first_line_does_not_change_cwd` invariants (Claude Code JSONL is
        // append-only, never truncated/rewritten). LRU eviction handles stale deleted paths.
        if self.fs.kind() != FsKind::Ssh {
            if let Some(cache) = &self.cwd_cache {
                if let Some(cached) = cache.lock().ok().and_then(|mut c| c.get(path).cloned()) {
                    return Some(cached);
                }
            }
        }

        let _permit = self.read_semaphore.acquire().await.ok()?;
        let head = self
            .fs
            .read_lines_head(path, SESSION_HEAD_LINES)
            .await
            .ok()?;
        if let Some(cwd) = extract_cwd_from_lines(path, &head) {
            if self.fs.kind() != FsKind::Ssh {
                if let Some(cache) = &self.cwd_cache {
                    if let Ok(mut guard) = cache.lock() {
                        guard.put(path.to_path_buf(), cwd.clone());
                    }
                }
            }
            return Some(cwd);
        }
        if self.fs.kind() == FsKind::Ssh {
            return None;
        }
        let content = self.fs.read_to_string(path).await.ok()?;
        let result = extract_cwd_from_iter(path, content.lines());
        if let Some(ref cwd) = result {
            if let Some(cache) = &self.cwd_cache {
                if let Ok(mut guard) = cache.lock() {
                    guard.put(path.to_path_buf(), cwd.clone());
                }
            }
        }
        result
    }

    async fn decode_historical_worktree_dir(&self, dir_name: &str) -> Option<PathBuf> {
        let (repo_encoded, worktree_encoded) = split_worktree_encoded_path(dir_name)?;
        if worktree_encoded.is_empty() {
            return None;
        }
        let repo = self.extract_cwd_from_project_dir(repo_encoded).await?;
        Some(
            repo.join(".claude")
                .join("worktrees")
                .join(worktree_encoded),
        )
    }

    async fn extract_cwd_from_project_dir(&self, dir_name: &str) -> Option<PathBuf> {
        let dir = self.projects_dir.join(dir_name);
        if !self.fs.exists(&dir).await {
            return Some(decode_path(dir_name));
        }
        let entries = self.fs.read_dir(&dir).await.ok()?;
        for entry in entries {
            if !entry.kind.is_file()
                || !Path::new(&entry.name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
            {
                continue;
            }
            let path = dir.join(entry.name);
            if let Some(cwd) = self.extract_session_cwd(&path).await {
                return Some(PathBuf::from(cwd));
            }
        }
        Some(decode_path(dir_name))
    }
}

fn extract_cwd_from_lines(path: &Path, lines: &[String]) -> Option<String> {
    extract_cwd_from_iter(path, lines.iter().map(String::as_str))
}

fn extract_cwd_from_iter<'a>(
    path: &Path,
    lines: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    for (idx, line) in lines.into_iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        match parse_entry_at(line, idx + 1) {
            Ok(Some(msg)) => {
                if let Some(cwd) = msg.cwd {
                    if !cwd.is_empty() {
                        return Some(cwd);
                    }
                }
            }
            Ok(None) => {}
            Err(err) => {
                tracing::debug!(path = %path.display(), line = idx + 1, error = ?err, "skip malformed line");
            }
        }
    }
    None
}

struct SessionStat {
    id: String,
    path: PathBuf,
    mtime_ms: i64,
    created_ms: i64,
    size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_provider::LocalFileSystemProvider;
    use cdt_fs::{InstrumentedFs, with_fs_counter};
    use std::fmt::Write as _;
    use tempfile::tempdir;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn missing_root_returns_empty() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("nope");
        let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
        let mut scanner = ProjectScanner::new(fs, missing);
        let projects = scanner.scan().await.unwrap();
        assert!(projects.is_empty());
    }

    #[tokio::test]
    async fn empty_root_returns_empty() {
        let dir = tempdir().unwrap();
        let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
        let mut scanner = ProjectScanner::new(fs, dir.path().to_path_buf());
        let projects = scanner.scan().await.unwrap();
        assert!(projects.is_empty());
    }

    /// 契约：`extract_session_cwd` SHALL 在首行命中 cwd，MUST NOT 走
    /// `read_to_string` 整文件兜底。
    /// Spec：`openspec/specs/project-discovery/spec.md` §`extract_session_cwd 仅读首行的不变量`。
    #[tokio::test]
    async fn extract_session_cwd_uses_first_line_only() {
        let dir = tempdir().unwrap();
        let jsonl_path = dir.path().join("session.jsonl");
        let mut content = String::new();
        content.push_str(
            r#"{"uuid":"u1","type":"user","cwd":"/path/to/proj","message":{"role":"user"}}"#,
        );
        content.push('\n');
        for i in 0..999 {
            writeln!(
                content,
                r#"{{"uuid":"u{}","type":"assistant","message":{{"role":"assistant"}}}}"#,
                i + 2
            )
            .unwrap();
        }
        tokio::fs::write(&jsonl_path, content.as_bytes())
            .await
            .unwrap();

        let fs: Arc<dyn FileSystemProvider> =
            Arc::new(InstrumentedFs::new(LocalFileSystemProvider::new()));
        let scanner = ProjectScanner::new(fs, dir.path().to_path_buf());

        let (cwd, counts) =
            with_fs_counter(|| async { scanner.extract_session_cwd(&jsonl_path).await }).await;

        assert_eq!(cwd, Some("/path/to/proj".to_string()));
        assert_eq!(
            counts.read_to_string, 0,
            "read_to_string fallback MUST NOT 触发——首行 cwd 已命中"
        );
        assert_eq!(counts.read_lines_head, 1, "SHALL 仅调一次 read_lines_head");
    }

    /// 契约：JSONL 后续 append SHALL NOT 改变 `extract_session_cwd` 抽取结果。
    /// 这是 `ipc-data-api::ProjectScanCache 按事件语义分级失效` 的"普通 append 不
    /// 失效 cache"决策的前提。
    #[tokio::test]
    async fn jsonl_append_after_first_line_does_not_change_cwd() {
        let dir = tempdir().unwrap();
        let jsonl_path = dir.path().join("session.jsonl");
        tokio::fs::write(
            &jsonl_path,
            br#"{"uuid":"u1","type":"user","cwd":"/path/to/proj","message":{"role":"user"}}
"#,
        )
        .await
        .unwrap();

        let fs: Arc<dyn FileSystemProvider> =
            Arc::new(InstrumentedFs::new(LocalFileSystemProvider::new()));
        let scanner = ProjectScanner::new(fs, dir.path().to_path_buf());

        let (r1, counts1) =
            with_fs_counter(|| async { scanner.extract_session_cwd(&jsonl_path).await }).await;
        assert_eq!(r1, Some("/path/to/proj".to_string()));
        assert_eq!(counts1.read_to_string, 0);

        // append 100 行 assistant message，不含 cwd
        let mut f = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&jsonl_path)
            .await
            .unwrap();
        for i in 0..100 {
            let line = format!(
                r#"{{"uuid":"u{}","type":"assistant","message":{{"role":"assistant"}}}}
"#,
                i + 2
            );
            f.write_all(line.as_bytes()).await.unwrap();
        }
        f.flush().await.unwrap();
        drop(f);

        let (r2, counts2) =
            with_fs_counter(|| async { scanner.extract_session_cwd(&jsonl_path).await }).await;
        assert_eq!(r2, r1, "append 后 cwd SHALL 不变");
        assert_eq!(counts2.read_to_string, 0);
    }

    #[tokio::test]
    async fn cwd_cache_hit_skips_file_io() {
        let dir = tempdir().unwrap();
        let jsonl_path = dir.path().join("session.jsonl");
        tokio::fs::write(
            &jsonl_path,
            br#"{"uuid":"u1","type":"user","cwd":"/cached/path","message":{"role":"user"}}
"#,
        )
        .await
        .unwrap();

        let fs: Arc<dyn FileSystemProvider> =
            Arc::new(InstrumentedFs::new(LocalFileSystemProvider::new()));
        let cache = new_cwd_cache();
        let scanner = ProjectScanner::new_with_cwd_cache(
            fs,
            dir.path().to_path_buf(),
            Arc::new(Semaphore::new(64)),
            cache.clone(),
        );

        let (r1, c1) =
            with_fs_counter(|| async { scanner.extract_session_cwd(&jsonl_path).await }).await;
        assert_eq!(r1, Some("/cached/path".to_string()));
        assert_eq!(c1.read_lines_head, 1);

        let (r2, c2) =
            with_fs_counter(|| async { scanner.extract_session_cwd(&jsonl_path).await }).await;
        assert_eq!(r2, Some("/cached/path".to_string()));
        assert_eq!(
            c2.read_lines_head, 0,
            "cache hit SHALL skip read_lines_head"
        );
    }

    #[tokio::test]
    async fn cwd_cache_does_not_cache_none_result() {
        let dir = tempdir().unwrap();
        let jsonl_path = dir.path().join("session.jsonl");
        tokio::fs::write(
            &jsonl_path,
            br#"{"uuid":"u1","type":"assistant","message":{"role":"assistant"}}
"#,
        )
        .await
        .unwrap();

        let fs: Arc<dyn FileSystemProvider> =
            Arc::new(InstrumentedFs::new(LocalFileSystemProvider::new()));
        let cache = new_cwd_cache();
        let scanner = ProjectScanner::new_with_cwd_cache(
            fs,
            dir.path().to_path_buf(),
            Arc::new(Semaphore::new(64)),
            cache.clone(),
        );

        let r1 = scanner.extract_session_cwd(&jsonl_path).await;
        assert_eq!(r1, None);
        assert!(
            cache.lock().unwrap().peek(&jsonl_path).is_none(),
            "None result SHALL NOT be cached"
        );
    }

    #[tokio::test]
    async fn cwd_cache_lru_eviction() {
        let dir = tempdir().unwrap();
        let small_cache = Arc::new(Mutex::new(LruCache::new(NonZero::new(2).unwrap())));
        let fs: Arc<dyn FileSystemProvider> =
            Arc::new(InstrumentedFs::new(LocalFileSystemProvider::new()));

        for i in 0..3 {
            let p = dir.path().join(format!("s{i}.jsonl"));
            tokio::fs::write(
                &p,
                format!(
                    r#"{{"uuid":"u1","type":"user","cwd":"/path/{i}","message":{{"role":"user"}}}}{}"#,
                    "\n"
                ),
            )
            .await
            .unwrap();
            let scanner = ProjectScanner::new_with_cwd_cache(
                fs.clone(),
                dir.path().to_path_buf(),
                Arc::new(Semaphore::new(64)),
                small_cache.clone(),
            );
            let r = scanner.extract_session_cwd(&p).await;
            assert_eq!(r, Some(format!("/path/{i}")));
        }

        let guard = small_cache.lock().unwrap();
        assert_eq!(guard.len(), 2, "LRU cap=2 should evict oldest");
        assert!(
            guard.peek(&dir.path().join("s0.jsonl")).is_none(),
            "oldest entry evicted"
        );
        assert!(guard.peek(&dir.path().join("s1.jsonl")).is_some());
        assert!(guard.peek(&dir.path().join("s2.jsonl")).is_some());
    }

    #[tokio::test]
    async fn cwd_cache_only_used_for_local_fs_kind() {
        let dir = tempdir().unwrap();
        let jsonl_path = dir.path().join("session.jsonl");
        tokio::fs::write(
            &jsonl_path,
            br#"{"uuid":"u1","type":"user","cwd":"/real/path","message":{"role":"user"}}
"#,
        )
        .await
        .unwrap();

        let cache = new_cwd_cache();
        let fs: Arc<dyn FileSystemProvider> =
            Arc::new(InstrumentedFs::new(LocalFileSystemProvider::new()));
        let scanner = ProjectScanner::new_with_cwd_cache(
            fs,
            dir.path().to_path_buf(),
            Arc::new(Semaphore::new(64)),
            cache.clone(),
        );

        let r = scanner.extract_session_cwd(&jsonl_path).await;
        assert_eq!(r, Some("/real/path".to_string()));
        assert!(
            cache.lock().unwrap().peek(&jsonl_path).is_some(),
            "Local FsKind SHALL write to cache"
        );
    }
}
