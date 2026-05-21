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
    /// `path` 自身就是 working tree 根目录（walk-up 0 步即命中 `.git`）。
    /// 区别于 `is_main_worktree`：子目录 cwd walk-up 到主 `.git` 时
    /// `is_main_worktree=true` 但 `is_repo_root=false`，避免 UI 把主仓
    /// 子目录 cwd 误标为独立 "main" 撞名。
    /// change `simplify-repository-as-project::D1`。
    pub is_repo_root: bool,
}

impl Default for RepoLookup {
    fn default() -> Self {
        Self {
            identity: None,
            branch: None,
            is_main_worktree: true,
            is_repo_root: false,
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
            // trait 默认实现无法判定 working tree 根；保守 false。Fake / SSH impl
            // 需要更精确语义时应 override `resolve_all`。
            is_repo_root: false,
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
    let (git_dir, common_dir, is_main, is_repo_root) = locate_git_dirs(path).await?;

    // Windows 兼容：`tokio::fs::canonicalize` 返 `\\?\C:\...` UNC 前缀，会让下游
    // `strip_prefix(repo_root)` 永远失败。`dunce::canonicalize` 在 Unix 等价
    // `fs::canonicalize`、在 Windows 上自动剥 UNC，跨平台一致。包 `spawn_blocking`
    // 避免阻塞 tokio worker。
    let canonical_common = {
        let common_dir = common_dir.clone();
        tokio::task::spawn_blocking(move || dunce::canonicalize(&common_dir).unwrap_or(common_dir))
            .await
            .ok()?
    };
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
        is_repo_root,
    })
}

