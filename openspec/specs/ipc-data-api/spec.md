# ipc-data-api Specification

## Purpose

`LocalDataApi` 在 Tauri 进程内对前端 webview 暴露的所有 IPC 操作契约：项目 / 会话查询、搜索、配置、通知、SSH、agent configs、CLAUDE.md 读取、subagent trace 与 image asset 懒加载、tool output 懒加载、teammate 消息嵌入、session metadata 异步推送、file-change / detected-error broadcast。本 capability 同时定义首屏 IPC payload 的瘦身策略（`OMIT_*` 系列开关 + `xxxOmitted` flag），让大会话首次打开仍能在 webview 端流畅渲染。
## Requirements
### Requirement: Expose project and session queries

系统 SHALL 在请求 / 响应式 IPC 通道上暴露项目与会话相关数据查询，至少包括：列项目、列项目下 sessions（含分页）、取 session 详情、取 session metrics、取 waterfall 数据、取 subagent 详情。

`get_session_detail` SHALL 在返回 session 详情时集成 subagent 解析：**从主 session 所在 `projects_dir`（即 `~/.claude/projects/` 或 SSH 远端等价路径）下所有 project 目录扫描 `{rootSessionId}/subagents/agent-*.jsonl`（新结构）**，合并去重后调用 `resolve_subagents` 填充 `AIChunk.subagents` 字段。旧结构（flat `{project_dir}/agent-*.jsonl`）SHALL 保持只扫描主 `project_dir` 并按首行 `parentUuid` / `sessionId` 字段过滤。若扫描失败或无候选，`subagents` SHALL 为空数组（不报错）。回滚开关 `CROSS_PROJECT_SUBAGENT_SCAN: bool` 顶层 const，设为 `false` 时 SHALL 退回"只扫主 `project_dir`"的原行为。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.subagents[i].messages` 数组 MUST 默认被裁剪为空 Vec，且 `messagesOmitted=true`** —— 用于把首屏 IPC payload 控制在原大小 ~40%（subagent 嵌套 chunks 全文是大头，行为契约见 change `subagent-messages-lazy-load`）。`Process.headerModel` / `Process.lastIsolatedTokens` / `Process.isShutdownOnly` 三个 derived 字段 MUST 在 `candidate_to_process` 阶段由 messages 预算后填充，让 SubagentCard header 不依赖完整 `messages` 也能正常渲染。回滚开关 `OMIT_SUBAGENT_MESSAGES: bool` 设 false 时 SHALL 退回完整 payload（messages 不裁剪、`messagesOmitted=false`）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `ContentBlock::Image.source.data` 字段 MUST 默认被替换为空字符串 `""`，且同时设 `source.dataOmitted=true`** —— 用于把首屏 IPC payload 中内联截图的 base64 字符串裁掉（行为契约见本 spec `Lazy load inline image asset` Requirement）。`source.kind` / `source.media_type` 字段 SHALL 保留（前端渲染时仍需要），仅 `data` 字段被清空。回滚开关 `OMIT_IMAGE_DATA: bool` 设 false 时 SHALL 退回完整 base64 payload（`data` 保留原值、`dataOmitted=false`）。该裁剪 SHALL 应用于所有 chunk 类型（UserChunk / AIChunk responses / subagent.messages 内嵌套——但 subagent.messages 默认已被裁剪，仅在回滚 `OMIT_SUBAGENT_MESSAGES=false` 时才会触及嵌套层）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.responses[i].content` 字段 MUST 默认被替换为空 `MessageContent::Text("")`，且同时设 `contentOmitted=true`** —— 用于把首屏 IPC payload 中最大单一字段（实测 `46a25772` case 下 1257 KB / 41%）裁掉（行为契约见 change `session-detail-response-content-omit`）。该字段在前端任何代码路径中都不被读取（chunk 显示文本走 `semanticSteps` 的 thinking / text 步骤），裁剪后 UI 渲染零变化。回滚开关 `OMIT_RESPONSE_CONTENT: bool` 设 false 时 SHALL 退回完整 payload（`content` 携带原 `MessageContent`、`contentOmitted=false`）。该裁剪 SHALL 应用于顶层 AIChunk 及 subagent.messages 嵌套层（与 `OMIT_IMAGE_DATA` 同模式：在 `OMIT_SUBAGENT_MESSAGES=true` 默认路径下嵌套层为 no-op；`OMIT_SUBAGENT_MESSAGES=false` 回滚时仍能命中嵌套层）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.tool_executions[i].output` 内 `text` / `value` 字段 MUST 默认被替换为空（`Text { text: "" }` / `Structured { value: Null }` / `Missing` 不变），且同时设 `outputOmitted=true`** —— 用于把首屏 IPC payload 中 tool 输出（实测 `46a25772` case 下 436 KB / 26%）裁掉（行为契约见本 spec `Lazy load tool output` Requirement）。`output` enum 的 variant kind SHALL 保留（前端 ToolViewer 路由仍需要），仅内层 `text` / `value` 被清空。回滚开关 `OMIT_TOOL_OUTPUT: bool` 设 false 时 SHALL 退回完整 payload（`output` 内字段保留原值、`outputOmitted=false`）。该裁剪 SHALL 应用于顶层 AIChunk 及 subagent.messages 嵌套层（与其它 OMIT 同模式：默认嵌套层 no-op；`OMIT_SUBAGENT_MESSAGES=false` 回滚时仍能命中嵌套层）。

**`OMIT_TOOL_OUTPUT=true` 路径下 `ToolExecution.outputBytes: Option<u64>` MUST 在 `trim` output 之前按 variant 记录原始字节长度**（`Text` → `text.len()`、`Structured` → `serde_json::to_string(value).map(|s| s.len()).unwrap_or(0)`、`Missing` → 不填，保持 `None`），让前端在懒加载之前即可估算 output token 数（按 `outputBytes / 4` 启发式），从而 BaseItem 头部 token 显示 SHALL **在懒加载展开前后保持一致**——不再因 `getToolOutputTokens` 在 OMIT 状态返回 0、懒加载后返回真实值而抖动。`OMIT_TOOL_OUTPUT=false` 回滚路径下 `outputBytes` SHALL 保持 `None`（前端 fallback 到直接读 `text.length`）。解析层（`cdt-parse` / `cdt-analyze`）SHALL **不**主动填充 `outputBytes`——该字段仅在 IPC OMIT 层语义有意义。

`list_sessions` 返回的每个 `SessionSummary` MUST 携带 `sessionId` / `projectId` / `timestamp` 的真实值（可直接从目录扫描得出），但 `title` / `messageCount` / `isOngoing` SHALL 允许为占位值（`null` / `0` / `false`）——这些元数据字段的真实值由后端异步扫描后通过 `session-metadata-update` push event 逐条推送（见本 spec `Emit session metadata updates` Requirement）。`get_session_detail` 返回的 `SessionDetail.isOngoing` 仍 MUST 为同步计算后的真实值（因为 detail 已在调用链内完成全文件解析）。

**`isOngoing` 真实值 SHALL 由两路 AND 计算**：(a) `cdt_analyze::check_messages_ongoing(messages)` 返回 `true`（结构性活动栈五信号判定），**且** (b) session JSONL 文件 mtime 距当前时刻 `< 5 分钟`。任一条件不满足时 `isOngoing` MUST 为 `false`。stale 阈值常量 `STALE_SESSION_THRESHOLD = 5 min` 对齐原版 `claude-devtools/src/main/services/discovery/ProjectScanner.ts` 的 `STALE_SESSION_THRESHOLD_MS = 5 * 60 * 1000`（issue #94：用户 Ctrl+C / kill cli / 关机导致 cli 异常退出时，session 末尾停在 `tool_result` 之类 AI 活动而无 ending 信号，活动栈会误判 ongoing；mtime 兜底将其纠正）。`list_sessions` 异步扫描路径与 `get_session_detail` 同步路径行为 MUST 一致；HTTP `GET /api/projects/{projectId}/sessions` 路径共用同一 `extract_session_metadata` 实现（详见本 spec §"HTTP `list_sessions` 复用 IPC 骨架 + push 实现"），自动适用。stat 失败时 SHALL 保守保留 messages_ongoing 判定（避免 fs 偶发错误把活跃 session 错判 dead）；时钟回拨导致 mtime > now 时 SHALL 判 not stale（避免未来 mtime 把活跃 session 误判 dead）。

序列化 SHALL 使用 camelCase（`isOngoing`、`messagesOmitted`、`headerModel`、`lastIsolatedTokens`、`isShutdownOnly`、`dataOmitted`、`contentOmitted`、`outputOmitted`、`outputBytes`）。例外：`ImageSource.media_type` 与 `ImageSource.type`（kind）保持 snake_case，与上游 Anthropic JSONL 格式一致——同 `TokenUsage` 例外。

**HTTP `GET /api/projects/{projectId}/sessions` 路径 SHALL 与 IPC `list_sessions` 共用骨架 + push 实现**——即 `cdt-api::http::routes::list_sessions` SHALL 调 `DataApi::list_sessions(...)`（骨架快返 + `try_lookup_cached_metadata` lookup-fast-path + spawn 后台扫描 + `broadcast::Sender<SessionMetadataUpdate>` emit），**不**得调 `DataApi::list_sessions_sync(...)`。后台扫描产物 SHALL 通过 `cdt-api::http::bridge::forward_session_metadata` 桥接到 `/api/events` SSE，浏览器 client 按 `session-metadata-update` event 收到与 IPC 路径同形的 patch。

`DataApi::list_sessions_sync` trait method SHALL 保留作为 trait 默认 fallback（供未来非 SSE-aware HTTP client 或 `cdt-cli` 直接 trait 调用使用），但 axum HTTP route 实现 **不**得再调用它。HTTP 路径同样 SHALL NOT 应用 `OMIT_IMAGE_DATA` / `OMIT_RESPONSE_CONTENT` / `OMIT_TOOL_OUTPUT` 裁剪（HTTP 当前无活跃用户、且无对应 asset 协议端点 / 懒拉接口，保留完整 payload 传输）。

#### Scenario: outputBytes filled before trim under OMIT_TOOL_OUTPUT

- **WHEN** `OMIT_TOOL_OUTPUT=true` 路径触发 `apply_tool_output_omit` 处理一个 `ToolExecution`
- **AND** 该 `ToolExecution.output` 是 `Text { text: "abcde" }`（5 字节）
- **THEN** 处理后 `output.text` SHALL 为 `""`、`outputOmitted` SHALL 为 `true`、`outputBytes` SHALL 为 `Some(5)`

#### Scenario: outputBytes for structured uses serialized length

- **WHEN** `apply_tool_output_omit` 处理 `Structured { value: {"stdout": "ok", "exit": 0} }`
- **THEN** `outputBytes` SHALL 为 `Some(serde_json::to_string(value).unwrap().len())`，`output.value` SHALL 为 `Null`

#### Scenario: outputBytes none for missing variant

- **WHEN** `apply_tool_output_omit` 处理 `output: Missing`
- **THEN** `outputBytes` SHALL 保持 `None`、`output` 不变

#### Scenario: BaseItem token count stable across expand

- **WHEN** 前端 `BaseItem` 渲染一条 `outputOmitted=true` 的 tool 行
- **AND** 用户点击展开触发懒加载，展开后 `output.text` 替换为完整原始内容
- **THEN** 头部 token badge 显示的数字 SHALL **在展开前后相等**（前端 `getToolOutputTokens` 在懒加载前从 `outputBytes` 估算、懒加载后从 `outputBytes` 读取——两次结果一致）

#### Scenario: get_session_detail 跨 project_dir 装载 subagent
- **WHEN** caller 调 `get_session_detail(A, S)`，A 是主 `project_id`，S 是 root session id
- **AND** subagent JSONL 物理位于 `project_dir = B`（`B/S/subagents/agent-<subUuid>.jsonl`）
- **THEN** 返回 `SessionDetail.chunks` 内对应 Task tool_use 的 `AIChunk.subagents` SHALL 含 `Process { session_id: <subUuid>, ... }`
- **AND** subagent 关联三阶段 fallback SHALL 正常评估，与"主 project_dir 自带 subagent"等价

#### Scenario: CROSS_PROJECT_SUBAGENT_SCAN=false 回滚到原行为
- **WHEN** 顶层 const `CROSS_PROJECT_SUBAGENT_SCAN: bool = false`
- **AND** subagent JSONL 位于非主 `project_dir`
- **THEN** `get_session_detail` SHALL NOT 装载该 candidate，对应 Task SHALL 保留为未解析（原行为）

#### Scenario: HTTP list_sessions 走骨架而非 sync

- **WHEN** 客户端发起 `GET /api/projects/{projectId}/sessions?pageSize=N&cursor=C`
- **THEN** axum handler `cdt-api::http::routes::list_sessions` SHALL 调 `DataApi::list_sessions(project_id, pagination)`（**不**得调 `list_sessions_sync`）
- **AND** 响应 body SHALL 是骨架 `PaginatedResponse<SessionSummary>`：每条 `SessionSummary` 的 `sessionId` / `projectId` / `timestamp` SHALL 为真实值；`title` / `messageCount` / `isOngoing` / `gitBranch` SHALL 允许为占位值（除非 `try_lookup_cached_metadata` 命中可直接 inline 填回真值）

#### Scenario: HTTP list_sessions 后台扫描产物经 SSE 推送

- **WHEN** HTTP `list_sessions` 返回骨架后，后台 `scan_metadata_for_page` 任务对 cache miss 的 session 完成扫描并调 `broadcast::Sender<SessionMetadataUpdate>::send(update)`
- **THEN** 该 update SHALL 通过 `cdt-api::http::bridge::forward_session_metadata` 转换为 `PushEvent::SessionMetadataUpdate { projectId, sessionId, title, messageCount, isOngoing, gitBranch }` 推送到所有 `/api/events` 客户端
- **AND** 浏览器 client `transport.ts::BrowserTransport` SHALL 按既有归一化路径转交 `session-metadata-update` 事件给 listener，与 IPC 路径行为一致

### Requirement: Expose search queries

系统 SHALL 暴露三类搜索操作：单 session 搜索、单 project 搜索、跨全项目搜索。`search` SHALL 委托给 `SessionSearcher`（来自 `session-search` capability）执行真实全文搜索，而非返回空结果。

#### Scenario: Search all projects via IPC
- **WHEN** 调用方拿一个 query 调用全局搜索操作
- **THEN** 响应 SHALL 含与 `session-search` capability 一致的 per-project 命中分组

#### Scenario: Search returns real results from SessionSearcher
- **WHEN** 调用方拿一个匹配某 session 内容的 query 调用搜索操作
- **THEN** 响应 SHALL 含 `message_uuid`、`offset`、`preview`、`message_type` 字段的命中条目

#### Scenario: Search with empty query
- **WHEN** 调用方用空 query 字符串调用搜索操作
- **THEN** 响应 SHALL 返回空结果数组，不报错

### Requirement: Expose config and notification operations

系统 SHALL 在 IPC 上暴露配置读 / 写操作以及通知列表 / 标记已读操作，行为与 `configuration-management` 与 `notification-triggers` 描述一致。

#### Scenario: Update config field via IPC
- **WHEN** 调用方调用配置更新操作
- **THEN** 变更 SHALL 被持久化，响应 SHALL 含新配置

### Requirement: Validate inputs and return structured errors

系统 SHALL 校验 IPC 入参，并以错误码 enum 配合可读消息的结构化错误返回，而非把原始异常向上传递。

#### Scenario: Missing required field
- **WHEN** 调用方调用某操作但缺少必填字段
- **THEN** 响应 SHALL 含 `code: validation_error` 的校验错误，并描述缺失字段

#### Scenario: Resource not found
- **WHEN** 调用方请求不存在的 session 或 project
- **THEN** 响应 SHALL 含 `code: not_found` 的错误，附资源标识符

### Requirement: Expose file and path validation operations

系统 SHALL 暴露文件系统路径校验操作，以及把 `@mention` 引用按 session 的 cwd 校验的操作。

#### Scenario: Validate a path that doesn't exist
- **WHEN** 调用方校验一个不存在的路径
- **THEN** 响应 SHALL 标明 not-found，不抛错

### Requirement: Expose auxiliary read operations

系统 SHALL 暴露除核心 session / project 查询之外、renderer 也会用到的辅助数据操作：读取 agent configs（subagent 定义）、按 id 批量取 sessions、取 session chat groups、取仓库分组、取 worktree sessions、读取 CLAUDE.md（global / project / directory 三类作用域）、读取指定目录的 CLAUDE.md、读取一条 `@mention` 解析后的文件。

针对 Rust 侧实现，`read_agent_configs` SHALL 由 `LocalDataApi::read_agent_configs()` 提供并经 Tauri `read_agent_configs` command 暴露给前端；返回值 SHALL 为 `Vec<AgentConfig>` 序列化结果（详见 `agent-configs` capability）。

#### Scenario: Batch get sessions by ids
- **WHEN** 调用方拿一组 session id 调用 batch get-sessions-by-ids 操作
- **THEN** 响应 SHALL 为每个请求 id 返回一条 session 条目，缺失 id 以 not-found 占位返回

#### Scenario: Read three-scope CLAUDE.md
- **WHEN** 调用方对指定 project 调用 read-claude-md-files 操作
- **THEN** 响应 SHALL 含 global、project、（如适用）directory 三类作用域条目

#### Scenario: Get worktree sessions
- **WHEN** 调用方对一个仓库分组调用 get-worktree-sessions 操作
- **THEN** 响应 SHALL 列出该分组下每个 worktree 对应的 sessions

#### Scenario: Read agent configs
- **WHEN** 调用方调用 read-agent-configs 操作
- **THEN** 响应 SHALL 含来自 `~/.claude/agents/` 与项目级 agent 目录的解析后 subagent 定义

#### Scenario: Read agent configs via Tauri command
- **WHEN** 前端调用 `invoke("read_agent_configs")`
- **THEN** 响应 SHALL 为 JSON 数组，每个元素含 `name`、`color`、`description`、`scope`、`filePath` 字段（camelCase）

#### Scenario: Agent configs 在两个作用域目录都不存在时
- **WHEN** 全局 `~/.claude/agents/` 与所有项目的 `.claude/agents/` 目录都缺失
- **THEN** 命令 SHALL 返回空数组，不返回错误

### Requirement: Expose search via Tauri IPC command

系统 SHALL 暴露 `search_sessions` Tauri command：接受 `project_id` 与 `query` 参数，委托给 `LocalDataApi.search()`，把搜索结果以 JSON 形式返回。

#### Scenario: Tauri search command invocation
- **WHEN** 前端拿 `project_id` 与 `query` 调用 `search_sessions`
- **THEN** 命令 SHALL 返回与 `session-search` capability 同形的搜索结果

#### Scenario: Tauri search command with nonexistent project
- **WHEN** 前端拿一个非法 `project_id` 调用 `search_sessions`
- **THEN** 命令 SHALL 返回描述问题的错误字符串

### Requirement: Lazy load subagent trace

新 IPC `get_subagent_trace(parentSessionId, subagentSessionId)` MUST 返回该 subagent 的完整 chunks 流，用于 SubagentCard 展开时按需拉取被 `messagesOmitted` 裁剪的 trace 数据。后端 SHALL **在 caller 所在 `projects_dir` 下所有 project 目录**扫描 `<parentSessionId>/subagents/agent-<subagentSessionId>.jsonl`（新结构），命中即返；未命中时 fallback 到旧结构兼容路径（仅在主 `project_dir` 内查找 flat `agent-<subagentSessionId>.jsonl`）。`parse_file` + `build_chunks` 后返回 `Vec<Chunk>`。

#### Scenario: 拉取存在的 subagent trace

- **WHEN** caller 调用 `get_subagent_trace("parent-uuid", "sub-uuid")`，对应 subagent jsonl 存在
- **THEN** 响应 SHALL 含完整的 `Vec<Chunk>`（与未裁剪时 `Process.messages` 内容一致）

#### Scenario: subagent jsonl 不存在

- **WHEN** caller 调用 `get_subagent_trace`，但目标 jsonl 不存在
- **THEN** 响应 SHALL 为空 `[]`，不报错（与"不存在"等价于"无 trace"——caller UI 显示空 trace 即可）

#### Scenario: 嵌套 subagent 各自独立拉取

- **WHEN** SubagentCard A 展开后含嵌套 SubagentCard B；用户展开 B
- **THEN** 前端 SHALL 用 B 的 sessionId 单独调 `get_subagent_trace(rootSessionId, B.sessionId)`，不复用 A 的结果

#### Scenario: 跨 project_dir 定位 subagent jsonl
- **WHEN** caller 调用 `get_subagent_trace("parent-uuid", "sub-uuid")`，subagent jsonl 物理位于非主 `project_dir`（例如 `<projects_dir>/B/parent-uuid/subagents/agent-sub-uuid.jsonl`）
- **THEN** 后端 SHALL 跨 `project_dir` 扫描定位到 B 下的 jsonl，并返回完整 `Vec<Chunk>`

### Requirement: Lazy load inline image asset

新 IPC `get_image_asset(rootSessionId, sessionId, blockId) -> String` MUST 返回前端可直接用作 `<img src>` 的 URL，用于 ImageBlock 进入视口时按需加载被 `dataOmitted` 裁剪的内联截图。`blockId` 编码为 `"<chunkUuid>:<blockIndex>"`（chunk uuid + 该 image 在 `MessageContent::Blocks` 数组中的 index），唯一定位一条 user message 内的某个 ImageBlock。

后端实现 SHALL：
1. 在 `rootSessionId` 同 project 下定位 `sessionId` 对应的 jsonl（root 自身或子 subagent jsonl，路径解析与 `get_subagent_trace` 一致）。
2. 解析对应行 message → 按 blockIndex 取出 `ContentBlock::Image.source.data`（base64 字符串）。
3. 对 base64 字符串算 SHA256，截取前 16 hex 字符作为文件名 hash；扩展名从 `media_type`（例如 `image/png` → `.png`）映射，未知类型 fallback `.bin`。
4. 落盘路径：`<app_cache_dir()>/cdt-images/<hash>.<ext>`。若文件已存在 SHALL 直接返回 URL（不重写、不报错）。
5. 落盘成功后返回 Tauri `asset://localhost/<absolute_path>` 形式的 URL（前端通过 `convertFileSrc` API 也能等价构造）。
6. 任何步骤失败（jsonl 找不到、blockIndex 越界、磁盘写失败、media_type 解析异常）SHALL fallback 返回 `data:<mediaType>;base64,<原始 base64>` 字符串——前端 `<img src>` 仍可加载，可用性优先于性能。

