## MODIFIED Requirements

### Requirement: file-change payload 形态

`file-change` push event 通知前端某个 session 文件发生变更（新增 / 修改 / 删除）。Payload 字段 SHALL 含：

- `projectId`（camelCase）/ `project_id`（SSE wire）：标识项目
- `sessionId` / `session_id`：标识 session
- `deleted`：布尔值，标记文件是否被删除
- `projectListChanged` / `project_list_changed`：布尔值，标记是否影响项目列表
- `sessionListChanged` / `session_list_changed`：布尔值，标记该事件是否会改变某 group 内 session 集合（已知 project 下首次见 session / 删除 / 重命名等场景为 `true`；普通内容追加为 `false`）
- `mtimeMs` / `mtime_ms`：可选整数，事件涉及文件的 mtime（毫秒 since UNIX epoch）。watcher 在能取到 mtime 时 SHALL 填入；取不到（典型：SFTP server 不返 mtime / 删除事件）SHALL 省略字段

`sessionListChanged` 字段缺失时消费方 SHALL 视为 `false`（向后兼容退化——不触发 loadProjects 刷新）。

`mtimeMs` / `mtime_ms` 字段缺失时消费方 SHALL 视为"无 hint"——退化到既有行为：cache 仍按三档 invalidate 决策；后端 `ProjectScanCache` mtime overlay 路径不消费该事件（详 `[[ipc-data-api]]` 同名 capability 的 overlay Requirement）。

HTTP/SSE wire 形态：`{"type":"file_change","project_id":"...","session_id":"...","deleted":false,"project_list_changed":false,"session_list_changed":false,"mtime_ms":1234567890123}`（`mtime_ms` 为 optional，缺失时整字段省略）。

Tauri IPC 形态：`{ projectId: "...", sessionId: "...", deleted: false, projectListChanged: false, sessionListChanged: false, mtimeMs: 1234567890123 }`（`mtimeMs` 为 optional，缺失时整字段省略）。

`sessionListChanged` 字段填写规则由 `[[file-watching]]` 的 watcher 视角 Requirement 定义（unified invalidator enrich + SSH polling 对称填写）。`mtimeMs` 字段填写规则同样由 `[[file-watching]]` 定义（本地 watcher 在既有 deleted 判定路径产出 + SSH polling 透传 fingerprint mtime）。本 capability 仅定义字段名 / 字段类型 / 字段语义。

#### Scenario: file-change payload camelCase（Tauri IPC 路径）

- **WHEN** Tauri host emit 一条 `file-change` 事件
- **THEN** 序列化后的 JSON SHALL 使用 camelCase 字段名（`projectId` / `sessionId` / `deleted` / `projectListChanged` / `sessionListChanged` / `mtimeMs`），与既有 IPC 类型约定一致

#### Scenario: file-change payload snake_case（HTTP/SSE wire）

- **WHEN** HTTP/SSE 通过 PushEvent::FileChange 序列化一条 file-change 事件
- **THEN** 输出 SHALL 含 `"type":"file_change"` + 内部字段 `project_id` / `session_id` / `deleted` / `project_list_changed` / `session_list_changed`（snake_case），并在能取到 mtime 时 SHALL 含 `mtime_ms`

#### Scenario: sessionListChanged 字段缺失向后兼容

- **WHEN** 旧后端（未升级）发出的 file-change payload 缺 `sessionListChanged`（IPC）或 `session_list_changed`（SSE）字段
- **THEN** 消费方 SHALL 视为 `false`，行为退化为"不触发 loadProjects 刷新"

#### Scenario: mtimeMs 字段缺失向后兼容

- **WHEN** 旧后端（未升级 / 远端 SFTP 不返 mtime / 删除事件等场景）发出的 file-change payload 缺 `mtimeMs`（IPC）或 `mtime_ms`（SSE）字段
- **THEN** 消费方 SHALL 视为"无 mtime hint"
- **AND** 后端 `ProjectScanCache` mtime overlay 路径 SHALL NOT 因此 event 推进任何 project 的 overlay
- **AND** 行为退化到既有路径：仅当 watcher 字段 `projectListChanged` / `sessionListChanged` / `deleted` 命中三档时仍按 `[[ipc-data-api]]::ProjectScanCache 按事件语义分级失效` 决定 invalidate

#### Scenario: mtimeMs 单调推进（同一 session 连续 append）

- **WHEN** 同一 session jsonl 在两次 watcher event 之间持续被 append，前后 mtime 单调递增
- **THEN** 两条 file-change payload SHALL 各自携带对应时刻的 `mtimeMs` / `mtime_ms`（后者 ≥ 前者）
- **AND** 即便后端 / 前端中间有事件丢弃或乱序，最大值反映最新 mtime 的语义 SHALL 不被破坏（消费方按 max 合并即可）
