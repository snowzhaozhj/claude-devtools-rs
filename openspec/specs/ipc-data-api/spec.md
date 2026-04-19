# ipc-data-api Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Expose project and session queries

The system SHALL expose data queries for projects and sessions over a request/response IPC channel set, including at minimum: list projects, list sessions for a project (with pagination), get session detail, get session metrics, get waterfall data, and get subagent detail.

`get_session_detail` SHALL 在返回 session 详情时集成 subagent 解析：从同 project 的其他 session 中扫描候选 subagent，调用 `resolve_subagents` 填充 `AIChunk.subagents` 字段。若扫描失败或无候选，`subagents` SHALL 为空数组（不报错）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.subagents[i].messages` 数组 MUST 默认被裁剪为空 Vec，且 `messagesOmitted=true`** —— 用于把首屏 IPC payload 控制在原大小的 ~40%（subagent 嵌套 chunks 全文是大头，行为契约见 change `subagent-messages-lazy-load`）。`Process.headerModel` / `Process.lastIsolatedTokens` / `Process.isShutdownOnly` 三个 derived 字段 MUST 在 `candidate_to_process` 阶段由 messages 预算后填充，让 SubagentCard header 不依赖完整 `messages` 也能正常渲染。回滚开关 `OMIT_SUBAGENT_MESSAGES: bool` 设 false 时 SHALL 退回完整 payload（messages 不裁剪、`messagesOmitted=false`）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `ContentBlock::Image.source.data` 字段 MUST 默认被替换为空字符串 `""`，且同时设 `source.dataOmitted=true`** —— 用于把首屏 IPC payload 中内联截图的 base64 字符串裁掉（行为契约见本 spec `Lazy load inline image asset` Requirement）。`source.kind` / `source.media_type` 字段 SHALL 保留（前端渲染时仍需要），仅 `data` 字段被清空。回滚开关 `OMIT_IMAGE_DATA: bool` 设 false 时 SHALL 退回完整 base64 payload（`data` 保留原值、`dataOmitted=false`）。该裁剪 SHALL 应用于所有 chunk 类型（UserChunk / AIChunk responses / subagent.messages 内嵌套——但因 subagent.messages 默认已被裁剪，仅在回滚 `OMIT_SUBAGENT_MESSAGES=false` 时才会触及嵌套层）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.responses[i].content` 字段 MUST 默认被替换为空 `MessageContent::Text("")`，且同时设 `contentOmitted=true`** —— 用于把首屏 IPC payload 最大单一字段（实测 46a25772 case 1257 KB / 41%）裁掉（行为契约见 change `session-detail-response-content-omit`）。该字段在前端任何代码路径中都不被读取（chunk 显示文本走 `semanticSteps` 的 thinking / text 步骤），裁剪后 UI 渲染零变化。回滚开关 `OMIT_RESPONSE_CONTENT: bool` 设 false 时 SHALL 退回完整 payload（`content` 携带原 `MessageContent`、`contentOmitted=false`）。该裁剪 SHALL 应用于顶层 AIChunk 及 subagent.messages 嵌套层（与 `OMIT_IMAGE_DATA` 同模式：在 `OMIT_SUBAGENT_MESSAGES=true` 默认路径下嵌套层为 no-op；`OMIT_SUBAGENT_MESSAGES=false` 回滚时仍能命中嵌套层）。

`list_sessions` 返回的每个 `SessionSummary` MUST 携带 `sessionId` / `projectId` / `timestamp` 的真实值（可直接从目录扫描得出），但 `title` / `messageCount` / `isOngoing` SHALL 允许为占位值（`null` / `0` / `false`）——这些元数据字段的真实值由后端异步扫描后通过 `session-metadata-update` push event 逐条推送（见本 spec "Emit session metadata updates" requirement）。`get_session_detail` 返回的 `SessionDetail.isOngoing` 仍 MUST 为同步计算后的真实值（因为 detail 已在调用链内完成全文件解析）。

序列化 SHALL 使用 camelCase（`isOngoing`、`messagesOmitted`、`headerModel`、`lastIsolatedTokens`、`isShutdownOnly`、`dataOmitted`、`contentOmitted`）。例外：`ImageSource.media_type` 与 `ImageSource.type`（kind）保持 snake_case 与上游 Anthropic JSONL 格式一致——同 `TokenUsage` 例外。