Tauri 配置 SHALL 在 `tauri.conf.json::app.security.assetProtocol.scope` 中允许 `<app_cache_dir>/cdt-images/**`，并在 `capabilities/default.json` 中包含 `core:asset:default` 权限——否则 webview 拒绝加载 `asset://` URL。

#### Scenario: 拉取存在的 image asset

- **WHEN** caller 调用 `get_image_asset("root-uuid", "root-uuid", "chunk-abc:1")`，对应 jsonl 存在、blockIndex 1 是 ImageBlock
- **THEN** 响应 SHALL 为 `asset://localhost/...cdt-images/<sha256前16>.png` 形式的 URL
- **AND** 该 URL 指向的文件 SHALL 已存在于磁盘，内容是原 base64 解码后的 raw bytes

#### Scenario: 同 hash 跨调用去重

- **WHEN** 两次 `get_image_asset` 调用解析出的 base64 内容字节完全相同
- **THEN** 两次调用 SHALL 返回完全相同的 URL（同一文件名）
- **AND** 第二次调用 SHALL NOT 重写已存在文件（按 `path.exists()` 短路）

#### Scenario: blockId 定位失败 fallback 到 data URI

- **WHEN** caller 调用 `get_image_asset` 但 jsonl 不存在 / blockIndex 越界 / 该 block 不是 ImageBlock
- **THEN** 响应 SHALL 为 `data:application/octet-stream;base64,` 形式的占位字符串（或空 base64）；前端 `<img>` 加载失败时显示 broken-image 图标
- **AND** 后端 SHALL NOT panic，SHALL NOT 返回 IPC error（image 显示失败不应阻塞 session 渲染）

