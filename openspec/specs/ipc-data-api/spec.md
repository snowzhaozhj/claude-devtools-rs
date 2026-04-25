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

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.tool_executions[i].output` 内 `text` / `value` 字段 MUST 默认被替换为空（`Text { text: "" }` / `Structured { value: Null }` / `Missing` 不变），且同时设 `outputOmitted=true`** —— 用于把首屏 IPC payload 中 tool 输出（实测 46a25772 case 436 KB / 26%）裁掉（行为契约见本 spec `Lazy load tool output` Requirement）。`output` enum 的 variant kind SHALL 保留（前端 ToolViewer 路由仍需要），仅 inner `text` / `value` 被清空。回滚开关 `OMIT_TOOL_OUTPUT: bool` 设 false 时 SHALL 退回完整 payload（`output` 内字段保留原值、`outputOmitted=false`）。该裁剪 SHALL 应用于顶层 AIChunk 及 subagent.messages 嵌套层（与其它 OMIT 同模式：默认嵌套层 no-op；`OMIT_SUBAGENT_MESSAGES=false` 回滚时仍能命中嵌套层）。

`list_sessions` 返回的每个 `SessionSummary` MUST 携带 `sessionId` / `projectId` / `timestamp` 的真实值（可直接从目录扫描得出），但 `title` / `messageCount` / `isOngoing` SHALL 允许为占位值（`null` / `0` / `false`）——这些元数据字段的真实值由后端异步扫描后通过 `session-metadata-update` push event 逐条推送（见本 spec "Emit session metadata updates" requirement）。`get_session_detail` 返回的 `SessionDetail.isOngoing` 仍 MUST 为同步计算后的真实值（因为 detail 已在调用链内完成全文件解析）。

**`isOngoing` 真实值 SHALL 由两路 AND 计算**：(a) `cdt_analyze::check_messages_ongoing(messages)` 返回 `true`（结构性活动栈五信号判定），**且** (b) session JSONL 文件 mtime 距当前时刻 `< 5 分钟`。任一条件不满足时 `isOngoing` MUST 为 `false`。stale 阈值常量 `STALE_SESSION_THRESHOLD = 5 min` 对齐原版 `claude-devtools/src/main/services/discovery/ProjectScanner.ts` 的 `STALE_SESSION_THRESHOLD_MS = 5 * 60 * 1000`（issue #94：用户 Ctrl+C / kill cli / 关机导致 cli 异常退出时，session 末尾停在 `tool_result` 之类 AI 活动而无 ending 信号，活动栈会误判 ongoing；mtime 兜底将其纠正）。`list_sessions` 异步扫描路径与 `get_session_detail` 同步路径行为 MUST 一致；HTTP `list_sessions_sync` 共用同一 `extract_session_metadata` 实现，自动适用。stat 失败时 SHALL 保守保留 messages_ongoing 判定（避免 fs 偶发错误把活跃 session 错判 dead）；时钟回拨导致 mtime > now 时 SHALL 判 not stale（避免未来 mtime 把活跃 session 误判 dead）。

序列化 SHALL 使用 camelCase（`isOngoing`、`messagesOmitted`、`headerModel`、`lastIsolatedTokens`、`isShutdownOnly`、`dataOmitted`、`contentOmitted`、`outputOmitted`）。例外：`ImageSource.media_type` 与 `ImageSource.type`（kind）保持 snake_case 与上游 Anthropic JSONL 格式一致——同 `TokenUsage` 例外。

HTTP API 路径（`GET /projects/:id/sessions`）SHALL 保留同步完整返回语义（不适用骨架化）——因 HTTP 无 push 通道；IPC 路径适用骨架化。HTTP 路径同样 SHALL NOT 应用 `OMIT_IMAGE_DATA` / `OMIT_RESPONSE_CONTENT` / `OMIT_TOOL_OUTPUT` 裁剪（HTTP 当前无活跃用户、且无对应 asset 协议端点 / 懒拉接口，保留完整 payload 传输）。

#### Scenario: Stale ongoing session is reported as not ongoing

- **WHEN** session JSONL 文件 mtime 距当前时刻已超过 5 分钟
- **AND** `cdt_analyze::check_messages_ongoing(messages)` 仍返回 `true`（消息序列结构上无 ending 信号）
- **THEN** `list_sessions` / `list_sessions_sync` 返回的 `SessionSummary.isOngoing` SHALL 为 `false`
- **AND** `get_session_detail` 返回的 `SessionDetail.isOngoing` SHALL 为 `false`
- **AND** sidebar 与 SessionDetail UI 因此 SHALL NOT 渲染绿色脉冲圆点 / 蓝色 ongoing 横幅

#### Scenario: Fresh ongoing session keeps ongoing flag

- **WHEN** session JSONL 文件 mtime 距当前时刻 < 5 分钟
- **AND** `cdt_analyze::check_messages_ongoing(messages)` 返回 `true`
- **THEN** `isOngoing` SHALL 为 `true`，UI 渲染 ongoing 指示

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

### Requirement: Lazy load tool output

新 IPC `get_tool_output(rootSessionId, sessionId, toolUseId) -> ToolOutput` MUST 返回该 tool execution 的完整 `output`，用于 ExecutionTrace 在用户点击展开时按需拉取被 `outputOmitted` 裁剪的 output 数据。后端 SHALL 按 `sessionId` 在同 project 下定位对应 jsonl（root 自身或 `subagents/agent-<sessionId>.jsonl`），`parse_file` 后在所有 ToolExecution 中线性 scan 找 `tool_use_id` 匹配项返回其 `output`。

#### Scenario: 拉取存在的 tool output

- **WHEN** caller 调用 `get_tool_output("root-uuid", "root-uuid", "tool-use-abc")` 且对应 jsonl 存在、ToolExecution 存在
- **THEN** 响应 SHALL 含完整的 `ToolOutput`（与未裁剪时 `tool_executions[i].output` 内容一致）
- **AND** 响应的 variant kind 与未裁剪时一致（`Text` / `Structured` / `Missing`）

#### Scenario: tool_use_id 找不到

- **WHEN** caller 调用 `get_tool_output` 但 jsonl 内无对应 tool_use_id
- **THEN** 响应 SHALL 为 `ToolOutput::Missing`，不报错（与"找不到"等价于"无 output"——caller UI 显示 missing state 即可）

#### Scenario: jsonl 不存在

- **WHEN** caller 调用 `get_tool_output` 但 sessionId 对应 jsonl 不存在
- **THEN** 响应 SHALL 为 `ToolOutput::Missing`，不报错

#### Scenario: 嵌套 subagent 内的 tool output 通过 sessionId 定位

- **WHEN** caller 调用 `get_tool_output("root-uuid", "subagent-sub-uuid", "tool-use-xyz")`
- **THEN** 后端 SHALL 在 `<root>/subagents/agent-<subagent-sub-uuid>.jsonl` 路径下定位 ToolExecution，与 `get_subagent_trace` 路径解析逻辑一致

### Requirement: Bulk and per-item notification operations

系统 SHALL 暴露三个 IPC 操作用于通知面板的批量与单条管理：`delete_notification(id)` / `mark_all_notifications_read()` / `clear_notifications(trigger_id?)`。所有三个操作 MUST 在成功后让宿主（Tauri / HTTP）能够 emit `notification-update` 事件以驱动前端 badge 与列表刷新。三个操作 MUST 在 `DataApi` trait 中定义并在 `LocalDataApi` 与任何其它实现上显式实现（无默认 impl）。

#### Scenario: 单条通知按 id 删除

- **WHEN** 调用方调用 `delete_notification(id)` 且通知存在
- **THEN** 系统 SHALL 从 `NotificationManager` 持久化存储中移除该条、返回 `true`、写盘
- **AND** 后续 `get_notifications` 的结果 SHALL 不再包含该 id 记录
- **AND** `unread_count` SHALL 对应减少（若被删记录原为未读）

#### Scenario: 删除不存在 id 返回 false

- **WHEN** `delete_notification(id)` 的 id 不存在于存储中
- **THEN** 操作 SHALL 返回 `false` 且磁盘文件 SHALL 不被写入

#### Scenario: 批量标记已读

- **WHEN** 调用方调用 `mark_all_notifications_read()`
- **THEN** 系统 SHALL 将所有持久化通知的 `is_read` 置为 `true`、写盘
- **AND** 后续 `get_notifications` 返回的所有 notification 的 `isRead` SHALL 为 `true`
- **AND** `unread_count` SHALL 为 `0`

#### Scenario: 清空全部通知

- **WHEN** 调用方调用 `clear_notifications(None)`
- **THEN** 系统 SHALL 清空持久化存储、写盘、返回被删条数
- **AND** 后续 `get_notifications` 返回的 `notifications` SHALL 为空数组、`total` 与 `unread_count` SHALL 为 0

#### Scenario: 按 trigger 清空（预留）

- **WHEN** 调用方调用 `clear_notifications(Some(trigger_id))`
- **THEN** 系统 SHALL 仅删除 `error.trigger_id == trigger_id` 的通知、写盘、返回被删条数
- **AND** 其它 trigger 产生的通知 SHALL 保留

#### Scenario: 操作成功 emit 事件

- **WHEN** 上述任一操作在 Tauri 宿主成功执行
- **THEN** 宿主 SHALL emit `notification-update` 事件供前端 badge 与 NotificationsView reload