HTTP API 路径（`GET /projects/:id/sessions`）SHALL 保留同步完整返回语义（不适用骨架化）——因 HTTP 无 push 通道；IPC 路径适用骨架化。HTTP 路径同样 SHALL NOT 应用 `OMIT_IMAGE_DATA` 与 `OMIT_RESPONSE_CONTENT` 裁剪（HTTP 当前无活跃用户、且无对应 asset 协议端点 / 懒拉接口，保留完整 payload 传输）。

#### Scenario: List projects

- **WHEN** a caller invokes the list-projects operation
- **THEN** the response SHALL contain all discovered projects with their id, decoded path, display name, and session count

#### Scenario: Paginated session list

- **WHEN** a caller invokes the paginated sessions operation with a page size and cursor
- **THEN** the response SHALL contain at most page-size entries and a next-cursor token if more exist

#### Scenario: Get session detail

- **WHEN** a caller requests detail for a session id
- **THEN** the response SHALL contain the built chunks, metrics, and metadata for that session

#### Scenario: Get session detail with subagent resolution

- **WHEN** a caller requests detail for a session that contains Task tool calls
- **THEN** the response SHALL include resolved subagent processes in the corresponding `AIChunk.subagents` fields, matched via the three-phase resolution algorithm (result-based → description-based → positional)

#### Scenario: Get session detail when no subagent candidates exist

- **WHEN** a caller requests detail for a session whose project has no other sessions or no matching candidates
- **THEN** `AIChunk.subagents` SHALL be empty arrays and no error SHALL be returned

#### Scenario: Subagent messages omitted by default

- **WHEN** `get_session_detail` 返回的 `SessionDetail` 含至少一个 `AIChunk.subagents[i]`
- **THEN** 该 subagent 的 `messages` 数组 SHALL 为空 `[]`，`messagesOmitted` SHALL 为 `true`
- **AND** `headerModel` / `lastIsolatedTokens` / `isShutdownOnly` SHALL 为后端预算后的真实值

#### Scenario: 回滚开关恢复完整 payload

- **WHEN** `OMIT_SUBAGENT_MESSAGES: bool = false`
- **THEN** `get_session_detail` 返回的 subagent `messages` SHALL 携带完整 chunks 流，`messagesOmitted` SHALL 为 `false`

#### Scenario: Image base64 data omitted by default

- **WHEN** `get_session_detail` 返回的 `SessionDetail.chunks` 含至少一个 `ContentBlock::Image`
- **THEN** 该 image block 的 `source.data` 字段 SHALL 为空字符串 `""`，`source.dataOmitted` SHALL 为 `true`
- **AND** `source.kind` 与 `source.media_type` 字段 SHALL 保留原值（用于前端 fallback 与 alt 渲染）

#### Scenario: Image OMIT 回滚开关恢复完整 base64

- **WHEN** `OMIT_IMAGE_DATA: bool = false`
- **THEN** `get_session_detail` 返回的所有 `ContentBlock::Image.source.data` SHALL 携带完整 base64 字符串，`source.dataOmitted` SHALL 为 `false`

#### Scenario: AssistantResponse content omitted by default

- **WHEN** `get_session_detail` 返回的 `SessionDetail.chunks` 含至少一个 `AIChunk.responses[i]`
- **THEN** 该 response 的 `content` 字段 SHALL 为空 `MessageContent::Text("")`，`contentOmitted` SHALL 为 `true`
- **AND** `uuid` / `timestamp` / `model` / `usage` / `toolCalls` 字段 SHALL 保留原值（前端 chunkKey / model summary / SubagentCard header 仍依赖）

#### Scenario: Response content OMIT 回滚开关恢复完整 payload

- **WHEN** `OMIT_RESPONSE_CONTENT: bool = false`
- **THEN** `get_session_detail` 返回的所有 `AIChunk.responses[i].content` SHALL 携带原 `MessageContent`，`contentOmitted` SHALL 为 `false`

#### Scenario: Response content OMIT 命中 subagent.messages 嵌套层

- **WHEN** `OMIT_SUBAGENT_MESSAGES: bool = false` 且 `OMIT_RESPONSE_CONTENT: bool = true`
- **THEN** `get_session_detail` 返回的 `AIChunk.subagents[i].messages` 内嵌套 `AIChunk.responses[j].content` SHALL 同样被替换为空 + `contentOmitted=true`

#### Scenario: list_sessions IPC 返回骨架元数据