#### Scenario: 嵌套 subagent 内的 image 通过 sessionId 定位

- **WHEN** caller 调用 `get_image_asset("root-uuid", "subagent-sub-uuid", "chunk-xyz:0")`
- **THEN** 后端 SHALL 在 `<root>/subagents/agent-<subagent-sub-uuid>.jsonl` 路径下定位 chunk，与 `get_subagent_trace` 路径解析逻辑一致

#### Scenario: 落盘失败 fallback 到 data URI

- **WHEN** cache 目录不可写（permission denied / 磁盘满）
- **THEN** 响应 SHALL 为 `data:<mediaType>;base64,<完整 base64>` 字符串，前端按 `<img src>` 仍可加载
- **AND** 后端 SHALL `tracing::warn!` 记录失败原因供排查

### Requirement: Lazy load tool output

新 IPC `get_tool_output(rootSessionId, sessionId, toolUseId) -> ToolOutput` MUST 返回该 tool execution 的完整 `output`，用于 ExecutionTrace 在用户点击展开时按需拉取被 `outputOmitted` 裁剪的 output 数据。后端 SHALL 按 `sessionId` 在同 project 下定位对应 jsonl（root 自身或 `subagents/agent-<sessionId>.jsonl`），`parse_file` 后在所有 ToolExecution 中线性 scan 找 `tool_use_id` 匹配项返回其 `output`。

#### Scenario: 拉取存在的 tool output

- **WHEN** caller 调用 `get_tool_output("root-uuid", "root-uuid", "tool-use-abc")`，对应 jsonl 存在、ToolExecution 存在
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

- **WHEN** 调用方调用 `delete_notification(id)`，通知存在
- **THEN** 系统 SHALL 从 `NotificationManager` 持久化存储中移除该条、返回 `true`、写盘
- **AND** 后续 `get_notifications` 的结果 SHALL 不再包含该 id 记录
- **AND** `unread_count` SHALL 对应减少（若被删记录原为未读）

#### Scenario: 删除不存在 id 返回 false

- **WHEN** `delete_notification(id)` 的 id 不存在于存储中
- **THEN** 操作 SHALL 返回 `false`，磁盘文件 SHALL NOT 被写入

#### Scenario: 批量标记已读

- **WHEN** 调用方调用 `mark_all_notifications_read()`
- **THEN** 系统 SHALL 把所有持久化通知的 `is_read` 置为 `true`、写盘
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
- **THEN** 宿主 SHALL emit `notification-update` 事件，供前端 badge 与 NotificationsView reload

### Requirement: Session list pagination avoids duplicate full scans

IPC clients that need all sessions for a project SHALL consume `list_sessions` through cursor pagination without re-requesting an already returned page as part of a larger full-list request. The `list_sessions` response MUST preserve the existing skeleton-first contract: returned `SessionSummary` entries may omit expensive metadata fields while background metadata updates fill them later.

#### Scenario: Client accumulates pages without restarting from the first page

- **WHEN** a project has more sessions than the initial client page size and `list_sessions` returns a non-null `nextCursor`
- **THEN** the client requests the next page using that cursor and appends the new sessions to the already returned sessions
- **AND** the client does NOT issue a second request from the beginning with `pageSize = total`

#### Scenario: Skeleton response remains available before metadata completes

- **WHEN** `list_sessions` returns session entries before background metadata parsing has completed
- **THEN** each returned entry remains a valid skeleton `SessionSummary`
- **AND** later `session-metadata-update` events may patch `title`, `messageCount`, `isOngoing`, and related metadata in place

### Requirement: List sessions uses project-scoped light pagination

`list_sessions(projectId, pagination)` SHALL act as a project-scoped cursor pagination API for session list UI. The synchronous response MUST only require lightweight fields that can be obtained without parsing session contents: `sessionId`, `projectId`, `timestamp`, and pagination metadata. Deep metadata fields (`title`, `messageCount`, `isOngoing`, `gitBranch`) SHALL be allowed to remain placeholders in the synchronous response and be filled later through `session-metadata-update`.

`list_sessions` SHALL NOT require callers to compute or consume an exact total count for the session list first page. Pagination continuation MUST be driven by `nextCursor` / equivalent `hasMore` semantics. If the response type keeps a `total` field for compatibility, callers SHALL treat it as informational and MUST NOT rely on it being a complete project count unless a future dedicated count API states so.

#### Scenario: first page returns light summaries

- **WHEN** caller invokes `list_sessions("projectA", { pageSize: 20, cursor: null })`
- **THEN** response SHALL contain at most 20 `SessionSummary` items for `projectA`
- **AND** each item SHALL contain real `sessionId`, `projectId`, and `timestamp`
- **AND** each item MAY contain placeholder `title = null`, `messageCount = 0`, `isOngoing = false`, and `gitBranch = null`

#### Scenario: continuation uses cursor not total

- **WHEN** first `list_sessions("projectA", { pageSize: 20, cursor: null })` response contains `nextCursor`
- **THEN** caller SHALL request the next page with that cursor
- **AND** caller SHALL NOT need an exact total count to continue pagination

#### Scenario: pageSize zero is rejected

- **WHEN** caller invokes `list_sessions("projectA", { pageSize: 0, cursor: null })`
- **THEN** API SHALL return a validation error instead of silently clamping the page size

### Requirement: Fetch session summaries by id

The API SHALL expose a narrow capability to fetch light `SessionSummary` records for a bounded set of `sessionId` values within a project. This capability exists for pinned/hidden session reconciliation and MUST NOT be used as a replacement for full-history listing.

The response SHALL include summaries for ids that exist in the requested project and SHALL omit ids that do not exist or belong to another project. Returned summaries SHALL follow the same light metadata rules as `list_sessions`: deep metadata MAY be placeholder and MAY be filled through `session-metadata-update` when implementation chooses to scan those ids.

#### Scenario: pinned session outside first page can be fetched

- **WHEN** caller has pinned id `sid-old` that is not present in the first `list_sessions("projectA")` page
- **AND** caller invokes the by-id summary fetch for `projectA` with `["sid-old"]`
- **THEN** response SHALL include `sid-old` if the session exists under `projectA`

#### Scenario: foreign project id is omitted

- **WHEN** caller invokes the by-id summary fetch for `projectA` with an id that exists only under `projectB`
- **THEN** response SHALL NOT include that session summary

### Requirement: Dispatch project/session reads by active context

所有"读项目 / 读会话 / 读会话产物 / 全局搜索 / 项目 memory CRUD"类 IPC method 在 active context = `Ssh<host>` 时 SHALL 走当前 SSH `FileSystemProvider`（通过 `LocalDataApi::active_scanner()` 或 `LocalDataApi::active_fs_and_projects_dir()` helper），**不得**直接锁 `self.scanner` / `self.projects_dir` 字段而退化到本地数据。本 Requirement 覆盖的 method 集合 SHALL 至少包含以下 13 个：

**本 change 修复（8 处）**：
- `list_repository_groups`
- `project_memory_dir`
- `find_session_project`
- `get_session_summaries_by_ids`
- `get_subagent_trace`
- `get_image_asset`
- `get_tool_output`
- `search`

**已正确实现（3 处，本 change 加回归测试）**：
- `list_sessions` / `list_sessions_sync` / `list_sessions_paginated`
- `get_session_detail`
- `list_projects`

**memory 读写（4 处，change `ssh-project-memory-remote-rw` 起 SHALL 走 active SSH provider，不再 graceful skip）**：
- `get_project_memory` —— 走 `fs.read_dir` + `fs.read_to_string` 读远端 memory 目录
- `read_memory_file` —— 走 `fs.read_to_string` 读远端 memory 文件
- `add_memory` —— 走 `fs.create_dir_all` + `fs.write_atomic` 写远端 memory 文件，写完调 `discover_memory_layers` 返新 ProjectMemory
- `delete_memory` —— 走 `fs.remove_file` 删远端 memory 文件，删完调 `discover_memory_layers` 返新 ProjectMemory

**例外**：仅"重置本地数据根路径"语义的 method（`set_projects_dir` / `reconfigure_claude_root`）保持 local provider，不受本条约束。

#### Scenario: list_repository_groups 在 SSH context 下走远端 provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `list_repository_groups` IPC
- **THEN** 系统 SHALL 通过当前 SSH context 的 `FileSystemProvider` 扫描 `<remote_home>/.claude/projects/`
- **AND** 返回的 `RepositoryGroup.worktrees[]` SHALL 来自远端 fixture 的项目集合
- **AND** 返回结果 SHALL NOT 包含本地宿主机 `.git` 解析出的 `gitBranch` 值

#### Scenario: 辅助读类 method 在 SSH context 下走远端 provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `find_session_project(session_id)` / `project_memory_dir(project_id)` / `get_session_summaries_by_ids(ids)` 任一
- **THEN** 后端 SHALL 通过当前 SSH context 的 provider 读远端文件
- **AND** 返回的 project_id / path 字段 SHALL 与远端 fake fixture 一致
- **AND** 返回的路径字段（若存在，如 `project_memory_dir`）SHALL 以远端 `<remote_home>` 为根

#### Scenario: 会话产物读取在 SSH context 下走远端 provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `get_subagent_trace(session_id, ...)` / `get_image_asset(session_id, ...)` / `get_tool_output(session_id, ...)` 任一
- **THEN** 后端 SHALL 通过远端 SFTP 读取对应文件
- **AND** 远端 provider 的 `read_file` 调用计数 SHALL ≥ 1（fake provider 通过 `Mutex<usize>` 计数器观测）
- **AND** 本地 `LocalFileSystemProvider` 的同名方法 SHALL NOT 被调用

#### Scenario: search 在 SSH context 下使用 active provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `search(query)` IPC
- **THEN** `SessionSearcher` SHALL 接收当前 SSH provider 作为 `Arc<dyn FileSystemProvider>` 入参
- **AND** 搜索结果 SHALL 来自远端 `<remote_home>/.claude/projects/` 下的 jsonl 内容
- **AND** 后端**不得**硬编码 `LocalFileSystemProvider::new()` 作为 search 的数据源
- **AND** 远端 provider 的 `read_to_string` 或 `open_read_stream` 调用计数 SHALL ≥ 1

#### Scenario: 根路径重置类 method 仍用 local provider

- **WHEN** 调用方调 `set_projects_dir(new_path)` 或 `reconfigure_claude_root(new_root)`
- **THEN** 系统 SHALL 重置 `self.scanner` 为 `LocalFileSystemProvider` 包装下的新 `projects_dir`
- **AND** 该重置**不影响**已注册的 SSH context 的 provider 状态
- **AND** 若 active context 是 SSH，**仍**保持 SSH 为 active；后续调"读项目/会话"类 method 仍走 SSH provider
- **AND** 仅当 active context 切回 local 后，新的 local `projects_dir` 才生效

#### Scenario: 已实现的 method 在 SSH context 下保持远端行为

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `list_projects` / `list_sessions` / `list_sessions_sync` / `list_sessions_paginated` / `get_session_detail` 任一
- **THEN** 后端 SHALL 走 SSH provider 读远端数据（行为与本 change 前一致）
- **AND** 本 Requirement 配套的回归测试 SHALL 覆盖这 5 个 method，防止后续改动误退化为 local

#### Scenario: memory CRUD 在 SSH context 下走远端 provider

- **WHEN** active context 是 `Ssh<host>`
- **AND** 调用方调 `get_project_memory(project_id)` / `read_memory_file(project_id, file)` / `add_memory(project_id, file, content)` / `delete_memory(project_id, file)` 任一
- **THEN** 后端 SHALL 通过当前 SSH context 的 fs provider 调对应远端 fs ops（read_dir / read_to_string / write_atomic / create_dir_all / remove_file）
- **AND** 远端 fake provider 的对应 op counter（`read_dir_count` / `read_count` / `write_count` / `mkdir_count` / `rename_count` / `remove_count`）SHALL ≥ 1
- **AND** 本地 `LocalFileSystemProvider` 的同名方法 SHALL NOT 被调用
- **AND** 旧的 graceful skip 行为（`has_memory: false` / not_found 含 "SSH context" 字样）SHALL NOT 出现

### Requirement: Session 列表序列化暴露 cwd 字段

