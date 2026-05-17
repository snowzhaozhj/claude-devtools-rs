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

系统 SHALL 在 `LocalDataApi` 上暴露 in-process 订阅机制 `subscribe_session_metadata()`，返回一个 `broadcast::Receiver<SessionMetadataUpdate>`。`SessionMetadataUpdate` SHALL 携带 `projectId` / `sessionId` / `title` / `messageCount` / `isOngoing` / `gitBranch`（序列化时为 camelCase）。Tauri host SHALL 把该订阅桥接到 webview，向前端 emit `session-metadata-update` 事件。

`list_sessions` 的骨架阶段 SHALL 对每条 `(session_id, jsonl_path)` 先调用 `try_lookup_cached_metadata`（lookup-only fast-path：查 `MetadataCache` + `FileSignature` 等价校验 + `is_session_stale(mtime)` 实时合成 `isOngoing`，**不**触发扫描）。命中条 SHALL 在骨架阶段直接 inline 填回 `title` / `messageCount` / `isOngoing` / `gitBranch` 真实值，且 SHALL NOT 入 `page_jobs`（即不 spawn 后台扫描、不通过 `broadcast::Sender<SessionMetadataUpdate>` 推送对应 update）；未命中场景包括 cache miss、`tokio::fs::metadata` stat 失败、`FileSignature` 不等（mtime / size / identity 任一不等）—— 任一未命中条 SHALL 入 `page_jobs` 走原后台扫描路径，扫完通过 broadcast 推送 update。

骨架阶段的 lookup 并发度 SHALL 通过 `Semaphore(METADATA_SCAN_CONCURRENCY=8)` 限流，与后台扫描使用同一上限常量。后台扫描自身的并发度 SHALL 同样被限流（固定上限 8），避免 50+ 文件同时打开；每次 `list_sessions(projectId, pagination)` 触发新扫描前 SHALL 取消同一 `(projectId, cursor)` 维度上一轮未完成的扫描，避免**同分页**的事件串扰；不同 `cursor` 的扫描 SHALL 并存而互不 abort（典型场景：page 1 与 page 2 的并发扫描相互独立）。扫描范围 SHALL 限定为本次 `list_sessions` 返回页中的**未命中** sessions；实现 MUST NOT 因为请求第一页而后台扫描完整项目历史。

cache 全命中场景下 `page_jobs.is_empty()` 时 `list_sessions` SHALL 跳过 `tokio::spawn(scan_metadata_for_page(...))` 分支，**不**触碰 `active_scans` 注册表——既有的 abort + generation + insert race-free 抢占逻辑（详见本 spec `Session list pagination avoids duplicate full scans` 与历史 codex 二轮二审）由"cache miss 时进入 spawn 分支"路径自然继承。

`active_scans` 注册表的 key SHALL 为 `(projectId, cursor)` 组合编码字符串（实现以 `format!("{project_id}|{cursor_or_empty}")`，`|` 字符为 reserved 分隔符；当前 cursor 由 offset 数字字符串生成，不会冲突）。同 key 抢占 + per-key generation cleanup 的 race-free 语义不变。

#### Scenario: 订阅接收当前页未命中条的元数据更新

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 随后调用 `list_sessions("projectA", { pageSize: 3, cursor: null })`，响应页包含 3 个 session，**所有** session 在 `MetadataCache` 中均为 miss（如冷启动场景）
- **THEN** receiver SHALL 在扫描完成后**最多**收到 3 条 `SessionMetadataUpdate`，每条携带对应 sessionId 的真实 `title` / `messageCount` / `isOngoing` / `gitBranch`

#### Scenario: Tauri host emit session-metadata-update

- **WHEN** Tauri host 在 `setup` 调用 `subscribe_session_metadata()` 并在后台 task 内订阅
- **AND** 后端产出 `SessionMetadataUpdate { project_id: "p", session_id: "s", title: Some("T"), message_count: 12, is_ongoing: false, git_branch: Some("main") }`
- **THEN** webview SHALL 通过 `listen("session-metadata-update", ...)` 收到 payload `{ projectId: "p", sessionId: "s", title: "T", messageCount: 12, isOngoing: false, gitBranch: "main" }`（camelCase）

#### Scenario: 同 projectId 同 cursor 的新扫描取消旧扫描

- **WHEN** `list_sessions("projectA", { pageSize: 20, cursor: null })` 正在扫描中（后台有未完成任务，至少一条 cache miss 进入 spawn 分支）
- **AND** 调用方再次调用 `list_sessions("projectA", { pageSize: 20, cursor: null })`（**同 cursor**，典型场景：silent 刷新或重复触发同一页加载），新页中有 cache miss 条触发新扫描
- **THEN** 旧扫描任务 SHALL 被 abort，未完成的 session 元数据 SHALL NOT 再被推送；新扫描 SHALL 只扫描新响应页中的未命中 sessions

#### Scenario: 同 projectId 不同 cursor 的扫描并存互不 abort

- **WHEN** `list_sessions("projectA", { pageSize: 20, cursor: null })`（page 1）正在扫描中
- **AND** 调用方紧接着调用 `list_sessions("projectA", { pageSize: 20, cursor: "20" })`（page 2，典型场景：Sidebar 首次加载后 `queueMicrotask(() => maybeLoadMoreSessions(true))` 自动补满视口），新页中有 cache miss 条
- **THEN** page 1 扫描任务 SHALL **继续运行**，page 1 内未完成的 session 元数据 SHALL 通过 broadcast 正常推送；同时 page 2 SHALL 启动独立扫描任务推送其未命中 session 的 update

