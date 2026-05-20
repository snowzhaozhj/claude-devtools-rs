## ADDED Requirements

### Requirement: Session 列表序列化暴露 cwd 字段

`list_sessions` 与 `get_session_detail` 返回的 `Session`（或 `SessionSummary`）IPC payload SHALL 暴露 `cwd?: string` 字段（camelCase）。该字段值来自 `cdt-core::Session.cwd`（详见 `project-discovery` spec `Expose session cwd for downstream display` Requirement），表示该 session jsonl 内首条带 `cwd` 字段消息的 `cwd` 值。

无 cwd 信息（jsonl 不含 `cwd`）时 SHALL 通过 `#[serde(skip_serializing_if = "Option::is_none")]` 在 payload 中省略该键，**不**得序列化为 `"cwd": null`，以保持老前端 / 老 fixture 兼容。

HTTP 路径（`GET /api/projects/:id/sessions` / `GET /api/projects/:id/sessions/:sid`）SHALL 同步暴露 `cwd` 字段——与 IPC 路径共享 `LocalDataApi::list_sessions` / `get_session_detail` 实现，自动适用。

#### Scenario: 含 cwd 的 session 在 list_sessions 返回中带 cwd

- **WHEN** `list_sessions(projectId)` 命中一个 jsonl session，其首条消息 `cwd = "/Users/foo/myrepo/.claude/worktrees/feat-x"`
- **THEN** 返回数组对应条目 SHALL 含 `"cwd": "/Users/foo/myrepo/.claude/worktrees/feat-x"`

#### Scenario: 无 cwd 的 session 在 list_sessions 返回中省略 cwd

- **WHEN** `list_sessions(projectId)` 命中一个 jsonl session，所有消息均不含 `cwd` 字段
- **THEN** 返回数组对应条目 SHALL NOT 包含 `cwd` 键
- **AND** 该 session 其它字段（`id` / `lastModified` / `size` / `isPinned`）SHALL 保留

#### Scenario: get_session_detail 元数据带 cwd

- **WHEN** `get_session_detail(projectId, sessionId)` 命中目标 session
- **THEN** `SessionDetail.metadata` 或顶层等价位置 SHALL 含 `"cwd": <value or omitted>`，与 `list_sessions` 同口径

### Requirement: get_session_detail 本地路径以单文件 stat 取元数据

`LocalDataApi::get_session_detail` 在本地（非 SSH）路径 SHALL 通过 `tokio::fs::metadata(jsonl_path)` 单次 stat 系统调用获取目标 session 的 `lastModified` 与 `size`，**SHALL NOT** 触发跨 project 的全量扫描（即不得调用 `ProjectScanner::scan()` 或等价的"列举 `~/.claude/projects/` 下所有目录"路径），也 SHALL NOT 为获取 mtime / size 而读取目标 jsonl 之外的任何文件。

session jsonl 文件不存在时，本地路径 SHALL fallback 至现有 `find_subagent_jsonl` 路径搜索 subagent jsonl（沿用现状行为）；fallback 仍不存在时 SHALL 返回 `ApiError::not_found`。

远程 SSH 路径行为不变（沿用现有 `list_sessions(projectId)` 轻量列举 + 单文件元数据获取）。

#### Scenario: 本地打开 session 详情不触发全量扫描

- **WHEN** `get_session_detail("foo-project", "session-1")` 在本地环境调用，`foo-project` 与 `session-1.jsonl` 在 `~/.claude/projects/foo-project/` 下存在
- **THEN** 后端实现 SHALL 仅对 `~/.claude/projects/foo-project/session-1.jsonl` 调一次 `tokio::fs::metadata`
- **AND** SHALL NOT 对 `~/.claude/projects/` 下其它 project 目录调 `read_dir` / `stat`
- **AND** SHALL NOT 调 `read_lines_head` / `read_to_string` 读取除目标 jsonl 之外的任何 jsonl

#### Scenario: 目标 jsonl 不存在 fallback 到 subagent 查找

- **WHEN** `get_session_detail("foo-project", "missing-session")` 调用，`missing-session.jsonl` 不在主目录但存在于 `subagents/agent-*.jsonl`
- **THEN** 后端 SHALL 通过 `find_subagent_jsonl` 路径定位到 subagent jsonl 并返回其 detail

#### Scenario: 目标 session id 完全不存在返回 not_found

- **WHEN** `get_session_detail("foo-project", "nope")` 调用，`nope.jsonl` 既不在主目录也不在 `subagents/` 下
- **THEN** 后端 SHALL 返回 `ApiError::not_found`，**不**触发全量扫描以试图反查

### Requirement: Contract test asserts get_session_detail does not cross project boundary

contract test 层 SHALL 通过 spy `FileSystemProvider` 包装（在测试 wrapper 里记录每个 `read_dir` / `read_lines_head` / `read_to_string` / `stat` 方法被调次数 + 路径列表），覆盖 `get_session_detail` 的本地路径，断言：调用 `get_session_detail(P, S)` 后，spy 记录的 `read_dir` 调用次数 == 0；`read_lines_head` 与 `stat` 的 path 集合 SHALL ⊆ {target jsonl path}（解析 jsonl 内容的 head-read 与目标 stat 允许）；spy 记录的所有 path 都 SHALL NOT 落在 `~/.claude/projects/<P>` 之外的兄弟 project 目录。

该 contract test SHALL 跑在 `crates/cdt-api/tests/ipc_contract.rs`（与 `#[ignore]` 的 perf bench 互补）；本断言 SHALL 在 CI 默认 job 内执行，对"不全扫"行为契约提供机器验证保护。

#### Scenario: spy FileSystemProvider 验证不读取兄弟 project

- **WHEN** 测试搭建 `tempdir` 下铺 3 个 project（`P_A` / `P_B` / `P_C`），每个 2 个 session jsonl
- **AND** 调用 `LocalDataApi::get_session_detail("P_A", "session_1")`
- **THEN** spy 记录的 `read_dir` 调用次数 SHALL 为 0
- **AND** spy 记录的所有 path 中 SHALL NOT 含 `P_B/` 或 `P_C/` 下任何文件
- **AND** `read_lines_head` / `stat` 的 path 集合 SHALL ⊆ `{tempdir/P_A/session_1.jsonl}`