`list_sessions` 与 `get_session_detail` 返回的 `Session`（或 `SessionSummary`）IPC payload SHALL 暴露 `cwd?: string` 字段（camelCase）。该字段值来自 `cdt-core::Session.cwd`（详见 `project-discovery` spec `Expose session cwd for downstream display` Requirement），表示该 session jsonl 内首条带 `cwd` 字段消息的 `cwd` 值。

无 cwd 信息（jsonl 不含 `cwd`）时 SHALL 通过 `#[serde(skip_serializing_if = "Option::is_none")]` 在 payload 中省略该键，**不**得序列化为 `"cwd": null`，以保持老前端 / 老 fixture 兼容。

HTTP 路径（`GET /api/projects/:id/sessions` / `GET /api/projects/:id/sessions/:sid`）SHALL 同步暴露 `cwd` 字段——与 IPC 路径共享 `LocalDataApi::list_sessions` / `get_session_detail` 实现，自动适用。

#### Scenario: 含 cwd 的 session 在 list_sessions 返回中带 cwd

- **WHEN** `list_sessions(projectId)` 命中一个 jsonl session，其首条消息 `cwd = "/Users/foo/myrepo/.claude/worktrees/feat-x"`
- **THEN** 返回数组对应条目 SHALL 含 `"cwd": "/Users/foo/myrepo/.claude/worktrees/feat-x"`

#### Scenario: 无 cwd 的 session 在 list_sessions 返回中省略 cwd

- **WHEN** `list_sessions(projectId)` 命中一个 jsonl session，所有消息均不含 `cwd` 字段
- **THEN** 返回数组对应条目 SHALL NOT 包含 `cwd` 键
- **AND** 该 session 其它字段（`id` / `lastModified` / `size` / `isPinned`）SHALL 保留

#### Scenario: get_session_detail 元数据带 cwd

- **WHEN** `get_session_detail(projectId, sessionId)` 命中目标 session
- **THEN** `SessionDetail.metadata` 或顶层等价位置 SHALL 含 `"cwd": <value or omitted>`，与 `list_sessions` 同口径

### Requirement: get_session_detail 本地路径以单文件 stat 取元数据

`LocalDataApi::get_session_detail` 在本地（非 SSH）路径 SHALL 通过 `tokio::fs::metadata(jsonl_path)` 单次 stat 系统调用获取目标 session 的 `lastModified` 与 `size`，**SHALL NOT** 触发跨 project 的全量扫描（即不得调用 `ProjectScanner::scan()` 或等价的"列举 `~/.claude/projects/` 下所有目录"路径），也 SHALL NOT 为获取 mtime / size 而读取目标 jsonl 之外的任何文件。

session jsonl 文件不存在时，本地路径 SHALL fallback 至现有 `find_subagent_jsonl` 路径搜索 subagent jsonl（沿用现状行为）；fallback 仍不存在时 SHALL 返回 `ApiError::not_found`。

远程 SSH 路径行为不变（沿用现有 `list_sessions(projectId)` 轻量列举 + 单文件元数据获取）。

#### Scenario: 本地打开 session 详情不触发全量扫描

- **WHEN** `get_session_detail("foo-project", "session-1")` 在本地环境调用，`foo-project` 与 `session-1.jsonl` 在 `~/.claude/projects/foo-project/` 下存在
- **THEN** 后端实现 SHALL 仅对 `~/.claude/projects/foo-project/session-1.jsonl` 调一次 `tokio::fs::metadata`
- **AND** SHALL NOT 对 `~/.claude/projects/` 下其它 project 目录调 `read_dir` / `stat`
- **AND** SHALL NOT 调 `read_lines_head` / `read_to_string` 读取除目标 jsonl 之外的任何 jsonl

#### Scenario: 目标 jsonl 不存在 fallback 到 subagent 查找

- **WHEN** `get_session_detail("foo-project", "missing-session")` 调用，`missing-session.jsonl` 不在主目录但存在于 `subagents/agent-*.jsonl`
- **THEN** 后端 SHALL 通过 `find_subagent_jsonl` 路径定位到 subagent jsonl 并返回其 detail

#### Scenario: 目标 session id 完全不存在返回 not_found

- **WHEN** `get_session_detail("foo-project", "nope")` 调用，`nope.jsonl` 既不在主目录也不在 `subagents/` 下
- **THEN** 后端 SHALL 返回 `ApiError::not_found`，**不**触发全量扫描以试图反查

### Requirement: Contract test asserts get_session_detail does not cross project boundary

contract test 层 SHALL 通过 spy `FileSystemProvider` 包装（在测试 wrapper 里记录每个 `read_dir` / `read_lines_head` / `read_to_string` / `stat` 方法被调次数 + 路径列表），覆盖 `get_session_detail` 的本地路径，断言：调用 `get_session_detail(P, S)` 后，spy 记录的 `read_dir` 调用次数 == 0；`read_lines_head` 与 `stat` 的 path 集合 SHALL ⊆ {target jsonl path}（解析 jsonl 内容的 head-read 与目标 stat 允许）；spy 记录的所有 path 都 SHALL NOT 落在 `~/.claude/projects/<P>` 之外的兄弟 project 目录。

该 contract test SHALL 跑在 `crates/cdt-api/tests/ipc_contract.rs`（与 `#[ignore]` 的 perf bench 互补）；本断言 SHALL 在 CI 默认 job 内执行，对"不全扫"行为契约提供机器验证保护。

#### Scenario: spy FileSystemProvider 验证不读取兄弟 project

- **WHEN** 测试搭建 `tempdir` 下铺 3 个 project（`P_A` / `P_B` / `P_C`），每个 2 个 session jsonl
- **AND** 调用 `LocalDataApi::get_session_detail("P_A", "session_1")`
- **THEN** spy 记录的 `read_dir` 调用次数 SHALL 为 0
- **AND** spy 记录的所有 path 中 SHALL NOT 含 `P_B/` 或 `P_C/` 下任何文件
- **AND** `read_lines_head` / `stat` 的 path 集合 SHALL ⊆ `{tempdir/P_A/session_1.jsonl}`

### Requirement: ProjectScanner shared read semaphore injection

`ProjectScanner` SHALL 接受外部注入的 `Arc<tokio::sync::Semaphore>` 控制 head-read 并发，所有 `LocalDataApi` 内部调用 SHALL 复用同一 `Arc<Semaphore>` 实例（容量默认 `SHARED_READ_CONCURRENCY = 64`）；MUST NOT 在每次 IPC（含 `list_sessions` / `list_group_sessions` / `list_repository_groups`）新建独立 semaphore，否则多 IPC 并发时实际并发上限会变为 `IPC 数 × 64`，违反 `.claude/rules/perf.md::CPU 反模式`。

`ProjectScanner::new(projects_dir, fs)` 旧构造器 SHALL 保留为 `#[cfg(test)]` 便利构造（内部仍新建 semaphore），生产代码 SHALL 调 `ProjectScanner::new_with_semaphore(projects_dir, fs, semaphore)`。

`LocalDataApi` SHALL 在构造时创建 / 接受 `shared_read_semaphore: Arc<Semaphore>` 字段，所有内部 `ProjectScanner` 构造点 SHALL 传入该字段。

#### Scenario: 19 worktree 并发拉骨架共享 semaphore
- **WHEN** `list_group_sessions` 内部并发跑 19 个 `scan_project_dir`，每个 worktree 含 100 个 session
- **THEN** 同时 in-flight 的 `read_lines_head` 调用数 SHALL 不超过 64（共享 semaphore 容量）
- **AND** SHALL NOT 出现 19 × 64 = 1216 并发的击穿

#### Scenario: 测试代码可用 new 便利构造
- **WHEN** `#[cfg(test)]` 单测调 `ProjectScanner::new(projects_dir, fs)`
- **THEN** 测试代码无需手动创建 semaphore；编译通过

#### Scenario: 生产代码强制走 new_with_semaphore
- **WHEN** 生产代码 grep `ProjectScanner::new\(` （非 cfg(test) 块）
- **THEN** SHALL 仅出现 `new_with_semaphore` 调用，老 `new` 调用 SHALL 仅在 `#[cfg(test)]` 块内

### Requirement: `ProjectScanCache` 按事件语义分级失效

`LocalDataApi::new_with_watcher(...)` 构造路径 SHALL spawn 后台 task（"unified invalidator"），订阅 `FileWatcher::subscribe_files()` 广播。该 task 对每条 `FileChangeEvent` SHALL 仅根据 `FileChangeEvent` 字段（`project_id` / `session_id` / `deleted` / `project_list_changed`）+ `ProjectScanCache` snapshot lookup 决定**是否**失效 `ProjectScanCache` Local entry。三档判定结果 SHALL **仅**用于 invalidate 决策，**不**再用于填写 `FileChangeEvent.session_list_changed` 字段——后者由 watcher 层负责（详 `file-watching` Requirement `跟踪 session 首见性以填写 revalidation hint`）。本 Requirement 同时 SHALL 把 cache snapshot 视角的"unknown_session"判定结果作为**辅助 hint** 暴露给 unified invalidator emit 路径，与 watcher 字段做并集 OR（详 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement 的 emit 公式）。

**判定规则（三档，仅决定 invalidate）**：

1. `event.project_list_changed == true` **OR** `event.deleted == true` → 调 `ProjectScanCache::invalidate_local()`，inc counter `project_scan_cache.invalidate.structural`
2. `event.session_id` 非空 **AND** (`ProjectScanCache::has_entry(local_ctx) == true` **OR** `ProjectScanCache::has_in_flight_scan() == true`) **AND** `ProjectScanCache::contains_session_id(local_ctx, &event.project_id, &event.session_id) == false`（cache 已有该 ctx 的 entry 或当前有 in-flight scan 在跑，且 snapshot 不含此 session）→ 同规则 1：`invalidate_local()` + structural counter
3. 其他（普通 JSONL append + watcher 折叠的 subagent 修改 + 空 sid 事件 + cache 无 entry **且**无 in-flight scan 时的任意非 structural 事件）→ **不**调任何失效 API，保留现有 cache，inc counter `project_scan_cache.invalidate.content_append_skipped`

**为何需要规则 2**：`cdt-watch::FileWatcher` 在构造时**预填**当前已存在的 project 目录到 `known_projects` HashSet。已知 project 下新建 session 时 `mark_project_seen` 不会返回 true，watcher 输出 `plc=false, deleted=false`——与"已知 session JSONL 追加"在 `project_list_changed` / `deleted` 字段上**外观完全相同**。仅靠 (plc, deleted) 两 bool 判定会让新 session 最长 `LOCAL_CACHE_TTL = 300s` 不可见。规则 2 用 cache snapshot 反向查询补这个语义缺口决定是否清缓存。watcher 层填写的 `session_list_changed` 字段为 emit 路径直接提供前端 revalidate hint，不依赖 cache 状态，与本规则的 invalidate 决策独立。

**为何需要 `has_entry || has_in_flight_scan` 守护组合**：

- **`has_entry` 单条件不足以防风暴**：lag 路径调 `invalidate_local()` 后 cache 被清空，若不守护，后续普通 append 事件 `contains_session_id` 一律返 false → unknown_session 命中 → 又调 `invalidate_local()` 反复 bump `invalidation_generation` → 在重扫期间 `finish_scan_with_insert` 因 generation mismatch 一直丢弃 snapshot → cache 长期无法 repopulate（持续重扫风暴）。`has_entry` 守护让 cache 空时直接走规则 3 等待业务路径重扫填回。

- **仅 `has_entry` 又会漏掉 in-flight scan 期间结构事件**：cache 空 + 业务路径已经 `begin_scan` 在跑 scan 期间到达"已知 project 下新 session"事件被吞 → generation 不 bump → scan 完成 `finish_scan_with_insert` 旧 snapshot 因 generation 未变成功落地 → 新 session 最长等 TTL 5min 才能看到。

- **联合条件 `has_entry || has_in_flight_scan` 二者兼得**：cache 有 entry 或 scan 在途时走规则 2 判定 bump；cache 空且无 scan 在途时走规则 3 不 bump。**注意**：cache 空且无 scan 在途时本规则**不**清缓存，但此时 watcher 层已经在 `session_list_changed` 字段上承载了"first-seen" 信号，下游 emit 路径仍能让前端正确触发 revalidate（前端 revalidate 路径自然走 cache miss + 重 scan 兜底）。