#### Scenario: 切 project 不主动 abort 旧 project 扫描

- **WHEN** `list_sessions("projectA", ...)` 后台扫描进行中，调用方紧接着调用 `list_sessions("projectB", ...)`
- **THEN** projectA 的扫描 SHALL **继续运行**至完成，旧 project 的 `SessionMetadataUpdate` 仍会被 broadcast；前端 listener 已按 `payload.projectId !== selectedProjectId` 过滤，UI 不受影响

#### Scenario: 后台扫描并发度限制

- **WHEN** 扫描任务在并发处理某页 50 个 cache-miss session 文件
- **THEN** 同一时刻打开的 JSONL 文件句柄数 SHALL 不超过 8（通过 `tokio::sync::Semaphore` 或等价机制限流）

#### Scenario: 骨架 lookup 并发度限制

- **WHEN** `list_sessions("projectA", { pageSize: 50, cursor: null })` 骨架阶段对 50 个 session 并发执行 `try_lookup_cached_metadata`
- **THEN** 同一时刻进行 `tokio::fs::metadata` stat 的 future 数 SHALL 不超过 8（通过与后台扫描共享的 `METADATA_SCAN_CONCURRENCY=8` 上限）

#### Scenario: 无 watcher 构造器下 subscribe 安全

- **WHEN** `LocalDataApi` 通过不带 watcher 的构造器实例化（集成测试路径）
- **AND** 调用方 `subscribe_session_metadata()`
- **THEN** 返回有效 `broadcast::Receiver`；`list_sessions` 仍能正常推送（broadcast 不依赖 watcher）

#### Scenario: Cache 命中时骨架直接带值且零 emit

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 已对 "projectA" 调用过一次 `list_sessions`，期间 `MetadataCache` 已写入该页所有 session 的元数据
- **AND** 在 session jsonl 文件 mtime/size 未变化的前提下，再次调用 `list_sessions("projectA", { pageSize: 3, cursor: null })`
- **THEN** 第二次 `list_sessions` 返回的 `SessionSummary[]` SHALL 在骨架阶段直接携带每条的真实 `title` / `messageCount` / `isOngoing` / `gitBranch`（非占位）
- **AND** receiver SHALL 在第二次调用后短时间内（如 300 ms）**不**收到任何新的 `SessionMetadataUpdate`

#### Scenario: Cache 部分命中时未命中条仍走后台扫描

- **WHEN** `list_sessions` 骨架阶段对 3 个 session 调用 `try_lookup_cached_metadata`，其中 2 个命中（`FileSignature` 等价）、1 个 miss（jsonl 文件被追加新消息，size 与 mtime 已变更，`FileSignature` 不等）
- **THEN** 返回的 `SessionSummary[]` 中 2 个命中条骨架阶段 SHALL 已带真实元数据，1 个 miss 条骨架阶段 SHALL 仍为占位（`title=null` / `messageCount=0` / `isOngoing=false`）
- **AND** 该 miss 条 SHALL 入 `page_jobs` 走后台扫描，扫完通过 broadcast 推送 1 条 `SessionMetadataUpdate`；receiver 收到的 update 数 SHALL 为 1（只覆盖 miss 条）

#### Scenario: Cache 全命中时不触发 spawn 不触碰 active_scans

- **WHEN** `list_sessions` 骨架阶段对所有 session 都 cache 命中（page_jobs 为空）
- **THEN** 实现 SHALL NOT 调用 `tokio::spawn(scan_metadata_for_page(...))`
- **AND** SHALL NOT 改动 `active_scans` 注册表（既不 abort 旧 entry 也不 insert 新 entry）
- **AND** receiver SHALL 不收到任何对应该次调用的 `SessionMetadataUpdate`

#### Scenario: lookup stat 失败 fallback 到后台扫描

- **WHEN** `try_lookup_cached_metadata` 内 `tokio::fs::metadata(path).await` 返回 `Err`（罕见 IO 错误）
- **THEN** 函数 SHALL 返回 `None`
- **AND** 该 session SHALL 入 `page_jobs` 走后台扫描，由 `extract_session_metadata_cached` 内部的 uncached 路径处理（详见 `extract_session_metadata` 按 `FileSignature` 缓存 Requirement）

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

1. **Fast-path（teammate 主导消息）**：若 trim 后 text 以 `<teammate-message` 开头，先 regex 抽 `summary="..."` 属性内容；非空时 SHALL 直接返回 summary 内容作为标题候选（截断长度由常量 `TITLE_MAX_CHARS` 控制，见本 spec 同名 Requirement）。
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

### Requirement: `extract_session_metadata` 按 `FileSignature` 缓存

`LocalDataApi` SHALL 持有一个内部 LRU 缓存（不使用全局单例），以文件 `PathBuf` 为 key，记录上一次扫描时的 `(FileSignature, title, message_count, messages_ongoing, git_branch)`。`FileSignature` MUST 至少包含：

- `mtime`：文件最后修改时间
- `size`：文件字节数
- `identity`：文件身份 —— Unix `(dev, ino)`；Windows 与其它平台退化为空（详 design D1f）

