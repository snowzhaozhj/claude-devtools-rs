## MODIFIED Requirements

### Requirement: Group projects by git worktree

系统 SHALL 把同一 git 仓库的多个 worktree 对应的 project 目录归为一个逻辑仓库条目，同时把每个 worktree 保留为该条目的独立成员；MUST 区分"主 working tree 根"与"主 working tree 子目录"两种 walk-up 都能到达同一 `.git` 的情况，避免子目录 cwd 被误标为独立的 main worktree。

仓库分组通过 `LocalGitIdentityResolver` 的**纯 fs 路径**（`crates/cdt-discover/src/worktree_grouper.rs::LocalGitIdentityResolver`，0 个 git 子进程）：向上 walk 找到 `.git` 条目，目录 → main worktree `(common_dir = git_dir = <repo>/.git)`；文件（gitlink）→ 解析 `gitdir:` 行后看 `<gitdir>/commondir` 文件区分 linked worktree（用 commondir）vs submodule（common = gitdir）。`identity = canonical(common_dir)` 字符串、`name = canonical.parent().file_name()`、`git_branch` 解析 `<git_dir>/HEAD`。**整个解析路径 MUST 不 spawn 任何 git 子进程**（替换 git 子进程为 syscall 是历史性能改造的成果，详 `worktree_grouper.rs::78-117`，27 project 累计 ~50ms 量级）。

聚合结果 `RepositoryGroup` MUST 含 `id`（稳定的 repo id，通常是 git-common-dir 的绝对路径）/ `identity`（`Option<RepositoryIdentity>`，无 git 时为 `None`）/ `name`（展示名）/ `worktrees`（`Vec<Worktree>`）/ `most_recent_session`（`Option<i64>`，所有 worktree 的 max）/ `total_sessions`（所有 worktree 的 sessions 总和）字段。

每个 `Worktree` MUST 含 `id`（对齐底层 `Project.id`）/ `path` / `name` / `git_branch`（`Option<String>`）/ `is_main_worktree`（`bool`，语义：common-dir 是主 `.git` 而非 linked worktree gitdir，用于排序与 main worktree 子目录分组）/ `is_repo_root`（`bool`，语义：`path` 自身就是主 working tree 的根目录，**仅当** `start == <repo>` 且 `<repo>/.git` 是目录时为 `true`；子目录 cwd 即便 walk-up 到主 `.git` 也 SHALL 为 `false`）/ `cwd_relative_to_repo_root`（`Option<String>`，repo 根本身为 `None`，子目录为相对路径如 `crates`、`.claude/worktrees/feat-x`，无法计算 repo 根时为 `None`；计算 SHALL 是纯字符串 `path.strip_prefix(repo_root)`，**0 额外 syscall**）/ `sessions`（`Vec<String>`）/ `created_at`（`Option<i64>`）/ `most_recent_session`（`Option<i64>`）字段。

Worktree 排序 SHALL 按 `is_repo_root` 优先（repo 根排前）、再按 `is_main_worktree` 优先（main common-dir 排前）、再按 `most_recent_session` 倒序（活跃 worktree 排前）。Group 排序 SHALL 按 `most_recent_session` 倒序。

#### Scenario: Two worktrees of one repo
- **WHEN** 两个 project 路径分别落在同一仓库的两个 worktree（共享同一 `git common dir`）
- **THEN** 系统 SHALL 输出一个仓库分组，含两个 worktree 成员

#### Scenario: Standalone project not in a worktree
- **WHEN** 一个 project 路径无 git 元数据
- **THEN** 系统 SHALL 把它输出为只含自己的单成员分组，`identity` 字段 SHALL 为 `None`

#### Scenario: Main worktree 排在附加 worktree 之前
- **WHEN** 一个 group 内含主 worktree 与附加 worktree，附加 worktree 的 `most_recent_session` 更新
- **THEN** group.worktrees[0].is_main_worktree SHALL 为 true，附加 worktree 排在后面（main 优先级压过时间）

#### Scenario: Group 排序按最近活动倒序
- **WHEN** 两个独立 repo group A、B，A 的最近 session 比 B 早
- **THEN** `group_by_repository` 返回数组 SHALL 含 B 在前、A 在后

