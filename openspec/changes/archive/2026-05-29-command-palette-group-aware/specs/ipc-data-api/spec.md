## ADDED Requirements

### Requirement: Expose group-aware search via Tauri IPC command

系统 SHALL 暴露 `search_group_sessions` Tauri command：接受 `group_id` 与 `query` 参数，遍历该 repository group 内所有 worktree 的 sessions 合并搜索，返回与 `session-search` capability 同形的搜索结果（`SearchSessionsResult`）。后端 SHALL 复用 `list_repository_groups_inner()` 获取 group 与当前 active context 的 fs/projects_dir，避免跨 await race。

#### Scenario: Search group sessions across multiple worktrees
- **WHEN** 前端拿一个含 3 个 worktree 的 group_id 与 query 调用 `search_group_sessions`
- **AND** query 命中 worktree A 的 2 个 sessions 和 worktree C 的 1 个 session
- **THEN** 命令 SHALL 返回 3 个 session 结果条目，按最近修改时间倒序排列
- **AND** 每条结果的 `projectId` SHALL 是各自 worktree 的 encoded path（非 group id）

#### Scenario: Search group sessions with nonexistent group
- **WHEN** 前端拿一个不存在的 group_id 调用 `search_group_sessions`
- **THEN** 命令 SHALL 返回 not_found 错误

#### Scenario: Search group sessions with empty query
- **WHEN** 前端拿一个有效 group_id 与空 query 调用 `search_group_sessions`
- **THEN** 命令 SHALL 返回空结果数组，不报错

#### Scenario: Search group sessions when a worktree directory is missing
- **WHEN** group 包含 worktree X 但对应 projects_dir 下无该目录
- **THEN** 命令 SHALL 跳过该 worktree 继续搜索其余 worktree，不整体报错