**等价性是 best-effort**：在常规 append-only 写入路径下，`FileSignature` 字段 byte-equal 即视为文件未变。inode reuse + mtime/size 三维同时撞车的极端场景可能假命中，由后续任何文件变化的 file-change 自然恢复。

再次调用相同 path 时 SHALL 先 stat 目标文件，若 stat 拿到的 `FileSignature` 字段 byte-equal 等于缓存记录 THEN MUST 直接返回基于缓存数据合成的 `SessionMetadata`，**不**再 line-by-line 重读全文件；否则正常扫描并把结果写回缓存。

由于 `is_ongoing` 字段含 `is_file_stale(path)` 时间敏感判定，缓存 MUST 仅缓存"基于消息序列结构"的 `messages_ongoing` 中间值（即 `cdt_analyze::check_messages_ongoing` 的结果），而 `is_ongoing = messages_ongoing && !is_session_stale(signature.mtime, SystemTime::now())` MUST 在每次 lookup 时根据当前 wall clock 实时计算合成——不得直接缓存 `is_ongoing` 终态。

缓存 SHALL 在以下任一条件下走 cache miss：

- 缓存中无该 path
- `mtime` / `size` / `identity` 任一不一致
- stat 失败

缓存容量 SHALL 上限 200 entries，按 LRU 淘汰；命中时 MUST 把命中 key bump 到队首避免冷热混淆。

#### Scenario: 相同文件 `FileSignature` 不变命中缓存

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 拿到的 `FileSignature` 与缓存记录字段 byte-equal 等于缓存记录
- **THEN** MUST 直接返回基于缓存数据合成的 `SessionMetadata`，且 SHALL NOT 再调用 `tokio::io::AsyncBufReadExt::lines` 读全文件

#### Scenario: mtime 不一致触发重扫

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 拿到的 `mtime` 与缓存记录不同
- **THEN** MUST 走原有 line-by-line 全文件扫描路径，并以新 `FileSignature` 与新结果覆盖缓存

#### Scenario: 文件被 rename 替换（inode 变化）触发重扫（仅 Unix）

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 拿到的 `identity`（Unix `(dev, ino)`）与缓存记录不同 —— 即便 mtime 与 size 巧合相同
- **THEN** MUST 走 cache miss 分支重新扫描
- Windows 与其它平台 identity 退化为 `None`，此 Scenario 由 mtime/size 维度兜底（best-effort，详 design D1f）

#### Scenario: 缓存命中后实时重算 stale 状态

- **WHEN** 缓存命中（`FileSignature` 一致），且缓存条目的 `messages_ongoing = true`，且当前 wall clock 距 `mtime` 已超过 `STALE_SESSION_THRESHOLD`（5 分钟）
- **THEN** 返回的 `SessionMetadata.is_ongoing` MUST 为 `false`（`messages_ongoing && !stale = true && !true = false`）；缓存 SHALL NOT 因此被 invalidate（`FileSignature` 仍正确反映文件未变，下次访问还能复用其它字段）

#### Scenario: 文件 size 变小触发重扫

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 拿到的 `size` 比缓存记录小
- **THEN** MUST 走 cache miss 分支重新扫描

#### Scenario: stat 失败时走 cache miss

- **WHEN** 调用 metadata 缓存 wrapper 但 `tokio::fs::metadata(path)` 失败
- **THEN** MUST 走原路径（由 `File::open` 自身决定返回空 `SessionMetadata`），且 SHALL NOT 把空结果写入缓存

#### Scenario: 缓存超过容量按 LRU 淘汰

- **WHEN** 缓存已达 200 entries 时再调用一个新 path
- **THEN** MUST 淘汰当前最久未访问的条目后再写入新条目，缓存大小始终 ≤ 200

#### Scenario: 缓存命中时把 key bump 到队首

- **WHEN** lookup 在缓存中命中 `path`
- **THEN** MUST 把该 path 的 LRU 位置移到队首（最新访问），后续淘汰循环中该 path 不会被冷热顺序错误淘汰

### Requirement: metadata 缓存 ownership 由 `LocalDataApi` 持有

`LocalDataApi` SHALL 通过一个 `Arc<std::sync::Mutex<MetadataCache>>` 字段持有缓存实例。所有构造器（`new` / `new_with_xxx`）MUST 初始化为空 cache。**禁止**用全局 `OnceLock` / `static` 单例 ——多个 `LocalDataApi` 实例（HTTP server + Tauri IPC 各自构造）必须各自独立持有 cache，互相不共享。

`extract_session_metadata` 自身 MUST 保留为纯函数（不持 cache），缓存查询 wrapper（如 `extract_session_metadata_cached(cache, path)`）MUST 作为内部辅助函数，由 `LocalDataApi` 的方法或 `scan_metadata_for_page` 调用。

#### Scenario: 多个 `LocalDataApi` 实例独立持有 cache

- **WHEN** 测试或运行时构造两个 `LocalDataApi` 实例 A 与 B
- **THEN** A 的 `metadata_cache` 与 B 的 `metadata_cache` MUST 是独立 `Arc<Mutex<MetadataCache>>` 实例，A 中的缓存写入 SHALL NOT 影响 B 中的 lookup 结果

#### Scenario: `extract_session_metadata` 保持纯函数签名

