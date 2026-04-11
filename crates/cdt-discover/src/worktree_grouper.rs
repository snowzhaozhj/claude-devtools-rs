//! 按 git repo 把 projects 分组。
//!
//! 同一 repo 的多个 worktree 会合并进同一个 `RepositoryGroup`；非 git
//! 目录各自成组。真实 git 调用由 `GitIdentityResolver` trait 接管，
//! `LocalGitIdentityResolver` 会 spawn `git` 子进程，SSH 版本留给
//! `port-ssh-remote-context`。
//!
//! Spec：`openspec/specs/project-discovery/spec.md` 的
//! `Group projects by git worktree` Requirement。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use cdt_core::{Project, RepositoryGroup, RepositoryIdentity, Worktree};

/// 抽象 git 身份识别，使得 SSH 版本可以直接替换。
#[async_trait]
pub trait GitIdentityResolver: Send + Sync {
    async fn resolve_identity(&self, path: &Path) -> Option<RepositoryIdentity>;
    async fn get_branch(&self, path: &Path) -> Option<String>;
    async fn is_main_worktree(&self, path: &Path) -> bool;
}

/// 本地实现：shell out 到 `git`。子进程失败（非 git 目录等）一律返回 `None`。
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalGitIdentityResolver;

impl LocalGitIdentityResolver {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    async fn run_git(path: &Path, args: &[&str]) -> Option<String> {
        let output = tokio::process::Command::new("git")
            .current_dir(path)
            .args(args)
            .output()
            .await
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8(output.stdout).ok()?;
        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

#[async_trait]
impl GitIdentityResolver for LocalGitIdentityResolver {
    async fn resolve_identity(&self, path: &Path) -> Option<RepositoryIdentity> {
        let common_dir = Self::run_git(path, &["rev-parse", "--git-common-dir"]).await?;
        // common_dir 可能是相对路径（相对于 path），规范化一下。
        let abs = if Path::new(&common_dir).is_absolute() {
            PathBuf::from(&common_dir)
        } else {
            path.join(&common_dir)
        };
        let canonical = tokio::fs::canonicalize(&abs).await.unwrap_or(abs);
        let name = canonical.parent().and_then(|p| p.file_name()).map_or_else(
            || canonical.to_string_lossy().into_owned(),
            |s| s.to_string_lossy().into_owned(),
        );
        Some(RepositoryIdentity {
            id: canonical.to_string_lossy().into_owned(),
            name,
        })
    }

    async fn get_branch(&self, path: &Path) -> Option<String> {
        Self::run_git(path, &["rev-parse", "--abbrev-ref", "HEAD"]).await
    }

    async fn is_main_worktree(&self, path: &Path) -> bool {
        let Some(git_dir) = Self::run_git(path, &["rev-parse", "--git-dir"]).await else {
            return true;
        };
        let Some(common_dir) = Self::run_git(path, &["rev-parse", "--git-common-dir"]).await else {
            return true;
        };
        // 主 worktree 的 `git-dir == git-common-dir`；附加 worktree 的 git-dir
        // 指向 `common/worktrees/<name>`。
        git_dir == common_dir
    }
}

/// 分组器。
pub struct WorktreeGrouper<G: GitIdentityResolver> {
    git: G,
}

impl<G: GitIdentityResolver> WorktreeGrouper<G> {
    pub fn new(git: G) -> Self {
        Self { git }
    }

