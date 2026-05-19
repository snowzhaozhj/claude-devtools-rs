## ADDED Requirements

### Requirement: Dispatch project/session reads by active context

所有"读项目 / 读会话 / 读会话产物 / 全局搜索"类 IPC method 在 active context = `Ssh<host>` 时 SHALL 走当前 SSH `FileSystemProvider`（通过 `LocalDataApi::active_scanner()` 或 `LocalDataApi::active_fs_and_projects_dir()` helper），**不得**直接锁 `self.scanner` / `self.projects_dir` 字段而退化到本地数据。本 Requirement 覆盖的 method 集合 SHALL 至少包含以下 11 个：

**本 change 修复（8 处）**：
- `list_repository_groups`
- `project_memory_dir`
- `find_session_project`
- `get_session_summaries_by_ids`
- `get_subagent_trace`
- `get_image_asset`
- `get_tool_output`
- `search`

**已正确实现（3 处，本 change 加回归测试）**：
- `list_sessions` / `list_sessions_sync` / `list_sessions_paginated`
- `get_session_detail`
- `list_projects`

**例外**：仅"重置本地数据根路径"语义的 method（`set_projects_dir` / `reconfigure_claude_root`）保持 local provider，不受本条约束。

#### Scenario: list_repository_groups 在 SSH context 下走远端 provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `list_repository_groups` IPC
- **THEN** 系统 SHALL 通过当前 SSH context 的 `FileSystemProvider` 扫描 `<remote_home>/.claude/projects/`
- **AND** 返回的 `RepositoryGroup.worktrees[]` SHALL 来自远端 fixture 的项目集合
- **AND** 返回结果 SHALL NOT 包含本地宿主机 `.git` 解析出的 `gitBranch` 值

#### Scenario: 辅助读类 method 在 SSH context 下走远端 provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `find_session_project(session_id)` / `project_memory_dir(project_id)` / `get_session_summaries_by_ids(ids)` 任一
- **THEN** 后端 SHALL 通过当前 SSH context 的 provider 读远端文件
- **AND** 返回的 project_id / path 字段 SHALL 与远端 fake fixture 一致
- **AND** 返回的路径字段（若存在，如 `project_memory_dir`）SHALL 以远端 `<remote_home>` 为根

#### Scenario: 会话产物读取在 SSH context 下走远端 provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `get_subagent_trace(session_id, ...)` / `get_image_asset(session_id, ...)` / `get_tool_output(session_id, ...)` 任一
- **THEN** 后端 SHALL 通过远端 SFTP 读取对应文件
- **AND** 远端 provider 的 `read_file` 调用计数 SHALL ≥ 1（fake provider 通过 `Mutex<usize>` 计数器观测）
- **AND** 本地 `LocalFileSystemProvider` 的同名方法 SHALL NOT 被调用

#### Scenario: search 在 SSH context 下使用 active provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `search(query)` IPC
- **THEN** `SessionSearcher` SHALL 接收当前 SSH provider 作为 `Arc<dyn FileSystemProvider>` 入参
- **AND** 搜索结果 SHALL 来自远端 `<remote_home>/.claude/projects/` 下的 jsonl 内容
- **AND** 后端**不得**硬编码 `LocalFileSystemProvider::new()` 作为 search 的数据源
- **AND** 远端 provider 的 `read_to_string` 或 `open_read_stream` 调用计数 SHALL ≥ 1

#### Scenario: 根路径重置类 method 仍用 local provider

- **WHEN** 调用方调 `set_projects_dir(new_path)` 或 `reconfigure_claude_root(new_root)`
- **THEN** 系统 SHALL 重置 `self.scanner` 为 `LocalFileSystemProvider` 包装下的新 `projects_dir`
- **AND** 该重置**不影响**已注册的 SSH context 的 provider 状态
- **AND** 若 active context 是 SSH，**仍**保持 SSH 为 active；后续调"读项目/会话"类 method 仍走 SSH provider
- **AND** 仅当 active context 切回 local 后，新的 local `projects_dir` 才生效

#### Scenario: 已实现的 method 在 SSH context 下保持远端行为

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `list_projects` / `list_sessions` / `list_sessions_sync` / `list_sessions_paginated` / `get_session_detail` 任一
- **THEN** 后端 SHALL 走 SSH provider 读远端数据（行为与本 change 前一致）
- **AND** 本 Requirement 配套的回归测试 SHALL 覆盖这 5 个 method，防止后续改动误退化为 local