- **WHEN** 现有调用方（含单元测试 `extract_*`）直接调用 `extract_session_metadata(path)`
- **THEN** 该函数签名 MUST 保持 `pub async fn extract_session_metadata(path: &Path) -> SessionMetadata`，不接受 cache 参数；行为与本 change 之前完全一致（line-by-line 全文件扫描）

### Requirement: Expose memory read operations

系统 SHALL 暴露只读 memory IPC 操作，允许前端按项目查询 memory layers 与读取单个 memory 文件。响应字段 MUST 使用 camelCase，且新 Tauri command MUST 同步登记到 command contract 与前端 mock command 清单。

#### Scenario: Query project memory via IPC
- **WHEN** 前端调用 `invoke("get_project_memory", { projectId: "p" })`
- **THEN** 响应 SHALL 为 JSON object，含 `projectId`、`hasMemory`、`count`、`defaultFile`、`layers` 字段
- **AND** `layers` 内每项 SHALL 含 `file`、`title`、`hook`、`kind` 字段（camelCase）

#### Scenario: Read memory file via IPC
- **WHEN** 前端调用 `invoke("read_memory_file", { projectId: "p", file: "MEMORY.md" })`
- **THEN** 响应 SHALL 为 JSON object，含 `projectId`、`file`、`filePath`、`content` 字段

#### Scenario: Tauri commands registered
- **WHEN** `cargo test -p cdt-api --test ipc_contract` 执行
- **THEN** `EXPECTED_TAURI_COMMANDS` SHALL 包含 `get_project_memory` 与 `read_memory_file`

#### Scenario: Memory IPC camelCase serialization
- **WHEN** Rust 侧 `ProjectMemory` 与 `MemoryLayer` 被序列化为 JSON
- **THEN** 字段名 SHALL 为 `hasMemory`、`defaultFile`、`projectId`，而不是 snake_case

### Requirement: `extract_session_metadata` 流式判定 isOngoing 不收集全量消息向量

`extract_session_metadata_with_ongoing` SHALL 流式判定 `messages_ongoing`：在 JSONL 逐行解析的 loop 内，将每条 `ParsedMessage` 即时喂给 `cdt_analyze::IsOngoingStateMachine` 的 `feed(&msg)` 接口，并在文件读取完毕后调用 `state_machine.finalize()` 得到最终 `messages_ongoing` 值。该函数 MUST NOT 在内存中保留 `Vec<ParsedMessage>` —— 即 `messages_ongoing` 的计算路径上不得 collect 全量解析结果到容器。

`cdt_analyze::IsOngoingStateMachine` SHALL 提供以下公开接口：
- `pub fn new() -> Self`：构造空状态机（`ongoing = false`，shutdown_tool_ids 为空集）
- `pub fn feed(&mut self, msg: &ParsedMessage)`：吃一条消息，按 `MessageType::Assistant` / `MessageType::User` 分发并更新内部状态
- `pub fn finalize(self) -> bool`：消费状态机得到最终 `is_ongoing` 判定

`IsOngoingStateMachine` 流式喂入的最终结果 SHALL 与既有 `cdt_analyze::check_messages_ongoing(&messages)` 在任意有限消息序列上完全等价。`check_messages_ongoing` MAY 内部委托给 `IsOngoingStateMachine`（thin wrapper：`for msg in messages { sm.feed(msg); } sm.finalize()`），公开签名保持 `pub fn check_messages_ongoing(messages: &[ParsedMessage]) -> bool`。

#### Scenario: 流式状态机不在内存保留全量 ParsedMessage

- **WHEN** 调用 `extract_session_metadata_with_ongoing` 处理一个含 N 条消息的 JSONL 文件
- **THEN** 函数实现路径 SHALL NOT 创建 `Vec<ParsedMessage>` 或等价容器以累积全部解析结果用于 `is_ongoing` 计算
- **AND** 实际驻留内存峰值 SHALL 不随 N 线性增长（仅 `IsOngoingStateMachine` 自身字段 + 当前正解析的单行消息）

#### Scenario: 状态机与切片版 check_messages_ongoing 结果等价

- **GIVEN** 一组覆盖 normal completed / ongoing tool-use / interrupted / teammate-message / shutdown_response / resumed-after-interrupt 六类典型场景的 fixture 消息序列
- **WHEN** 用 `IsOngoingStateMachine.feed(...).finalize()` 流式处理
- **AND** 用 `check_messages_ongoing(&[..])` 切片处理同一序列
- **THEN** 两种处理方式 SHALL 在每个 fixture 上返回相同 `bool` 结果

#### Scenario: 空消息序列返回 false

- **WHEN** 在新建的 `IsOngoingStateMachine` 上不调用任何 `feed`，直接 `finalize()`
- **THEN** SHALL 返回 `false`（与 `check_messages_ongoing(&[])` 一致）

#### Scenario: SHUTDOWN_RESPONSE tool 跨消息追踪

- **GIVEN** 序列：assistant 消息含 `tool_use { id: "tu-shutdown", name: "SendMessage", input: { type: "shutdown_response", approve: true } }`，紧随 user 消息含 `tool_result { tool_use_id: "tu-shutdown", ... }`
- **WHEN** 依次 `sm.feed(assistant_msg); sm.feed(user_msg); sm.finalize()`
- **THEN** 状态机内部 `shutdown_tool_ids` SHALL 在 feed assistant 时插入 `"tu-shutdown"`
- **AND** feed user 时识别匹配的 `tool_use_id`，将对应事件归类为 Interruption（ending），最终 `finalize()` SHALL 返回 `false`

