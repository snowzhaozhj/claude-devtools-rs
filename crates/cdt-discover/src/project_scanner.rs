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
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cdt_core::{Project, Session};
use cdt_parse::parse_entry_at;
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::Semaphore;

use crate::error::DiscoverError;
use crate::fs_provider::{FileSystemProvider, FsKind};
use crate::path_decoder::{
    decode_path, extract_base_dir, extract_project_name, is_valid_encoded_path,
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

pub struct ProjectScanner {
    fs: Arc<dyn FileSystemProvider>,
    projects_dir: PathBuf,
    path_resolver: ProjectPathResolver,
    read_semaphore: Arc<Semaphore>,
}

impl ProjectScanner {
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemProvider>, projects_dir: PathBuf) -> Self {
        let path_resolver = ProjectPathResolver::new(fs.clone(), projects_dir.clone());
        Self {
            fs,
            projects_dir,
            path_resolver,
            read_semaphore: Arc::new(Semaphore::new(FILE_READ_CONCURRENCY)),
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
        if self.fs.kind() == FsKind::Ssh {
            for dir_name in dirs {
                match self.scan_project_dir(&dir_name).await {
                    Ok(projects) => all_projects.extend(projects),
                    Err(err) => {
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
                size: stat.size,
            });
        }

        let cwds: Vec<Option<String>> = self.extract_cwds_for(&records).await;

        let mut sessions: Vec<Session> = records
            .into_iter()
            .zip(cwds)
            .map(|(rec, cwd)| Session {
                last_modified: rec.mtime_ms,
                size: rec.size,
                is_pinned: pinned.contains(&rec.id),
                id: rec.id,
                cwd,
            })
            .collect();
        sessions.sort_by_key(|s| std::cmp::Reverse(s.last_modified));
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
        let created_ms: i64 = records.iter().map(|r| r.mtime_ms).min().unwrap_or(i64::MAX);

        // 收集所有 session 的 cwd 去重集合，保留 mtime 倒序：
        // 让 `agent-configs` 等消费方覆盖所有 cwd 的 `.claude/agents/` 扫描，
        // 避免合并 composite 后丢失非代表 cwd 的配置。Spec：
        // `agent-configs::Scan agent config files from global and project scopes`。
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut distinct_cwds: Vec<String> = Vec::new();
        for cwd in cwds.iter().flatten() {
            if seen.insert(cwd.clone()) {
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
        // permit 在真正发起 fd-密集型 fs 读之前 acquire——并发顶层 8 project ×
        // 各自 join_all 的子 future 在这里排队，确保全局 in-flight read 数量
        // 不超过 `FILE_READ_CONCURRENCY`，否则会撞 macOS 默认 256 fd 软上限。
        let _permit = self.read_semaphore.acquire().await.ok()?;
        let head = self
            .fs
            .read_lines_head(path, SESSION_HEAD_LINES)
            .await
            .ok()?;
        if let Some(cwd) = extract_cwd_from_lines(path, &head) {
            return Some(cwd);
        }
        if self.fs.kind() == FsKind::Ssh {
            return None;
        }
        let content = self.fs.read_to_string(path).await.ok()?;
        extract_cwd_from_iter(path, content.lines())
    }

    async fn decode_historical_worktree_dir(&self, dir_name: &str) -> Option<PathBuf> {
        let (repo_encoded, worktree_encoded) = dir_name
            .split_once("-.claude-worktrees-")
            .or_else(|| dir_name.split_once("--claude-worktrees-"))?;
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
    size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_provider::LocalFileSystemProvider;
    use tempfile::tempdir;

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
}
