# push-events Specification Changes

## ADDED Requirements

### Requirement: PushEvent enum 形态契约

系统 SHALL 定义跨进程 push event payload 为有限 variant 枚举（`PushEvent`），每个 variant 对应一类后端状态变化事件。所有 variant 通过统一 tagged union 形态序列化：JSON 顶层 SHALL 含 `type` 字段标识 variant 名（snake_case，如 `"file_change"` / `"session_metadata_update"` / `"detected_error"` / `"sse_lagged"` / `"ssh_status_change"`）；variant 内部字段保留 snake_case（HTTP/SSE wire 形态）。

Tauri IPC 路径 SHALL 对每个 variant 使用独立的 camelCase 形态直接 emit 到 webview event bus，字段名为 camelCase（如 `projectId` / `sessionId` / `sessionListChanged`）。前端浏览器 transport 收到 SSE wire（snake_case）SHALL 通过归一化层映射为与 Tauri 路径同形的 camelCase payload，让消费方代码不区分 transport 来源。

variant 清单 SHALL 随后续 Requirement 逐项定义。新增 variant 时 SHALL 同步更新本 capability spec。

**Scope 说明**：`context_changed` 事件（payload `{ activeContextId, kind }`）与 `updater` 事件不属 PushEvent enum（独立协议 / 未实现），不在本 capability 范围内。

#### Scenario: PushEvent 所有 variant 共享 type 字段

- **WHEN** 系统通过 HTTP/SSE 序列化任意 PushEvent variant
- **THEN** 输出 JSON 顶层 SHALL 含 `"type"` 字段，取值为该 variant 的 snake_case 名

#### Scenario: PushEvent variant 内部字段 snake_case（SSE wire）

- **WHEN** 系统通过 HTTP/SSE 序列化含多字段的 PushEvent variant
- **THEN** 该 variant 内部字段名 SHALL 为 snake_case（如 `project_id` / `session_list_changed`）

#### Scenario: Tauri IPC 路径字段 camelCase

- **WHEN** Tauri host 通过 `app.emit(event_name, payload)` 推送 push event 到 webview
- **THEN** payload 字段名 SHALL 为 camelCase（如 `projectId` / `sessionListChanged`）

### Requirement: file-change payload 形态

`file-change` push event 通知前端某个 session 文件发生变更（新增 / 修改 / 删除）。Payload 字段 SHALL 含：

- `projectId`（camelCase）/ `project_id`（SSE wire）：标识项目
- `sessionId` / `session_id`：标识 session
- `deleted`：布尔值，标记文件是否被删除
- `projectListChanged` / `project_list_changed`：布尔值，标记是否影响项目列表
- `sessionListChanged` / `session_list_changed`：布尔值，标记该事件是否会改变某 group 内 session 集合（已知 project 下首次见 session / 删除 / 重命名等场景为 `true`；普通内容追加为 `false`）

`sessionListChanged` 字段缺失时消费方 SHALL 视为 `false`（向后兼容退化——不触发 loadProjects 刷新）。

HTTP/SSE wire 形态：`{"type":"file_change","project_id":"...","session_id":"...","deleted":false,"project_list_changed":false,"session_list_changed":false}`。

Tauri IPC 形态：`{ projectId: "...", sessionId: "...", deleted: false, projectListChanged: false, sessionListChanged: false }`（webview event name 为 `"file-change"`）。

`sessionListChanged` 字段填写规则由 `[[file-watching]]` 的 watcher 视角 Requirement 定义（unified invalidator enrich + SSH polling 对称填写）。本 capability 仅定义字段名 / 字段类型 / 字段语义。

#### Scenario: file-change payload camelCase（Tauri IPC 路径）

- **WHEN** Tauri host emit 一条 `file-change` 事件
- **THEN** 序列化后的 JSON SHALL 使用 camelCase 字段名（`projectId` / `sessionId` / `deleted` / `projectListChanged` / `sessionListChanged`），与既有 IPC 类型约定一致

#### Scenario: file-change payload snake_case（HTTP/SSE wire）

- **WHEN** HTTP/SSE 通过 PushEvent::FileChange 序列化一条 file-change 事件
- **THEN** 输出 SHALL 含 `"type":"file_change"` + 内部字段 `project_id` / `session_id` / `deleted` / `project_list_changed` / `session_list_changed`（snake_case）

#### Scenario: sessionListChanged 字段缺失向后兼容

- **WHEN** 旧后端（未升级）发出的 file-change payload 缺 `sessionListChanged`（IPC）或 `session_list_changed`（SSE）字段
- **THEN** 消费方 SHALL 视为 `false`，行为退化为"不触发 loadProjects 刷新"

### Requirement: session-metadata-update payload 形态

`session-metadata-update` push event 通知前端某个 session 的元数据扫描完成。Payload 字段 SHALL 含：

- `projectId` / `project_id`：标识项目
- `sessionId` / `session_id`：标识 session
- `title`：session 标题（可选）
- `messageCount` / `message_count`：消息数
- `isOngoing` / `is_ongoing`：布尔值，标记是否正在进行
- `gitBranch` / `git_branch`：Git 分支名（可选）
- `groupId` / `group_id`：标识该 session 所属的 repository group（取自 worktree git common dir）。多 worktree group 场景下 `groupId` 与 `projectId` 不等值（`groupId` 为 `.git` 后缀路径，`projectId` 为 encoded 项目标识符）。`groupId` 缺失时消费方 SHALL fallback 到 `projectId`（单 worktree group 下二者等值）。