#### Scenario: extract_session_metadata 公开签名保持纯函数语义

- **WHEN** 现有调用方直接调用 `extract_session_metadata(path)`
- **THEN** 该函数签名 SHALL 保持 `pub async fn extract_session_metadata(path: &Path) -> SessionMetadata` 不变
- **AND** 行为 SHALL 与本 change 之前完全一致（含 `is_ongoing` 取值，仅内部实现改流式）

### Requirement: `get_tool_output` 与 `get_image_asset` 走 parsed-message LRU 缓存

`LocalDataApi` SHALL 持有一个内部 parsed-message LRU 缓存（不使用全局单例），以 JSONL 文件 `PathBuf` 为 key，缓存值为 `(FileSignature, Arc<Vec<ParsedMessage>>)` 二元组。`get_tool_output` 与 `get_image_asset` MUST 在调用 `cdt_parse::parse_file(...)` 之前先查该缓存，命中时 MUST 直接复用缓存中的 `Arc<Vec<ParsedMessage>>`、SHALL NOT 重读 JSONL 全文件，亦 SHALL NOT 重新执行 line-by-line parse。

`FileSignature` 等价性 MUST 与 `MetadataCache` 同源（即 `crates/cdt-api/src/cache_signature.rs::FileSignature` 的 `(mtime, size, identity)` 三元组，identity 在 Unix 上为 `(dev, ino)`，Windows 与其它平台退化为 `None`），best-effort 语义与 `extract_session_metadata` 按 `FileSignature` 缓存 Requirement 完全一致。

缓存 SHALL 在以下任一条件下走 cache miss：

- 缓存中无该 path
- stat 拿到的 `FileSignature` 与缓存记录任一字段不一致
- stat 失败

miss 路径 MUST 调用 `parse_file(path)`：成功时把结果包装为 `Arc::new(messages)`，与新 `FileSignature` 一起写入缓存；解析失败时 SHALL NOT 写入缓存（避免 negative cache 引入新失效边界），由 caller 走原有错误兜底（`get_image_asset` 返回 `empty_data_uri()`、`get_tool_output` 返回 `ToolOutput::Missing`）。

`get_tool_output` 在命中缓存后 MUST 在 `Arc<Vec<ParsedMessage>>` 上重新调用 `cdt_analyze::build_chunks(&messages)` 完成 tool_use_id 匹配——本 change 不缓存 `build_chunks` 结果，仅缓存 parse 一层（详 change `parsed-message-lru-cache` design D2/D6 决策）。

缓存容量 SHALL 上限 50 entries，按 LRU 淘汰；命中时 MUST 把命中 key bump 到队首避免冷热混淆。

#### Scenario: `get_tool_output` 命中缓存时不重读 JSONL

- **WHEN** 调用方第一次调 `get_tool_output(root, sid, tool_use_id_a)`，cache 写入对应 session 的 JSONL parse 结果
- **AND** 同一 session 文件未变（`FileSignature` 一致），调用方再次调 `get_tool_output(root, sid, tool_use_id_b)`
- **THEN** 第二次调用 MUST 直接从缓存读取 `Arc<Vec<ParsedMessage>>`，SHALL NOT 调用 `cdt_parse::parse_file(...)` 重读 JSONL 全文件
- **AND** 缓存条目的 `Arc` 引用计数 SHALL 通过 `Arc::clone` 共享而非整个 `Vec<ParsedMessage>` 数据复制

#### Scenario: `get_image_asset` 命中缓存时不重读 JSONL

- **WHEN** 调用方第一次调 `get_image_asset(root, sid, block_id_a)`，cache 写入对应 session 的 JSONL parse 结果
- **AND** 同一 session 文件未变，调用方再次调 `get_image_asset(root, sid, block_id_b)`
- **THEN** 第二次调用 MUST 直接从缓存读取 `Arc<Vec<ParsedMessage>>`，SHALL NOT 调用 `cdt_parse::parse_file(...)` 重读 JSONL 全文件

#### Scenario: 同 session 在 `get_tool_output` 与 `get_image_asset` 之间共享缓存

- **WHEN** 调用方先调 `get_tool_output(root, sid, tu)` 完成 cache 写入
- **AND** 同 session 文件未变，调用方再调 `get_image_asset(root, sid, block_id)`
- **THEN** `get_image_asset` MUST 命中同一缓存条目，SHALL NOT 重新 parse JSONL

#### Scenario: `FileSignature` 不一致走 cache miss

- **WHEN** 调用 `get_tool_output` / `get_image_asset` 时 stat 拿到的 `FileSignature` 与缓存记录任一字段（mtime / size / identity）不一致
- **THEN** MUST 走 cache miss 分支，调 `parse_file(...)` 重新解析全文件，并以新 `FileSignature` + 新结果覆盖缓存

#### Scenario: parse 失败时 SHALL NOT 写入缓存

- **WHEN** 调用 `get_tool_output` / `get_image_asset` 时 cache miss，但 `parse_file(...)` 返回 `Err`
- **THEN** MUST 走 caller 的原有错误兜底路径（`get_image_asset` 返回 `empty_data_uri()`、`get_tool_output` 返回 `ToolOutput::Missing`），且 SHALL NOT 把空 `Vec` 或任何条目写入缓存

