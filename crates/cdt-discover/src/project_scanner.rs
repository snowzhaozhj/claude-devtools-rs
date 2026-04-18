//! 扫描 `~/.claude/projects/` 并输出 `Vec<Project>`。
//!
//! 职责：
//! 1. 枚举合法编码目录。
//! 2. 对每个目录抽取 `cwd` 以拆分 subproject（产生 composite ID）。
//! 3. 列出 `.jsonl` session，按 mtime 降序附在 `Project` 上。
//! 4. 全量按 `most_recent_session` 降序返回。
//!
//! Spec：`openspec/specs/project-discovery/spec.md` 的
//! `Scan Claude projects directory`、`List sessions per project`、
//! `Resolve subprojects and pinned sessions` Requirement。

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cdt_core::{Project, Session};
use cdt_parse::parse_entry_at;

use crate::error::DiscoverError;
use crate::fs_provider::{FileSystemProvider, FsKind};
use crate::path_decoder::{
    decode_path, extract_base_dir, extract_project_name, is_valid_encoded_path,
};
use crate::project_path_resolver::ProjectPathResolver;
use crate::subproject_registry::SubprojectRegistry;

/// 扫描 session 头时读取的最大行数。
const SESSION_HEAD_LINES: usize = 20;

pub struct ProjectScanner {
    fs: Arc<dyn FileSystemProvider>,
    projects_dir: PathBuf,
    registry: SubprojectRegistry,
    path_resolver: ProjectPathResolver,
}

