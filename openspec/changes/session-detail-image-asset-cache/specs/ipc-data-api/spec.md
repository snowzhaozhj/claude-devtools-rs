# ipc-data-api Spec Delta

## MODIFIED Requirements

### Requirement: Expose project and session queries

The system SHALL expose data queries for projects and sessions over a request/response IPC channel set, including at minimum: list projects, list sessions for a project (with pagination), get session detail, get session metrics, get waterfall data, and get subagent detail.

`get_session_detail` SHALL 在返回 session 详情时集成 subagent 解析：从同 project 的其他 session 中扫描候选 subagent，调用 `resolve_subagents` 填充 `AIChunk.subagents` 字段。若扫描失败或无候选，`subagents` SHALL 为空数组（不报错）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.subagents[i].messages` 数组 MUST 默认被裁剪为空 Vec，且 `messagesOmitted=true`** —— 用于把首屏 IPC payload 控制在原大小的 ~40%（subagent 嵌套 chunks 全文是大头，行为契约见 change `subagent-messages-lazy-load`）。`Process.headerModel` / `Process.lastIsolatedTokens` / `Process.isShutdownOnly` 三个 derived 字段 MUST 在 `candidate_to_process` 阶段由 messages 预算后填充，让 SubagentCard header 不依赖完整 `messages` 也能正常渲染。回滚开关 `OMIT_SUBAGENT_MESSAGES: bool` 设 false 时 SHALL 退回完整 payload（messages 不裁剪、`messagesOmitted=false`）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `ContentBlock::Image.source.data` 字段 MUST 默认被替换为空字符串 `""`，且同时设 `source.dataOmitted=true`** —— 用于把首屏 IPC payload 中内联截图的 base64 字符串裁掉（行为契约见本 spec `Lazy load inline image asset` Requirement）。`source.kind` / `source.media_type` 字段 SHALL 保留（前端渲染时仍需要），仅 `data` 字段被清空。回滚开关 `OMIT_IMAGE_DATA: bool` 设 false 时 SHALL 退回完整 base64 payload（`data` 保留原值、`dataOmitted=false`）。该裁剪 SHALL 应用于所有 chunk 类型（UserChunk / AIChunk responses / subagent.messages 内嵌套——但因 subagent.messages 默认已被裁剪，仅在回滚 `OMIT_SUBAGENT_MESSAGES=false` 时才会触及嵌套层）。

`list_sessions` 返回的每个 `SessionSummary` MUST 携带 `sessionId` / `projectId` / `timestamp` 的真实值（可直接从目录扫描得出），但 `title` / `messageCount` / `isOngoing` SHALL 允许为占位值（`null` / `0` / `false`）——这些元数据字段的真实值由后端异步扫描后通过 `session-metadata-update` push event 逐条推送（见本 spec "Emit session metadata updates" requirement）。`get_session_detail` 返回的 `SessionDetail.isOngoing` 仍 MUST 为同步计算后的真实值（因为 detail 已在调用链内完成全文件解析）。

序列化 SHALL 使用 camelCase（`isOngoing`、`messagesOmitted`、`headerModel`、`lastIsolatedTokens`、`isShutdownOnly`、`dataOmitted`）。例外：`ImageSource.media_type` 与 `ImageSource.type`（kind）保持 snake_case 与上游 Anthropic JSONL 格式一致——同 `TokenUsage` 例外。

HTTP API 路径（`GET /projects/:id/sessions`）SHALL 保留同步完整返回语义（不适用骨架化）——因 HTTP 无 push 通道；IPC 路径适用骨架化。HTTP 路径同样 SHALL NOT 应用 `OMIT_IMAGE_DATA` 裁剪（HTTP 当前无活跃用户、且无对应 asset 协议端点，保留完整 base64 传输）。

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

## ADDED Requirements

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
