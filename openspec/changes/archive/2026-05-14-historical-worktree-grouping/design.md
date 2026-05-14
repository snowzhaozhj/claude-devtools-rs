## Context

当前 `ProjectScanner` 优先从 session JSONL 的 `cwd` 字段恢复真实项目路径；缺失 `cwd` 时回退到 encoded 目录名 best-effort decode。真实验证发现，已删除的 Claude Code worktree 历史目录常见形态为 `<repo-encoded>--claude-worktrees-<worktree-name>`，且该目录内 JSONL 可能没有 `cwd`。由于 repo 名本身可含连字符（如 `claude-devtools-rs`），直接 decode 该目录名会把 repo 名拆坏，导致后续 git identity 分组失败。

## Goals / Non-Goals

**Goals:**

- 无 `cwd` 的历史 worktree 会话仍能恢复到父 repo 下的 `.claude/worktrees/<name>` 逻辑路径。
- 已删除 worktree 的历史会话仍能出现在父 repo 的 `RepositoryGroup` 中。
- 不伪造不可恢复的 branch 信息。

**Non-Goals:**

- 不恢复已删除 worktree 的真实 git branch（目录不存在时无可靠来源）。
- 不改变 IPC 字段结构或前端渲染协议。
- 不改变普通 standalone project 的 fallback decode 行为。

## Decisions

### D1: scanner 识别 encoded worktree 目录，而不是修改通用 decoder

`path_decoder::decode_path` 是跨平台 best-effort 工具，不能可靠区分路径分隔符和文件名中的连字符。历史 worktree 目录有更强的结构信号：`--claude-worktrees-`。因此在 `ProjectScanner` 的无 `cwd` fallback 分支识别该形态，而不是让通用 decoder 学习特殊业务语义。

候选方案：直接改 `decode_path`。放弃原因：会影响所有 encoded path 解码调用方，并且仍无法从 `<repo-encoded>` 本身恢复带连字符 repo 名的真实 cwd。

### D2: 父 repo cwd 从同级父 repo encoded 目录的 session 中读取

当命中 `<repo-encoded>--claude-worktrees-<worktree-name>` 时，scanner 优先读取同级 `<repo-encoded>/` 目录中的 session `cwd`，用它作为父 repo 真实路径，再构造 `<parent-cwd>/.claude/worktrees/<worktree-name>`。如果父 repo 目录不存在或也无 `cwd`，才退回 `decode_path(repo_encoded)`。

候选方案：只用 `decode_path(repo_encoded)`。放弃原因：`claude-devtools-rs` 这类带连字符的 repo 名会被错误拆成 `/claude/devtools/rs`。

### D3: grouper 只回退 identity，不回填 branch

`WorktreeGrouper` 对历史 worktree path 跑 git 失败时，从 `.claude/worktrees/<name>` 反推父 repo path 并解析 identity，使其归入父 repo group。branch 仍只从 worktree path 自身获取；目录不存在时保持 `None`。

候选方案：用父 repo branch 填历史 worktree branch。放弃原因：这会把已删除 worktree 误显示为父 repo 当前分支，信息不可信。

## Risks / Trade-offs

- 历史 worktree 名本身保留 encoded suffix 原样；这与 Claude Code 目录编码一致，避免进一步猜测。
- 如果父 repo encoded 目录也没有任何可用 `cwd`，仍会 fallback 到 best-effort decode，带连字符 repo 名可能不完美；这是缺少权威数据时的退化路径。
- 对 SSH provider 仍不做全文件扫描，保持远端性能约束；本 change 主要修本地历史 worktree。 