#### Scenario: stat 失败时走 cache miss 且不写入

- **WHEN** 调用 `get_tool_output` / `get_image_asset` 时 `tokio::fs::metadata(path)` 失败
- **THEN** MUST 走原 caller 错误兜底路径，SHALL NOT 把任何条目写入缓存

#### Scenario: 缓存超过容量按 LRU 淘汰

- **WHEN** 缓存已达 50 entries 时再调 `get_tool_output` / `get_image_asset` 触发一个新 path 写入
- **THEN** MUST 淘汰当前最久未访问的条目后再写入新条目，缓存大小始终 ≤ 50

#### Scenario: 缓存命中时把 key bump 到队首

- **WHEN** lookup 在缓存中命中 `path`
- **THEN** MUST 把该 path 的 LRU 位置移到队首（最新访问），后续淘汰循环中该 path 不会被冷热顺序错误淘汰

### Requirement: parsed-message 缓存按 file-change 广播主动失效

`LocalDataApi::new_with_watcher(...)` 构造路径 SHALL 在 spawn 自动通知管线的同时，额外 spawn 一个后台 task，订阅 `FileWatcher::subscribe_files()` 广播，对每条 `FileChangeEvent` 按 `projects_dir / project_id / "{session_id}.jsonl"` 推算出 cache key。

**stat 校验语义**：收到事件后 task MUST 先 `tokio::fs::metadata(&path)` 拿当前 `FileSignature`，与 cache 中记录的 signature 比对：
- 两者一致 → SHALL NOT 移除（视为 spurious watcher 事件——典型场景：CI 上 inotify 启动期对刚创建的 watch dir 偶发"无内容变化"事件、metadata-only touch、跨平台 backend 行为差异等。若无 stat 比对会错杀仍有效的 cache，导致下次 hot path 不必要重 parse）
- 两者不一致 → MUST `remove(path)` 让下次 lookup 重 parse
- `tokio::fs::metadata` 失败（文件被删 / 权限）→ MUST `remove(path)` 保守剔除——反正下次 hot path lookup 也会 stat fail 走原兜底（`empty_data_uri()` / `ToolOutput::Missing`），提前清掉不影响正确性

该失效路径与 `FileChangeEvent.deleted` 字段无关——文件被删 / 改 / 新建都同样进入"stat → 比对 signature → 决定 remove"流程。

`LocalDataApi::new()` 构造路径（无 watcher）SHALL NOT 启动该订阅 task；此场景仅依赖被动 `FileSignature` 失效路径兜底——与 `MetadataCache` 在 `new()` 路径下的行为对齐。

broadcast lag（`broadcast::Receiver::recv` 返回 `Err(RecvError::Lagged)`）时 SHALL 静默继续 loop——lag 仅代表事件激增，下次 lookup 由被动 `FileSignature` mismatch 兜底，不影响正确性。channel close（`Err(RecvError::Closed)`）时 task SHALL 退出。

#### Scenario: 文件真改后 file-change 广播主动 invalidate

- **WHEN** `LocalDataApi` 由 `new_with_watcher` 构造且缓存中已有 `<projects_dir>/<encoded_project>/<sid>.jsonl` 的 parsed-message 条目
- **AND** session JSONL 文件被追加 / 重写（mtime+size 变化）
- **AND** `FileWatcher` 广播一条对应 `FileChangeEvent`
- **THEN** 后台 invalidate task MUST 先 stat 拿当前 `FileSignature`、与 cache 记录比对、发现不一致后 remove 该 path 对应的条目，使下一次 `get_tool_output` / `get_image_asset` 走 cache miss + 重 parse

#### Scenario: spurious file-change 事件 SHALL NOT 错杀有效 cache

- **WHEN** 缓存中已有某 session 的 parsed-message 条目
- **AND** `FileWatcher` 因 backend 行为发出了一条 `FileChangeEvent`，但目标文件内容 / mtime / size 实际未变（典型 CI inotify 启动期 spurious 事件）
- **THEN** invalidate task MUST stat 拿当前 `FileSignature` 与 cache 记录比对，发现两者一致后 SHALL NOT remove 条目；后续 lookup MUST 仍命中 cache

#### Scenario: 文件被删时 stat 失败走保守 remove

- **WHEN** 缓存中已有某 session 的 parsed-message 条目
- **AND** `FileWatcher` 广播 `FileChangeEvent { ..., deleted: true }` 之后文件已不存在
- **THEN** invalidate task 的 `tokio::fs::metadata(&path)` 失败，task MUST 调 `remove(path)` 保守剔除条目

#### Scenario: `new()` 构造不启动 invalidate 订阅

- **WHEN** `LocalDataApi` 由 `new(scanner, config_mgr, notif_mgr, ssh_mgr)` 构造（无 watcher 参数）
- **THEN** SHALL NOT spawn 任何订阅 `FileWatcher::subscribe_files()` 的后台 task；parsed-message cache 仅依赖被动 `FileSignature` 失效

### Requirement: parsed-message 缓存 ownership 由 `LocalDataApi` 持有

