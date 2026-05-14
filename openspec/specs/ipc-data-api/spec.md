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

**`isOngoing` 真实值 SHALL 由两路 AND 计算**：(a) `cdt_analyze::check_messages_ongoing(messages)` 返回 `true`（结构性活动栈五信号判定），**且** (b) session JSONL 文件 mtime 距当前时刻 `< 5 分钟`。任一条件不满足时 `isOngoing` MUST 为 `false`。stale 阈值常量 `STALE_SESSION_THRESHOLD = 5 min` 对齐原版 `claude-devtools/src/main/services/discovery/ProjectScanner.ts` 的 `STALE_SESSION_THRESHOLD_MS = 5 * 60 * 1000`（issue #94：用户 Ctrl+C / kill cli / 关机导致 cli 异常退出时，session 末尾停在 `tool_result` 之类 AI 活动而无 ending 信号，活动栈会误判 ongoing；mtime 兜底将其纠正）。`list_sessions` 异步扫描路径与 `get_session_detail` 同步路径行为 MUST 一致；HTTP `list_sessions_sync` 共用同一 `extract_session_metadata` 实现，自动适用。stat 失败时 SHALL 保守保留 messages_ongoing 判定（避免 fs 偶发错误把活跃 session 错判 dead）；时钟回拨导致 mtime > now 时 SHALL 判 not stale（避免未来 mtime 把活跃 session 误判 dead）。

序列化 SHALL 使用 camelCase（`isOngoing`、`messagesOmitted`、`headerModel`、`lastIsolatedTokens`、`isShutdownOnly`、`dataOmitted`、`contentOmitted`、`outputOmitted`、`outputBytes`）。例外：`ImageSource.media_type` 与 `ImageSource.type`（kind）保持 snake_case，与上游 Anthropic JSONL 格式一致——同 `TokenUsage` 例外。

HTTP API 路径（`GET /projects/:id/sessions`）SHALL 保留同步完整返回语义（不适用骨架化）——因 HTTP 无 push 通道；IPC 路径适用骨架化。HTTP 路径同样 SHALL NOT 应用 `OMIT_IMAGE_DATA` / `OMIT_RESPONSE_CONTENT` / `OMIT_TOOL_OUTPUT` 裁剪（HTTP 当前无活跃用户、且无对应 asset 协议端点 / 懒拉接口，保留完整 payload 传输）。

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

### Requirement: Expose SSH and context operations

系统 SHALL 暴露列出上下文、切换激活上下文、SSH 连接 / 断开 / 测试、查询 SSH 状态、解析 SSH host alias 这些操作。

#### Scenario: Resolve ssh host alias via IPC
- **WHEN** 调用方请求解析一个 alias
- **THEN** 响应 SHALL 含解析后的 hostname、port、user、identity file 路径（或在 not-found 时返回明确错误）

### Requirement: Emit push events for file changes and notifications

系统 SHALL 从 main 进程向 renderer 推送以下事件：session 文件变更、todo 文件变更、新通知、SSH 状态变化、updater 进度。

桌面（Tauri）host SHALL 在 `setup` 阶段订阅 `FileWatcher::subscribe_files()` 广播，并向前端 webview `emit("file-change", payload)`。Payload SHALL 是 `FileChangeEvent` 的 camelCase 序列化结果（字段 `projectId`、`sessionId`、`deleted`），与其它 IPC payload 命名约定一致。

#### Scenario: New notification while renderer is subscribed
- **WHEN** renderer 已订阅通知事件，期间产出一条新通知
- **THEN** renderer SHALL 在 debounce 窗口内收到一条 push 事件，携带通知 payload

#### Scenario: Tauri 转发 file-change 事件
- **WHEN** `cdt-watch::FileWatcher` 在 100 ms debounce 后产出 `FileChangeEvent { project_id: "p", session_id: "s", deleted: false }`
- **AND** Tauri host 在 `setup` 已 spawn 桥任务订阅 `subscribe_files()`
- **THEN** webview SHALL 通过 `listen("file-change", ...)` 收到 payload `{ projectId: "p", sessionId: "s", deleted: false }`

#### Scenario: file-change payload 是 camelCase
- **WHEN** Tauri 桥任务 emit 一条 `file-change` 事件
- **THEN** 序列化后的 JSON SHALL 使用 camelCase 字段名（`projectId` / `sessionId` / `deleted`），与既有 IPC 类型约定一致

#### Scenario: file-change 桥与通知管线并存
- **WHEN** Tauri host 同时持有 `subscribe_files()`（emit `file-change`）与 `subscribe_detected_errors()`（emit `notification-added`）两个订阅
- **THEN** 两个桥 SHALL 独立运行，文件变更不会因通知 pipeline 的 lag 被丢弃，反之亦然

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

### Requirement: Stream detected errors to subscribers

系统 SHALL 在 `LocalDataApi` 上暴露一个 in-process 订阅机制，让宿主 runtime（例如 Tauri 应用）能够接收自动通知 pipeline 产出的新检测错误，无需轮询持久化通知存储。