HTTP/SSE wire 形态：`{"type":"session_metadata_update","project_id":"...","session_id":"...","title":"...","message_count":N,"is_ongoing":false,"git_branch":"...","group_id":"..."}`。

Tauri IPC 形态：webview event name 为 `"session-metadata-update"`，payload 字段 camelCase（`projectId` / `sessionId` / `title` / `messageCount` / `isOngoing` / `gitBranch` / `groupId`）。

#### Scenario: session-metadata-update payload 含 groupId

- **WHEN** 后端完成一条 session 元数据扫描并推送 session-metadata-update
- **THEN** payload SHALL 含 `groupId`（IPC）/ `group_id`（SSE）字段

#### Scenario: session-metadata-update 缺 groupId 向后兼容

- **WHEN** 旧后端发出的 session-metadata-update payload 缺 `groupId` / `group_id` 字段
- **THEN** 消费方 SHALL fallback 到 `projectId` / `project_id`——单 worktree group 下仍能匹配；多 worktree group 下保持既有行为不报错

### Requirement: detected-error payload 形态

`detected-error` push event 通知前端检测到新的错误。系统 SHALL 将检测到的错误推送到所有已连接的 transport 端点。Tauri host emit 到 webview event name `"notification-added"`；HTTP/SSE wire 形态 type 为 `"detected_error"`。

payload 字段 SHALL 含确定性 id（用于去重）、错误分类、来源 project / session 标识。具体字段集由 `[[notification-triggers]]` 的 error detection pipeline 产出结构决定；本 capability 仅定义 transport 序列化形态（type tag + snake_case wire / camelCase IPC 双形态）。

#### Scenario: 错误通知同时推送到 Tauri 端和 SSE 客户端

- **WHEN** notification pipeline 产出一条新 DetectedError
- **THEN** Tauri host bridge SHALL emit webview event `"notification-added"` 携带该 error payload
- **AND** HTTP/SSE bridge SHALL 序列化为 `{"type":"detected_error",...}` 推送到 SSE 客户端

### Requirement: sse-lagged payload 形态

`sse-lagged` push event 通知前端某条 broadcast 上游因容量打满丢失了事件。Payload 字段 SHALL 含：

- `source`：字符串，标识丢失事件来源的 broadcast 标识符（如 `"file-change"`）。取值不限定固定清单——由产生 lag 的具体 broadcast bridge 决定（best-effort 诊断信息）。
- `missed`：正整数，标识丢失的事件数量。

HTTP/SSE wire 形态：`{"type":"sse_lagged","source":"...","missed":N}`。该形态 SHALL 与既有 SSE lagged sentinel `'{"type":"sse_lagged"}'` 向后兼容——前端解析 `type === "sse_lagged"` 走统一 handler；旧 sentinel 缺 `source` / `missed` 字段时前端读 undefined 不报错。

Tauri IPC 路径：webview event name 为 `"sse-lagged"`，payload `{ source: "...", missed: N }`。

#### Scenario: sse-lagged payload 含 source 和 missed

- **WHEN** 某条 broadcast bridge 检测到 Lagged(n) 错误
- **THEN** 系统 SHALL 推送 sse-lagged event，payload 含 `source`（标识来源 broadcast）和 `missed`（丢失事件数）

#### Scenario: sse-lagged 缺失 source/missed 字段时前端不报错

- **WHEN** 前端收到 sse-lagged payload 缺 `source` / `missed` 字段（旧版 sentinel）
- **THEN** 前端 SHALL 仍走 sse-lagged handler（按 `type === "sse_lagged"` 判定），读 `source` / `missed` 为 undefined 不报错

### Requirement: ssh-status-change payload 形态

`ssh-status-change` push event 通知前端 SSH 连接状态变化。HTTP/SSE wire payload SHALL 含 `type` 为 `"ssh_status_change"` + 字段 `context_id` / `state`（snake_case）。

Tauri IPC 路径：webview event name 为 `"ssh_status"`，payload camelCase 字段（`contextId` / `status` / `error?` / `authChain?`）。

前端浏览器 transport 归一化 SHALL 映射 `payload.context_id` → `contextId`、`payload.state` → `status`，输出与 Tauri 路径同形的 camelCase payload。

#### Scenario: ssh-status-change SSE wire 形态

- **WHEN** 后端 SSH 连接状态变化通过 PushEvent::SshStatusChange 序列化到 SSE
- **THEN** 输出 SHALL 含 `"type":"ssh_status_change"` + 字段 `context_id` / `state`（snake_case）

#### Scenario: ssh-status-change 成功路径 error/authChain 省略

- **WHEN** SSH 连接成功（state = `"connected"`）
- **THEN** payload 的 `error` 与 `authChain` 字段 SHALL 为 null 或省略

### Requirement: todo-change payload 形态

`todo-change` push event 通知前端 todo 文件发生变更。Payload 字段 SHALL 含：

- `sessionId` / `session_id`：标识 session（todo 文件名仅含 session_id）
- `projectId` / `project_id`：SHALL 填空字符串占位以保留 schema 一致（todo 文件不归属单一 project）

HTTP/SSE wire 形态：`{"type":"todo_change","project_id":"","session_id":"..."}`。

Tauri IPC 路径：webview event name 为 `"todo-change"`，payload camelCase 字段（`projectId: ""` / `sessionId`）。

#### Scenario: todo-change payload project_id 为空字符串占位

- **WHEN** todo watcher 检测到 todo 文件变更并推送 todo-change event
- **THEN** payload `projectId`（IPC）/ `project_id`（SSE）字段 SHALL 为空字符串