`LocalDataApi` SHALL 通过一个 `Arc<std::sync::Mutex<ParsedMessageCache>>` 字段持有缓存实例。所有构造器（`new` / `new_with_watcher` / 任何后续 `new_with_xxx`）MUST 初始化为空 cache。**禁止**用全局 `OnceLock` / `static` 单例 ——多个 `LocalDataApi` 实例（HTTP server + Tauri IPC 各自构造）必须各自独立持有 cache，互相不共享。

构造器扩展（如本 change 引入的 cache 注入路径）MUST 遵循"`new()` / `new_with_watcher()` 签名不变 + 链式 `with_xxx` 或新 `new_with_xxx`"模式（CLAUDE.md `LocalDataApi 构造器扩展` 硬约束）；本 change SHALL 仅在 `LocalDataApi` 现有 `new()` / `new_with_watcher()` 内部初始化新字段，**不**改这两个构造器的参数签名。

#### Scenario: 多个 `LocalDataApi` 实例独立持有 parsed-message cache

- **WHEN** 测试或运行时构造两个 `LocalDataApi` 实例 A 与 B
- **THEN** A 的 parsed-message cache 与 B 的 parsed-message cache MUST 是独立 `Arc<Mutex<ParsedMessageCache>>` 实例，A 中的缓存写入 SHALL NOT 影响 B 中的 lookup 结果

#### Scenario: 不改 `new()` / `new_with_watcher()` 签名

- **WHEN** 既有调用方（集成测试 / `src-tauri/src/lib.rs` 等）按现有签名调用 `LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr)` 或 `LocalDataApi::new_with_watcher(scanner, config_mgr, notif_mgr, ssh_mgr, watcher, projects_dir)`
- **THEN** 这两个构造器签名 MUST 保持不变；parsed-message cache 字段 MUST 在构造器内部初始化为空 `ParsedMessageCache`

### Requirement: Stable chunk identifiers in SessionDetail

`get_session_detail` 返回的 `SessionDetail.chunks` 中每个 `Chunk` SHALL 暴露 `chunkId` 字段（camelCase 序列化），且同一次返回内所有 `chunkId` MUST 唯一。同一 session 文件内容未变化时，重复调用 `get_session_detail(projectId, sessionId)` MUST 返回相同顺序、相同 `chunkId` 的 chunks。

**统一 `chunkId` 形态**（本 change 引入）：所有 `Chunk` 类型（`AIChunk` / `UserChunk` / `SystemChunk` / `CompactChunk`）的 `chunkId` MUST 形如 `<base>:<n>`（`n` 从 0 起的十进制整数）。`AIChunk` 的 `base` MUST 取 `responses[0].uuid`（空 responses 时 fallback 字面量 `"empty"`）；`UserChunk` / `SystemChunk` / `CompactChunk` 的 `base` MUST 取自身消息 `uuid`。**MUST NOT** 使用裸 `<uuid>` 形态（即使首次出现也必须带 `:0` 后缀），**MUST NOT** 使用 `ai:` / `user:` / `sys:` / `compact:` 等类型前缀——chunk 类型由 `Chunk::kind` 字段区分，**不**靠 `chunkId` 字面前缀。

**Collision-free 兜底**：后端在分配 `chunkId` 时 MUST 维护一个跨所有 chunk 类型共享的 build 阶段全局已分配集合（`HashSet<String>`），命中冲突时 MUST 继续递增 ordinal 后缀 `n` 直到 candidate 未被占用——以兜底 uuid 自身恰好形如 `<base>:<n>` 等极端上游输入下"跨形态撞车"以及"跨类型撞车"的 corner case，确保整体 `chunkId` 集合 MUST 唯一。

#### Scenario: 所有 chunk 首次出现使用 `<uuid>:0`

- **WHEN** `get_session_detail` 返回 `UserChunk` / `SystemChunk` / `CompactChunk` / `AIChunk`，且其 base（`uuid` 或 `responses[0].uuid`）在同一次返回的其余 chunk 中**未**出现过
- **THEN** 该 chunk 的 `chunkId` SHALL 等于 `format!("{base}:0")`
- **AND** SHALL NOT 等于裸 `base`（无后缀）
- **AND** SHALL NOT 含 `ai:` / `user:` / `sys:` / `compact:` 等类型前缀

#### Scenario: 重复 assistant response uuid 仍生成唯一 chunkId

- **WHEN** 一个 session 在 compact/replay 后产生两个 `AIChunk`，且两个 chunk 的 `responses[0].uuid` 相同（值 `"dup"`）
- **THEN** `get_session_detail` 返回的两个 `AIChunk.chunkId` SHALL 分别为 `"dup:0"` 与 `"dup:1"`
- **AND** 整体 `chunks.map(chunk => chunk.chunkId)` MUST 唯一

#### Scenario: 未变化 session 重复调用时 chunkId 稳定

- **WHEN** 同一 `projectId` / `sessionId` 对应的 session JSONL 文件内容未变化
- **AND** caller 连续两次调用 `get_session_detail(projectId, sessionId)`
- **THEN** 两次返回的 `chunks.map(chunk => chunk.chunkId)` SHALL 完全相同

#### Scenario: 重复 user uuid 仍生成唯一 chunkId