#### Scenario: Tauri runtime subscribes and forwards to renderer
- **WHEN** Tauri runtime 在应用 setup 时调用 `subscribe_detected_errors()`
- **AND** 通知 pipeline 产出一条新的 `DetectedError`
- **THEN** 订阅者持有的 `broadcast::Receiver` SHALL yield 该 `DetectedError`，宿主可据此向前端 emit 一个事件（例如 `notification-added`）

#### Scenario: Subscription without a watcher attached
- **WHEN** `LocalDataApi` 通过不带 watcher 的构造器实例化（集成测试或仅 HTTP 宿主路径）
- **AND** 调用方调用 `subscribe_detected_errors()`
- **THEN** 调用 SHALL 返回一个永不 yield 的有效 `broadcast::Receiver`（静默 no-op），而非错误

#### Scenario: Multiple subscribers receive the same error
- **WHEN** 两个独立订阅者各自调用 `subscribe_detected_errors()`
- **AND** pipeline 产出一条 `DetectedError`
- **THEN** 两个订阅者 SHALL 各自**恰好**收到一次同一条 `DetectedError`

### Requirement: Emit session metadata updates

系统 SHALL 在 `LocalDataApi` 上暴露 in-process 订阅机制 `subscribe_session_metadata()`，返回一个 `broadcast::Receiver<SessionMetadataUpdate>`。`SessionMetadataUpdate` SHALL 携带 `projectId` / `sessionId` / `title` / `messageCount` / `isOngoing`（序列化时为 camelCase）。Tauri host SHALL 把该订阅桥接到 webview，向前端 emit `session-metadata-update` 事件。

并发度 SHALL 被限流（固定上限 8），避免 50+ 文件同时打开；每次 `list_sessions(projectId)` 触发新扫描前 SHALL 取消上一轮未完成的扫描（同一 `projectId` 维度），避免事件串扰。

#### Scenario: 订阅接收元数据更新

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 随后调用 `list_sessions("projectA")`，项目下有 3 个 session
- **THEN** receiver SHALL 在扫描完成后**最多**收到 3 条 `SessionMetadataUpdate`，每条携带对应 sessionId 的真实 `title` / `messageCount` / `isOngoing`

#### Scenario: Tauri host emit session-metadata-update

- **WHEN** Tauri host 在 `setup` 调用 `subscribe_session_metadata()` 并在后台 task 内订阅
- **AND** 后端产出 `SessionMetadataUpdate { project_id: "p", session_id: "s", title: Some("T"), message_count: 12, is_ongoing: false }`
- **THEN** webview SHALL 通过 `listen("session-metadata-update", ...)` 收到 payload `{ projectId: "p", sessionId: "s", title: "T", messageCount: 12, isOngoing: false }`（camelCase）

#### Scenario: 同 projectId 新扫描取消旧扫描

- **WHEN** `list_sessions("projectA")` 正在扫描中（后台有未完成任务）
- **AND** 调用方再次调用 `list_sessions("projectA")`（file-change silent refresh 场景）
- **THEN** 旧扫描任务 SHALL 被 abort，未完成的 session 元数据 SHALL NOT 再被推送；新扫描 SHALL 从头开始

#### Scenario: 并发度限制

- **WHEN** 扫描任务在并发处理某页 50 个 session 文件
- **THEN** 同一时刻打开的 JSONL 文件句柄数 SHALL 不超过 8（通过 `tokio::sync::Semaphore` 或等价机制限流）

#### Scenario: 无 watcher 构造器下 subscribe 安全

- **WHEN** `LocalDataApi` 通过不带 watcher 的构造器实例化（集成测试路径）
- **AND** 调用方 `subscribe_session_metadata()`
- **THEN** 返回有效 `broadcast::Receiver`；`list_sessions` 仍能正常推送（broadcast 不依赖 watcher）

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

### Requirement: Expose teammate messages on AIChunk

`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk` MUST 暴露新字段 `teammateMessages: TeammateMessage[]`（camelCase 序列化）。无 teammate 嵌入的 AIChunk MUST 通过 `#[serde(skip_serializing_if = "Vec::is_empty")]` 在 IPC payload 中省略该字段，保持老前端 / 老 fixture 兼容。

`TeammateMessage` IPC schema MUST 含以下字段（camelCase 序列化、字段语义详见 `team-coordination-metadata::Render teammate messages as dedicated items`）：

| 字段                 | 类型             | 说明                                                  |
| -------------------- | ---------------- | ----------------------------------------------------- |
| `uuid`               | `string`         | 来自原始 user 消息 uuid                              |
| `teammateId`         | `string`         | 队友标识                                              |
| `color`              | `string \| null` | 队友色（teamColors 调色板键）                        |
| `summary`            | `string \| null` | 队友自填主题                                          |
| `body`               | `string`         | 队友消息正文（已 trim 标签）                         |
| `timestamp`          | `string`         | ISO8601                                               |
| `replyToToolUseId`   | `string \| null` | 配对的 SendMessage tool_use_id；orphan 时为 null      |
| `tokenCount`         | `number \| null` | body 灌入主 session 的 token 估算                    |
| `isNoise`            | `boolean`        | 运维噪声（idle / shutdown / terminated）             |
| `isResend`           | `boolean`        | 是否检测到重复发送关键词                              |