- **WHEN** a caller invokes IPC `list_sessions(projectId)` for a project with N sessions
- **THEN** the response SHALL return within ~200ms carrying N `SessionSummary` entries, each with real `sessionId` / `projectId` / `timestamp` but `title = null` / `messageCount = 0` / `isOngoing = false`
- **AND** 后端 SHALL 在返回后 spawn 并发元数据扫描任务，每扫完一个 session 向订阅者广播一条 `SessionMetadataUpdate`

#### Scenario: 骨架返回后元数据通过 event 推送

- **WHEN** IPC `list_sessions(projectId)` 返回骨架后
- **AND** 后端扫描某个 session 文件完成（得出 title / messageCount / isOngoing）
- **THEN** 订阅者 SHALL 收到一条 `SessionMetadataUpdate { projectId, sessionId, title, messageCount, isOngoing }`；扫描全部完成前允许收到任意顺序、任意数量（0 到 N）的 updates

#### Scenario: SessionDetail carries isOngoing

- **WHEN** a caller invokes `get_session_detail` on a session id
- **THEN** the resulting `SessionDetail.isOngoing` SHALL be the true value computed from the full-file scan (not a placeholder)

#### Scenario: HTTP list_sessions 保留同步完整返回

- **WHEN** a caller invokes HTTP `GET /projects/:id/sessions`
- **THEN** the response SHALL contain `SessionSummary` entries with real `title` / `messageCount` / `isOngoing` values (同步扫描后返回)，不走骨架化路径

### Requirement: Expose search queries

The system SHALL expose search operations: search within one session, search across one project, and search across all projects. `search` SHALL 委托给 `SessionSearcher`（来自 `session-search` capability）执行真实的全文搜索，而非返回空结果。

#### Scenario: Search all projects via IPC
- **WHEN** a caller invokes the global search operation with a query
- **THEN** the response SHALL contain per-project match groups consistent with the `session-search` capability

#### Scenario: Search returns real results from SessionSearcher
- **WHEN** a caller invokes the search operation with a query that matches session content
- **THEN** the response SHALL contain search hits with `message_uuid`、`offset`、`preview` 和 `message_type` 字段

#### Scenario: Search with empty query
- **WHEN** a caller invokes the search operation with an empty query string
- **THEN** the response SHALL return an empty results array without error

### Requirement: Expose config and notification operations

The system SHALL expose config read/update operations and notification list/mark-read operations over IPC, matching the behavior described in `configuration-management` and `notification-triggers`.

#### Scenario: Update config field via IPC
- **WHEN** a caller invokes the config update operation
- **THEN** the change SHALL be persisted and the response SHALL contain the new configuration

### Requirement: Expose SSH and context operations

The system SHALL expose operations to list contexts, switch active context, connect/disconnect/test SSH, get SSH status, and resolve SSH host aliases.

#### Scenario: Resolve ssh host alias via IPC
- **WHEN** a caller requests to resolve an alias
- **THEN** the response SHALL contain the resolved hostname, port, user, and identity file path (or a clear error if not found)

### Requirement: Emit push events for file changes and notifications

The system SHALL push events from main to renderer for: session file changes, todo file changes, new notifications, SSH status changes, and updater progress.

桌面 (Tauri) host SHALL 在 `setup` 阶段订阅 `FileWatcher::subscribe_files()`
广播，并 `emit("file-change", payload)` 给前端 webview。Payload SHALL 是
`FileChangeEvent` 的 camelCase 序列化结果（字段 `projectId`、`sessionId`、
`deleted`），与其它 IPC payload 的命名约定一致。

#### Scenario: New notification while renderer is subscribed
- **WHEN** a new notification is emitted while the renderer has subscribed to notification events
- **THEN** the renderer SHALL receive a push event carrying the notification payload within the debounce window

#### Scenario: Tauri 转发 file-change 事件
- **WHEN** `cdt-watch::FileWatcher` 在 100 ms debounce 后产出
  `FileChangeEvent { project_id: "p", session_id: "s", deleted: false }`
- **AND** Tauri host 在 `setup` 已经 spawn 桥任务订阅 `subscribe_files()`
- **THEN** webview SHALL 通过 `listen("file-change", ...)` 收到 payload
  `{ projectId: "p", sessionId: "s", deleted: false }`

#### Scenario: file-change payload 是 camelCase
- **WHEN** Tauri 桥任务 emit 一条 `file-change` 事件
- **THEN** 序列化后的 JSON SHALL 使用 camelCase 字段名（`projectId` /
  `sessionId` / `deleted`），与既有 IPC 类型约定一致

