## ADDED Requirements

### Requirement: Watch project directory additions

系统 SHALL 在 `~/.claude/projects/` 运行中新建一级 project 目录或该 project 下首个 `.jsonl` 会话文件时发出可被订阅者识别的项目刷新事件。该事件用于触发项目列表重扫，不替代 `project-discovery` 的权威扫描结果。系统 MUST NOT 因已知 project 下的普通 `.jsonl` 修改反复标记项目列表变化。

#### Scenario: New project directory created

- **WHEN** watcher 已启动，且 `~/.claude/projects/new-project/` 被创建
- **THEN** 订阅者 SHALL 在 debounce 窗口内收到一条表示项目列表可能变化的事件
- **AND** 该事件 SHALL 携带新 project id 或足够的信息让 UI 触发项目列表重扫

#### Scenario: First session file in new project created

- **WHEN** watcher 已启动，且 `~/.claude/projects/new-project/session-a.jsonl` 被创建
- **THEN** 订阅者 SHALL 收到可触发项目列表重扫的事件
- **AND** 订阅者 SHALL 仍能收到针对 `session-a` 的 `file-change` 事件

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