序列化约定（与本 spec `Expose project and session queries` 既有 camelCase 约定一致）：`teammateMessages` / `teammateId` / `replyToToolUseId` / `tokenCount` / `isNoise` / `isResend` 全部 camelCase。

HTTP API 路径（`GET /projects/:id/sessions/:sid`）SHALL 同步暴露 `teammateMessages` 字段——与 IPC 路径共享 `LocalDataApi::get_session_detail` 实现，自动适用。

回滚开关：`cdt-analyze::chunk::builder` 顶部 `const EMBED_TEAMMATES: bool = true;`；为 `false` 时所有 `AIChunk.teammateMessages` SHALL 为 `[]`（IPC 序列化省略字段），等价于本 change 落地前的 payload 形态。

#### Scenario: AIChunk with teammate replies serializes teammateMessages
- **WHEN** `get_session_detail` 返回的某 AIChunk 含 2 条 teammate 嵌入
- **THEN** 该 chunk 的 JSON SHALL 含 `"teammateMessages": [{...}, {...}]`，每条 object SHALL 含全部 10 个 camelCase 字段

#### Scenario: AIChunk without teammate omits the field
- **WHEN** `get_session_detail` 返回的某 AIChunk 无 teammate 嵌入
- **THEN** 该 chunk 的 JSON SHALL NOT 含 `teammateMessages` 键（由 `skip_serializing_if = "Vec::is_empty"` 控制），与本 change 落地前 payload 形态一致

#### Scenario: Orphan teammate has null replyToToolUseId
- **WHEN** 某 teammate 嵌入未配对到任何 SendMessage（orphan）
- **THEN** 其 IPC JSON 字段 `replyToToolUseId` SHALL 为 `null`

#### Scenario: EMBED_TEAMMATES=false reverts payload shape
- **WHEN** 编译期常量 `EMBED_TEAMMATES = false`
- **THEN** 所有 AIChunk 的 IPC JSON SHALL NOT 含 `teammateMessages` 键，与本 change 落地前的 payload 形态等价

### Requirement: Expose teammate spawn metadata on ToolExecution

`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.toolExecutions[i]` MUST 暴露新字段 `teammateSpawn?: TeammateSpawnInfo | null`（camelCase 序列化）。无 spawn 信息的 ToolExecution MUST 通过 `#[serde(skip_serializing_if = "Option::is_none")]` 在 IPC payload 中省略该字段，保持老前端 / 老 fixture 兼容。

`TeammateSpawnInfo` IPC schema MUST 含以下字段：

| 字段     | 类型                | 说明                                  |
| -------- | ------------------- | ------------------------------------- |
| `name`   | `string`            | 队友成员名（如 `"member-1"`）        |
| `color`  | `string \| null`    | 队友色（teamColors 调色板键）        |

字段语义详见 `tool-execution-linking::Detect teammate-spawned tool results`。

#### Scenario: Tool execution with teammate spawn populates teammateSpawn
- **WHEN** `get_session_detail` 返回的某 AIChunk 含一条 ToolExecution，对应 user msg `tool_use_result.status == "teammate_spawned"`、`name="member-1"`、`color="blue"`
- **THEN** 该 toolExecution JSON SHALL 含 `"teammateSpawn":{"name":"member-1","color":"blue"}`

#### Scenario: Tool execution without spawn omits teammateSpawn
- **WHEN** `get_session_detail` 返回的某 ToolExecution 无 spawn 信息
- **THEN** 该 toolExecution JSON SHALL NOT 含 `teammateSpawn` 键

### Requirement: Strip teammate-message tags from session title

`extract_session_metadata` 提取的 `SessionSummary.title` MUST 在做长度截断之前剥除任何 `<teammate-message ...>...</teammate-message>` 包裹片段，避免 sidebar 标题吐出原始 XML。

实现 SHALL 在 `cdt-api::session_metadata::sanitize_for_title` 同函数内完成两步：

1. **Fast-path（teammate 主导消息）**：若 trim 后 text 以 `<teammate-message` 开头，先 regex 抽 `summary="..."` 属性内容；非空时 SHALL 直接返回 summary 内容作为标题候选（截断同既有 200 字符上限）。
2. **Fallback（剥标签）**：若 fast-path 未命中（无 summary 属性 / 文本含混合内容），SHALL 在既有标签剥除循环中追加 `teammate-message` 标签——把整段 `<teammate-message ...>body</teammate-message>` 从文本中删除（含 attributes 与 inner body）。剥除后若文本为空，SHALL 回退到 `command_fallback` 或 `None`，按既有路径处理。