#### Scenario: file-change 桥与通知管线并存
- **WHEN** Tauri host 同时持有 `subscribe_files()`（emit `file-change`）和
  `subscribe_detected_errors()`（emit `notification-added`）两个订阅
- **THEN** 两个桥 SHALL 独立运行，文件变更不会因为通知管线的 lag 而被丢弃，
  反之亦然

### Requirement: Validate inputs and return structured errors

The system SHALL validate IPC input parameters and return structured errors with an error code enum and a human-readable message, rather than propagating raw exceptions.

#### Scenario: Missing required field
- **WHEN** a caller invokes an operation missing a required field
- **THEN** the response SHALL contain a validation error with code `validation_error` and a description of the missing field

#### Scenario: Resource not found
- **WHEN** a caller requests a session or project that does not exist
- **THEN** the response SHALL contain an error with code `not_found` and the resource identifier

### Requirement: Expose file and path validation operations

The system SHALL expose operations to validate filesystem paths and to validate `@mention` references against a session's cwd.

#### Scenario: Validate a path that doesn't exist
- **WHEN** a caller validates a nonexistent path
- **THEN** the response SHALL indicate not-found without throwing

### Requirement: Expose auxiliary read operations

The system SHALL expose auxiliary data operations used by the renderer beyond the core session and project queries: read agent configs (subagent definitions), batch get sessions by ids, get session chat groups, get repository groups, get worktree sessions, read CLAUDE.md files (global/project/directory scopes), read a specific directory's CLAUDE.md, and read a single `@mention`-resolved file.

针对 Rust 侧实现，`read_agent_configs` SHALL 由 `LocalDataApi::read_agent_configs()` 提供并经 Tauri `read_agent_configs` command 暴露给前端；返回值 SHALL 为 `Vec<AgentConfig>` 序列化结果（详见 `agent-configs` capability）。

#### Scenario: Batch get sessions by ids
- **WHEN** a caller invokes the batch get-sessions-by-ids operation with an array of session ids
- **THEN** the response SHALL contain one session entry per requested id, with missing ids returned as not-found placeholders

#### Scenario: Read three-scope CLAUDE.md
- **WHEN** a caller invokes the read-claude-md-files operation for a given project
- **THEN** the response SHALL include entries for the global, project, and (when applicable) directory scopes

#### Scenario: Get worktree sessions
- **WHEN** a caller invokes the get-worktree-sessions operation for a repository group
- **THEN** the response SHALL list sessions belonging to every worktree in that group

#### Scenario: Read agent configs
- **WHEN** a caller invokes the read-agent-configs operation
- **THEN** the response SHALL contain the parsed subagent definitions from `~/.claude/agents/` and project-scoped agent directories

#### Scenario: Read agent configs via Tauri command
- **WHEN** 前端调用 `invoke("read_agent_configs")`
- **THEN** 响应 SHALL 为 JSON 数组，每个元素含 `name`、`color`、`description`、`scope`、`filePath` 字段（camelCase）

#### Scenario: Agent configs 在两个作用域目录都不存在时
- **WHEN** 全局 `~/.claude/agents/` 与所有项目的 `.claude/agents/` 目录都缺失
- **THEN** 命令 SHALL 返回空数组并且不返回错误

### Requirement: Expose search via Tauri IPC command

The system SHALL expose a `search_sessions` Tauri command that accepts project_id and query parameters, delegates to `LocalDataApi.search()`, and returns the search results as JSON.

#### Scenario: Tauri search command invocation
- **WHEN** the frontend invokes `search_sessions` with a project_id and query
- **THEN** the command SHALL return search results matching the `session-search` capability format

#### Scenario: Tauri search command with nonexistent project
- **WHEN** the frontend invokes `search_sessions` with an invalid project_id
- **THEN** the command SHALL return an error string describing the issue

### Requirement: Stream detected errors to subscribers

The system SHALL expose an in-process subscription mechanism on `LocalDataApi` that lets host runtimes (such as the Tauri application) receive newly detected errors emitted by the automatic notification pipeline, without polling the persistent notifications store.

#### Scenario: Tauri runtime subscribes and forwards to renderer
- **WHEN** the Tauri runtime calls `subscribe_detected_errors()` during application setup
- **AND** a new `DetectedError` is produced by the notification pipeline
- **THEN** the subscriber's `broadcast::Receiver` SHALL yield the `DetectedError`, allowing the host to emit a frontend event (e.g. `notification-added`)