**对各类真实 fs 事件的语义覆盖**（对应 `cdt-watch::FileWatcher::parse_project_event` 的输出）：

- 新 project 目录创建（`<projects_root>/<pid>` dir-create）→ watcher 输出 `plc=true, sid=""` → 走规则 1（invalidate_local）
- 启动后第一次见某 pid（典型场景：watcher 重启）→ watcher 输出 `plc=true` → 走规则 1（invalidate_local）
- **已知 project 下新 session 首次出现** → watcher 输出 `plc=false, deleted=false` 且 `contains_session_id == false` → 走规则 2（invalidate_local，仅当 has_entry||has_in_flight 时）；watcher 同时填 `session_list_changed=true` 给 emit 路径
- 已知 project 已知 session JSONL 追加（普通 hot path）→ watcher 输出 `plc=false, deleted=false` 且 `contains_session_id == true` → 走规则 3（不清缓存）
- watcher 折叠的 subagent JSONL **修改**（事件 `(pid, sid=父, deleted=false, plc=false)` + `contains_session_id(父 sid) == true`）→ 走规则 3（不清缓存）
- 主 session JSONL 删除 → watcher 输出 `deleted=true` → 走规则 1（invalidate_local）；watcher 同时填 `session_list_changed=true` 给 emit 路径
- watcher 折叠的 subagent JSONL **删除**（事件 `(pid, sid=父, deleted=true, plc=false)`）→ 走规则 1（**false-positive**：事件无法区分主 vs subagent 删除；触发一次重扫即结束，无正确性问题，详 design R6）

**MUST NOT**：

- MUST NOT 扩展或读取 `cdt-core::FileChangeEvent` 中除 `project_id` / `session_id` / `deleted` / `project_list_changed` 之外的其他字段做**判定**输入（`session_list_changed` 字段由 watcher 层填，本规则**仅消费 `event.session_id` 等输入字段**，不依赖 emit 字段做判定输入）
- MUST NOT 在事件回调路径内调任何 fs 操作（`fs::stat` / `fs::metadata` 等）—— 完全基于事件字段 + cache snapshot lookup 判定
- MUST NOT 引入 per-project 失效粒度（`ProjectScanCache.entries: HashMap<ContextId, Arc<Vec<Project>>>` 当前数据结构无 per-project entry 概念，per-project 重构超本 Requirement scope）
- MUST NOT 让 invalidate 决策影响 `FileChangeEvent.session_list_changed` 字段填写——该字段由 watcher 层独立决定，本规则只产出"是否 invalidate" + "供 emit 路径 OR 兜底的 cache_unknown_hint"

**`ProjectScanCache::contains_session_id` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `contains_session_id(&self, ctx: &ContextId, project_id: &str, session_id: &str) -> bool`，遍历指定 ctx 对应 entry 的 `Arc<Vec<Project>>`，定位 `Project.id == project_id` 后检查 `Project.sessions: Vec<String>` 是否含 `session_id`；ctx 无 entry 或 project 不存在时返回 `false`。复杂度 O(N project × N session_per_project)，对 30 project × 538 session corpus 单次 ~10µs，可在 hot 路径调用。

**`ProjectScanCache::has_entry` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `has_entry(&self, ctx: &ContextId) -> bool`，返回 `entries` 是否含此 ctx 的 entry。invalidator 在规则 2 判定前 SHALL 先用本方法守护——cache 空时跳过 unknown_session 判定，避免 lag 后被普通 append 事件持续触发 invalidate 导致重扫风暴。

**`ProjectScanCache::has_in_flight_scan` API 契约**：`ProjectScanCache` SHALL 暴露公开方法 `has_in_flight_scan(&self) -> bool`，返回当前 `in_flight_scans > 0`。invalidator 在规则 2 判定前 SHALL 与 `has_entry` 共同 OR 守护——cache 空但有 scan 在途时仍 bump generation，让 in-flight scan 完成回写时识别 race 丢弃 stale snapshot。

**`ProjectScanCache::begin_scan` / `finish_scan_with_insert` / `abort_scan` API 契约**：业务路径 `scan_projects_cached_with` SHALL 用 `begin_scan` 替代裸 `invalidation_generation()` 拿 recorded_generation 同时 `in_flight_scans += 1`；scan 成功时 SHALL 用 `finish_scan_with_insert` 替代 `try_insert`（内部 `in_flight_scans -= 1` + race 校验）；scan 失败时 SHALL 调 `abort_scan` 配对 `begin_scan` 不漏减。这三 API 联合保护 in-flight scan 与 invalidator 之间的 race 协议。

**SSH context entry 不受 file-change 影响 + SSH event 跳过 local cache hint**：watcher 是 Tauri 本地 fs 的硬不变量。invalidator 推算 `ContextId::local(projects_dir)` 决定失效作用域；`ProjectScanCache::invalidate_local()` 实现仅对 `FsKind::Local` entry 生效，SSH entry 仍按既有 TTL 自然过期。SSH `polling_watcher` 通过 `FileWatcher::attach_remote` 喂入同一 watcher broadcast 的事件，进入 unified invalidator 后 SHALL 通过 watcher 来源判定守护——unified invalidator SHALL 调 `FileWatcher::is_local_project(&event.project_id)` 检查 event 的 project_id 是否在 `local_projects_seen` 集合内（该集合由 `parse_project_event` 所有分支在 emit 前通过 `mark_local_origin` 写入，与 `known_projects` 的 first-seen 语义解耦；SSH 事件由远端 polling 直接构造不进 `parse_project_event`），若返回 `false`（即 SSH 事件）则 SHALL 跳过本规则的三档判定，直接走"不 invalidate + emit_session_list_changed_hint=false"——`session_list_changed` 字段已由 SSH polling watcher 在远端事件上对称填好，**不**需要 local cache hint OR 兜底，否则 SSH 普通 size/mtime append（watcher 已填 `false`）会因 `contains_session_id(local_ctx, ssh_pid, ssh_sid)` 永远返 false 让 `emit_session_list_changed_hint=true`，破坏 SSH/Local 字段对称语义并侵蚀 PR #291 append 降噪收益。详 `file-watching` Requirement `Watch SSH remote project directory via SFTP polling`。`is_local_project` 限制：仅按 project_id 字符串判定，SSH 远端与 local 同名 project 共存时可能误判 local；本 spec 接受此 edge case，根治留 followup（需 watcher 注入 ContextId 做来源排除）。

**`new()` 构造路径不启动该订阅**：`LocalDataApi::new()`（无 watcher 参数）SHALL NOT spawn 此 task；该场景仅依赖被动 generation 校验路径兜底，与 `MetadataCache` / `ParsedMessageCache` 在 `new()` 路径的行为对齐。

**broadcast lag 走保守全失效**：`broadcast::Receiver::recv` 返回 `Err(RecvError::Lagged(_))` 时 SHALL 调 `invalidate_local()` 并 inc counter `project_scan_cache.invalidate.lag_conservative`，因为 lag 期间可能错过 `plc=true` / `deleted=true` 事件且 `ProjectScanCache` 没有 path-level 被动校验机制可兜底。lag 路径下 file_tx emit 行为契约由 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement 单独承担（synthetic structural event 兜底）。返回 `Err(RecvError::Closed)` 时 SHALL 退出 loop。

> 该 lag 行为与 `parsed-message 缓存按 file-change 广播主动失效` Requirement 的 lag 静默继续策略**有意不一致**：parsed-message cache 在 lookup 时 stat 比对 `FileSignature` 兜底 lag 错过的事件；ProjectScanCache 无类似被动校验，lag 时必须保守清空。

**telemetry counter 注册**：实现 SHALL 在 `cdt-telemetry` 静态白名单中注册以下 3 个 counter：

- `project_scan_cache.invalidate.structural`
- `project_scan_cache.invalidate.content_append_skipped`
- `project_scan_cache.invalidate.lag_conservative`

每条事件 SHALL 按规则结果 inc 对应 counter 各 1 次。

**性能契约**：长时间使用场景（活跃 claude-code 会话每秒多次追加 JSONL）下，`content_append_skipped` 计数 SHALL 远超 `structural`（典型预期 ≥ 95% 走 skipped 分支）；偏离此预期是判定逻辑或 watcher 字段填充偏差的信号。

#### Scenario: 已知 session JSONL 追加 SHALL NOT 失效 cache

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，且 `ProjectScanCache` 已经因前一次 `list_repository_groups` 写入了某 ctx 的 entry，含 project `pa` 和 session `sa`
- **AND** `<projects_root>/pa/sa.jsonl` 被 claude-code 追加新行
- **AND** `FileWatcher` 广播一条对应 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false, session_list_changed: false }`（watcher 跟踪集合已含 `(pa, sa)`）
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa")` 得到 `true`（规则 2 不命中）
- **AND** MUST NOT 调 `invalidate_local`
- **AND** `ProjectScanCache::lookup` 后续仍 SHALL 命中既有 entry（同一 `root_generation` / `context_generation` 下）
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1

#### Scenario: 已知 project 下新 session 首次出现 SHALL 失效 cache

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，`ProjectScanCache` 已写入某 ctx 的 entry，含已知 project `pa` 与已知 sessions `{sa1, sa2}`（`sa_new` 不在此列表）
- **AND** claude-code 在已知 project `pa` 下创建新 session `sa_new`，写入 `<projects_root>/pa/sa_new.jsonl`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false, session_list_changed: true }`（watcher 跟踪集合此前不含 `(pa, sa_new)` → first-seen → `session_list_changed=true`）
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa_new")` 得到 `false`
- **AND** MUST 调 `ProjectScanCache::invalidate_local()`（规则 2 触发）
- **AND** 下一次 `list_repository_groups` SHALL 走 cache miss 并把 `sa_new` 纳入返回值
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: cache 空 + 新 session 事件 SHALL NOT invalidate（emit 不受影响）

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造，`ProjectScanCache::entries` 为空（冷启 / `reconfigure_claude_root` 后 / SSH context 切换让 Local entry 被驱逐），`has_in_flight_scan() == false`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false, session_list_changed: true }`（watcher 跟踪集合此前不含 `(pa, sa_new)`）
- **THEN** 后台 invalidator MUST NOT 调 `invalidate_local`（规则 2 守护命中：`has_entry == false && has_in_flight_scan == false`）
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1
- **AND** invalidate 决策的"未触发"SHALL NOT 影响 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement 定义的 emit 行为——前端仍 SHALL 收到 `session_list_changed=true` 触发兜底 revalidate

#### Scenario: 顶层 dir-create 标 plc=true 时直接走规则 1

- **WHEN** `ProjectScanCache` 已存若干 ctx entry
- **AND** claude-code 创建新 project 顶层目录 `<projects_root>/p_new`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "p_new", session_id: "", deleted: false, project_list_changed: true, session_list_changed: false }`
- **THEN** 后台 invalidator MUST 仅基于 `event.project_list_changed == true` 走规则 1，调 `invalidate_local()`
- **AND** SHALL NOT 调 `contains_session_id`（事件 `session_id == ""` 触发规则 2 的 `!session_id.is_empty()` 守护跳过）
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: 删除已知 session JSONL SHALL 失效 cache

- **WHEN** `ProjectScanCache` 已存某 ctx entry 且内含 project `pa` / session `sa`
- **AND** 用户或外部工具删除 `<projects_root>/pa/sa.jsonl`
- **AND** `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: true, project_list_changed: false, session_list_changed: true }`
- **THEN** 后台 invalidator MUST 仅基于 `event.deleted == true` 走规则 1，调 `ProjectScanCache::invalidate_local()`
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: subagent JSONL 修改 SHALL NOT 失效 cache