`sanitize_for_title` MUST 不再在标题里输出任何 `<teammate-message` / `</teammate-message>` 字面量。

#### Scenario: Title takes summary attribute when message is wrapped solely by teammate-message
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice" summary="Set up project">body</teammate-message>`
- **THEN** `extract_session_metadata.title` SHALL 为 `Some("Set up project")`

#### Scenario: Title falls back when teammate-message has no summary
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice">body</teammate-message>`（无 summary 属性）
- **THEN** `extract_session_metadata.title` SHALL NOT 含 `<teammate-message`，且 SHALL 退回 `None` 或 `command_fallback`

#### Scenario: Mixed content strips teammate-message tag
- **WHEN** 第一条 user 消息 content 为 `Hello team. <teammate-message teammate_id="alice">body</teammate-message> please continue.`
- **THEN** title SHALL 不含 `<teammate-message`，剥除后 SHALL 仅保留 `Hello team.  please continue.`（trim 后），整体走既有截断路径

### Requirement: Resolve project id from session id alone

`DataApi` trait SHALL 暴露 `find_session_project(session_id: &str) -> Result<Option<String>, ApiError>`，让仅持有 `session_id` 的调用方反查所属 `project_id`。HTTP `GET /api/sessions/:id` 与 trait 内 `get_sessions_by_ids` MUST 走该方法配合 `get_session_detail(project_id, session_id)` 的复合路径，**不**得直接调 `get_session_detail("", session_id)`。

trait 默认实现 SHALL 遍历 `list_projects()` 取每个 `project_id`，依次调 `list_sessions_sync(project_id, { page_size: usize::MAX, cursor: None })`，命中第一个含 `session_id` 的项目立即返回 `Ok(Some(project_id))`；遍历完无命中返 `Ok(None)`。**主会话**（`<projects_dir>/<encoded>/<session_id>.jsonl`）必然能被默认实现命中；subagent jsonl 是否被命中 SHALL 视具体实现的覆盖能力而定（默认实现不强制覆盖）。

`LocalDataApi` SHALL 覆盖默认实现，直接 `read_dir(scanner.projects_dir())` 扫每个 project 子目录，按以下顺序匹配（命中即返回 `Ok(Some(<encoded_project_id>))`）：

1. **主会话快路径**：`<project_dir>/<session_id>.jsonl` 存在。
2. **legacy subagent**：`<project_dir>/agent-<session_id>.jsonl` 存在。
3. **新结构 subagent**：`<project_dir>/<parent>/subagents/agent-<session_id>.jsonl` 存在（任一 parent）。

实现 SHALL 复用既有 `find_subagent_jsonl` helper，与 `LocalDataApi::get_session_detail` 的查找口径完全一致——避免出现"`find_session_project` 命中但 `get_session_detail` 又取不到"的不一致状态。

#### Scenario: 默认实现命中主会话
- **WHEN** 调用方对一个 mock `DataApi` 调 `find_session_project("sid-A")`，`sid-A` 是项目 `proj-1` 下的主会话
- **AND** mock 实现走 trait 默认 `list_projects` + `list_sessions_sync` 路径
- **THEN** 返回 SHALL 为 `Ok(Some("proj-1"))`

#### Scenario: 默认实现找不到时返 None
- **WHEN** 调用方对 mock `DataApi` 调 `find_session_project("sid-ghost")`，所有 project 的 `list_sessions_sync` 都不含该 id
- **THEN** 返回 SHALL 为 `Ok(None)`

#### Scenario: LocalDataApi 直扫 FS 命中主会话
- **WHEN** tmpdir 下构造 `LocalDataApi`，写入 `<projects_dir>/<encoded-A>/sid-1.jsonl`
- **AND** 调用方调 `find_session_project("sid-1")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-A>"))`

#### Scenario: LocalDataApi 命中 subagent jsonl
- **WHEN** tmpdir 下构造 `LocalDataApi`，写入 `<projects_dir>/<encoded-B>/parent/subagents/agent-sid-2.jsonl`
- **AND** 调用方调 `find_session_project("sid-2")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-B>"))`

#### Scenario: LocalDataApi 多 project 命中第一个
- **WHEN** tmpdir 下两个 project 目录都不含目标 sid，第三个含 `sid-3.jsonl`
- **AND** 调用方调 `find_session_project("sid-3")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-的第三个>"))`，不报错且只命中一次

#### Scenario: LocalDataApi 找不到时返 None 不报错
- **WHEN** tmpdir 下所有 project 目录都不含目标 sid
- **AND** 调用方调 `find_session_project("sid-ghost")`
- **THEN** 返回 SHALL 为 `Ok(None)`（**不**得返回 `Err`、**不**得 panic）