#### Scenario: Subscription without a watcher attached
- **WHEN** `LocalDataApi` is constructed via the non-watcher constructor (used in integration tests or HTTP-only hosts)
- **AND** a caller calls `subscribe_detected_errors()`
- **THEN** the call SHALL return a valid `broadcast::Receiver` that never yields (silent no-op), not an error

#### Scenario: Multiple subscribers receive the same error
- **WHEN** two independent subscribers call `subscribe_detected_errors()`
- **AND** the pipeline produces one `DetectedError`
- **THEN** both subscribers SHALL independently receive the same `DetectedError` exactly once

### Requirement: Emit session metadata updates

The system SHALL expose an in-process subscription mechanism on `LocalDataApi` named `subscribe_session_metadata()` that yields a `broadcast::Receiver<SessionMetadataUpdate>`. `SessionMetadataUpdate` SHALL carry `projectId` / `sessionId` / `title` / `messageCount` / `isOngoing` (camelCase when serialized). Tauri host SHALL bridge this subscription to the webview by emitting `session-metadata-update` frontend events.

并发度 SHALL 被限流（固定上限 8），避免 50+ 文件同时打开；每次 `list_sessions(projectId)` 触发新扫描前 SHALL 取消上一轮未完成的扫描（同一 `projectId` 维度），避免事件串扰。

#### Scenario: 订阅接收元数据更新

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 随后调用 `list_sessions("projectA")`，项目下有 3 个 session
- **THEN** receiver SHALL 最多在扫描完成后收到 3 条 `SessionMetadataUpdate`，每条携带对应 sessionId 的真实 title/messageCount/isOngoing

#### Scenario: Tauri host emit session-metadata-update

- **WHEN** Tauri host 在 `setup` 调用 `subscribe_session_metadata()` 并在后台 task 内订阅
- **AND** 后端产出 `SessionMetadataUpdate { project_id: "p", session_id: "s", title: Some("T"), message_count: 12, is_ongoing: false }`
- **THEN** webview SHALL 通过 `listen("session-metadata-update", ...)` 收到 payload `{ projectId: "p", sessionId: "s", title: "T", messageCount: 12, isOngoing: false }`（camelCase）

#### Scenario: 同 projectId 新扫描取消旧扫描

- **WHEN** `list_sessions("projectA")` 正在扫描中（后台有未完成任务）
- **AND** 调用方再次调用 `list_sessions("projectA")`（file-change silent refresh 场景）
- **THEN** 旧扫描任务 SHALL 被 abort，未完成的 session 元数据 SHALL NOT 再推送；新扫描 SHALL 从头开始

#### Scenario: 并发度限制

- **WHEN** 扫描任务在并发处理某页 50 个 session 文件
- **THEN** 同一时刻打开的 JSONL 文件句柄数 SHALL 不超过 8（通过 `tokio::sync::Semaphore` 或等价机制限流）

#### Scenario: 无 watcher 构造器下 subscribe 安全

- **WHEN** `LocalDataApi` 通过不带 watcher 的构造器实例化（集成测试路径）
- **AND** 调用方 `subscribe_session_metadata()`
- **THEN** 返回有效 `broadcast::Receiver`；`list_sessions` 仍能正常推送（broadcast 不依赖 watcher）

### Requirement: Lazy load subagent trace

新 IPC `get_subagent_trace(parentSessionId, subagentSessionId)` MUST 返回该 subagent 的完整 chunks 流，用于 SubagentCard 展开时按需拉取被 `messagesOmitted` 裁剪的 trace 数据。后端 SHALL 在父 session 同 project 下查找 `<parentSessionId>/subagents/agent-<subagentSessionId>.jsonl`（新结构）或旧结构兼容路径，`parse_file` + `build_chunks` 后返回 `Vec<Chunk>`。

#### Scenario: 拉取存在的 subagent trace

- **WHEN** caller 调用 `get_subagent_trace("parent-uuid", "sub-uuid")` 且对应 subagent jsonl 存在
- **THEN** 响应 SHALL 含完整的 `Vec<Chunk>`（与未裁剪时 `Process.messages` 内容一致）

#### Scenario: subagent jsonl 不存在

- **WHEN** caller 调用 `get_subagent_trace` 但目标 jsonl 不存在
- **THEN** 响应 SHALL 为空 `[]`，不报错（与"不存在"等价于"无 trace"——caller UI 显示空 trace 即可）

