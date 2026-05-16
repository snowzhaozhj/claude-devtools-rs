## MODIFIED Requirements

### Requirement: Watch project directory additions

系统 SHALL 在 `~/.claude/projects/` 运行中新建一级 project 目录或该 project 下首个 `.jsonl` 会话文件时发出可被订阅者识别的项目刷新事件。该事件用于触发项目列表重扫，不替代 `project-discovery` 的权威扫描结果。系统 MUST NOT 因已知 project 下的普通 `.jsonl` 修改反复标记项目列表变化。

顶层 dir-create 事件（`<projects_dir>/<project_id>/` 目录创建本身）SHALL emit `FileChangeEvent { project_id, session_id: "", project_list_changed: true }`。该分支 MUST NOT 调用 `mark_project_seen` 写入 `known_projects` —— "首次见到 `project_id`"的标记 SHALL 在紧随的第一条 `<projects_dir>/<project_id>/<session_id>.jsonl` 写入事件中独占消耗，使该 jsonl 事件 SHALL 仍 emit `project_list_changed=true`，触发前端项目列表重扫时 scanner 能看到已落盘的 jsonl。理由：dir-create 事件触发的 scan 在空目录下因 `project-discovery` 的 `scan_project_dir` 会跳过无 `.jsonl` 的目录而拿不到新 project，必须依赖 jsonl 事件**再次**触发刷新；若 dir-create 提前消耗 mark，后续 jsonl 事件会降级为 `project_list_changed=false`，前端永不重扫。

#### Scenario: New project directory created

- **WHEN** watcher 已启动，且 `~/.claude/projects/new-project/` 被创建
- **THEN** 订阅者 SHALL 在 debounce 窗口内收到一条表示项目列表可能变化的事件
- **AND** 该事件 SHALL 携带新 project id 或足够的信息让 UI 触发项目列表重扫

#### Scenario: First session file in new project created

- **WHEN** watcher 已启动，且 `~/.claude/projects/new-project/session-a.jsonl` 被创建
- **THEN** 订阅者 SHALL 收到可触发项目列表重扫的事件
- **AND** 订阅者 SHALL 仍能收到针对 `session-a` 的 `file-change` 事件

#### Scenario: dir-create followed by first jsonl both signal project list change

- **WHEN** watcher 已启动，先收到 `~/.claude/projects/new-project/` 顶层目录创建事件，紧随其后收到该 project 下首个 `~/.claude/projects/new-project/session-a.jsonl` 写入事件
- **THEN** 订阅者 SHALL 先收到一条 `FileChangeEvent { project_id: "new-project", session_id: "", project_list_changed: true }`
- **AND** 订阅者 SHALL 接着收到一条 `FileChangeEvent { project_id: "new-project", session_id: "session-a", project_list_changed: true }`
- **AND** dir-create 分支 MUST NOT 把 `new-project` 写入 `known_projects` —— 首次 mark 由 jsonl 事件独占消耗

#### Scenario: dir-create does not consume mark_project_seen

- **WHEN** watcher 已启动，收到 `~/.claude/projects/never-written/` 顶层目录创建事件，但此后**未**写入任何 `.jsonl`
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "never-written", session_id: "", project_list_changed: true }`
- **AND** `known_projects` 内部状态 MUST NOT 包含 `never-written`，使得未来该 project 下首次出现 `.jsonl` 时仍能 emit `project_list_changed=true`

#### Scenario: Project refresh event is broadcast to all subscribers

- **WHEN** 新 project 目录触发项目刷新事件且当前有两个订阅者
- **THEN** 两个订阅者 SHALL 各自收到该事件，任一订阅者滞后 SHALL NOT 阻塞另一订阅者

#### Scenario: Existing project session change does not refresh projects

- **WHEN** watcher 已启动，且已知 project 下的 `~/.claude/projects/project-a/session-a.jsonl` 被追加内容
- **THEN** 订阅者 SHALL 收到针对 `session-a` 的 `file-change` 事件
- **AND** 该事件 MUST NOT 标记项目列表变化

#### Scenario: Nested jsonl is ignored by project watcher

- **WHEN** watcher 收到 `~/.claude/projects/project-a/subagents/agent-a.jsonl` 的变化
- **THEN** `file-watching` SHALL NOT 把 `project-a/subagents` 当成 project id 发出 session `file-change`