- **WHEN** `ProjectScanCache` 已存某 ctx entry，含 project `pa` / 父 session `s_parent`
- **AND** claude-code 写入 `<projects_root>/pa/s_parent/subagents/agent-xyz.jsonl`
- **AND** watcher 折叠到父 session 后广播 `FileChangeEvent { project_id: "pa", session_id: "s_parent", deleted: false, project_list_changed: false, session_list_changed: false }`（subagent 路径 SHALL NOT 进入跟踪集合 → `session_list_changed=false`）
- **THEN** 后台 invalidator MUST 调 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "s_parent")` 得到 `true`
- **AND** MUST NOT 调任何失效 API
- **AND** counter `project_scan_cache.invalidate.content_append_skipped` MUST inc 1
- **AND** `ProjectScanCache::lookup` 后续仍 SHALL 命中既有 entry

#### Scenario: subagent JSONL 删除触发 false-positive invalidate（接受）

- **WHEN** `ProjectScanCache` 已存某 ctx entry，含 project `pa` / 父 session `s_parent`
- **AND** subagent 文件 `<projects_root>/pa/s_parent/subagents/agent-xyz.jsonl` 被删除
- **AND** watcher 折叠到父 session 后广播 `FileChangeEvent { project_id: "pa", session_id: "s_parent", deleted: true, project_list_changed: false, session_list_changed: false }`（subagent 删除路径靠 `deleted=true` 触发刷新，`session_list_changed` 由 watcher 嵌套分支固定填 `false`）
- **THEN** 后台 invalidator MUST 基于 `event.deleted == true` 走规则 1，调 `ProjectScanCache::invalidate_local()`
- **AND** 这是已知的 **false-positive 行为**：事件字段无 path，无法区分主 session 删除 vs subagent 删除；本 spec 显式接受此 false-positive，触发一次 ProjectScanner 重扫的成本可接受
- **AND** counter `project_scan_cache.invalidate.structural` MUST inc 1

#### Scenario: SSH context entry 不受 file-change 影响

- **WHEN** `ProjectScanCache` 已存 SSH ctx entry（由 SSH `polling_watcher` 间接触发或通过其它路径写入）
- **AND** 本地 `FileWatcher` 广播 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false, session_list_changed: false }`
- **THEN** unified invalidator MUST 调 `ProjectScanCache::invalidate_local()`，仅对 `FsKind::Local` entry 生效
- **AND** SSH ctx entry SHALL NOT 被失效，按既有 TTL 自然过期

### Requirement: SessionDetail 与高频 DataApi 方法 SHALL 用 typed Rust struct 暴露字段

`crates/cdt-api/src/ipc/types.rs::SessionDetail` 的 6 个字段（`chunks` / `metrics` / `metadata` / `context_injections` / `injections_by_phase` / `phase_info`）SHALL 用 typed Rust struct（含本 capability 新增的 `SessionDetailMetrics` / `SessionDetailMetadata` 与 `cdt-core` 已有的 `Chunk` / `ContextInjection` / `ContextPhaseInfo`）持有；`DataApi` trait 中至少以下 5 个高频方法的返回类型 SHALL 是 typed `Result<XxxResponse, ApiError>` 而非 `Result<serde_json::Value, ApiError>`：`search` / `get_config` / `update_config` / `get_subagent_trace` / `get_notifications`。typed 化 SHALL **不**改变任何 wire JSON 形状——所有 typed struct 的 serde 字段名、camelCase 命名、enum tag（`Chunk.kind` / `ContextInjection.category`）、`xxxOmitted` 标记 SHALL 与本要求被引入之前的 wire 形状逐字节一致。其余 13 个 `Result<serde_json::Value, ApiError>` 方法（SSH 子集 / 文件路径子集 / Trigger CRUD / `validate_path`）SHALL 暂留 `Value`，由后续 change 按本 capability 提供的判定准则（`design.md::D2`）逐批 typed 化。

#### Scenario: SessionDetail 6 个字段编译期为 typed

- **WHEN** 调用方在 Rust 代码中按 `let detail: SessionDetail = local_data_api.get_session_detail(...).await?;` 取得 `SessionDetail`
- **THEN** `detail.chunks` SHALL 直接是 `Vec<cdt_core::Chunk>`，`detail.metrics` SHALL 是 `SessionDetailMetrics`，`detail.metadata` SHALL 是 `SessionDetailMetadata`，`detail.context_injections` SHALL 是 `Vec<cdt_core::ContextInjection>`，`detail.injections_by_phase` SHALL 是 `BTreeMap<String, Vec<cdt_core::ContextInjection>>`，`detail.phase_info` SHALL 是 `cdt_core::ContextPhaseInfo`
- **AND** 上述任一字段 SHALL **不**是 `serde_json::Value`
- **AND** 调用方按 `detail.metrics.message_count` 直接访问字段 SHALL 编译通过（不需 `serde_json::from_value` / `as_object()` 之类 runtime 解构）

#### Scenario: SessionDetail 序列化 wire 形状不变

- **WHEN** 同样的输入数据分别走 typed 化前与 typed 化后的 `LocalDataApi::get_session_detail`，并各自 `serde_json::to_value(&detail)?` 序列化
- **THEN** 两次序列化产物 SHALL 在所有 key 名 / value 形状 / 嵌套层次上逐字段一致——具体含 `chunks[*].kind`（`"user"` / `"ai"` / `"system"` / `"compact"`）、`chunks[*].subagents[*].messages` / `messagesOmitted`、`chunks[*].toolExecutions[*].output` / `outputOmitted`、`chunks[*].responses[*].content` / `contentOmitted`、`metrics.message_count`（snake_case 历史 wire，**不**是 `messageCount`，详 `design.md::D5` + `D7`）、`metadata.last_modified` / `metadata.size` / `metadata.cwd`（snake_case 历史 wire）、`contextInjections[*].category`、`injectionsByPhase` 的 key 形状（`String`，由 `phase_number.to_string()` 得出）、`phaseInfo` 内字段
- **AND** `crates/cdt-api/tests/ipc_contract.rs` 现有覆盖 SessionDetail 的所有断言（含 `session_detail_single_phase_injections_by_phase_equals_context_injections` / `session_detail_multi_phase_preserves_phase1_injections` / `session_detail_title_field_round_trip`）SHALL 保持绿

#### Scenario: 5 个高频 DataApi 方法返回 typed

- **WHEN** 调用方在 Rust 代码中按 `let cfg: AppConfig = local_data_api.get_config().await?;`（或 `update_config` / `search` / `get_subagent_trace` / `get_notifications` 同形）取得返回值
- **THEN** 返回类型 SHALL 是 typed struct（`cdt_config::AppConfig` / `cdt_core::SearchSessionsResult` / `Vec<cdt_core::Chunk>` / `cdt_config::GetNotificationsResult`）而非 `serde_json::Value`
- **AND** 编译期访问字段（如 `cfg.theme` / `result.results[0].sessionId`）SHALL 通过类型检查
- **AND** `serde_json::to_value(&typed_return)?` 产物 SHALL 与 typed 化前的 hand-built JSON 形状逐字段一致；以下两处 EXCEPTION：
  - `search` empty query 路径：typed 化后形状从 `{query, results}` 扩为 `{query, results, totalMatches, sessionsSearched, isPartial}`（`SearchSessionsResult` 完整字段），属于 bug fix（详 `design.md::D8`），新增字段全部为 `0` / `[]` / `false` 默认值，不破坏前端 `CommandPalette.svelte:116` 现有 `"totalMatches" in session ? ... : ...` "in" 判定路径
  - `get_sessions_by_ids` not-found fallback 路径：typed 化后 `metadata` 从 `{"status":"not_found"}` 改为 typed default `{"last_modified":null,"size":null,"cwd":null}`（移除 ad-hoc status 带外标记），`chunks` / `phase_info` / `metrics` 从 `null` 改为各自 typed default；前端按 `result.projectId === ""` 判定 not-found（已有信号），详 `design.md::D9`

#### Scenario: 13 个低频方法暂留 Value 是 spec-allowed

- **WHEN** 调用方按 `let resp: serde_json::Value = local_data_api.ssh_connect(...).await?;`（或其他 13 个低频方法之一）取得返回值
- **THEN** 实现 SHALL 仍允许返回 `Result<serde_json::Value, ApiError>`，不要求本 change 必须 typed 化
- **AND** 该方法源码处 SHALL 含 `// TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2` 形式的注释链向后续 change

#### Scenario: 前端 SessionDetail TS interface 同步 typed

- **WHEN** 前端 `ui/src/lib/api.ts` 中定义 `SessionDetail` interface
- **THEN** `metrics` / `metadata` / `contextInjections` / `injectionsByPhase` 四个字段 SHALL **不**是 `Record<string, unknown>` / `unknown[]`
- **AND** 上述字段 SHALL 引用与 Rust 端 `SessionDetailMetrics` / `SessionDetailMetadata` / `ContextInjection` / `Record<string, ContextInjection[]>` 镜像的 typed TS interface
- **AND** `pnpm --dir ui run check`（svelte-check）SHALL 在引入本 typed 后通过

### Requirement: SessionDetailMetrics 与 SessionDetailMetadata 字段定义 SHALL 与历史 snake_case wire 逐字段对齐

新增 typed struct `SessionDetailMetrics` SHALL 含 `message_count: usize` 单字段（serde **snake_case** rename，与 `local.rs:3243` 历史 hand-built `json!({"message_count": ...})` wire 一致）；`SessionDetailMetadata` SHALL 含 `last_modified: Option<String>` / `size: Option<u64>` / `cwd: Option<String>` 三字段（serde **snake_case** rename，与 `local.rs:3244-3247` 历史 wire 一致，全部 nullable）。两个 struct 序列化产物 SHALL 与 `crates/cdt-api/src/ipc/local.rs` 历史 hand-built JSON 在所有可能输入下逐字段一致——typed 化 SHALL **不**修正 camelCase IPC 契约违规（详 `design.md::D7`，留 followup issue）。

#### Scenario: SessionDetailMetrics 序列化 wire 形状

- **WHEN** 实现按 `serde_json::to_value(&SessionDetailMetrics { message_count: 42 })?` 序列化
- **THEN** 产物 SHALL 是 `{"message_count": 42}`（snake_case，**不**是 `{"messageCount": 42}`）
- **AND** 与 `local.rs:3243` 历史 `json!({"message_count": 42})` 形状逐字节一致

#### Scenario: SessionDetailMetadata 字段全 nullable + snake_case wire

- **WHEN** 文件系统 `metadata()` 调用失败 / jsonl 中 `cwd` 字段缺失
- **THEN** `SessionDetailMetadata { last_modified: None, size: None, cwd: None }` 序列化 SHALL 产出 `{"last_modified": null, "size": null, "cwd": null}`（snake_case，**不**是 `lastModified`）
- **AND** 与 `local.rs:3244-3247` 历史 `json!({"last_modified": null, "size": null, "cwd": null})` 形状逐字节一致
- **AND** 前端 `SessionDetail.svelte:856` 按 `detail.metadata.cwd` 消费 SHALL 与改动前行为一致（其余 `last_modified` / `size` 当前前端未消费但 wire 形状仍 SHALL 保留以兼容 HTTP transport / 未来 consumer）

### Requirement: ipc_contract 测试 SHALL 覆盖 typed 字段命名 round-trip

`crates/cdt-api/tests/ipc_contract.rs` SHALL 在本 change 后含至少一个新测试（例如 `session_detail_typed_metrics_metadata_round_trip`）覆盖 `SessionDetail` typed 化后的 wire 形状：从 typed struct 出发 `serde_json::to_value` 再 `serde_json::from_value::<SessionDetail>` 反序列化回 typed，断言所有字段值不变。

#### Scenario: SessionDetail typed round-trip

- **WHEN** 测试构造 `SessionDetail { chunks: Vec::new(), metrics: SessionDetailMetrics { message_count: 0 }, metadata: SessionDetailMetadata::default(), context_injections: Vec::new(), injections_by_phase: BTreeMap::new(), phase_info: ContextPhaseInfo::default(), is_ongoing: false, title: None, session_id: "s".into(), project_id: "p".into() }`，序列化为 `Value`，再反序列化回 typed
- **THEN** 反序列化产物 SHALL 与原始 `SessionDetail` 字段逐一相等（`PartialEq`）
- **AND** 序列化产物的顶层 key 集合 SHALL 是 `{sessionId, projectId, chunks, metrics, metadata, contextInjections, injectionsByPhase, phaseInfo, isOngoing, title}`（顶层 SessionDetail 是 camelCase）
- **AND** `metrics` / `metadata` 内部字段 SHALL 仍是 snake_case（`message_count` / `last_modified` / `size` / `cwd`），与 `local.rs:3243-3247` 历史 hand-built wire 一致（详 `design.md::D5` + `D7`）

