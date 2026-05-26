# push-events Specification

## Purpose

让桌面端与浏览器调试端在后端状态变化时即时更新——文件变更、元数据扫描完成、错误检出、广播丢失。push-events 是所有跨进程 push event payload 字段形态的单一定义处：Tauri webview event 与 HTTP `/api/events` SSE 两条 transport 共享同一 payload schema，消费方（sidebar / session detail / notification / connection indicator 等）按本 capability 定义的字段名与字段语义解析。
## Requirements
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

### Requirement: Emit push events for file changes and notifications

系统 SHALL 从 main 进程向 renderer 推送以下事件：session 文件变更、todo 文件变更、新通知、SSH 状态变化、context 切换、updater 进度。

桌面（Tauri）host SHALL 在 `setup` 阶段订阅内部 file-change broadcast（enriched event 流——见 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement），并向前端 webview `emit("file-change", payload)`。Payload 形态契约见 `[[push-events::file-change]]`。

桌面 host SHALL 在 `setup` 阶段订阅 SSH 状态 broadcast，并向前端 webview `emit("ssh_status", payload)`。Payload 形态契约见 `[[push-events::ssh-status-change]]`。同样订阅 context-changed broadcast emit `context_changed` 事件 payload `{ activeContextId, kind }`。

**lag 兜底契约（transport 层行为）**：

- **Tauri host file-change bridge** 在 broadcast receiver `Lagged(n)` 路径 SHALL 通过 `app.emit("sse-lagged", payload)` 通知前端 webview（payload 形态见 `[[push-events::sse-lagged]]`）；`Closed` 时退出 loop
- **Tauri host 其它 bridge**（如 metadata / error）在同类 `Lagged` 路径 SHALL 走同形 sse-lagged 通知
- 前端 Sidebar 收到 `sse-lagged` SHALL 按 `[[sidebar-navigation]]` 已有 Requirement 走保守 silent refresh，覆盖 lag 期间错过的 structural 信号

#### Scenario: New notification while renderer is subscribed

- **WHEN** renderer 已订阅通知事件，期间产出一条新通知
- **THEN** renderer SHALL 在 debounce 窗口内收到一条 push 事件，携带通知 payload

#### Scenario: Tauri 转发 file-change 事件

- **WHEN** 后端 file-change broadcast 产出一条事件
- **AND** Tauri host 在 `setup` 已 spawn 桥任务订阅该 broadcast
- **THEN** webview SHALL 通过 `listen("file-change", ...)` 收到 payload，字段形态与 `[[push-events::file-change]]` 一致

#### Scenario: file_tx 满时 Tauri bridge 通知前端 sse-lagged

- **WHEN** Tauri host file-change bridge 的 broadcast receiver 返回 Lagged(n)（broadcast capacity 满 + slow subscriber）
- **THEN** bridge SHALL emit `sse-lagged` 通知前端（payload 形态见 `[[push-events::sse-lagged]]`）
- **AND** bridge SHALL NOT 退出 loop，继续处理后续 event

#### Scenario: file-change 桥与通知管线并存

- **WHEN** Tauri host 同时持有 file-change bridge 与 detected-error bridge 两个订阅
- **THEN** 两个桥 SHALL 独立运行，文件变更不会因通知 pipeline 的 lag 被丢弃，反之亦然

#### Scenario: ssh_status event broadcast on connect

- **WHEN** 后端 SSH 连接状态从 `connecting` 切到 `connected`
- **AND** Tauri host 在 setup 已 spawn 桥任务订阅 SSH 状态 broadcast
- **THEN** webview SHALL 通过 `listen("ssh_status", ...)` 收到 payload，字段形态与 `[[push-events::ssh-status-change]]` 一致
- **AND** 成功路径下 payload `error` 与 `authChain` 字段 SHALL 为 null 或省略

#### Scenario: ssh_status event carries error detail on failure

- **WHEN** SSH 连接失败导致状态切到 `error`
- **THEN** webview 收到的 `ssh_status` payload SHALL 含 `error` 字段（错误详情）

### Requirement: Stream detected errors to subscribers

