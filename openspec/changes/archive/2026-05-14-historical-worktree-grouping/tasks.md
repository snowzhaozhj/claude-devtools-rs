## 1. cdt-discover 实现

- [x] 1.1 在 `ProjectScanner` 的无 `cwd` fallback 中识别 `<repo-encoded>--claude-worktrees-<worktree-name>` 历史 worktree 目录。
- [x] 1.2 从同级父 repo encoded 目录的 session `cwd` 恢复父 repo 路径，并构造 `<parent-cwd>/.claude/worktrees/<worktree-name>`。
- [x] 1.3 在 `WorktreeGrouper` 对历史 worktree path 解析 git identity 失败时，回退到父 repo path 解析 identity。
- [x] 1.4 保持历史 worktree 的 `git_branch = None`，不使用父 repo 当前 branch 伪造。

## 2. 测试与验证

- [x] 2.1 补充 scanner 回归测试：无 `cwd` 的历史 worktree 目录从父 repo `cwd` 恢复 path，且不把带连字符 repo 名拆坏。
- [x] 2.2 补充 grouper 回归测试：已删除历史 worktree 归入父 repo group，`is_main_worktree=false` 且 `git_branch=None`。
- [x] 2.3 跑 `cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、相关 Rust/UI/OpenSpec 验证。
- [x] 2.4 用本机真实 `~/.claude/projects` 数据自测截图中的历史 worktree 路径推断结果。
- [x] 2.5 让 codex / reviewer 审查修复与 OpenSpec delta；codex 调用未返回正文，本地 reviewer 发现真实 canonical encoding 问题并已修复。
