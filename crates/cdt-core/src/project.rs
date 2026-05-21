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
/// `id` 是 `~/.claude/projects/` 下的编码目录名（例如
/// `-Users-alice-code-foo`）。同一编码目录下不同 `cwd` 的 session
/// 始终归属同一 `Project`；session 之间 cwd 差异由 `Session.cwd` 字段
/// 暴露，由消费方（UI）按需展示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub sessions: Vec<String>,
    pub most_recent_session: Option<i64>,
    pub created_at: Option<i64>,
    /// 该 encoded 目录下所有 session 的 `cwd` 去重集合，按 session mtime 降序。
    /// 由 `ProjectScanner` 填充，供 `agent-configs` 等消费方覆盖所有 cwd 的
    /// `.claude/agents/` 扫描；为空时省略序列化保持对老前端非破坏。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub distinct_cwds: Vec<String>,
}

/// 单个 session 文件的 UI 元数据视图。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub last_modified: i64,
    pub size: u64,
    pub is_pinned: bool,
    /// session jsonl 内首条带 `cwd` 字段消息的 `cwd` 值；缺失时为 `None`。
    /// 由 `ProjectScanner` 在扫描时通过 head-read 填充，供消费方（UI）
    /// 在同一 `Project` 下区分不同 cwd 的 session。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// `analyze_session_file_metadata` 返回的薄 metadata，本 port 只用 size / mtime。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub session_id: String,
    pub size: u64,
    pub last_modified: i64,
}

/// git repo 的唯一身份，由 `WorktreeGrouper` 通过
/// `git rev-parse --git-common-dir` 等命令解析。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryIdentity {
    /// 稳定的 repo id（通常是 git-common-dir 的绝对路径）。
    pub id: String,
    /// 展示名（通常是 common-dir 的最后一段）。
    pub name: String,
}

/// 一个 worktree —— 同一 `Project` 的 git 视图封装。
///
/// `is_main_worktree` 语义：`common-dir` 是主 `.git` 而非 linked worktree
/// gitdir（用于 worktree 排序与同 main-tree 分组）；**不**等同于"该 path
/// 自身是 working tree 根目录"——子目录 cwd walk-up 到主 `.git` 时
/// `is_main_worktree=true` 但 `is_repo_root=false`。
///
/// `is_repo_root` 语义：`path` 自身就是主 working tree 的根目录，仅当
/// walk-up 起点等于解析出的 repo root 时为 `true`。同一 group 内只应有
/// 一个 worktree `is_repo_root=true`。
///
/// `cwd_relative_to_repo_root`：相对 repo 根的子路径（如 `crates`、
/// `.claude/worktrees/feat-x`）。repo 根本身或解析失败时为 `None`；UI 用
/// 作 chip / filter hint。change `simplify-repository-as-project`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub git_branch: Option<String>,
    pub is_main_worktree: bool,
    #[serde(default)]
    pub is_repo_root: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd_relative_to_repo_root: Option<String>,
    pub sessions: Vec<String>,
    pub created_at: Option<i64>,
    pub most_recent_session: Option<i64>,
}

/// 一组共享 repo identity 的 worktree。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryGroup {
    pub id: String,
    pub identity: Option<RepositoryIdentity>,
    pub name: String,
    pub worktrees: Vec<Worktree>,
    pub most_recent_session: Option<i64>,
    pub total_sessions: usize,
}