系统 SHALL 在数据 API 层上暴露一个 in-process 订阅机制，让宿主 runtime（例如 Tauri 应用）能够接收自动通知 pipeline 产出的新检测错误，无需轮询持久化通知存储。

#### Scenario: Tauri runtime subscribes and forwards to renderer
- **WHEN** Tauri runtime 在应用 setup 时调用 `subscribe_detected_errors()`
- **AND** 通知 pipeline 产出一条新的 `DetectedError`
- **THEN** 订阅者持有的接收端 SHALL yield 该 `DetectedError`，宿主可据此向前端 emit 一个事件（例如 `notification-added`）

#### Scenario: Subscription without a watcher attached
- **WHEN** 数据 API 层通过不带 watcher 的构造器实例化（集成测试或仅 HTTP 宿主路径）
- **AND** 调用方调用 `subscribe_detected_errors()`
- **THEN** 调用 SHALL 返回一个永不 yield 的有效接收端（静默 no-op），而非错误

#### Scenario: Multiple subscribers receive the same error
- **WHEN** 两个独立订阅者各自调用 `subscribe_detected_errors()`
- **AND** pipeline 产出一条 `DetectedError`
- **THEN** 两个订阅者 SHALL 各自**恰好**收到一次同一条 `DetectedError`

### Requirement: Emit session metadata updates

系统 SHALL 在数据 API 层上暴露 in-process 订阅机制 `subscribe_session_metadata()`，返回一个可接收 `SessionMetadataUpdate` 的接收端。`SessionMetadataUpdate` SHALL 携带 `projectId` / `sessionId` / `title` / `messageCount` / `isOngoing` / `gitBranch`（序列化时为 camelCase）。Tauri host SHALL 把该订阅桥接到 webview，向前端 emit `session-metadata-update` 事件。

`list_sessions` 的骨架阶段 SHALL 对每条 `(session_id, jsonl_path)` 先调用 `try_lookup_cached_metadata`（lookup-only fast-path：查 MetadataCache + `FileSignature` 等价校验 + stale 实时合成 `isOngoing`，**不**触发扫描）。命中条 SHALL 在骨架阶段直接 inline 填回 `title` / `messageCount` / `isOngoing` / `gitBranch` 真实值，且 SHALL NOT 入 `page_jobs`（即不 spawn 后台扫描、不推送对应 update）；未命中场景包括 cache miss、stat 失败、`FileSignature` 不等（mtime / size / identity 任一不等）—— 任一未命中条 SHALL 入 `page_jobs` 走原后台扫描路径，扫完通过广播推送 update。

骨架阶段的 lookup 并发度 SHALL 通过限流机制（上限 8）控制，与后台扫描使用同一上限常量。后台扫描自身的并发度 SHALL 同样被限流（固定上限 8），避免 50+ 文件同时打开；每次 `list_sessions(projectId, pagination)` 触发新扫描前 SHALL 取消同一 `(projectId, cursor)` 维度上一轮未完成的扫描，避免**同分页**的事件串扰；不同 `cursor` 的扫描 SHALL 并存而互不 abort。扫描范围 SHALL 限定为本次 `list_sessions` 返回页中的**未命中** sessions；实现 MUST NOT 因为请求第一页而后台扫描完整项目历史。

cache 全命中场景下 `page_jobs` 为空时 `list_sessions` SHALL 跳过 spawn 后台扫描分支，**不**触碰 active_scans 注册表。

active_scans 注册表的 key SHALL 为 `(projectId, cursor)` 组合编码字符串。同 key 抢占 + per-key generation cleanup 的 race-free 语义不变。

**后台扫描按 backend kind 分流**：cache miss 后 dispatch 函数 SHALL 按 context 的 backend kind 选择：

- **Local backend**：调既有 per-session 扫描路径
- **SSH backend**：调 batched helper，工作流为：
  1. 一次 read_dir_with_metadata 操作拿全 dir entry metadata（限流 ≤ 8 并发）
  2. build path → metadata 索引
  3. 逐条 page_jobs：索引命中 → lookup_with_known_signature cache 命中 → 广播现值；mismatch → spawn sub-task 走 cache wrapper miss 路径
  4. dir read 失败 SHALL fallback 到 per-session 路径

