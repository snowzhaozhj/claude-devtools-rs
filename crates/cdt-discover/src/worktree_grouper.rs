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
use futures::future::join_all;

/// 单 path 的 git 元数据合并结果。`LocalGitIdentityResolver` 用一次 `git rev-parse`
/// 同时取回 `--git-common-dir` / `--git-dir` / `--abbrev-ref HEAD`——避免每个 project
/// 串行 spawn 3 ~ 5 个 git 子进程（首屏冷启动主瓶颈）。trait 默认实现 fallback
/// 到三个独立调用，让 SSH / Fake impl 不必改。
///
/// `is_main_worktree` 在 path 自身非 git 仓库时保守取 `true`（对齐老
/// `LocalGitIdentityResolver::is_main_worktree` 失败保守 true 的行为）；fallback
/// 借 parent identity 的场景由 caller 显式覆写为 `false`（附加 worktree）。
#[derive(Debug, Clone)]
pub struct RepoLookup {
    pub identity: Option<RepositoryIdentity>,
    pub branch: Option<String>,
    pub is_main_worktree: bool,
}

impl Default for RepoLookup {
    fn default() -> Self {
        Self {
            identity: None,
            branch: None,
            is_main_worktree: true,
        }
    }
}

/// 抽象 git 身份识别，使得 SSH 版本可以直接替换。
#[async_trait]
pub trait GitIdentityResolver: Send + Sync {
    async fn resolve_identity(&self, path: &Path) -> Option<RepositoryIdentity>;
    async fn get_branch(&self, path: &Path) -> Option<String>;
    async fn is_main_worktree(&self, path: &Path) -> bool;

    /// 一次取回 `identity` + `branch` + `is_main_worktree`。默认实现 fallback 到三个
    /// 独立调用（Fake / SSH impl 走默认路径无需改造）；`LocalGitIdentityResolver`
    /// override 为单次 `git rev-parse` 拿三个参数，大幅减少子进程 spawn 数量。
    async fn resolve_all(&self, path: &Path) -> RepoLookup {
        let identity = self.resolve_identity(path).await;
        if identity.is_none() {
            return RepoLookup::default();
        }
        let branch = self.get_branch(path).await;
        let is_main = self.is_main_worktree(path).await;
        RepoLookup {
            identity,
            branch,
            is_main_worktree: is_main,
        }
    }
}

/// 本地实现：**纯 fs**——0 个 git 子进程。
///
/// 历史上 `LocalGitIdentityResolver` 每个 project 串行 spawn 3~5 个
/// `git rev-parse` 子进程，27 个 project 累计 ~3700ms 卡冷启动。改造
/// 后所有元数据从 `.git` / `HEAD` 文件直接读取——子进程换 syscall，
/// 数量级压缩到 ~50ms（详见 `perf_cold_scan` bench）。
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalGitIdentityResolver;

impl LocalGitIdentityResolver {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GitIdentityResolver for LocalGitIdentityResolver {
    async fn resolve_identity(&self, path: &Path) -> Option<RepositoryIdentity> {
        self.resolve_all(path).await.identity
    }

    async fn get_branch(&self, path: &Path) -> Option<String> {
        self.resolve_all(path).await.branch
    }

    async fn is_main_worktree(&self, path: &Path) -> bool {
        self.resolve_all(path).await.is_main_worktree
    }

