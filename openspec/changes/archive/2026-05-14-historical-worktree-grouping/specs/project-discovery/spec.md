## ADDED Requirements

### Requirement: Resolve historical Claude worktree directories

系统 SHALL 在扫描历史 / 已删除 Claude Code worktree 会话目录时，从 encoded 目录结构和父 repo session `cwd` 恢复可归组的逻辑 worktree 路径。

当 encoded project 目录名形如 `<repo-encoded>-.claude-worktrees-<worktree-name>`（即 `encode_path("<repo>/.claude/worktrees/<worktree-name>")` 的 canonical 形态；实现可兼容历史 `--claude-worktrees-` 形态），且该目录内 session JSONL 没有可用 `cwd` 时，scanner SHALL 优先读取同级 `<repo-encoded>/` 目录下 session 的 `cwd` 作为父 repo 路径，并把该历史 worktree 的 `Project.path` 设为 `<parent-cwd>/.claude/worktrees/<worktree-name>`。如果父 repo 目录不存在或无可用 `cwd`，scanner MAY fallback 到对 `<repo-encoded>` 的 best-effort decode。

`WorktreeGrouper` 在历史 worktree path 本身无法解析 git identity 时，SHALL 识别 `<parent>/.claude/worktrees/<worktree-name>` 形态并使用 `<parent>` 解析 repo identity，使该历史 worktree 归入父 repo `RepositoryGroup`。无法从历史 worktree path 解析 branch 时，`git_branch` SHALL 保持 `None`，MUST NOT 使用父 repo 当前 branch 伪造。

#### Scenario: 无 cwd 的历史 worktree 从父 repo cwd 恢复路径
- **WHEN** `~/.claude/projects/` 下存在 `<repo-encoded>/`，其 session JSONL 含 `cwd = "/repo-with-hyphen"`
- **AND** 同级存在 `<repo-encoded>-.claude-worktrees-old-feature/`，其 session JSONL 不含 `cwd`
- **THEN** scanner SHALL 输出该历史 worktree `Project.path = "/repo-with-hyphen/.claude/worktrees/old-feature"`
- **AND** SHALL NOT 通过 best-effort decode 把 `repo-with-hyphen` 拆成多级目录

#### Scenario: 已删除历史 worktree 归入父 repo group
- **WHEN** `WorktreeGrouper` 处理一个 path 为 `/repo/.claude/worktrees/old-feature` 的 project
- **AND** 该历史 worktree path 本身无法通过 git 解析 identity
- **AND** `/repo` 能解析出 repo identity
- **THEN** 系统 SHALL 把该 project 归入 `/repo` 对应的 `RepositoryGroup`
- **AND** 该 worktree 的 `is_main_worktree` SHALL 为 false
- **AND** 该 worktree 的 `git_branch` SHALL 为 `None`