两条路径 SHALL 共享 active_scans 注册表、限流机制（上限 8）、context_generation 与 root_generation 双轴 race-free 校验、广播形态。所有广播调用前 SHALL 校验 root_generation **与** context_generation 同时匹配，否则 silent drop。

batched helper 内部的 mismatch sub-task SHALL 通过 JoinSet 持有，顶层 abort 时 sub-task 跟随 drop 自动 abort。JoinSet cleanup 循环 SHALL 显式处理 join 错误。

**SSH backend 全命中广播例外**：SSH ctx 下即使骨架阶段全命中（SSH 路径 need_background_validation 恒为 true），SSH 路径仍 SHALL spawn batched task 异步校验并广播 cache 现值；Local backend 全命中仍 SHALL 不 spawn 不广播。

#### Scenario: 订阅接收当前页未命中条的元数据更新

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 随后调用 `list_sessions("projectA", { pageSize: 3, cursor: null })`，所有 session 均 cache miss
- **THEN** receiver SHALL 在扫描完成后**最多**收到 3 条 `SessionMetadataUpdate`

#### Scenario: Tauri host emit session-metadata-update

- **WHEN** Tauri host 在 setup 订阅并产出 `SessionMetadataUpdate`
- **THEN** webview SHALL 通过 `listen("session-metadata-update", ...)` 收到 camelCase payload

#### Scenario: 同 projectId 同 cursor 的新扫描取消旧扫描

- **WHEN** 同 cursor 的 `list_sessions` 正在扫描中，调用方再次调用（同 cursor）
- **THEN** 旧扫描 SHALL 被 abort；新扫描只扫新页未命中 sessions

#### Scenario: 同 projectId 不同 cursor 的扫描并存互不 abort

- **WHEN** page 1 正在扫描，调用方调 page 2
- **THEN** page 1 扫描 SHALL 继续运行；page 2 启动独立扫描

#### Scenario: 后台扫描并发度限制

- **WHEN** 扫描任务在并发处理某页 50 个 cache-miss session 文件
- **THEN** 同一时刻打开的 JSONL 文件句柄数 SHALL 不超过 8

#### Scenario: 骨架 lookup 并发度限制

- **WHEN** 骨架阶段对 50 个 session 并发执行 `try_lookup_cached_metadata`
- **THEN** 同一时刻进行 stat 操作的 future 数 SHALL 不超过 8

#### Scenario: Local backend cache 命中时骨架直接带值且零 emit

- **WHEN** Local context 下已有 cache 命中，再次调用 `list_sessions`（文件未变）
- **THEN** 骨架阶段 SHALL 直接携带真实元数据
- **AND** receiver SHALL 在短时间内（如 300 ms）**不**收到任何新的 `SessionMetadataUpdate`

#### Scenario: Local backend cache 全命中时不触发 spawn 不触碰 active_scans

- **WHEN** Local context 下所有 session 都 cache 命中
- **THEN** SHALL NOT spawn 后台扫描，SHALL NOT 改动 active_scans

#### Scenario: SSH backend cache 全命中仍 spawn batched 校验

- **WHEN** SSH context 下所有 session 都 trust cached 命中，但 SSH need_background_validation 恒为 true
- **THEN** 仍 SHALL spawn batched 校验并广播 cache 现值

#### Scenario: lookup stat 失败 fallback 到后台扫描

- **WHEN** `try_lookup_cached_metadata` 内 stat 返回错误
- **THEN** 函数 SHALL 返回 None，该 session 入后台扫描

#### Scenario: SSH ctx 后台校验走 batch read_dir_with_metadata

- **WHEN** SSH context 下 page_jobs 非空
- **THEN** dispatch SHALL 走 batched helper；首先调一次 read_dir_with_metadata

#### Scenario: SSH ctx batch helper dir read 失败时 fallback

- **WHEN** batched helper 调 read_dir_with_metadata 返回错误
- **THEN** SHALL fallback 到 per-session 路径，日志记录让运维可见

#### Scenario: Local ctx 后台扫描走既有 per-session 路径不变

- **WHEN** Local context 下 page_jobs 非空
- **THEN** dispatch SHALL 走 per-session 路径，不走 batched