#### Scenario: 嵌套 subagent 各自独立拉取

- **WHEN** SubagentCard A 展开后含嵌套 SubagentCard B；用户展开 B
- **THEN** 前端 SHALL 用 B 的 sessionId 单独调 `get_subagent_trace(rootSessionId, B.sessionId)`，不复用 A 的结果

### Requirement: Lazy load inline image asset

新 IPC `get_image_asset(rootSessionId, sessionId, blockId) -> String` MUST 返回前端可直接用作 `<img src>` 的 URL，用于 ImageBlock 在视口内时按需加载被 `dataOmitted` 裁剪的内联截图。`blockId` 编码为 `"<chunkUuid>:<blockIndex>"`（chunk uuid + 该 image 在 `MessageContent::Blocks` 数组中的 index），唯一定位一条 user message 内的某个 ImageBlock。

后端实现 SHALL：
1. 在 `rootSessionId` 同 project 下定位 `sessionId` 对应的 jsonl（root 自身或子 subagent jsonl，路径解析与 `get_subagent_trace` 一致）。
2. 解析对应行 message → 按 blockIndex 取出 `ContentBlock::Image.source.data`（base64 字符串）。
3. 对 base64 字符串算 SHA256，截取前 16 hex 字符作为文件名 hash；扩展名从 `media_type`（如 `image/png` → `.png`）映射，未知类型 fallback `.bin`。
4. 落盘路径：`<app_cache_dir()>/cdt-images/<hash>.<ext>`。若文件已存在 SHALL 直接返回 URL（不重写不报错）。
5. 落盘成功后返回 Tauri `asset://localhost/<absolute_path>` 形式 URL（前端通过 `convertFileSrc` API 也能等价构造）。
6. 任何步骤失败（jsonl 找不到、blockIndex 越界、磁盘写失败、media_type 解析异常）SHALL fallback 返回 `data:<mediaType>;base64,<原始 base64>` 字符串——前端 `<img src>` 仍可加载，可用性优先于性能。

Tauri 配置 SHALL 在 `tauri.conf.json::app.security.assetProtocol.scope` 中允许 `<app_cache_dir>/cdt-images/**`，并在 `capabilities/default.json` 中包含 `core:asset:default` 权限——否则 webview 拒绝加载 `asset://` URL。

#### Scenario: 拉取存在的 image asset

- **WHEN** caller 调用 `get_image_asset("root-uuid", "root-uuid", "chunk-abc:1")` 且对应 jsonl 存在、blockIndex 1 是 ImageBlock
- **THEN** 响应 SHALL 为 `asset://localhost/...cdt-images/<sha256前16>.png` 形式 URL
- **AND** 该 URL 指向的文件 SHALL 已存在于磁盘且内容是原 base64 解码后的 raw bytes

#### Scenario: 同 hash 跨调用去重

- **WHEN** 两次 `get_image_asset` 调用解析出的 base64 内容字节完全相同
- **THEN** 两次调用 SHALL 返回完全相同的 URL（同一文件名）
- **AND** 第二次调用 SHALL NOT 重写已存在文件（按 `path.exists()` 短路）

#### Scenario: blockId 定位失败 fallback 到 data URI

- **WHEN** caller 调用 `get_image_asset` 但 jsonl 不存在 / blockIndex 越界 / 该 block 不是 ImageBlock
- **THEN** 响应 SHALL 为 `data:application/octet-stream;base64,` 形式占位字符串（或空 base64）；前端 `<img>` 加载失败时显示 broken-image 图标
- **AND** 后端 SHALL NOT panic，SHALL NOT 返回 IPC error（image 显示失败不应阻塞 session 渲染）

#### Scenario: 嵌套 subagent 内的 image 通过 sessionId 定位

- **WHEN** caller 调用 `get_image_asset("root-uuid", "subagent-sub-uuid", "chunk-xyz:0")`
- **THEN** 后端 SHALL 在 `<root>/subagents/agent-<subagent-sub-uuid>.jsonl` 路径下定位 chunk，与 `get_subagent_trace` 路径解析逻辑一致

#### Scenario: 落盘失败 fallback 到 data URI

- **WHEN** cache 目录不可写（权限拒绝 / 磁盘满）
- **THEN** 响应 SHALL 为 `data:<mediaType>;base64,<完整 base64>` 字符串，前端按 `<img src>` 仍可加载
- **AND** 后端 SHALL `tracing::warn!` 记录失败原因供排查