- **WHEN** 同一 sessionId 的 JSONL 在 `claude --bg` 启动子会话等场景下出现两条 `uuid` 相同的 user 消息（值 `"u-dup"`）
- **AND** `get_session_detail` 为这两条消息分别构造 `UserChunk`
- **THEN** 两个 `UserChunk.chunkId` SHALL 分别为 `"u-dup:0"` 与 `"u-dup:1"`
- **AND** 整体 `chunks.map(chunk => chunk.chunkId)` MUST 唯一，前端 `{#each ... as chunk (chunk.chunkId)}` MUST NOT 触发 duplicate key 错误

#### Scenario: uuid 与 `<uuid>:<n>` 后缀形态撞车时仍唯一

- **WHEN** 同一次 `get_session_detail` 返回内既有 `uuid == "abc"` 的 user chunk，又有另一条 `uuid == "abc:1"` 的 user chunk
- **AND** `uuid == "abc"` 的 chunk 第二次出现（按统一规则 candidate 应为 `"abc:1"`，但已被 `uuid == "abc:1"` 首次出现产出的 `"abc:1:0"` 之前的 candidate 占用）
- **THEN** 后端 MUST 校验 candidate 是否已被占用
- **AND** MUST 继续递增 ordinal 直到 candidate 未被占用（实际产 `"abc:0"` / `"abc:1:0"` / `"abc:1"` 三条互不撞）
- **AND** 整体 `chunks.map(chunk => chunk.chunkId)` MUST 唯一

#### Scenario: AI chunk 与 user chunk 跨类型不撞

- **WHEN** 同一次 `get_session_detail` 返回内有一条 `AIChunk`（`responses[0].uuid == "x"`）和一条 `UserChunk`（`uuid == "x"`）
- **THEN** 两个 chunk 的 `chunkId` 候选都是 `"x:0"`，全局集合检测冲突
- **AND** 后到的 chunk MUST 递增到 `"x:1"`
- **AND** 两个 `chunkId` SHALL 不相同

### Requirement: Title length is bounded by TITLE_MAX_CHARS constant

`extract_session_metadata` 提取的 `SessionSummary.title` 最终字符数 SHALL ≤ `TITLE_MAX_CHARS = 500`（Unicode `char` 计数，不是 byte 数）。所有截断路径（teammate summary fast-path / slash-with-args 直接路径 / 普通 sanitize 路径）SHALL 调用同一 `truncate_str(_, TITLE_MAX_CHARS)` helper，禁止散落不同 magic number。

常量 `TITLE_MAX_CHARS` SHALL 定义在 `crates/cdt-api/src/ipc/session_metadata.rs` 顶部并 `pub` 暴露给同 crate 测试。

#### Scenario: Plain-text title longer than 500 chars is truncated at 500

- **WHEN** session 第一条 user 消息 content 为 700 个中文字符的纯文本
- **THEN** `extract_session_metadata.title.unwrap().chars().count()` SHALL ≤ 500

#### Scenario: Slash with args longer than 500 chars is truncated at 500

- **WHEN** session 第一条 user 消息为 `<command-name>/foo</command-name><command-args>` + 700 字符 + `</command-args>`
- **THEN** `extract_session_metadata.title.unwrap().chars().count()` SHALL ≤ 500

### Requirement: Title algorithm changes do not invalidate MetadataCache

`extract_session_metadata` 的 title 提取算法（含 slash 处理 / interrupted 过滤 / sanitize 规则 / 截断长度）发生变化时 SHALL NOT 主动 invalidate `MetadataCache`。命中旧 `FileSignature`（mtime / size / identity 全部不变）的条目 SHALL 继续返回缓存里的旧 title 字符串，直到文件签名发生变化（用户写入新行）或被 LRU 淘汰后才按新算法重扫并写回。

理由：title 算法变更属于"对老 session 文件展示形态的语义优化"，老缓存按旧算法计算的 title 在用户视角上"不够好但不离谱"；强制 invalidate 会触发下次启动时数百 session 文件的扫描风暴（违反 perf 预算）。新会话 / 文件改动后的会话天然走新算法。

实现含义：

- `MetadataCache` 数据结构 SHALL NOT 因 title 算法版本变化而新加 `algorithm_version` 字段或类似 cache-busting 机制
- `LocalDataApi` SHALL NOT 在启动 / 配置变更 / app 升级路径触发 `cache.clear()` 等批量 invalidate
- 单条 cache miss 的判定 SHALL 仅依据 `FileSignature != stored.signature`（既有行为）

#### Scenario: Stored cache entry with old title is reused on hit

- **GIVEN** `MetadataCache` 已存在某 path 的 entry，其 `title = Some("旧规则算出的 title")`，`signature` 与磁盘文件当前 `FileSignature` 一致
- **WHEN** `extract_session_metadata_cached` 被以同一 path 再次调用
- **THEN** 返回的 `SessionMetadata.title` SHALL 等于 `Some("旧规则算出的 title")`
- **AND** 实现 SHALL NOT 重新读取或重新解析该 session JSONL 文件

#### Scenario: New title algorithm applies only to fresh scans

- **GIVEN** 同一 session JSONL 文件，缓存中存的旧 title 是 `"提一下PR吧，我审查一下"`（按旧算法）
- **WHEN** 该 session 文件被追加新内容导致 `FileSignature` 变化（mtime / size 改变）
- **THEN** 下一次 `extract_session_metadata_cached` SHALL 触发重扫
- **AND** 返回的 title SHALL 按新算法重新计算（截图 case 应得 `/impeccable 根据项目的已有代码生成一下设计规范`）