impl ProjectScanner {
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemProvider>, projects_dir: PathBuf) -> Self {
        let path_resolver = ProjectPathResolver::new(fs.clone(), projects_dir.clone());
        Self {
            fs,
            projects_dir,
            registry: SubprojectRegistry::new(),
            path_resolver,
        }
    }

    /// 返回 scanner 持有的 projects 根目录路径。
    ///
    /// 用于上层 crate（如 `cdt-api`）需要在 scanner 维度构造文件路径
    /// 时复用同一个 base，避免再依赖 `path_decoder::get_projects_base_path()`
    /// 这种全局环境路径——便于集成测试通过 tempdir override。
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

        self.registry.clear();
        self.path_resolver.clear();

        let entries = self.fs.read_dir(&self.projects_dir).await?;
        let dirs: Vec<String> = entries
            .into_iter()
            .filter(|e| e.kind.is_dir() && is_valid_encoded_path(&e.name))
            .map(|e| e.name)
            .collect();

        let mut all_projects = Vec::new();
        for dir_name in dirs {
            match self.scan_project_dir(&dir_name).await {
                Ok(mut projects) => all_projects.append(&mut projects),
                Err(err) => {
                    tracing::warn!(dir = %dir_name, error = ?err, "skip unreadable project dir");
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

    /// 列出某个 project 的所有 session（带 mtime / size）。
    ///
    /// 若 `pinned` 集合里命中某个 session id，返回条目的 `is_pinned = true`。
    /// composite project ID 只返回 registry filter 命中的 session。
    pub async fn list_sessions(
        &self,
        project_id: &str,
        pinned: &BTreeSet<String>,
    ) -> Result<Vec<Session>, DiscoverError> {
        let base_dir = extract_base_dir(project_id);
        let dir = self.projects_dir.join(base_dir);
        if !self.fs.exists(&dir).await {
            return Ok(Vec::new());
        }
        let entries = self.fs.read_dir(&dir).await?;
        let filter = self.registry.get_session_filter(project_id);
        let mut sessions: Vec<Session> = Vec::new();
        for entry in entries {
            if !entry.kind.is_file() {
                continue;
            }
            let Some(id) = entry.name.strip_suffix(".jsonl") else {
                continue;
            };
            if let Some(f) = filter {
                if !f.contains(id) {
                    continue;
                }
            }
            let stat = self.fs.stat(&dir.join(&entry.name)).await?;
            sessions.push(Session {
                id: id.to_string(),
                last_modified: stat.mtime_ms(),
                size: stat.size,
                is_pinned: pinned.contains(id),
            });
        }
        sessions.sort_by_key(|s| std::cmp::Reverse(s.last_modified));
        Ok(sessions)
    }

    pub fn registry(&self) -> &SubprojectRegistry {
        &self.registry
    }

    pub fn path_resolver(&self) -> &ProjectPathResolver {
        &self.path_resolver
    }

    async fn scan_project_dir(&mut self, dir_name: &str) -> Result<Vec<Project>, DiscoverError> {
        let dir_path = self.projects_dir.join(dir_name);
        let entries = self.fs.read_dir(&dir_path).await?;
        let mut session_stats: Vec<SessionStat> = Vec::new();
        for entry in entries {
            if !entry.kind.is_file() {
                continue;
            }
            let Some(id) = entry.name.strip_suffix(".jsonl") else {
                continue;
            };
            let full = dir_path.join(&entry.name);
            let stat = self.fs.stat(&full).await?;
            session_stats.push(SessionStat {
                id: id.to_string(),
                path: full,
                mtime_ms: stat.mtime_ms(),
            });
        }
        if session_stats.is_empty() {
            return Ok(Vec::new());
        }

        session_stats.sort_by_key(|s| std::cmp::Reverse(s.mtime_ms));

        // Group sessions by extracted cwd. `None` bucket = sessions without a cwd.
        let mut cwd_buckets: BTreeMap<String, CwdBucket> = BTreeMap::new();
        let mut unknown_cwd: Vec<SessionStat> = Vec::new();

        let ssh_mode = self.fs.kind() == FsKind::Ssh;
        for stat in session_stats {
            let cwd = self.extract_session_cwd(&stat.path).await;
            match cwd {
                Some(cwd) if !cwd.is_empty() => {
                    let bucket = cwd_buckets.entry(cwd.clone()).or_insert_with(|| CwdBucket {
                        cwd: PathBuf::from(&cwd),
                        session_ids: Vec::new(),
                        most_recent_ms: 0,
                        created_ms: i64::MAX,
                    });
                    bucket.session_ids.push(stat.id.clone());
                    bucket.most_recent_ms = bucket.most_recent_ms.max(stat.mtime_ms);
                    bucket.created_ms = bucket.created_ms.min(stat.mtime_ms);
                    if ssh_mode {
                        // SSH 模式下只抽第一个 session 的 cwd，其余全部挂到同一个 bucket。
                        // 注意：只要已经成功拿到一个 cwd，后面就不再读。
                    }
                }
                _ => unknown_cwd.push(stat),
            }
        }

        // 把 unknown_cwd 合并到 bucket：若存在唯一 cwd bucket，就并入；否则自己成一个
        // 退化为 decoded path 的 bucket。
        if !unknown_cwd.is_empty() {
            if cwd_buckets.len() == 1 {
                let bucket = cwd_buckets.values_mut().next().expect("one bucket");
                for stat in unknown_cwd.drain(..) {
                    bucket.most_recent_ms = bucket.most_recent_ms.max(stat.mtime_ms);
                    bucket.created_ms = bucket.created_ms.min(stat.mtime_ms);
                    bucket.session_ids.push(stat.id);
                }
            } else {
                let decoded = decode_path(dir_name);
                let fallback = cwd_buckets
                    .entry(decoded.to_string_lossy().into_owned())
                    .or_insert_with(|| CwdBucket {
                        cwd: decoded.clone(),
                        session_ids: Vec::new(),
                        most_recent_ms: 0,
                        created_ms: i64::MAX,
                    });
                for stat in unknown_cwd.drain(..) {
                    fallback.most_recent_ms = fallback.most_recent_ms.max(stat.mtime_ms);
                    fallback.created_ms = fallback.created_ms.min(stat.mtime_ms);
                    fallback.session_ids.push(stat.id);
                }
            }
        }

        let mut projects = Vec::with_capacity(cwd_buckets.len());
        let bucket_count = cwd_buckets.len();
        for (_key, mut bucket) in cwd_buckets {
            // Session id list 已经按 mtime 插入顺序（降序）排好。
            let sessions = std::mem::take(&mut bucket.session_ids);
            let id = if bucket_count > 1 {
                let session_set: BTreeSet<String> = sessions.iter().cloned().collect();
                let owned_ids: Vec<String> = session_set.into_iter().collect();
                self.registry.register(dir_name, &bucket.cwd, owned_ids)
            } else {
                dir_name.to_string()
            };
            let name = extract_project_name(&bucket.cwd);
            projects.push(Project {
                id,
                name,
                path: bucket.cwd,
                sessions,
                most_recent_session: Some(bucket.most_recent_ms),
                created_at: if bucket.created_ms == i64::MAX {
                    None
                } else {
                    Some(bucket.created_ms)
                },
            });
        }
        projects.sort_by(|a, b| {
            b.most_recent_session
                .unwrap_or(0)
                .cmp(&a.most_recent_session.unwrap_or(0))
        });
        Ok(projects)
    }

    async fn extract_session_cwd(&self, path: &Path) -> Option<String> {
        let lines = self
            .fs
            .read_lines_head(path, SESSION_HEAD_LINES)
            .await
            .ok()?;
        for (idx, line) in lines.iter().enumerate() {
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
}

struct SessionStat {
    id: String,
    path: PathBuf,
    mtime_ms: i64,
}

struct CwdBucket {
    cwd: PathBuf,
    session_ids: Vec<String>,
    most_recent_ms: i64,
    created_ms: i64,
}

// 避免 "unused import" 警告：当没有 SSH 实现时 `HashMap` 只在测试使用。
#[allow(dead_code)]
fn _assert_hash<T>(_: &HashMap<T, T>) {}

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
