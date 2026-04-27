## ADDED Requirements

### Requirement: Resolve project id from session id alone

`DataApi` trait SHALL 暴露 `find_session_project(session_id: &str) -> Result<Option<String>, ApiError>`，让仅持有 `session_id` 的调用方反查所属 `project_id`。HTTP `GET /api/sessions/:id` 与 trait 内 `get_sessions_by_ids` MUST 走该方法配合 `get_session_detail(project_id, session_id)` 的复合路径，**不**得直接调 `get_session_detail("", session_id)`。

trait 默认实现 SHALL 遍历 `list_projects()` 取每个 `project_id`，依次调 `list_sessions_sync(project_id, { page_size: usize::MAX, cursor: None })`，命中第一个含 `session_id` 的项目立即返回 `Ok(Some(project_id))`；遍历完无命中返 `Ok(None)`。**主会话**（`<projects_dir>/<encoded>/<session_id>.jsonl`）必然能被默认实现命中；subagent jsonl 是否被命中 SHALL 视具体实现的覆盖能力而定（默认实现不强制覆盖）。

`LocalDataApi` SHALL 覆盖默认实现，直接 `read_dir(scanner.projects_dir())` 扫每个 project 子目录，按以下顺序匹配（命中即返回 `Ok(Some(<encoded_project_id>))`）：

1. **主会话快路径**：`<project_dir>/<session_id>.jsonl` 存在。
2. **legacy subagent**：`<project_dir>/agent-<session_id>.jsonl` 存在。
3. **新结构 subagent**：`<project_dir>/<parent>/subagents/agent-<session_id>.jsonl` 存在（任一 parent）。

实现 SHALL 复用既有 `find_subagent_jsonl` helper，与 `LocalDataApi::get_session_detail` 的查找口径完全一致——避免出现"`find_session_project` 命中但 `get_session_detail` 又取不到"的不一致状态。

#### Scenario: 默认实现命中主会话
- **WHEN** 调用方对一个 mock `DataApi` 调 `find_session_project("sid-A")`，`sid-A` 是项目 `proj-1` 下的主会话
- **AND** mock 实现走 trait 默认 `list_projects` + `list_sessions_sync` 路径
- **THEN** 返回 SHALL 为 `Ok(Some("proj-1"))`

#### Scenario: 默认实现找不到时返 None
- **WHEN** 调用方对 mock `DataApi` 调 `find_session_project("sid-ghost")`，所有 project 的 `list_sessions_sync` 都不含该 id
- **THEN** 返回 SHALL 为 `Ok(None)`

#### Scenario: LocalDataApi 直扫 FS 命中主会话
- **WHEN** tmpdir 下构造 `LocalDataApi`，写入 `<projects_dir>/<encoded-A>/sid-1.jsonl`
- **AND** 调用方调 `find_session_project("sid-1")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-A>"))`

#### Scenario: LocalDataApi 命中 subagent jsonl
- **WHEN** tmpdir 下构造 `LocalDataApi`，写入 `<projects_dir>/<encoded-B>/parent/subagents/agent-sid-2.jsonl`
- **AND** 调用方调 `find_session_project("sid-2")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-B>"))`

#### Scenario: LocalDataApi 多 project 命中第一个
- **WHEN** tmpdir 下两个 project 目录都不含目标 sid，第三个含 `sid-3.jsonl`
- **AND** 调用方调 `find_session_project("sid-3")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-的第三个>"))`，不报错且只命中一次

#### Scenario: LocalDataApi 找不到时返 None 不报错
- **WHEN** tmpdir 下所有 project 目录都不含目标 sid
- **AND** 调用方调 `find_session_project("sid-ghost")`
- **THEN** 返回 SHALL 为 `Ok(None)`（**不**得返回 `Err`、**不**得 panic）

#### Scenario: 与 get_session_detail 口径一致
- **WHEN** `find_session_project(sid)` 返回 `Ok(Some(pid))`
- **THEN** 紧接着调 `get_session_detail(pid, sid)` SHALL 成功返回 `SessionDetail`（不**得**返回 `not_found`）；反之，`Ok(None)` 时 `get_session_detail` 任意 `project_id` 调用 SHALL 都返回 `not_found`