    /// **0 个 git 子进程**，纯 fs 直接读 `.git` / `HEAD`。
    ///
    /// 老 grouper 每个 project 串行 spawn 3 ~ 5 个 `git rev-parse` 子进程
    /// (`resolve_identity` / `get_branch` / `is_main_worktree` 各调一次甚至重复调），
    /// 27 project 累计 ~3700ms 卡冷启动首屏。Git 子进程是杀鸡用牛刀——
    /// 我们要的三个值（git-common-dir / git-dir / HEAD ref）都直接落在
    /// 文件系统里，单次 `metadata` + `read_to_string` 微秒级就能拿到，
    /// 27 project 并发后总耗时降到 ~50ms 量级（详见 `perf_cold_scan` bench）。
    ///
    /// 算法：
    /// 1. 从 `path` 向上 walk 直到找到 `.git` 条目（命中即 git working tree 内）
    /// 2. `.git` 是目录 → main worktree：`common_dir = git_dir = <repo>/.git`
    /// 3. `.git` 是文件（gitlink）→ 附加 worktree：parse `gitdir: <abs>` 取 `git_dir`，
    ///    向上两级 `parent().parent()` 即 `common_dir`（`<common>/worktrees/<name>` → `<common>`）
    /// 4. branch 来自 `<git_dir>/HEAD`，格式 `ref: refs/heads/<branch>` 或裸 commit hash（detached）
    /// 5. `identity = canonical(common_dir)` 字符串，`name = canonical.parent().file_name()`
    async fn resolve_all(&self, path: &Path) -> RepoLookup {
        resolve_all_fs(path).await.unwrap_or_default()
    }
}

/// 纯 fs 解析 `path` 的 git 元数据；失败任一步返 `None`，caller 走 `RepoLookup::default()`。
async fn resolve_all_fs(path: &Path) -> Option<RepoLookup> {
    let (git_dir, common_dir, is_main) = locate_git_dirs(path).await?;

    let canonical_common = tokio::fs::canonicalize(&common_dir)
        .await
        .unwrap_or(common_dir);
    // identity id / name 与原 `git rev-parse --git-common-dir` 路径等价：
    // canonical 后取 parent.file_name 作为 repo name（main worktree 时
    // `<repo>/.git` 的 parent 就是 `<repo>`，file_name 就是 repo 目录名）。
    let name = canonical_common
        .parent()
        .and_then(|p| p.file_name())
        .map_or_else(
            || canonical_common.to_string_lossy().into_owned(),
            |s| s.to_string_lossy().into_owned(),
        );
    let identity = RepositoryIdentity {
        id: canonical_common.to_string_lossy().into_owned(),
        name,
    };

    let head_path = git_dir.join("HEAD");
    let branch = tokio::fs::read_to_string(&head_path)
        .await
        .ok()
        .and_then(|s| parse_head_branch(&s));

    Some(RepoLookup {
        identity: Some(identity),
        branch,
        is_main_worktree: is_main,
    })
}

/// 向上 walk 找到 `.git`；返回 `(git_dir, common_dir, is_main_worktree)`。
/// `.git` 是目录 → main，`git_dir == common_dir`。
/// `.git` 是文件 → 解析 gitlink，`git_dir = <abs from gitlink>`、`common_dir = git_dir.parent().parent()`。
async fn locate_git_dirs(start: &Path) -> Option<(PathBuf, PathBuf, bool)> {
    let mut current = start.to_path_buf();
    loop {
        let dot_git = current.join(".git");
        match tokio::fs::metadata(&dot_git).await {
            Ok(meta) if meta.is_dir() => {
                return Some((dot_git.clone(), dot_git, true));
            }
            Ok(meta) if meta.is_file() => {
                let content = tokio::fs::read_to_string(&dot_git).await.ok()?;
                let gitdir = parse_gitlink_dir(&content, &current)?;
                // worktree 的 gitdir 形如 `<common>/worktrees/<name>`；
                // 取两级 parent 拿 common dir。submodule 等其他 gitlink 形态
                // 不在 worktree 分组语义内（罕见）—— 当前实现仍按 worktree 处理，
                // 与 git 命令的语义对齐。
                let common = gitdir
                    .parent()
                    .and_then(Path::parent)
                    .map_or_else(|| gitdir.clone(), Path::to_path_buf);
                return Some((gitdir, common, false));
            }
            _ => {
                if !current.pop() {
                    return None;
                }
            }
        }
    }
}

/// 解析 `.git` 文件内容 `gitdir: <path>`；path 相对时按 `base` 拼。
fn parse_gitlink_dir(content: &str, base: &Path) -> Option<PathBuf> {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("gitdir:") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return None;
            }
            let p = Path::new(trimmed);
            return Some(if p.is_absolute() {
                p.to_path_buf()
            } else {
                base.join(p)
            });
        }
    }
    None
}

