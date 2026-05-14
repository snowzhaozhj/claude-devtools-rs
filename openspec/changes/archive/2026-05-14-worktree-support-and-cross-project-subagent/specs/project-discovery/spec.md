## MODIFIED Requirements

### Requirement: Group projects by git worktree

系统 SHALL 把同一 git 仓库的多个 worktree 对应的 project 目录归为一个逻辑仓库条目，同时把每个 worktree 保留为该条目的独立成员。

仓库分组通过 `LocalGitIdentityResolver` 调 `git rev-parse --git-common-dir`（解析 repo 唯一身份）+ `git rev-parse --abbrev-ref HEAD`（取分支名）+ `git rev-parse --git-dir`（判 main vs 附加 worktree）。聚合结果 `RepositoryGroup` MUST 含 `id`（稳定的 repo id，通常是 git-common-dir 的绝对路径）/ `identity`（`Option<RepositoryIdentity>`，无 git 时为 `None`）/ `name`（展示名）/ `worktrees`（`Vec<Worktree>`）/ `most_recent_session`（`Option<i64>`，所有 worktree 的 max）/ `total_sessions`（所有 worktree 的 sessions 总和）字段。

每个 `Worktree` MUST 含 `id`（对齐底层 `Project.id`）/ `path` / `name` / `git_branch`（`Option<String>`）/ `is_main_worktree`（`bool`）/ `sessions`（`Vec<String>`）/ `created_at`（`Option<i64>`）/ `most_recent_session`（`Option<i64>`）字段。

Worktree 排序 SHALL 按 `is_main_worktree` 优先（main 排前）、再按 `most_recent_session` 倒序（活跃 worktree 排前）。Group 排序 SHALL 按 `most_recent_session` 倒序。

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
