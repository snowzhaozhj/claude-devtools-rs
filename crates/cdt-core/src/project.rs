//! 跨 crate 共享的项目 / 会话 / worktree 结构。
//!
//! 这些类型由 `cdt-discover`（project-discovery capability）产出，
//! 之后被 `cdt-watch`、`cdt-api`、`cdt-ssh` 等下游 crate 消费。
//! 字段布局对齐 TS `src/main/types` 里的 `Project` / `Session` / `Worktree` /
//! `RepositoryGroup`。
//!
//! Spec：`openspec/specs/project-discovery/spec.md`。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 单个 Claude Code 工程条目。
///
/// `id` 既可能是 `~/.claude/projects/` 下的编码目录名（例如
/// `-Users-alice-code-foo`），也可能是 composite 形式
/// `{baseDir}::{hash8}` —— 当同一编码目录下的 session 属于多个 `cwd`
/// 时，port-project-discovery 会按 `cwd` 拆分为多个逻辑子工程。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub sessions: Vec<String>,
    pub most_recent_session: Option<i64>,
    pub created_at: Option<i64>,
}

/// 单个 session 文件的 UI 元数据视图。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub last_modified: i64,
    pub size: u64,
    pub is_pinned: bool,
}

/// `analyze_session_file_metadata` 返回的薄 metadata，本 port 只用 size / mtime。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub session_id: String,
    pub size: u64,
    pub last_modified: i64,
}

/// git repo 的唯一身份，由 `WorktreeGrouper` 通过
/// `git rev-parse --git-common-dir` 等命令解析。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryIdentity {
    /// 稳定的 repo id（通常是 git-common-dir 的绝对路径）。
    pub id: String,
    /// 展示名（通常是 common-dir 的最后一段）。
    pub name: String,
}

/// 一个 worktree —— 同一 `Project` 的 git 视图封装。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worktree {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub git_branch: Option<String>,
    pub is_main_worktree: bool,
    pub sessions: Vec<String>,
    pub created_at: Option<i64>,
    pub most_recent_session: Option<i64>,
}

/// 一组共享 repo identity 的 worktree。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryGroup {
    pub id: String,
    pub identity: Option<RepositoryIdentity>,
    pub name: String,
    pub worktrees: Vec<Worktree>,
    pub most_recent_session: Option<i64>,
    pub total_sessions: usize,
}