/// 向上 walk 找到 `.git`；返回 `(git_dir, common_dir, is_main_worktree, is_repo_root)`。
///
/// `.git` 是目录 → main，`git_dir == common_dir`。
/// `.git` 是文件（gitlink） → 进一步看 `<gitdir>/commondir`：
///   - 文件存在 → linked worktree（git 标准约定）：`common_dir` 从该文件读取，`is_main=false`
///   - 文件不存在 → submodule / 其它 gitlink 形态：`common_dir = gitdir`，`is_main=true`
///     （codex 二审 Bug 1：submodule 的 `.git` 也是 gitlink，但其 common dir 是自身 gitdir，
///     不是上两级，否则会把 submodule 错归到父仓库/错误的 common dir）
///
/// `is_repo_root` 语义：`start` 自身就是 walk-up 命中 `.git` 的目录（即 working
/// tree 根 / linked worktree 根 / submodule 根），未 pop 过；子目录 cwd walk-up
/// 多步命中 `.git` 时该字段为 `false`，避免 UI 把同 repo 的不同 cwd 都标 "main"
/// 撞名。change `simplify-repository-as-project::D1`。
///
/// 已知 trade-off（claude session 场景下概率近 0，未实现）：
/// - bare repo（用户 cd 进 bare repo 跑 claude）：当前返回 default 走非 git 路径
/// - `GIT_COMMON_DIR` 环境变量覆盖：纯 fs 实现无感（旧 `git rev-parse` 会受环境变量覆盖）
async fn locate_git_dirs(start: &Path) -> Option<(PathBuf, PathBuf, bool, bool)> {
    // 入口先校验 start path 自身存在；否则向上 walk 时会越过已被 git prune 掉的
    // worktree 目录（`<repo>/.claude/worktrees/<old-name>/`，path 已不存在），
    // 最终撞到父 repo 的 `.git` 把它当成自身 working tree，于是显示父 repo
    // 当前分支（典型为 `main`）。spec `project-discovery` §"历史 worktree
    // path 解析 branch 失败 SHALL 为 None"硬要求此情况让 caller 走 fallback。
    if tokio::fs::metadata(start).await.is_err() {
        return None;
    }
    let mut current = start.to_path_buf();
    let mut walked = false;
    loop {
        let dot_git = current.join(".git");
        match tokio::fs::metadata(&dot_git).await {
            Ok(meta) if meta.is_dir() => {
                return Some((dot_git.clone(), dot_git, true, !walked));
            }
            Ok(meta) if meta.is_file() => {
                // `.git` 是 file（linked worktree / submodule）—— 按 spec
                // `simplify-repository-as-project::D1`，is_repo_root **仅**当
                // `.git` 是目录（即主 working tree 根）才为 true。linked
                // worktree / submodule 即便 start 就是它们的根目录，也算独
                // 立 working tree 视角，**不**算所属 group 的 repo root，
                // 保证同 group 内 is_repo_root 唯一性。
                let content = tokio::fs::read_to_string(&dot_git).await.ok()?;
                let gitdir = parse_gitlink_dir(&content, &current)?;
                // 区分 linked worktree（has `commondir` file）vs submodule（no `commondir`）。
                // git 约定：`<gitdir>/commondir` **文件存在**即表示这是 linked worktree，
                // 内是 common dir 路径（相对则相对 `<gitdir>` 解析）。
                //
                // 错误细分（codex 二审第二轮 Bug 1）：
                // - `NotFound` → submodule 路径，common = gitdir
                // - 其他 IO 错误（如权限） → 视为不可读，整体返 None
                // - 文件存在但内容 trim 后为空 → 非法 linked worktree，整体返 None
                //   （不能 fallthrough 到 submodule，否则会把损坏的 linked worktree 误分类）
                let commondir_file = gitdir.join("commondir");
                match tokio::fs::read_to_string(&commondir_file).await {
                    Ok(common_content) => {
                        let raw = common_content.trim();
                        if raw.is_empty() {
                            return None;
                        }
                        let common_path = Path::new(raw);
                        let common = if crate::looks_like_absolute_path(raw) {
                            common_path.to_path_buf()
                        } else {
                            gitdir.join(common_path)
                        };
                        return Some((gitdir, common, false, false));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        // 没有 commondir 文件 → submodule 或独立 gitlink，common = gitdir
                        return Some((gitdir.clone(), gitdir, true, false));
                    }
                    Err(_) => {
                        // 其他 IO 错误（权限 / 损坏） → 跳过该项，不臆测语义
                        return None;
                    }
                }
            }
            _ => {
                walked = true;
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
            return Some(if crate::looks_like_absolute_path(trimmed) {
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
/// - `ref: refs/tags/...` / `refs/remotes/...` 等**非分支引用** → `None`
///   （tag / remote ref 不是分支名，sidebar 不应当成分支 chip 展示）
/// - 裸 commit hash（detached HEAD）→ `None`（字面 "HEAD" 对用户无意义，
///   `sidebar-navigation` §"gitBranch 为 null SHALL NOT 渲染 chip" 让 UI 自动收起）
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
        // 其它 ref 形态（refs/tags/... / refs/remotes/... 等）—— 不是分支引用，
        // codex 二审 Bug：原本 rsplit 拿最后一段当分支返回会让 `refs/tags/v1` 在
        // sidebar 显示为 "v1" 分支 chip，对用户具误导性。一律返 None。
        return None;
    }
    // detached HEAD：HEAD 文件是裸 commit hash。原版 Claude Code 在 detached
    // 时 JSONL 会把字面字符串 "HEAD" 写进 gitBranch 字段，我们这里同步返 None
    // 让 UI 在无可读分支名时不渲染分支 chip。
    None
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
                // identity 来自 fallback 时 is_main = false、branch = None、
                // is_repo_root = false）。
                let Some(parent) = infer_parent_repo_from_worktree_path(&project_path) else {
                    return primary;
                };
                let parent_lookup = self.git.resolve_all(&parent).await;
                RepoLookup {
                    identity: parent_lookup.identity,
                    branch: None,
                    is_main_worktree: false,
                    is_repo_root: false,
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
                is_repo_root,
            } = lookup;
            let group_id = identity
                .as_ref()
                .map_or_else(|| project.id.clone(), |i| i.id.clone());

            // cwd_relative_to_repo_root 纯字符串运算 0 syscall：
            // identity.id 已经是 canonical `<repo>/.git`，strip `/.git` suffix
            // 得 repo 根；project.path 减去前缀就是相对路径。
            // change `simplify-repository-as-project::D2`。
            let cwd_relative_to_repo_root =
                compute_cwd_relative_to_repo_root(identity.as_ref(), project.path.as_path());

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
                is_repo_root,
                cwd_relative_to_repo_root,
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
                // 排序优先级：is_repo_root 优先（repo 根排前）→ is_main_worktree
                // 优先 → most_recent_session 倒序。change
                // `simplify-repository-as-project::D1`：避免主仓 cwd 与
                // `crates/` 子目录 cwd 都判 is_main_worktree=true 时撞名首位。
                b.worktrees.sort_by(|a, c| {
                    c.is_repo_root
                        .cmp(&a.is_repo_root)
                        .then_with(|| c.is_main_worktree.cmp(&a.is_main_worktree))
                        .then_with(|| {
                            c.most_recent_session
                                .unwrap_or(0)
                                .cmp(&a.most_recent_session.unwrap_or(0))
                        })
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

/// 纯字符串运算（0 syscall / 0 spawn）：从 identity.id（canonical
/// `<repo>/.git`）反推 repo 根，再让 `project_path` strip 前缀。repo 根
/// 自身或 `strip_prefix` 失败 → `None`。
/// change `simplify-repository-as-project::D2`。
///
/// Windows 兼容：strip `.git` 后缀走 `Path::parent`（跨平台分隔符），
/// `strip_prefix` 走 `Path::strip_prefix`，避免硬编码 `/`。
fn compute_cwd_relative_to_repo_root(
    identity: Option<&RepositoryIdentity>,
    project_path: &Path,
) -> Option<String> {
    let identity = identity?;
    let common_dir = Path::new(&identity.id);
    // canonical 后的 `<repo>/.git` 的 parent 就是 repo 根；submodule 的
    // `<parent>/.git/modules/<name>` 的 parent 是 `<parent>/.git/modules`
    // 不构成 working tree 根——此时 strip_prefix 失败 → None，符合预期。
    let repo_root = common_dir.parent()?;
    let relative = project_path.strip_prefix(repo_root).ok()?;
    // Windows 兼容：`relative.to_string_lossy()` 在 Windows 输出 `crates\subdir`
    // 反斜杠形态，跨平台 IPC payload 会分叉。归一为 `/` 与前端期望对齐。
    let s = relative.to_string_lossy().replace('\\', "/");
    if s.is_empty() { None } else { Some(s) }
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
            distinct_cwds: Vec::new(),
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
        // linked worktree 的 gitdir 内必须有 `commondir` 文件指向 common git dir
        // （git 标准约定，区分 linked worktree vs submodule）
        let commondir_relative = "../..";
        tokio::fs::write(wt_git_dir.join("commondir"), commondir_relative)
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
    async fn fs_resolver_treats_submodule_as_independent_repo() {
        // codex 二审 Bug 1：submodule 的 `.git` 文件指向 `<parent>/.git/modules/<name>`，
        // 但 **不含** `commondir` 文件——不应被误归到 parent worktree 的 common dir。
        let dir = tempdir().unwrap();
        let parent = dir.path().join("parent");
        let parent_git = parent.join(".git");
        let submodule_git_dir = parent_git.join("modules").join("sub");
        tokio::fs::create_dir_all(&submodule_git_dir).await.unwrap();
        tokio::fs::write(parent_git.join("HEAD"), "ref: refs/heads/main\n")
            .await
            .unwrap();
        tokio::fs::write(
            submodule_git_dir.join("HEAD"),
            "ref: refs/heads/sub-branch\n",
        )
        .await
        .unwrap();
        // 注意：submodule 的 gitdir 内 **不写** `commondir` 文件

        let sub = parent.join("sub");
        tokio::fs::create_dir_all(&sub).await.unwrap();
        let gitlink = format!("gitdir: {}\n", submodule_git_dir.display());
        tokio::fs::write(sub.join(".git"), gitlink).await.unwrap();

        let resolver = LocalGitIdentityResolver::new();
        let lookup = resolver.resolve_all(&sub).await;
        let identity = lookup.identity.expect("submodule should resolve identity");
        // submodule common dir 是其自身 gitdir（`.git/modules/sub`），
        // 不是 parent 的 `.git`——否则会把 submodule 错归到父仓库 group
        assert_eq!(
            Path::new(&identity.id).file_name(),
            Some(std::ffi::OsStr::new("sub")),
            "submodule identity should point to its own gitdir, got {}",
            identity.id
        );
        assert_eq!(lookup.branch.as_deref(), Some("sub-branch"));
        // submodule 在自身视角是 main（不是 linked worktree）
        assert!(lookup.is_main_worktree);
    }

    /// 端到端验证 grouper：main worktree + linked worktree 跑完整 grouping，
    /// 确认两者归到同一个 `RepositoryGroup`、不被 canonicalize 差异分裂。
    ///
    /// codex 二审第二轮 Bug 3：linked worktree 的 commondir 相对路径
    /// （`../..`）在 macOS `/var` vs `/private/var` symlink 下 canonicalize
    /// 行为可能与 main worktree 直接 canonicalize 不同——两个 identity.id
    /// 字符串不等会让 grouper 分进两个 bucket，破坏 worktree 分组。
    #[tokio::test]
    async fn grouper_keeps_main_and_linked_worktree_in_one_bucket() {
        let dir = tempdir().unwrap();
        let main_repo = dir.path().join("main_repo");
        let main_git = main_repo.join(".git");
        let wt_git_dir = main_git.join("worktrees").join("feat");
        tokio::fs::create_dir_all(&wt_git_dir).await.unwrap();
        tokio::fs::write(main_git.join("HEAD"), "ref: refs/heads/main\n")
            .await
            .unwrap();
        tokio::fs::write(wt_git_dir.join("HEAD"), "ref: refs/heads/feat\n")
            .await
            .unwrap();
        // linked worktree 标志文件 `commondir`：内容 `../..` 指回 main `.git`
        tokio::fs::write(wt_git_dir.join("commondir"), "../..\n")
            .await
            .unwrap();

        let wt_dir = dir.path().join("feat_worktree");
        tokio::fs::create_dir_all(&wt_dir).await.unwrap();
        let gitlink = format!("gitdir: {}\n", wt_git_dir.display());
        tokio::fs::write(wt_dir.join(".git"), gitlink)
            .await
            .unwrap();

        let projects = vec![
            Project {
                id: "main".into(),
                name: "main_repo".into(),
                path: main_repo.clone(),
                sessions: vec!["s1".into()],
                most_recent_session: Some(100),
                created_at: None,
                distinct_cwds: Vec::new(),
            },
            Project {
                id: "feat".into(),
                name: "feat_worktree".into(),
                path: wt_dir.clone(),
                sessions: vec!["s2".into()],
                most_recent_session: Some(200),
                created_at: None,
                distinct_cwds: Vec::new(),
            },
        ];
        let grouper = WorktreeGrouper::new(LocalGitIdentityResolver::new());
        let groups = grouper.group_by_repository(projects).await;

        assert_eq!(
            groups.len(),
            1,
            "main + linked worktree SHALL share one group, got {} groups: {:?}",
            groups.len(),
            groups.iter().map(|g| &g.id).collect::<Vec<_>>()
        );
        let group = &groups[0];
        assert_eq!(group.worktrees.len(), 2);
        // main worktree 排前
        assert!(group.worktrees[0].is_main_worktree);
        assert!(!group.worktrees[1].is_main_worktree);
        assert_eq!(group.worktrees[0].git_branch.as_deref(), Some("main"));
        assert_eq!(group.worktrees[1].git_branch.as_deref(), Some("feat"));
    }

    /// codex 二审第二轮 Bug 1 验证：commondir 文件存在但内容为空 → 非法
    /// linked worktree，整体返 None（**不**误判为 submodule）。
    #[tokio::test]
    async fn fs_resolver_rejects_empty_commondir() {
        let dir = tempdir().unwrap();
        let main_git = dir.path().join("main").join(".git");
        let wt_git_dir = main_git.join("worktrees").join("broken");
        tokio::fs::create_dir_all(&wt_git_dir).await.unwrap();
        tokio::fs::write(wt_git_dir.join("HEAD"), "ref: refs/heads/x\n")
            .await
            .unwrap();
        // 损坏的 linked worktree：commondir 文件存在但内容空
        tokio::fs::write(wt_git_dir.join("commondir"), "  \n")
            .await
            .unwrap();

        let wt = dir.path().join("broken_wt");
        tokio::fs::create_dir_all(&wt).await.unwrap();
        let gitlink = format!("gitdir: {}\n", wt_git_dir.display());
        tokio::fs::write(wt.join(".git"), gitlink).await.unwrap();

        let resolver = LocalGitIdentityResolver::new();
        let lookup = resolver.resolve_all(&wt).await;
        // 整体返 default（identity None）——不臆测为 submodule 也不臆测为 linked
        assert!(
            lookup.identity.is_none(),
            "empty commondir SHALL NOT be misclassified as submodule"
        );
    }

    /// spec `project-discovery` §"无法从历史 worktree path 解析 branch 时
    /// SHALL 保持 None，MUST NOT 使用父 repo 当前 branch 伪造"。
    ///
    /// 历史 bug：被 git prune 掉的 worktree 目录（path 已不存在），
    /// `locate_git_dirs` 会一路 pop 向上 walk，撞到父 repo 的 `.git` 后
    /// 把它当成自身 working tree 返 `(parent_git, parent_git, is_main=true)`，
    /// 进而把父 repo 当前分支（典型 `main`）当成该 worktree 的分支。
    /// fix：入口 check start path 存在性，不存在直接 None 让 grouper fallback。
    #[tokio::test]
    async fn fs_resolver_returns_none_when_start_path_missing() {
        let dir = tempdir().unwrap();
        // 父 repo 真实存在
        let main_repo = dir.path().join("main_repo");
        let main_git = main_repo.join(".git");
        tokio::fs::create_dir_all(&main_git).await.unwrap();
        tokio::fs::write(main_git.join("HEAD"), "ref: refs/heads/main\n")
            .await
            .unwrap();

        // 模拟被 prune 的 worktree path：嵌套在 main_repo 下但目录不存在
        let pruned = main_repo.join(".claude").join("worktrees").join("gone");

        let resolver = LocalGitIdentityResolver::new();
        let lookup = resolver.resolve_all(&pruned).await;
        // resolve_all 应直接走 default（identity=None），让 grouper 走 fallback
        assert!(
            lookup.identity.is_none(),
            "missing path SHALL NOT inherit parent repo identity here, got {:?}",
            lookup.identity
        );
        assert!(
            lookup.branch.is_none(),
            "missing path SHALL NOT inherit parent repo branch, got {:?}",
            lookup.branch
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
    async fn fs_resolver_detached_head_returns_none() {
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
        // detached HEAD 对用户无可读分支名，返 None 让 UI 不渲染分支 chip
        assert!(
            lookup.branch.is_none(),
            "detached HEAD SHALL produce None branch, got {:?}",
            lookup.branch
        );
        assert!(lookup.identity.is_some());
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
        // detached HEAD：裸 commit hash 返 None（字面 "HEAD" 对用户无意义）
        assert!(parse_head_branch("abc123\n").is_none());
        assert!(parse_head_branch("").is_none());
        assert!(parse_head_branch("   \n").is_none());
        // 非分支引用 SHALL 返 None（不能把 tag / remote ref 当分支显示）
        assert!(parse_head_branch("ref: refs/tags/v1.0.0\n").is_none());
        assert!(parse_head_branch("ref: refs/remotes/origin/main\n").is_none());
        // 空分支名也是非法
        assert!(parse_head_branch("ref: refs/heads/\n").is_none());
    }

    /// change `simplify-repository-as-project::D1` Scenario:
    /// "主仓子目录 cwd 不被误标为 repo root"。
    #[tokio::test]
    async fn subdir_cwd_not_marked_as_repo_root() {
        let dir = tempdir().unwrap();
        // canonicalize 让 tempdir path 与 identity.id 走 canonical 一致
        // （macOS `/var/...` vs `/private/var/...` symlink 差异会让
        // `strip_prefix` 失败，但生产 cwd 通常 syscall getcwd 自带 canonical）
        let base = dir.path().canonicalize().unwrap();
        let repo = base.join("repo");
        let git = repo.join(".git");
        let crates_dir = repo.join("crates");
        tokio::fs::create_dir_all(&git).await.unwrap();
        tokio::fs::create_dir_all(&crates_dir).await.unwrap();
        tokio::fs::write(git.join("HEAD"), "ref: refs/heads/main\n")
            .await
            .unwrap();

        let projects = vec![
            Project {
                id: "main".into(),
                name: "repo".into(),
                path: repo.clone(),
                sessions: vec!["s1".into()],
                most_recent_session: Some(100),
                created_at: None,
                distinct_cwds: Vec::new(),
            },
            Project {
                id: "crates_sub".into(),
                name: "crates".into(),
                path: crates_dir.clone(),
                sessions: vec!["s2".into()],
                most_recent_session: Some(200),
                created_at: None,
                distinct_cwds: Vec::new(),
            },
        ];

        let grouper = WorktreeGrouper::new(LocalGitIdentityResolver::new());
        let groups = grouper.group_by_repository(projects).await;

        assert_eq!(groups.len(), 1, "subdir + repo root SHALL share one group");
        let group = &groups[0];
        assert_eq!(group.worktrees.len(), 2);

        let main_wt = group
            .worktrees
            .iter()
            .find(|w| w.id == "main")
            .expect("main worktree should exist");
        assert!(main_wt.is_repo_root, "repo root SHALL be is_repo_root=true");
        assert!(main_wt.is_main_worktree);
        assert_eq!(main_wt.cwd_relative_to_repo_root, None);

        let sub_wt = group
            .worktrees
            .iter()
            .find(|w| w.id == "crates_sub")
            .expect("subdir worktree should exist");
        assert!(
            !sub_wt.is_repo_root,
            "subdir cwd SHALL NOT be is_repo_root=true (历史 bug 把它误标为 main)"
        );
        assert_eq!(
            sub_wt.cwd_relative_to_repo_root.as_deref(),
            Some("crates"),
            "subdir 应有 cwd_relative_to_repo_root=Some(\"crates\")"
        );

        // 排序：is_repo_root=true 的 main_wt SHALL 排前，即便 sub 的 most_recent_session 更新
        assert!(
            group.worktrees[0].is_repo_root,
            "排序后 repo root SHALL 排第一位"
        );
        assert_eq!(group.worktrees[0].id, "main");
    }

    /// change `simplify-repository-as-project::D2` Scenario:
    /// "linked worktree cwd 含 `cwd_relative_to_repo_root`"。
    #[tokio::test]
    async fn linked_worktree_cwd_relative_to_repo_root_under_claude_worktrees() {
        let dir = tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let main_repo = base.join("main_repo");
        let main_git = main_repo.join(".git");
        let wt_git_dir = main_git.join("worktrees").join("feat-x");
        let wt_path = main_repo.join(".claude").join("worktrees").join("feat-x");
        tokio::fs::create_dir_all(&wt_git_dir).await.unwrap();
        tokio::fs::create_dir_all(&wt_path).await.unwrap();
        tokio::fs::write(main_git.join("HEAD"), "ref: refs/heads/main\n")
            .await
            .unwrap();
        tokio::fs::write(wt_git_dir.join("HEAD"), "ref: refs/heads/feat-x\n")
            .await
            .unwrap();
        tokio::fs::write(wt_git_dir.join("commondir"), "../..\n")
            .await
            .unwrap();
        let gitlink = format!("gitdir: {}\n", wt_git_dir.display());
        tokio::fs::write(wt_path.join(".git"), gitlink)
            .await
            .unwrap();

        let projects = vec![
            Project {
                id: "main".into(),
                name: "main_repo".into(),
                path: main_repo.clone(),
                sessions: vec!["s1".into()],
                most_recent_session: Some(100),
                created_at: None,
                distinct_cwds: Vec::new(),
            },
            Project {
                id: "feat-x".into(),
                name: "feat-x".into(),
                path: wt_path.clone(),
                sessions: vec!["s2".into()],
                most_recent_session: Some(200),
                created_at: None,
                distinct_cwds: Vec::new(),
            },
        ];

        let grouper = WorktreeGrouper::new(LocalGitIdentityResolver::new());
        let groups = grouper.group_by_repository(projects).await;

        assert_eq!(
            groups.len(),
            1,
            "main + linked worktree SHALL share one group"
        );
        let group = &groups[0];
        let linked = group
            .worktrees
            .iter()
            .find(|w| w.id == "feat-x")
            .expect("linked worktree should exist");
        assert!(
            !linked.is_repo_root,
            "linked worktree path 嵌套在 main repo 内不算 repo root"
        );
        assert!(
            !linked.is_main_worktree,
            "linked worktree common-dir 是主 .git 但本身是 linked"
        );
        assert_eq!(
            linked.cwd_relative_to_repo_root.as_deref(),
            Some(".claude/worktrees/feat-x"),
            "linked worktree 路径 SHALL strip 出 .claude/worktrees/feat-x"
        );
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
        assert_eq!(
            parse_gitlink_dir("gitdir: C:\\repo\\.git\\worktrees\\feat\n", base),
            Some(PathBuf::from(r"C:\repo\.git\worktrees\feat"))
        );
        assert!(parse_gitlink_dir("not a gitlink\n", base).is_none());
    }
}