#### Scenario: 与 get_session_detail 口径一致
- **WHEN** `find_session_project(sid)` 返回 `Ok(Some(pid))`
- **THEN** 紧接着调 `get_session_detail(pid, sid)` SHALL 成功返回 `SessionDetail`（不**得**返回 `not_found`）；反之，`Ok(None)` 时 `get_session_detail` 任意 `project_id` 调用 SHALL 都返回 `not_found`

### Requirement: Expose git branch on session summary and metadata updates

`SessionSummary` 与 `SessionMetadataUpdate` SHALL 在已有字段集（`sessionId` / `projectId` / `timestamp` / `title` / `messageCount` / `isOngoing`）之外**额外**携带 `git_branch: Option<String>` 字段（IPC 序列化时为 camelCase `gitBranch`）。骨架返回（`list_sessions` 同步阶段）SHALL 为 `None`，真实值由后端异步元数据扫描在 `LocalDataApi::list_sessions` 后台 JoinSet 任务内填充并通过 `session-metadata-update` 事件 push 到前端。

后端取值规则：解析 session JSONL 时 SHALL 遍历 `cdt_parse::ParsedMessage.message.git_branch`，记录**最后一条** `Some(...)` 作为最终值（与原版 `claude-devtools/src/renderer/utils/sessionExporter.ts` 取值方式一致——反映会话最后所在的 git 分支）。session 中所有行的 `git_branch` 都为 `None`（非 git 仓库）时 SHALL 保持 `None`。

`cdt-api/tests/ipc_contract.rs` SHALL 加断言验证 `SessionSummary` 与 `SessionMetadataUpdate` 序列化结果含 `gitBranch` camelCase 字段，与 `messageCount` 等同位。

#### Scenario: list_sessions skeleton has gitBranch null

- **WHEN** caller 调用 `list_sessions("p")`
- **THEN** 同步返回的每个 `SessionSummary` SHALL 含字段 `gitBranch`（值为 `null`，因尚未异步扫描）

#### Scenario: session-metadata-update payload contains gitBranch

- **WHEN** 后端后台扫描某个 session 完毕，最后一行 `git_branch` 为 `Some("feat/foo")`
- **AND** 该 session 通过 `session-metadata-update` 推送
- **THEN** event payload SHALL 含 `gitBranch: "feat/foo"`（camelCase）

#### Scenario: session without any git_branch line

- **WHEN** 后端扫描 session 所有行 `git_branch` 均为 `None`（非 git 项目）
- **AND** 该 session 通过 `session-metadata-update` 推送
- **THEN** event payload `gitBranch` SHALL 为 `null`

#### Scenario: backend takes last non-empty git_branch

- **WHEN** session 内消息行 `git_branch` 序列依次为 `Some("main")` / `None` / `Some("feat/x")` / `Some("feat/y")` / `None`
- **THEN** 该 session 元数据推送的 `gitBranch` SHALL 为 `"feat/y"`（最后一条非空）

#### Scenario: contract test asserts camelCase serialization

- **WHEN** `cargo test -p cdt-api --test ipc_contract` 执行
- **THEN** 断言 `SessionSummary { git_branch: Some("main"), ... }` 序列化为 JSON 后 SHALL 含字段名 `"gitBranch"`，且 `SessionMetadataUpdate` 同样

### Requirement: Expose CompactChunk derived metadata in SessionDetail

`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `CompactChunk` SHALL 携带由 chunks 自身派生填充的两个可选字段（数据形态契约见 capability `chunk-building` 的 Requirement `CompactChunk carries optional derived metadata`）：

- `tokenDelta: Option<CompactionTokenDelta>`
- `phaseNumber: Option<u32>`

派生算法 SHALL 在 IPC 组装层（`cdt-api` 内 `SessionDetail` 构造路径）实现，**不**修改 `cdt-analyze::chunk::builder` 算法层、**不**依赖 `ContextPhaseInfo`。派生函数 signature SHALL 是 `apply_compact_derived(chunks: &mut [Chunk], enabled: bool)`，输入仅 chunks 序列与回滚开关。

具体规则：

- **`phaseNumber`**：派生函数内维护 `compact_counter: u32 = 1`，按 chunks 顺序遍历，每遇 `Chunk::Compact(c)` 就 `compact_counter += 1`，立即赋 `c.phase_number = Some(compact_counter)`。对齐原版 `groupTransformer.ts:295-303` 与 `cdt-analyze::context::session.rs:101` 的"compact 触发新 phase"语义
- **`tokenDelta`**：对每个 `Chunk::Compact(c)` at index `i`，独立查 chunks 自身：
  - `last_ai_before` = `chunks[..i]` 中最后一个 `Chunk::Ai`
  - `first_ai_after` = `chunks[i+1..]` 中第一个 `Chunk::Ai`
  - `pre_tokens` = `last_ai_before` 的 last response 的 `usage` 各字段总和（`input_tokens + output_tokens + cache_read_input_tokens + cache_creation_input_tokens`）；`responses` 全 `usage = None` 时 `pre_tokens = None`
  - `post_tokens` = `first_ai_after` 的 first response 的 `usage` 总和；同上 fallback
  - 若 `pre_tokens` 与 `post_tokens` 都有值 → `c.token_delta = Some(CompactionTokenDelta { pre_compaction_tokens: pre, post_compaction_tokens: post, delta: post as i64 - pre as i64 })`；任一缺值 → `c.token_delta = None`
  - 该算法对齐原版 `groupTransformer.ts:305-315` 的 `findLastAiBefore` + `findFirstAiAfter`，对**连续 compact** 给每个 compact 独立计算（虽然连续 compact 中所有 compact 的 `last_ai_before` / `first_ai_after` 命中同一对 AI，结果相同——这是与原版一致的行为）

序列化 SHALL 使用 camelCase（`tokenDelta` / `phaseNumber`）。`None` 时按 `#[serde(default, skip_serializing_if = "Option::is_none")]` 省略字段。