/// 解析 `HEAD` 文件内容：
/// - `ref: refs/heads/<branch>` → `Some("<branch>")`
/// - 裸 commit hash（detached HEAD）→ `Some("HEAD")` 对齐 `git rev-parse --abbrev-ref HEAD` 行为
/// - 空 / 解析失败 → `None`
fn parse_head_branch(content: &str) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("ref:") {
        let r = rest.trim();
        if let Some(branch) = r.strip_prefix("refs/heads/") {
            if !branch.is_empty() {
                return Some(branch.to_owned());
            }
        }
        // 其它 ref 形态（refs/tags/... 等罕见）原样返回最后一段
        return r.rsplit('/').next().map(str::to_owned);
    }
    // detached HEAD：git rev-parse --abbrev-ref HEAD 返 "HEAD"
    Some("HEAD".to_owned())
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

        // 并发解析每个 project 的 git 元数据：合并 `resolve_identity` + `get_branch`
        // + `is_main_worktree` 为单次 `resolve_all`，再用 `join_all` 同时跑所有
        // project。本仓首屏 27 project 实测：串行 5×27=135 spawn → 并发 27 spawn
        // 大幅压低冷启动 grouper 阶段耗时（详见 `cdt-api/tests/perf_cold_scan.rs`）。
        let lookups = join_all(projects.iter().map(|project| {
            let project_path = project.path.clone();
            async move {
                let primary = self.git.resolve_all(&project_path).await;
                if primary.identity.is_some() {
                    return primary;
                }
                // path 自身已经不存在 / 不是 git 仓库时，尝试推断 parent repo——
                // 例：被 prune 掉的 `.claude/worktrees/<name>` 仍能挂到 parent
                // repo 的 group。borrow parent 的 identity，但 `branch` 与
                // `is_main_worktree` 对原 path 不再适用（保持与老逻辑等价：
                // identity 来自 fallback 时 is_main = false、branch = None）。
                let Some(parent) = infer_parent_repo_from_worktree_path(&project_path) else {
                    return primary;
                };
                let parent_lookup = self.git.resolve_all(&parent).await;
                RepoLookup {
                    identity: parent_lookup.identity,
                    branch: None,
                    is_main_worktree: false,
                }
            }
        }))
        .await;

        let mut buckets: BTreeMap<String, Bucket> = BTreeMap::new();
        for (project, lookup) in projects.into_iter().zip(lookups) {
            let RepoLookup {
                identity,
                branch,
                is_main_worktree: is_main,
            } = lookup;
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

fn infer_parent_repo_from_worktree_path(path: &Path) -> Option<PathBuf> {
    let mut components = path.components().peekable();
    let mut parent = PathBuf::new();
    while let Some(component) = components.next() {
        if component.as_os_str() == ".claude" {
            let Some(next) = components.peek() else {
                parent.push(component.as_os_str());
                continue;
            };
            if next.as_os_str() == "worktrees" {
                return Some(parent);
            }
        }
        parent.push(component.as_os_str());
    }
    None
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
    async fn removed_claude_worktree_uses_parent_repo_identity() {
        let identity = RepositoryIdentity {
            id: "repo-1".into(),
            name: "repo-1".into(),
        };
        let mut entries = HashMap::new();
        entries.insert(
            PathBuf::from("/repo"),
            FakeGitEntry {
                identity: Some(identity.clone()),
                branch: Some("main".into()),
                is_main: true,
            },
        );

        let grouper = WorktreeGrouper::new(FakeGitIdentityResolver { entries });
        let groups = grouper
            .group_by_repository(vec![
                proj("main", "/repo", 100),
                proj("removed", "/repo/.claude/worktrees/old-feature", 50),
            ])
            .await;

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].worktrees.len(), 2);
        assert_eq!(groups[0].identity, Some(identity));
        let removed = groups[0]
            .worktrees
            .iter()
            .find(|w| w.id == "removed")
            .unwrap();
        assert!(!removed.is_main_worktree);
        assert_eq!(removed.git_branch, None);
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

    // =========================================================================
    // 纯 fs `LocalGitIdentityResolver` 单元测试：无需真跑 `git init`，手工
    // 构造 `.git` 目录/文件 + `HEAD` 模拟 git 元数据布局。
    // =========================================================================

    use tempfile::tempdir;

    #[tokio::test]
    async fn fs_resolver_detects_main_worktree() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let git = repo.join(".git");
        tokio::fs::create_dir_all(&git).await.unwrap();
        tokio::fs::write(git.join("HEAD"), "ref: refs/heads/main\n")
            .await
            .unwrap();

        let resolver = LocalGitIdentityResolver::new();
        let lookup = resolver.resolve_all(&repo).await;
        assert!(lookup.identity.is_some());
        assert_eq!(lookup.branch.as_deref(), Some("main"));
        assert!(lookup.is_main_worktree);
        // identity.id 是 canonical(.git) 路径——`.git` 是 hidden file 没 extension，
        // 用 file_name 比对
        let identity = lookup.identity.unwrap();
        assert_eq!(
            Path::new(&identity.id).file_name(),
            Some(std::ffi::OsStr::new(".git"))
        );
        assert_eq!(identity.name, "repo");
    }

    #[tokio::test]
    async fn fs_resolver_detects_subdir_inside_worktree() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let sub = repo.join("src").join("inner");
        let git = repo.join(".git");
        tokio::fs::create_dir_all(&sub).await.unwrap();
        tokio::fs::create_dir_all(&git).await.unwrap();
        tokio::fs::write(git.join("HEAD"), "ref: refs/heads/feature/x\n")
            .await
            .unwrap();

        let resolver = LocalGitIdentityResolver::new();
        let lookup = resolver.resolve_all(&sub).await;
        assert_eq!(lookup.branch.as_deref(), Some("feature/x"));
        assert!(lookup.is_main_worktree);
    }

    #[tokio::test]
    async fn fs_resolver_detects_linked_worktree() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let main_git = repo.join(".git");
        let wt_git_dir = main_git.join("worktrees").join("feat");
        tokio::fs::create_dir_all(&wt_git_dir).await.unwrap();
        tokio::fs::write(main_git.join("HEAD"), "ref: refs/heads/main\n")
            .await
            .unwrap();
        tokio::fs::write(wt_git_dir.join("HEAD"), "ref: refs/heads/feat\n")
            .await
            .unwrap();

        // 附加 worktree 目录：`.git` 是 file，内容指向 wt_git_dir
        let wt = dir.path().join("feat");
        tokio::fs::create_dir_all(&wt).await.unwrap();
        let gitlink = format!("gitdir: {}\n", wt_git_dir.display());
        tokio::fs::write(wt.join(".git"), gitlink).await.unwrap();

        let resolver = LocalGitIdentityResolver::new();
        let lookup = resolver.resolve_all(&wt).await;
        assert!(lookup.identity.is_some());
        assert_eq!(lookup.branch.as_deref(), Some("feat"));
        assert!(!lookup.is_main_worktree);
        // identity.id 应该指向 main repo 的 .git（common dir），不是 worktree-specific dir
        let identity = lookup.identity.unwrap();
        assert_eq!(
            Path::new(&identity.id).file_name(),
            Some(std::ffi::OsStr::new(".git")),
            "identity should point to common .git dir, got {}",
            identity.id
        );
    }

    #[tokio::test]
    async fn fs_resolver_returns_default_for_non_git_dir() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("scratch");
        tokio::fs::create_dir_all(&path).await.unwrap();
        let resolver = LocalGitIdentityResolver::new();
        let lookup = resolver.resolve_all(&path).await;
        assert!(lookup.identity.is_none());
        assert!(lookup.branch.is_none());
        // default 保守 true（对齐老 is_main_worktree 失败保守 true 行为）
        assert!(lookup.is_main_worktree);
    }

    #[tokio::test]
    async fn fs_resolver_detached_head_returns_head_sentinel() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let git = repo.join(".git");
        tokio::fs::create_dir_all(&git).await.unwrap();
        // detached HEAD：HEAD 文件是裸 commit hash
        tokio::fs::write(git.join("HEAD"), "abcdef1234567890\n")
            .await
            .unwrap();

        let resolver = LocalGitIdentityResolver::new();
        let lookup = resolver.resolve_all(&repo).await;
        // 对齐 `git rev-parse --abbrev-ref HEAD` 在 detached 时返 "HEAD"
        assert_eq!(lookup.branch.as_deref(), Some("HEAD"));
        assert!(lookup.is_main_worktree);
    }

    #[test]
    fn parse_head_branch_handles_common_formats() {
        assert_eq!(
            parse_head_branch("ref: refs/heads/main\n").as_deref(),
            Some("main")
        );
        assert_eq!(
            parse_head_branch("ref: refs/heads/feature/x\n").as_deref(),
            Some("feature/x")
        );
        // detached HEAD
        assert_eq!(parse_head_branch("abc123\n").as_deref(), Some("HEAD"));
        assert!(parse_head_branch("").is_none());
        assert!(parse_head_branch("   \n").is_none());
    }

    #[test]
    fn parse_gitlink_dir_resolves_absolute_and_relative() {
        let base = Path::new("/tmp/wt");
        assert_eq!(
            parse_gitlink_dir("gitdir: /repo/.git/worktrees/feat\n", base),
            Some(PathBuf::from("/repo/.git/worktrees/feat"))
        );
        assert_eq!(
            parse_gitlink_dir("gitdir: ../repo/.git/worktrees/feat\n", base),
            Some(PathBuf::from("/tmp/wt/../repo/.git/worktrees/feat"))
        );
        assert!(parse_gitlink_dir("not a gitlink\n", base).is_none());
    }
}
