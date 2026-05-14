## Why

用户人工验证发现 Sidebar grouped dropdown 仍漏掉已清理的历史 Claude Code worktree 会话，例如 `rosetta-detect` / `sidebar-click-replace` / `fix-external-link-opener`。这些会话目录本身没有可用 `cwd`，真实 worktree 目录也已删除，现有 git identity 解析无法把它们归回父 repo group。

## What Changes

- `ProjectScanner` 在扫描无 `cwd` 的历史 worktree encoded 目录时，识别 `<repo-encoded>--claude-worktrees-<worktree-name>` 形态。
- scanner SHALL 优先从同级父 repo encoded 目录的 session `cwd` 恢复父 repo 路径，再构造历史 worktree path：`<parent-cwd>/.claude/worktrees/<worktree-name>`。
- `WorktreeGrouper` 在历史 worktree path 本身无法跑 git 时，SHALL 回退到父 repo path 解析 identity，从而仍归入父 repo group。
- 历史 worktree 的 branch 信息不可可靠恢复时 SHALL 保持 `None`，不伪造为父 repo 当前分支。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `project-discovery`: 补充历史 / 已删除 Claude Code worktree 会话目录的 path 恢复与 git group 归属要求。

## Impact

- 后端：`crates/cdt-discover/src/project_scanner.rs`、`crates/cdt-discover/src/worktree_grouper.rs`
- 测试：`crates/cdt-discover/tests/project_scanner.rs`、`worktree_grouper` unit tests
- 前端 / IPC schema：无字段变更；现有 `list_repository_groups` 返回更完整的分组数据