派生函数 SHALL 接收 `enabled: bool` 参数：调用方在生产代码传顶部 `const COMPACT_DERIVED_ENABLED: bool = true`（统一回滚点），测试代码可直接传 `false` 验回滚路径。`enabled = false` 时派生函数 SHALL 直接返回，不写入任何 `tokenDelta` / `phaseNumber`。

派生 SHALL 在 `get_session_detail` 共享路径（IPC 与 HTTP detail 共用同一组装入口）内调用一次。`list_sessions` / `list_sessions_sync` 等返回 `SessionSummary`（无 chunks）的入口 SHALL 不调用派生。

#### Scenario: Token delta computed from neighboring AI chunks

- **WHEN** session chunks 序列为 `[AIChunk(last response usage total = 30000), CompactChunk(uuid="c-1"), AIChunk(first response usage total = 5000)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `Some(CompactionTokenDelta { preCompactionTokens: 30000, postCompactionTokens: 5000, delta: -25000 })`
- **AND** 序列化 JSON SHALL 包含 `"tokenDelta":{"preCompactionTokens":30000,"postCompactionTokens":5000,"delta":-25000}`

#### Scenario: Token delta None when no AI before compact

- **WHEN** session chunks 序列为 `[UserChunk, CompactChunk(uuid="c-1"), AIChunk(...)]`（compact 之前无 AIChunk）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`
- **AND** 序列化 JSON SHALL **不包含** `tokenDelta` key

#### Scenario: Token delta None when no AI after compact

- **WHEN** session chunks 序列为 `[AIChunk(...), CompactChunk(uuid="c-1")]`（compact 在 chunks 末尾，之后无 AIChunk）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`

#### Scenario: Token delta None when neighboring AI lacks usage data

- **WHEN** session chunks 序列为 `[AIChunk(responses 全部 usage=None), CompactChunk(uuid="c-1"), AIChunk(...)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`（pre_tokens 无法计算）

#### Scenario: Consecutive compacts share identical token delta

- **WHEN** session chunks 序列为 `[AIChunk(last response usage total = 30000), CompactChunk(uuid="c-1"), CompactChunk(uuid="c-2"), AIChunk(first response usage total = 5000)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).tokenDelta` SHALL 等于 `CompactChunk(c-2).tokenDelta`（都是 `Some(CompactionTokenDelta { 30000, 5000, -25000 })`，因为两个 compact 的 `last_ai_before` 与 `first_ai_after` 命中同一对 AI；对齐原版 `groupTransformer.ts:305-315` 的 `findLastAiBefore`/`findFirstAiAfter` 独立查询语义，**不会**因 cdt-analyze 内部 `current_phase_compact_group_id` 覆盖问题让 c-1 拿到 None）

#### Scenario: Phase number assigned by compact ordinal

- **WHEN** session chunks 序列含 `[UserChunk, AIChunk(...), CompactChunk(uuid="c-1"), AIChunk(...)]`（chunks 中的第 1 个 compact）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)`（compact_counter 从 1 起，遇到 c-1 自增到 2）

#### Scenario: Consecutive compacts each get its own phase number

- **WHEN** session chunks 序列含 `[..., CompactChunk(uuid="c-1"), CompactChunk(uuid="c-2"), AIChunk(...)]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)` AND `CompactChunk(c-2).phaseNumber` SHALL 为 `Some(3)`

#### Scenario: Phase number stable when compact at end of chunks

- **WHEN** session chunks 序列为 `[AIChunk(...), CompactChunk(uuid="c-1"), AIChunk(...), CompactChunk(uuid="c-2")]`
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)` AND `CompactChunk(c-2).phaseNumber` SHALL 为 `Some(3)`（派生不依赖 compact 之后是否有 AIChunk）

#### Scenario: Compact followed only by user and system chunks

- **WHEN** session chunks 序列为 `[AIChunk(...), CompactChunk(uuid="c-1"), UserChunk, SystemChunk]`（compact 之后仅 User/System，无 AIChunk）
- **WHEN** `get_session_detail` 返回该 session
- **THEN** `CompactChunk(c-1).phaseNumber` SHALL 为 `Some(2)`（phaseNumber 派生与"compact 之后必须 AIChunk"无关）
- **AND** `CompactChunk(c-1).tokenDelta` SHALL 为 `None`（tokenDelta 需要 first_ai_after，不存在时 None）

#### Scenario: Rollback flag disables derivation

- **WHEN** 调用派生函数 `apply_compact_derived(chunks, enabled = false)`
- **AND** `chunks` 中含若干 `CompactChunk` 与相邻 `AIChunk` 含完整 usage
- **THEN** 处理后所有 `CompactChunk.tokenDelta` SHALL 为 `None` AND `phaseNumber` SHALL 为 `None`
- **AND** 该 Scenario SHALL 可在单元测试中独立断言（派生函数接收 `enabled: bool` 参数而非依赖运行时不可改的 `const`）

### Requirement: Expose subagent messages total count

`Process` / `SubagentProcess` 序列化 IPC payload MUST 含 `messagesTotalCount: u32` 字段（Rust 端字段名 `messages_total_count`，`#[serde(rename = "messagesTotalCount")]`），记录 subagent JSONL 内**裁剪前**的完整 `Vec<Chunk>` 长度（`cand.messages.len()`）。该字段 SHALL 在 `OMIT_SUBAGENT_MESSAGES=true`（默认裁剪路径）与 `OMIT_SUBAGENT_MESSAGES=false`（回滚路径）下行为一致——始终等于 subagent session build_chunks 后的 chunk 数。