### Requirement: Unified invalidator 作为 `LocalDataApi.file_tx` 唯一生产者

`LocalDataApi::new_with_watcher(...)` 启动路径 SHALL 把 `spawn_unified_cache_invalidator` 升级为 `LocalDataApi.file_tx` 的**唯一**生产者，**不**再 spawn 任何独立的 `bridge_task` 把 `watcher.subscribe_files()` 直接转发到 `file_tx`。invalidator 内部 sync 跑完三档判定（详 `ProjectScanCache 按事件语义分级失效` Requirement）后 SHALL 把 enriched `FileChangeEvent` 通过 `file_tx.send(enriched)` 广播给下游消费者（Tauri host emit / HTTP `spawn_file_bridge` / 其它 `subscribe_file_changes` 调用方）。

SSH 路径 SHALL 通过 `cdt-watch::FileWatcher::attach_remote(sftp, projects_dir, cancel_token)` 接入 watcher broadcast，`LocalDataApi::attach_remote_watcher` SHALL NOT 再走 `RemotePollingWatcher::spawn(..., self.file_tx.clone(), ...)` 路径——SSH event 必须经过同一 unified invalidator enrichment gateway，与 Local event 行为一致。`FileWatcher::attach_remote` 签名 SHALL 接受调用方注入的 `CancelToken`（替代原内部 `CancelToken::new()`），保留 `RemoteWatcherHandle` 返回值不变；调用方持有 token clone 用于 dead-signal monitor 路径，外部 disconnect 时仍能 cancel SSH polling。

**Emit 时机契约（unified invalidator loop 顺序）**：

1. `rx.recv().await` 收 raw event
2. sync 调 `apply_file_event_to_project_scan_cache(event)` 拿判定结果（返回 `EnrichDecision { invalidated: bool, emit_session_list_changed_hint: bool }`）；该函数内部锁在 sync block 末尾自动释放
3. 构造 `enriched_event = FileChangeEvent { session_list_changed: event.session_list_changed || decision.emit_session_list_changed_hint, ..raw_event }`——OR 公式让 watcher 视角 + cache 视角并集决定字段，最大兜底
4. 调 `file_tx.send(enriched_event)` broadcast emit（**锁已释放**，emit 永不在持锁路径）
5. async 调 `apply_file_event_to_parsed_cache(event).await`（**不**阻塞 emit）

emit MUST 在 step 4 完成（即 sync invalidate 之后，async parsed invalidate 之前）。这保证：(a) 前端拿到 file-change 时 `ProjectScanCache` 状态已是事件后的最新；(b) 前端无需等磁盘 stat I/O 完成；(c) `parsed_cache` 失效路径仍走 async 不阻塞 emit。

**emit 字段 OR 公式语义**：watcher 层填的 `event.session_list_changed` 是判定**主源**（基于 watcher 跟踪集合首见性，详 `file-watching` Requirement `跟踪 session 首见性以填写 revalidation hint`）；`decision.emit_session_list_changed_hint` 是 cache 视角的辅助 hint（值 = "本 event 命中 `ProjectScanCache 按事件语义分级失效` Requirement 规则 2 的 unknown_session 判定条件"）。两源并集 OR 兜底 watcher 重启 / `reconfigure_claude_root` 等让 watcher 跟踪集合重置但 cache 仍有有效 snapshot 的窗口。

**仅 Local event 参与 OR 兜底**：cache hint OR 仅对 Local event 应用——unified invalidator SHALL 调 `FileWatcher::is_local_project(&event.project_id)` 守护（基于 `local_projects_seen` 集合判定），**仅** `is_local_project=true` 的 event 才查 `apply_file_event_to_project_scan_cache` 取 hint 并参与 OR；SSH event（`is_local_project=false`）SHALL 跳过 cache 查询，`emit_session_list_changed_hint=false` 强制 emit 等于 `event.session_list_changed`。理由：local cache 的 `contains_session_id(local_ctx, ssh_pid, ssh_sid)` 对 SSH event 永远返 false，若不守护会让 SSH 普通 append 被错误升 `session_list_changed=true`，破坏对称 + 噪声回归。SSH 路径 watcher 字段已由 SSH polling baseline 视角对称填写（详 `file-watching` Requirement `Watch SSH remote project directory via SFTP polling`），无需 OR 兜底。

**反压**：`broadcast::Sender::send` 满时丢旧元素不阻塞，invalidator 自身永远不会被慢 subscriber 阻塞；slow subscriber 引发的 lag 走下游 bridge 的 `Lagged` 兜底（见 `Emit push events for file changes and notifications` Requirement 的 lag 兜底契约）。

**broadcast lag 路径 SHALL emit synthetic structural event**：`rx.recv().await` 返回 `Err(RecvError::Lagged(n))` 时除调 `apply_lag_to_project_scan_cache`（保守 `invalidate_local()` + counter `lag_conservative`）外，SHALL 显式 send 一条 synthetic `FileChangeEvent { project_id: "", session_id: "", deleted: false, project_list_changed: true, session_list_changed: true }` 到 `file_tx`。理由：本路径的 lag 在 `watcher.subscribe_files()` 上游 receiver 上，下游 `LocalDataApi.file_tx` 的下游 bridge（src-tauri Tauri host emit / HTTP SSE bridge）的 `RecvError::Lagged` 兜底监听的是 `file_tx`——上游 lag 不会让下游 receiver 同步 lag，下游 bridge 的 sse-lagged 通知路径不会触发，前端连兜底 silent refresh 都收不到。synthetic event 让前端三档守护命中并触发兜底全量 revalidate。返回 `Err(RecvError::Closed)` 时 SHALL 退出 loop。

**Synthetic event 在所有下游 bridge 路径的传播契约**：synthetic event 经 `LocalDataApi.file_tx` broadcast 后，下游两路 bridge SHALL 按既有 forward 路径处理，**不**对 synthetic event 做特殊识别 / 过滤：

- **Tauri host bridge**（src-tauri）：SHALL `app.emit("file-change", &payload)` 转发 synthetic event 到 webview，与 real event 行为一致
- **HTTP SSE bridge**（HTTP 客户端 / 浏览器 transport 路径）：SHALL 把 synthetic event 序列化为 `PushEvent::FileChange { ... }` 推到 `/api/events` SSE stream，与 real event 形态一致

**Synthetic event 在前端消费侧的副作用守护**：所有前端 surface（Tauri webview / 浏览器 transport `?http=1`）SHALL 在收到 `payload.projectId === ""` **且** `payload.sessionId === ""` 时跳过 per-session 操作（如 `loadSessions("")` / per-session DOM patch），仅触发"项目列表 / dashboard 全量 revalidate"等顶层兜底刷新。Tauri webview 与浏览器 transport 共用同一前端 handler 链（`fileChangeStore.svelte.ts`），守护实现 SHALL 集中在该 handler 链的入口处或各 surface 自身的回调内，避免跨 transport 漂移。

**MUST NOT**：

- MUST NOT 与额外的 `bridge_task` 并存——unified invalidator 是 `file_tx` 唯一生产者，避免双 producer 引发的事件顺序与重复问题
- MUST NOT 让 SSH `polling_watcher` 直接生产到 `LocalDataApi.file_tx` ——必须经过 `FileWatcher::attach_remote` → watcher broadcast → unified invalidator 的统一路径
- MUST NOT 在 emit 路径覆盖 `event.session_list_changed` 字段——OR 公式 SHALL 保留 watcher 已填值，cache hint 仅做 OR 提升

#### Scenario: unified invalidator 是 `file_tx` 唯一生产者

- **WHEN** `LocalDataApi::new_with_watcher` 构造完成，启动 watcher 桥任务
- **THEN** 启动路径 SHALL NOT spawn 任何独立的 `bridge_task` 把 `watcher.subscribe_files()` 直接转发到 `file_tx`
- **AND** `file_tx` 的所有事件 SHALL 来自 unified invalidator 的 `file_tx.send(enriched_event)` 调用

#### Scenario: SSH 路径走 attach_remote 进入 unified invalidator

- **WHEN** `LocalDataApi::attach_remote_watcher` 被调用（SSH 连接上时）
- **THEN** 实现 SHALL 调用 `FileWatcher::attach_remote(sftp, projects_dir, cancel_token)` 让 SSH polling 事件喂入 `watcher.file_tx`，且调用方 SHALL 注入自己持有的 `CancelToken`（用于 dead-signal monitor cancel 路径）
- **AND** SSH event SHALL 经过 unified invalidator 的判定（cache 无 SSH entry 时 invalidate 决策退化为只看 `project_list_changed || deleted`）
- **AND** enriched SSH event SHALL 通过 `file_tx.send` 广播给下游，与 Local event 形态一致；`session_list_changed` 字段已由 SSH polling watcher 在远端事件上填好（详 `file-watching` Requirement `Watch SSH remote project directory via SFTP polling`）
- **AND** 外部 disconnect 触发 `cancel_token.cancel()` 时 SHALL 让 SSH polling task 退出（dead-signal monitor 路径保持原行为）

#### Scenario: emit 顺序在 sync invalidate 之后、async parsed invalidate 之前

- **WHEN** unified invalidator loop 收到一条 raw `FileChangeEvent`
- **THEN** 实现 SHALL 先 sync 调 `apply_file_event_to_project_scan_cache` 拿 `EnrichDecision`
- **AND** 然后 sync 调 `file_tx.send(enriched_event)` emit（锁已释放）
- **AND** 最后 async 调 `apply_file_event_to_parsed_cache(event).await`
- **AND** `file_tx.send` MUST NOT 在 cache lock 临界区内调用

#### Scenario: emit 字段 OR 公式 watcher 主源 + cache hint 兜底

- **WHEN** unified invalidator 收到 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false, session_list_changed: true }`（watcher 已填 first-seen=true），且 `ProjectScanCache::contains_session_id(&local_ctx, "pa", "sa_new")` 返 `false` 让 `decision.emit_session_list_changed_hint = true`
- **THEN** enriched event 的 `session_list_changed` 字段 SHALL 是 `true || true == true`
- **AND** 通过 `file_tx` emit 给下游

#### Scenario: emit 字段 OR 公式 watcher 已填 false 且 cache hit 也 false

- **WHEN** unified invalidator 收到 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false, session_list_changed: false }`（watcher 跟踪集合已含 `(pa, sa)` → first-seen=false），且 `contains_session_id(&local_ctx, "pa", "sa")` 返 `true` 让 `decision.emit_session_list_changed_hint = false`
- **THEN** enriched event 的 `session_list_changed` 字段 SHALL 是 `false || false == false`
- **AND** 该事件 SHALL NOT 触发前端三档守护 revalidate

#### Scenario: emit 字段 OR 公式 watcher 重启窗口期 cache 兜底

- **WHEN** watcher 已重启（`reconfigure_claude_root` 触发）让跟踪集合清空，但 `ProjectScanCache` 仍持有旧 entry（含 project `pa` 与 `sa`）；用户在 `pa` 下追加已知 session `sa.jsonl`
- **AND** watcher 视为 first-seen 填 `session_list_changed=true`（lazy false-positive）
- **THEN** enriched event 的 `session_list_changed` 字段 SHALL 是 `true || (cache contains_session_id 返 true → hint=false) == true`
- **AND** 前端 revalidate 一次（false-positive，cache 视角下其实是已知 session 追加，但 watcher 视角是 first-seen，OR 取并集偏向 emit）

#### Scenario: SSH event 跳过 local cache hint OR

- **WHEN** Local cache 持有 entry（含 project `pa` 与 sessions `{sa1, sa2}`），SSH context 当前 active；远端 SSH polling watcher emit `FileChangeEvent { project_id: "pa-ssh", session_id: "sx", deleted: false, project_list_changed: false, session_list_changed: false }`（SSH 已知 session size/mtime 变化，watcher 字段填 `false`）
- **AND** unified invalidator 调 `FileWatcher::is_local_project("pa-ssh")` 返 `false`（SSH 远端 project_id 不在 local watcher `local_projects_seen` 集合内——SSH 事件由远端 polling 直接构造，不经过 `parse_project_event`，不会被 `mark_local_origin` 写入）
- **THEN** unified invalidator SHALL 跳过 `apply_file_event_to_project_scan_cache` 调用 / 跳过 cache hint 查询，强制 `decision.emit_session_list_changed_hint=false` + `decision.invalidated=false`
- **AND** enriched event 的 `session_list_changed` 字段 SHALL 等于 `event.session_list_changed` 即 `false`
- **AND** 该 SSH append 事件 SHALL NOT 触发前端三档守护 revalidate，保留 PR #291 append 降噪收益

