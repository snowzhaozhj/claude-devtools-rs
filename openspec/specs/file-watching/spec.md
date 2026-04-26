# file-watching Specification

## Purpose

监视 `~/.claude/projects/` 与 `~/.claude/todos/` 文件系统变化，以 debounce 后的 broadcast 通道把 `file-change` / `todo-change` 事件分发给多类订阅者（Tauri IPC 层、HTTP SSE、in-process 通知 pipeline），使前端 UI 与后台服务能够实时感知会话与待办变更。

## Requirements

### Requirement: Watch Claude projects directory for session changes

系统 SHALL 递归监视 `~/.claude/projects/`，在 `.jsonl` 会话文件创建、修改、删除时发出变更事件。

#### Scenario: New session file created
- **WHEN** 一个新的 `.jsonl` 文件出现在被监视的项目目录下
- **THEN** 订阅者 SHALL 在 debounce 窗口内收到一条 `file-change` 事件，携带 project id 与 session id

#### Scenario: Existing session file appended
- **WHEN** 已存在的 `.jsonl` 文件被追加内容
- **THEN** 订阅者 SHALL 收到对应 session 的 `file-change` 事件

#### Scenario: Session file deleted
- **WHEN** `.jsonl` 文件被删除
- **THEN** 订阅者 SHALL 收到带删除指示的 `file-change` 事件

### Requirement: Watch Claude todos directory

系统 SHALL 监视 `~/.claude/todos/` 下 `.json` 文件变化，并发出携带 session id 的 `todo-change` 事件。

#### Scenario: Todo file updated
- **WHEN** `~/.claude/todos/<sessionId>.json` 被更新
- **THEN** 订阅者 SHALL 收到携带该 session id 的 `todo-change` 事件

### Requirement: Debounce rapid file events

系统 SHALL 把同一文件在 100ms 窗口内的连续变更事件合并为一条事件后发出。

#### Scenario: Burst of writes
- **WHEN** 一个文件在 30ms 内发生 5 次写事件
- **THEN** 订阅者 SHALL 在 debounce 窗口结束后**恰好**收到一条 `file-change` 事件

### Requirement: Survive transient filesystem errors

系统 SHALL 记录并忽略瞬时错误（permission denied、临时锁占用），不终止 watcher。

#### Scenario: Temporary permission error on one file
- **WHEN** watcher 对单个文件 stat 时遇到权限错误
- **THEN** watcher SHALL 记录错误并继续监视其他文件

### Requirement: Broadcast events to multiple subscribers

系统 SHALL 把每条已发出的事件无差别地分发给所有当前活跃的订阅者（Electron renderer 经 IPC、HTTP 客户端经 SSE、in-process 后台服务如通知 pipeline），不重复也不遗漏。

#### Scenario: Two subscribers present
- **WHEN** 一次文件变更触发事件且当前有两个订阅者
- **THEN** 两个订阅者 SHALL 各自**恰好**收到一次该事件

#### Scenario: Notification pipeline subscribes alongside IPC consumers
- **WHEN** 通知 pipeline 启动时调用 `subscribe_files()`，同时 Tauri IPC 层也持有一个订阅
- **THEN** 两个订阅者 SHALL 独立收到每一次 debounce 后的 `FileChangeEvent`，且任一订阅者的滞后 SHALL NOT 影响另一订阅者的投递