该字段是前端 SubagentCard 在 `messagesOmitted=true` 下的唯一"messages 数量是否变化"的版本指纹来源；前端 SHALL 用 `(isOngoing, endTs, messagesTotalCount)` 三元组判定 trace 版本，版本递增即代表 subagent 内部有新 chunk 写入。

`messages_total_count` MUST 在 `candidate_to_process` 阶段（`cdt-analyze::tool_linking::resolver`）由 `cand.messages.len() as u32` 填充——与 `header_model` / `last_isolated_tokens` / `is_shutdown_only` 同阶段。IPC 层在 `apply_subagent_messages_omit` 之前 SHALL 保证该字段已填，避免裁剪 messages 后再读 length 永远是 0。

#### Scenario: messagesTotalCount in OMIT default path

- **WHEN** `OMIT_SUBAGENT_MESSAGES=true`，`Process` 由 subagent session 含 7 个 chunk 的 candidate 构造
- **THEN** IPC 序列化 JSON SHALL 含 `"messagesTotalCount": 7`、`"messagesOmitted": true`、`"messages": []`

#### Scenario: messagesTotalCount in rollback path

- **WHEN** `OMIT_SUBAGENT_MESSAGES=false`，同一 candidate 构造 `Process`
- **THEN** IPC 序列化 JSON SHALL 含 `"messagesTotalCount": 7`、`"messagesOmitted": false`、`"messages": <length=7>`

#### Scenario: messagesTotalCount 反映 ongoing subagent 内部增长

- **WHEN** 同一 subagent session 经两次 `get_session_detail`：第一次扫描时含 5 chunk，第二次扫描时（中间有 file-change 触发）含 8 chunk
- **THEN** 两次 IPC 响应中对应 `Process.messagesTotalCount` SHALL 分别为 `5` 与 `8`；前端可据此版本递增判定需要重拉 trace

#### Scenario: 嵌套 subagent 各自暴露 messagesTotalCount

- **WHEN** subagent A 的 messages 内嵌套含一条 subagent B 的引用，`get_subagent_trace` 返回 A 的 trace 含 B 的 `Process` 占位
- **THEN** A 与 B 的 `Process` MUST 各自携带独立的 `messagesTotalCount`，B 的值等于其自身 JSONL build_chunks 后的 chunk 数

### Requirement: Expose repository group queries

系统 SHALL 暴露 `list_repository_groups()` IPC：把 `ProjectScanner::scan()` 结果通过 `WorktreeGrouper::group_by_repository` 聚合为 `Vec<RepositoryGroup>`，每个 group 含 `id` / `identity` / `name` / `worktrees[]` / `mostRecentSession` / `totalSessions` 字段。Worktree 排序 SHALL 按 `is_main_worktree` 优先、再按 `most_recent_session` 倒序（已在 `WorktreeGrouper` 内部实现）。Group 排序 SHALL 按 `mostRecentSession` 倒序。

序列化 SHALL 使用 camelCase（`isMainWorktree`、`gitBranch`、`mostRecentSession`、`totalSessions`、`createdAt`）。

#### Scenario: 列出多 worktree 仓库分组
- **WHEN** 同一 git 仓库下存在主 worktree 与一个用户开的附加 worktree，且两者都有 sessions
- **THEN** `list_repository_groups()` SHALL 返回一个 group，`worktrees` 数组含两项，`worktrees[0].isMainWorktree=true`、`worktrees[1].isMainWorktree=false`