#### Scenario: 主仓子目录 cwd 不被误标为 repo root
- **WHEN** 主 repo `/repo` 含 `.git` 目录；另存在 project 路径 `/repo/crates`（用户在主仓子目录 cwd 跑 claude 产生的独立 encoded 目录）
- **THEN** grouper SHALL 把 `/repo` 与 `/repo/crates` 归到同一 group
- **AND** `/repo` 对应的 Worktree `is_repo_root` SHALL 为 `true`，`is_main_worktree` SHALL 为 `true`，`cwd_relative_to_repo_root` SHALL 为 `None`
- **AND** `/repo/crates` 对应的 Worktree `is_repo_root` SHALL 为 `false`，`cwd_relative_to_repo_root` SHALL 为 `Some("crates")`
- **AND** 排序后 `/repo` SHALL 排在 `/repo/crates` 之前

#### Scenario: linked worktree cwd 含 cwd_relative_to_repo_root
- **WHEN** 主 repo `/repo` 在 `/repo/.claude/worktrees/feat-x` 创建 linked worktree（已 prune 或仍在），有对应 encoded project
- **THEN** 对应 Worktree `is_repo_root` SHALL 为 `false`，`is_main_worktree` SHALL 为 `false`
- **AND** `cwd_relative_to_repo_root` SHALL 为 `Some(".claude/worktrees/feat-x")`

### Requirement: Expose session cwd for downstream display

系统 SHALL 在 `Session`（`cdt-core::Session`，IPC 序列化形态）中暴露 `cwd: Option<String>` 字段，值取自该 session jsonl 内首条带 `cwd` 字段消息的 `cwd` 值；该字段为空（jsonl 不含 cwd）时 SHALL 为 `None`。序列化 SHALL 使用 camelCase（`cwd`），并在为 `None` 时通过 `#[serde(skip_serializing_if = "Option::is_none")]` 省略输出。

`cdt-core::Session` SHALL NOT 增加 `cwd_relative_to_repo_root` 字段——该派生字段属于 worktree 维度展示信息，由 `Worktree.cwd_relative_to_repo_root` 持有（见 `Group projects by git worktree` Requirement）；IPC 层 `SessionSummary` 在序列化时通过 group→worktree join 填入（见 ipc-data-api spec `SessionSummary 增加 worktree 元信息字段`），避免 scanner 阶段重走 repo 解析。

`ProjectScanner::scan_project_dir` SHALL 在产生 `Session` 时把 `extract_session_cwd` 的结果直接写入 `Session.cwd`；该 cwd 提取沿用现有 head-read（仅读 jsonl 前 `SESSION_HEAD_LINES` 行）+ `FILE_READ_CONCURRENCY` 信号量限流路径，**不**得为获取 cwd 而触发全文件读取（除非 head 不含 cwd 字段时按现有 `extract_session_cwd` SSH fallback 路径回滚）。

#### Scenario: 单 cwd session 暴露 cwd 字段

- **WHEN** 一个 jsonl session 首条消息 `cwd = "/Users/foo/myrepo"`
- **THEN** 系统 SHALL 在 `Session.cwd` 中返回 `Some("/Users/foo/myrepo")`
- **AND** IPC 序列化结果 SHALL 包含 `"cwd": "/Users/foo/myrepo"`

#### Scenario: 缺 cwd session 暴露 None

- **WHEN** 一个 jsonl session 所有消息均不含 `cwd` 字段
- **THEN** 系统 SHALL 在 `Session.cwd` 中返回 `None`
- **AND** IPC 序列化结果 SHALL 省略 `cwd` 键（不出现 `"cwd": null`）

#### Scenario: 同一 encoded 目录多 cwd 的 session 各自暴露真实 cwd

- **WHEN** 一个 encoded 目录 `D` 下含两条 session，cwd 分别为 `/a/b` 与 `/a/c`
- **THEN** 系统 SHALL 输出**一条** `Project`（`id = D`，不再拆分），其 `sessions` 列表两条目分别带 `cwd = Some("/a/b")` 与 `cwd = Some("/a/c")`

#### Scenario: 提取 cwd 不触发全文件读

- **WHEN** 一个 session jsonl 文件大小 100 MB，cwd 在前 20 行内
- **THEN** 系统 SHALL 仅通过 head-read（`FileSystemProvider::read_lines_head`）拿到 cwd
- **AND** SHALL NOT 触发对该文件的 `read_to_string`

#### Scenario: cdt-core::Session 不含 cwd_relative_to_repo_root 字段

- **WHEN** grep `cdt-core/src/project.rs::Session` 的字段定义
- **THEN** SHALL 不出现 `cwd_relative_to_repo_root` 字段（该字段仅在 `cdt-core::Worktree` 与 IPC 层 `SessionSummary` 上存在）
