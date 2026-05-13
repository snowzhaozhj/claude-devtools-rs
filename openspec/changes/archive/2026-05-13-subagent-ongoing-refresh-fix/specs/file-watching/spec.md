## ADDED Requirements

### Requirement: Route nested subagent JSONL changes to parent session

系统 SHALL 把形如 `<projects_dir>/<project_id>/<session_id>/subagents/agent-<sub_session_id>.jsonl` 的嵌套 subagent JSONL 写入路由为父 `(project_id, session_id)` 的 `FileChangeEvent`，复用与父 session JSONL 相同的 broadcast channel 与 payload schema。`agent-acompact*.jsonl` 与非 `agent-*.jsonl` 命名的文件 SHALL NOT 触发 `FileChangeEvent`。旧结构 `<projects_dir>/<project_id>/agent-*.jsonl`（无父 session 目录嵌套）不在本 Requirement 范围。

嵌套分支 emit 的 `FileChangeEvent.project_list_changed` MUST 固定为 `false`，**不**走既有 2 层路径的 `!deleted && mark_project_seen(project_id)` 派生逻辑。理由：嵌套 subagent 写入只是"父 session 内部增量"信号，不应当让前端 `DashboardView` / `Sidebar` 误以为新项目出现而刷新整个项目列表（极端 race 下若父 session JSONL 尚未触发过事件而子 session 已写入，`mark_project_seen` 会返回 `true`，必须显式短路）。

#### Scenario: Subagent JSONL 文件追加触发父 session 刷新

- **WHEN** `<projects_dir>/p1/sess-A/subagents/agent-sub-1.jsonl` 被追加内容
- **THEN** 订阅者 SHALL 在 debounce 窗口结束后收到一条 `FileChangeEvent { project_id: "p1", session_id: "sess-A", deleted: false, project_list_changed: false }`

#### Scenario: 嵌套分支强制 project_list_changed=false

- **WHEN** `<projects_dir>/p2/sess-B/subagents/agent-sub-9.jsonl` 是 watcher 第一次看到 `p2` 项目（父 session JSONL 此前未触发过任何事件）
- **THEN** 即使内部 `mark_project_seen("p2")` 第一次会返回 `true`，emit 的 `FileChangeEvent.project_list_changed` SHALL 为 `false`（嵌套分支硬编码 `false`，不从 `mark_project_seen` 派生），避免前端误以为有新项目出现并刷新整个项目列表

#### Scenario: Subagent JSONL 文件首次创建触发父 session 刷新

- **WHEN** `<projects_dir>/p1/sess-A/subagents/agent-sub-2.jsonl` 首次出现
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "p1", session_id: "sess-A", deleted: false, .. }`，与父 session JSONL 写入的事件 schema 完全一致

#### Scenario: Subagent JSONL 文件删除触发父 session 刷新

- **WHEN** `<projects_dir>/p1/sess-A/subagents/agent-sub-1.jsonl` 被删除
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "p1", session_id: "sess-A", deleted: true, .. }`

#### Scenario: agent-acompact 文件被忽略

- **WHEN** `<projects_dir>/p1/sess-A/subagents/agent-acompact-xyz.jsonl` 被写入
- **THEN** 订阅者 SHALL NOT 收到任何 `FileChangeEvent`

#### Scenario: 非 agent- 前缀文件被忽略

- **WHEN** `<projects_dir>/p1/sess-A/subagents/notes.txt` 或 `<projects_dir>/p1/sess-A/subagents/random.jsonl` 被写入
- **THEN** 订阅者 SHALL NOT 收到 `FileChangeEvent`

#### Scenario: 旧结构 agent-*.jsonl 不进入本 Requirement 的嵌套判定分支

- **WHEN** `<projects_dir>/p1/agent-sub-3.jsonl`（旧结构 2 层路径，无 `<session_id>/subagents/` 嵌套）被写入
- **THEN** 本 Requirement 的"嵌套 subagent → 父 session"路由 SHALL NOT 触发；该路径 SHALL 由既有 `Watch Claude projects directory for session changes` Requirement 的 2 层判定处理（按 `agent-sub-3` 作为 `session_id` 发出 `FileChangeEvent`），其语义不属本 change 改动范围