#### Scenario: 独立项目作为单成员分组
- **WHEN** 一个 project 路径无 git 元数据（不属任何 worktree）
- **THEN** `list_repository_groups()` SHALL 返回一个 group，`worktrees` 数组含该项目一项，`identity` 为 `null`

#### Scenario: 序列化 camelCase
- **WHEN** `list_repository_groups()` 返回结果被序列化为 JSON
- **THEN** 字段名 SHALL 为 `isMainWorktree` / `gitBranch` / `mostRecentSession` / `totalSessions` / `createdAt`（不是 snake_case）

### Requirement: Expose worktree sessions query

系统 SHALL 实现 `get_worktree_sessions(group_id, pagination)` IPC：定位 `group_id` 对应 `RepositoryGroup`，把该 group 下所有 worktree 的 sessions 合并为单一列表，按 `timestamp` 倒序后再应用 `PaginatedRequest`（`pageSize` + `cursor`）。返回 `PaginatedResponse<SessionSummary>`，每个条目 SHALL 额外携带 `worktreeId` / `worktreeName` 字段以便 UI 标注归属。

`pageSize == 0` 时 SHALL 立即拒绝（`ApiError::validation`），`pageSize` 不再被静默 clamp 为 1，避免隐藏调用方错误参数。

未命中 `group_id` 时 SHALL 拒绝（`ApiError::not_found`）。

错误形态遵循既有项目约定：trait / HTTP 层产 `ApiError { code, message }` 结构化错误；Tauri command wrapper 沿用 `Result<_, String>` —— 把 `ApiError` 通过 `to_string()` 序列化为含错误前缀的人类可读字符串（与 `list_sessions` / `get_session_detail` 等既有 command 一致），结构化 `code` 字段仅在 HTTP `axum::IntoResponse` 路径暴露。

Tauri command 入参 SHALL 与既有 `list_sessions` 风格一致——顶层 `groupId: string` + `pageSize?: number` + `cursor?: string`，**不**嵌套 `pagination` 对象（保持 IPC 调用形态在所有 paginated command 间一致）。HTTP 路径走 `GET /api/worktrees/{groupId}/sessions?pageSize=...&cursor=...` query string。

#### Scenario: 合并多 worktree sessions 按时间排序
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "repo-1", pageSize: 10 })`，repo-1 含两个 worktree 各 5 个 session
- **THEN** 响应 `items` SHALL 含 10 项，按 `timestamp` 倒序排列
- **AND** 每项 SHALL 含 `worktreeId` / `worktreeName` 字段

#### Scenario: 分页继续
- **WHEN** caller 接上一页 `nextCursor` 再调 `invoke("get_worktree_sessions", { groupId, pageSize, cursor: nextCursor })`
- **THEN** 响应 SHALL 返回剩余 sessions，不重复返回上一页内容

#### Scenario: pageSize 为 0 时拒绝
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "g1", pageSize: 0 })`
- **THEN** trait 层 SHALL 立刻返 `ApiError::validation(...)`，message 含 `pageSize must be > 0`
- **AND** Tauri command wrapper 把 ApiError 字符串化后让 `invoke` Promise reject 含该 message；HTTP 层走 `IntoResponse` 返 400 + `{code: "validation_error", message}` 结构化 JSON
- **AND** SHALL NOT 静默 clamp 为 1 也 SHALL NOT 返回部分结果

#### Scenario: group_id 不存在
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "nonexistent-group", pageSize: 10 })`
- **THEN** trait 层 SHALL 返 `ApiError::not_found(...)`，message 含 group id 标识符
- **AND** Tauri command wrapper 把 ApiError 字符串化后让 `invoke` Promise reject；HTTP 层走 `IntoResponse` 返 404 + `{code: "not_found", message}` 结构化 JSON

### Requirement: Tauri commands for repository groups and worktree sessions

系统 SHALL 通过 Tauri `invoke_handler!` 注册 `list_repository_groups` 与 `get_worktree_sessions` 两个 IPC command，参数与返回类型 SHALL 与上述 IPC trait 方法一致。两个 command 名 SHALL 同步出现在 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 与 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 两处常量列表中。

#### Scenario: invoke list_repository_groups 返回 camelCase 数组
- **WHEN** 前端调用 `invoke("list_repository_groups")`
- **THEN** 响应 SHALL 为 JSON 数组，每项含 `id` / `identity` / `name` / `worktrees` / `mostRecentSession` / `totalSessions` 字段（camelCase）

#### Scenario: invoke get_worktree_sessions 返回 PaginatedResponse
- **WHEN** 前端调用 `invoke("get_worktree_sessions", { groupId: "g1", pageSize: 20, cursor: null })`（顶层 `pageSize` / `cursor` 与既有 `list_sessions` 一致，不嵌套 `pagination`）
- **THEN** 响应 SHALL 为 `{ items: SessionSummary[], nextCursor: string | null, total: number }` 形态