    pub async fn group_by_repository(&self, projects: Vec<Project>) -> Vec<RepositoryGroup> {
        if projects.is_empty() {
            return Vec::new();
        }

        let mut buckets: BTreeMap<String, Bucket> = BTreeMap::new();
        for project in projects {
            let identity = self.git.resolve_identity(&project.path).await;
            let branch = self.git.get_branch(&project.path).await;
            let is_main = self.git.is_main_worktree(&project.path).await;
            let group_id = identity
                .as_ref()
                .map_or_else(|| project.id.clone(), |i| i.id.clone());

            let bucket = buckets.entry(group_id.clone()).or_insert_with(|| Bucket {
                id: group_id.clone(),
                identity: identity.clone(),
                worktrees: Vec::new(),
            });
            // 保留第一次遇到的非 None identity。
            if bucket.identity.is_none() {
                bucket.identity = identity;
            }
            bucket.worktrees.push(Worktree {
                id: project.id,
                path: project.path.clone(),
                name: project.name,
                git_branch: branch,
                is_main_worktree: is_main,
                sessions: project.sessions,
                created_at: project.created_at,
                most_recent_session: project.most_recent_session,
            });
        }

        let mut groups: Vec<RepositoryGroup> = buckets
            .into_values()
            .filter_map(|mut b| {
                if b.worktrees.is_empty() {
                    return None;
                }
                b.worktrees
                    .sort_by(|a, c| match (a.is_main_worktree, c.is_main_worktree) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => c
                            .most_recent_session
                            .unwrap_or(0)
                            .cmp(&a.most_recent_session.unwrap_or(0)),
                    });
                let total_sessions: usize = b.worktrees.iter().map(|w| w.sessions.len()).sum();
                let most_recent = b
                    .worktrees
                    .iter()
                    .filter_map(|w| w.most_recent_session)
                    .max();
                let name = b
                    .identity
                    .as_ref()
                    .map_or_else(|| b.worktrees[0].name.clone(), |i| i.name.clone());
                Some(RepositoryGroup {
                    id: b.id,
                    identity: b.identity,
                    name,
                    worktrees: b.worktrees,
                    most_recent_session: most_recent,
                    total_sessions,
                })
            })
            .collect();

        groups.sort_by(|a, b| {
            b.most_recent_session
                .unwrap_or(0)
                .cmp(&a.most_recent_session.unwrap_or(0))
        });
        groups
    }
}

struct Bucket {
    id: String,
    identity: Option<RepositoryIdentity>,
    worktrees: Vec<Worktree>,
}

#[cfg(test)]
pub(crate) struct FakeGitIdentityResolver {
    pub entries: std::collections::HashMap<PathBuf, FakeGitEntry>,
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct FakeGitEntry {
    pub identity: Option<RepositoryIdentity>,
    pub branch: Option<String>,
    pub is_main: bool,
}

#[cfg(test)]
#[async_trait]
impl GitIdentityResolver for FakeGitIdentityResolver {
    async fn resolve_identity(&self, path: &Path) -> Option<RepositoryIdentity> {
        self.entries.get(path).and_then(|e| e.identity.clone())
    }
    async fn get_branch(&self, path: &Path) -> Option<String> {
        self.entries.get(path).and_then(|e| e.branch.clone())
    }
    async fn is_main_worktree(&self, path: &Path) -> bool {
        self.entries.get(path).is_some_and(|e| e.is_main)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn proj(id: &str, path: &str, recent: i64) -> Project {
        Project {
            id: id.to_string(),
            name: id.to_string(),
            path: PathBuf::from(path),
            sessions: vec!["s".to_string()],
            most_recent_session: Some(recent),
            created_at: None,
        }
    }

    #[tokio::test]
    async fn two_worktrees_share_one_group() {
        let identity = RepositoryIdentity {
            id: "repo-1".into(),
            name: "repo-1".into(),
        };
        let mut entries = HashMap::new();
        entries.insert(
            PathBuf::from("/repo/main"),
            FakeGitEntry {
                identity: Some(identity.clone()),
                branch: Some("main".into()),
                is_main: true,
            },
        );
        entries.insert(
            PathBuf::from("/repo/wt1"),
            FakeGitEntry {
                identity: Some(identity.clone()),
                branch: Some("feature".into()),
                is_main: false,
            },
        );

        let grouper = WorktreeGrouper::new(FakeGitIdentityResolver { entries });
        let groups = grouper
            .group_by_repository(vec![
                proj("wt1", "/repo/wt1", 100),
                proj("main", "/repo/main", 50),
            ])
            .await;

        assert_eq!(groups.len(), 1);
        let group = &groups[0];
        assert_eq!(group.worktrees.len(), 2);
        assert!(group.worktrees[0].is_main_worktree);
        assert_eq!(group.worktrees[1].id, "wt1");
        assert_eq!(group.total_sessions, 2);
    }

    #[tokio::test]
    async fn standalone_project_becomes_single_member_group() {
        let entries = HashMap::new();
        let grouper = WorktreeGrouper::new(FakeGitIdentityResolver { entries });
        let groups = grouper
            .group_by_repository(vec![proj("a", "/tmp/a", 10)])
            .await;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].worktrees.len(), 1);
        assert!(groups[0].identity.is_none());
    }
}