#### Scenario: Local event 仍应用 cache hint OR

- **WHEN** Local cache 持有 entry（含 project `pa` 但不含 `sa_new`），watcher `local_projects_seen` 集合含 `pa`（`parse_project_event` 已在前一次该 project 下事件 emit 前通过 `mark_local_origin` 写入）；用户新建 `<projects_root>/pa/sa_new.jsonl`，watcher emit `FileChangeEvent { project_id: "pa", session_id: "sa_new", session_list_changed: true }`
- **AND** unified invalidator 调 `FileWatcher::is_local_project("pa")` 返 `true`
- **THEN** unified invalidator SHALL 调 `apply_file_event_to_project_scan_cache` 拿 `EnrichDecision { invalidated: true, emit_session_list_changed_hint: true }`
- **AND** enriched event 的 `session_list_changed` 字段 SHALL 是 `true || true == true`

#### Scenario: lag 路径 SHALL emit synthetic structural event

- **WHEN** unified invalidator 的 `rx.recv().await` 返回 `Err(RecvError::Lagged(n))`
- **THEN** 实现 SHALL 调 `apply_lag_to_project_scan_cache`（保守 `invalidate_local()` + counter `lag_conservative`）
- **AND** SHALL 显式 send 一条 synthetic `FileChangeEvent { project_id: "", session_id: "", deleted: false, project_list_changed: true, session_list_changed: true }` 到 `file_tx`

#### Scenario: synthetic event 经 Tauri host bridge 转发到 webview

- **WHEN** synthetic event 进入 `file_tx` broadcast 后被 Tauri host bridge 接收
- **THEN** bridge SHALL `app.emit("file-change", &payload)` 把 synthetic event 转发给 webview，与 real event 行为一致
- **AND** SHALL NOT 对 synthetic event 做特殊识别 / 过滤
- **AND** webview 前端 handler 收到该 payload 后 SHALL 触发兜底全量 revalidate
- **AND** webview 前端 SHALL 按 `payload.projectId === "" && payload.sessionId === ""` 守护跳过 per-session 操作（`loadSessions("")` / per-session DOM patch），不引发副作用

#### Scenario: synthetic event 经 HTTP SSE bridge 推到浏览器客户端

- **WHEN** synthetic event 进入 `file_tx` broadcast 后被 HTTP SSE bridge 接收
- **THEN** bridge SHALL 把 synthetic event 序列化为 `PushEvent::FileChange` 推到 `/api/events` SSE stream，与 real event 形态一致
- **AND** SHALL NOT 对 synthetic event 做特殊识别 / 过滤
- **AND** 浏览器 transport 收到该 SSE 消息后 SHALL 走与 webview 同一 file-change handler 链
- **AND** 浏览器 transport 路径前端 SHALL 按 `payload.projectId === "" && payload.sessionId === ""` 同款守护跳过 per-session 操作，仅触发顶层 revalidate

### Requirement: ProjectScanCache 维护 per-project mtime overlay 让 cache 命中路径返回新鲜 mtime

系统 SHALL 在 project scan cache 主体之外维护每个 context、每个 project 的最大已观测 session mtime hint（"overlay"）。带 mtime 的非删除 file-change event SHALL 单调推进事件所属 context 对应 project 的 hint；不带 mtime 或删除事件 SHALL NOT 推进该 hint。

`list_projects` 与 `list_repository_groups` 返回数据时，系统 SHALL 使用 cache snapshot 中的 `most_recent_session` 与该 hint 的较大值作为对外返回的 `most_recent_session` 字段；`RepositoryGroup` 维度下的聚合 mtime SHALL 从合成后的 worktree 值再次取最大。本路径在 cache 命中与未命中两种情形下对外契约一致——调用方不感知合成发生在何处。

cache 重新扫描完成后，系统 SHALL 按以下规则合并 hint 与 fresh snapshot：

- fresh snapshot 已反映或超过 hint（snapshot value ≥ hint）→ SHALL 丢弃该 hint
- hint 仍大于 fresh snapshot（snapshot value < hint）→ SHALL 保留该 hint，避免 scan 期间发生的 append 被回退

cache 主体 invalidate 与 hint 的解耦规则：

- 三档 invalidate（`projectListChanged || deleted || unknown_session`，详 `ProjectScanCache 按事件语义分级失效` Requirement）触发的清空 SHALL **不**清 hint——hint 是 watcher 单调观测的中间结果，丢失无法重建；snapshot 是 fs 真相的快照，可重新 scan
- 显式 context 切换路径（`ssh_disconnect` / `reconfigure_claude_root` / 公开方法 `invalidate_project_scan_cache()`）SHALL 清空 hint 与 snapshot——上下文已切换，旧 hint 不再适用

context 隔离不变量：Local 与 SSH context 的 hint 互不影响——Local file-change event 仅推进 Local context 下的 hint；SSH polling event 仅推进对应 SSH context 下的 hint。本不变量与 spec `ProjectScanCache 按事件语义分级失效` 中"SSH event 跳过 local cache hint"的守护保持一致：跨 context 守护仅作用于"SSH event 是否参与 Local cache 三档 invalidate / cache hint OR"（不参与），SSH event 自身的 mtime hint 仍 SHALL 写入对应 SSH context overlay 让 SSH 用户的 dashboard 同样受益。

跨 context 同名 project 的边界（accepted limitation）：file-change event 不携带 source ContextId，invalidator 通过既有 `is_local_project` 字符串级守护推断 event 来源——SSH 远端与 local 同名 project 共存时旧 SSH event 进入队列后可能误推进 local hint。本 Requirement 接受该 edge case；根治留 followup（需 watcher 注入 ContextId 字段做精确 dispatch）。

#### Scenario: 已知 session 普通 append 推进 hint 但不 invalidate

- **WHEN** 本地 watcher 发出 file-change event：project `pa`、session `sa`、`deleted=false`、`projectListChanged=false`、`sessionListChanged=false`、`mtimeMs=Some(t1)`，且 cache 已含 Local context 的 entry，且 Local context 下 `pa` 的 hint 当前为 `t0 < t1`
- **THEN** 系统 SHALL 把 Local context 下 `pa` 的 hint 单调推进到 `t1`
- **AND** SHALL NOT 触发 cache 主体 invalidate
- **AND** 紧接着的 `list_repository_groups` cache hit 返回的 `RepositoryGroup.most_recent_session` SHALL ≥ `t1`
- **AND** SHALL NOT 增加 `project_scan_cache.invalidate.structural` counter

#### Scenario: 删除事件不推进 hint

- **WHEN** 本地 watcher 发出 file-change event：project `pa`、session `sa`、`deleted=true`、`mtimeMs` 缺省
- **THEN** 系统 SHALL NOT 改写任何 context 下 `pa` 的 hint
- **AND** 仍按既有规则触发 cache 主体 invalidate（`deleted` 命中三档第一档）

#### Scenario: cache hit 路径合成 hint 让用户看到最新 mtime

- **WHEN** cache snapshot 中 project `pa` 的 `most_recent_session=Some(t0)`，watcher 多次 append 事件让 Local context 下 `pa` 的 hint 推进到 `t2 > t0`，期间无结构性事件命中
- **AND** 调用方调 `list_repository_groups`
- **THEN** 返回的 `RepositoryGroup.most_recent_session`（聚合自该 project 对应 worktrees）SHALL 等于 `t2`
- **AND** 合成 SHALL 仅修改返回数据的 `most_recent_session` 字段；底层 cache snapshot 主体 SHALL NOT 被改写

#### Scenario: cache 重扫合并保留较大 hint

- **WHEN** Local context 下 `pa` 的 hint 当前值为 `t2`，scan 完成后新 snapshot 中 `pa.most_recent_session=Some(t1)` 且 `t1 < t2`
- **THEN** 重扫结果被接受并作为 fresh snapshot 生效时 SHALL 保留 Local context 下 `pa` 的 hint 为 `t2`
- **AND** 后续 `list_repository_groups` cache hit SHALL 仍返回 `t2`（合成路径继续生效）

#### Scenario: cache 重扫清除已被覆盖的旧 hint

- **WHEN** Local context 下 `pa` 的 hint 当前值为 `t1`，scan 完成后新 snapshot 中 `pa.most_recent_session=Some(t2)` 且 `t2 ≥ t1`
- **THEN** 重扫结果被接受并作为 fresh snapshot 生效时 SHALL 移除 Local context 下 `pa` 的 hint 条目（snapshot 已反映或超过该 mtime）
- **AND** 后续 `list_repository_groups` cache hit SHALL 直接返回 snapshot 内 `t2`，不依赖 hint

#### Scenario: 三档 invalidate 不清 hint

- **WHEN** Local context 下 `pa` 的 hint 含值 `t2`，watcher 收到一条结构性事件触发三档 invalidate
- **THEN** cache snapshot 中 Local entry SHALL 被清空
- **AND** Local context 下 `pa` 的 hint SHALL 保留为 `t2`
- **AND** 下一次 `list_repository_groups` cache miss 重扫后 SHALL 按合并规则处理 hint

#### Scenario: 显式 invalidate 总清同时清 hint

- **WHEN** 调用方调 `invalidate_project_scan_cache()` 公开方法（典型：IPC contract 测试 / SSH context 显式切换前 hook）
- **THEN** cache snapshot SHALL 全部清空（覆盖所有 backend kind）
- **AND** mtime hint SHALL 全部清空（覆盖所有 context）

#### Scenario: SSH event 推进对应 SSH context hint 但不影响 Local invalidate

- **WHEN** SSH polling watcher 发出 file-change event：project `pa`、`mtimeMs=Some(t1)`、`deleted=false`，且当前 active SSH context 已注册
- **THEN** 系统 SHALL 把 SSH context 下 `pa` 的 hint 单调推进到至少 `t1`
- **AND** SHALL NOT 推进 Local context 下 `pa` 的 hint
- **AND** SHALL NOT 因该 SSH event 触发 Local `ProjectScanCache` 三档 invalidate 或 Local cache hint OR

#### Scenario: 缺 mtimeMs 字段的 file-change event 不推进 hint

- **WHEN** 本地 watcher 因运行环境无法取到 mtime 发出 file-change event：`mtimeMs` 缺省
- **THEN** 系统 SHALL NOT 改写任何 context 下任何 project 的 hint
- **AND** 仍按 `mtimeMs` 之外字段（`projectListChanged` / `deleted` / `unknown_session` 判定）走既有三档失效逻辑

#### Scenario: cache 空时收到 mtime hint 仍写 hint

- **WHEN** cache snapshot 为空（冷启 / `reconfigure_claude_root` 后），Local context 下 `pa` 的 hint 也为空
- **AND** 本地 watcher 发出 `mtimeMs=Some(t1)` 的 event
- **THEN** 系统 SHALL 把 Local context 下 `pa` 的 hint 设为 `t1`（即便此时无 entry，hint 提前到位以便后续 scan 完成 populate 时合并阶段保留）
- **AND** 后续 `list_repository_groups` cache miss → 全扫 → 合并阶段按规则处理 hint

#### Scenario: cache 重扫不再含某 project 时清掉对应 hint

- **WHEN** Local context 下含 hint 条目 `pa→t2` 与 `pb→t3`，重扫后 fresh snapshot 不再含 project `pa`（用户已删除该 encoded 目录）
- **THEN** 重扫合并阶段 SHALL 移除 Local context 下 `pa` 的 hint 条目
- **AND** SHALL 保持 `pb` 的 hint 按合并规则处理（`pb` 仍存在）
- **AND** 同 context 下 hint 条目数 SHALL bounded by fresh snapshot 中 live project 数（避免已删除 project 的 hint 永久驻留）

