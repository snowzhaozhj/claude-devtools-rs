## MODIFIED Requirements

### Requirement: Expose project and session queries

系统 SHALL 在请求 / 响应式 IPC 通道上暴露项目与会话相关数据查询，至少包括：列项目、列项目下 sessions（含分页）、取 session 详情、取 session metrics、取 waterfall 数据、取 subagent 详情。

`get_session_detail` SHALL 在返回 session 详情时集成 subagent 解析：**从主 session 所在 `projects_dir`（即 `~/.claude/projects/` 或 SSH 远端等价路径）下所有 project 目录扫描 `{rootSessionId}/subagents/agent-*.jsonl`（新结构）**，合并去重后调用 `resolve_subagents` 填充 `AIChunk.subagents` 字段。旧结构（flat `{project_dir}/agent-*.jsonl`）SHALL 保持只扫描主 `project_dir` 并按首行 `parentUuid` / `sessionId` 字段过滤。若扫描失败或无候选，`subagents` SHALL 为空数组（不报错）。回滚开关 `CROSS_PROJECT_SUBAGENT_SCAN: bool` 顶层 const，设为 `false` 时 SHALL 退回"只扫主 `project_dir`"的原行为。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.subagents[i].messages` 数组 MUST 默认被裁剪为空 Vec，且 `messagesOmitted=true`** —— 用于把首屏 IPC payload 控制在原大小 ~40%（subagent 嵌套 chunks 全文是大头，行为契约见 change `subagent-messages-lazy-load`）。`Process.headerModel` / `Process.lastIsolatedTokens` / `Process.isShutdownOnly` 三个 derived 字段 MUST 在 `candidate_to_process` 阶段由 messages 预算后填充，让 SubagentCard header 不依赖完整 `messages` 也能正常渲染。回滚开关 `OMIT_SUBAGENT_MESSAGES: bool` 设 false 时 SHALL 退回完整 payload（messages 不裁剪、`messagesOmitted=false`）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `ContentBlock::Image.source.data` 字段 MUST 默认被替换为空字符串 `""`，且同时设 `source.dataOmitted=true`** —— 用于把首屏 IPC payload 中内联截图的 base64 字符串裁掉（行为契约见本 spec `Lazy load inline image asset` Requirement）。`source.kind` / `source.media_type` 字段 SHALL 保留（前端渲染时仍需要），仅 `data` 字段被清空。回滚开关 `OMIT_IMAGE_DATA: bool` 设 false 时 SHALL 退回完整 base64 payload（`data` 保留原值、`dataOmitted=false`）。该裁剪 SHALL 应用于所有 chunk 类型（UserChunk / AIChunk responses / subagent.messages 内嵌套——但 subagent.messages 默认已被裁剪，仅在回滚 `OMIT_SUBAGENT_MESSAGES=false` 时才会触及嵌套层）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.responses[i].content` 字段 MUST 默认被替换为空 `MessageContent::Text("")`，且同时设 `contentOmitted=true`** —— 用于把首屏 IPC payload 中最大单一字段（实测 `46a25772` case 下 1257 KB / 41%）裁掉（行为契约见 change `session-detail-response-content-omit`）。该字段在前端任何代码路径中都不被读取（chunk 显示文本走 `semanticSteps` 的 thinking / text 步骤），裁剪后 UI 渲染零变化。回滚开关 `OMIT_RESPONSE_CONTENT: bool` 设 false 时 SHALL 退回完整 payload（`content` 携带原 `MessageContent`、`contentOmitted=false`）。该裁剪 SHALL 应用于顶层 AIChunk 及 subagent.messages 嵌套层（与 `OMIT_IMAGE_DATA` 同模式：在 `OMIT_SUBAGENT_MESSAGES=true` 默认路径下嵌套层为 no-op；`OMIT_SUBAGENT_MESSAGES=false` 回滚时仍能命中嵌套层）。

**`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.tool_executions[i].output` 内 `text` / `value` 字段 MUST 默认被替换为空（`Text { text: "" }` / `Structured { value: Null }` / `Missing` 不变），且同时设 `outputOmitted=true`** —— 用于把首屏 IPC payload 中 tool 输出（实测 `46a25772` case 下 436 KB / 26%）裁掉（行为契约见本 spec `Lazy load tool output` Requirement）。`output` enum 的 variant kind SHALL 保留（前端 ToolViewer 路由仍需要），仅内层 `text` / `value` 被清空。回滚开关 `OMIT_TOOL_OUTPUT: bool` 设 false 时 SHALL 退回完整 payload（`output` 内字段保留原值、`outputOmitted=false`）。该裁剪 SHALL 应用于顶层 AIChunk 及 subagent.messages 嵌套层（与其它 OMIT 同模式：默认嵌套层 no-op；`OMIT_SUBAGENT_MESSAGES=false` 回滚时仍能命中嵌套层）。

**`OMIT_TOOL_OUTPUT=true` 路径下 `ToolExecution.outputBytes: Option<u64>` MUST 在 `trim` output 之前按 variant 记录原始字节长度**（`Text` → `text.len()`、`Structured` → `serde_json::to_string(value).map(|s| s.len()).unwrap_or(0)`、`Missing` → 不填，保持 `None`），让前端在懒加载之前即可估算 output token 数（按 `outputBytes / 4` 启发式），从而 BaseItem 头部 token 显示 SHALL **在懒加载展开前后保持一致**——不再因 `getToolOutputTokens` 在 OMIT 状态返回 0、懒加载后返回真实值而抖动。`OMIT_TOOL_OUTPUT=false` 回滚路径下 `outputBytes` SHALL 保持 `None`（前端 fallback 到直接读 `text.length`）。解析层（`cdt-parse` / `cdt-analyze`）SHALL **不**主动填充 `outputBytes`——该字段仅在 IPC OMIT 层语义有意义。

`list_sessions(projectId, pagination)` 的响应行为 SHALL 按 `pagination.cursor` 分叉：

- **首页路径（`pagination.cursor == None`）**：响应中**前** `min(page_size, EAGER_FIRST_PAGE_LIMIT = 20)` 条 `SessionSummary` MUST 携带 `sessionId` / `projectId` / `timestamp` / `title` / `messageCount` / `isOngoing` / `gitBranch` 的真实值（`title` 仍可为 `None` 当 jsonl 文件确实未包含 user message——这是真值，不是占位）；实现 SHALL 在 IPC return 之前**同步等待**前 20 条 items 的 `extract_session_metadata_cached` 完成（通过 `futures::future::join_all` + 共享 `Semaphore(METADATA_SCAN_CONCURRENCY=8)`，每条 `tokio::time::timeout(EAGER_PER_SESSION_TIMEOUT = 500ms)`）。`page_size > EAGER_FIRST_PAGE_LIMIT` 时**剩余** `page_size - 20` 条 items MAY 保留骨架占位（`title=null` / `messageCount=0` / `isOngoing=false` / `gitBranch=null`）+ 走原 `scan_metadata_for_page` 后台 spawn + broadcast 路径（与翻页路径同模型）。eager 同步等到的前 20 条**不**得 broadcast emit；超时 / 解析失败的条目 + remainder 骨架条目 SHALL 通过 broadcast 推送 `SessionMetadataUpdate`。
- **翻页路径（`pagination.cursor == Some(_)`）**：行为不变。响应每条 `SessionSummary` 的 `sessionId` / `projectId` / `timestamp` SHALL 为真实值；`title` / `messageCount` / `isOngoing` / `gitBranch` SHALL 允许为占位值（`null` / `0` / `false` / `null`）。`try_lookup_cached_metadata` lookup-fast-path 命中条 SHALL 在骨架阶段直接 inline 填回真值；未命中条 SHALL 入 `page_jobs` 走后台扫描 + `broadcast::Sender<SessionMetadataUpdate>` 推送。
- 单条 metadata 解析失败（`extract_session_metadata_cached` 返回字段全占位）SHALL **保留骨架占位**，**不**整页 IPC fail——首页路径与翻页路径行为一致。前端 `.metadata-pending` 视觉降级在两条路径上都仍可正常承载部分失败条。

`get_session_detail` 返回的 `SessionDetail.isOngoing` 仍 MUST 为同步计算后的真实值（因为 detail 已在调用链内完成全文件解析）。

**`isOngoing` 真实值 SHALL 由两路 AND 计算**：(a) `cdt_analyze::check_messages_ongoing(messages)` 返回 `true`（结构性活动栈五信号判定），**且** (b) session JSONL 文件 mtime 距当前时刻 `< 5 分钟`。任一条件不满足时 `isOngoing` MUST 为 `false`。stale 阈值常量 `STALE_SESSION_THRESHOLD = 5 min` 对齐原版 `claude-devtools/src/main/services/discovery/ProjectScanner.ts` 的 `STALE_SESSION_THRESHOLD_MS = 5 * 60 * 1000`（issue #94：用户 Ctrl+C / kill cli / 关机导致 cli 异常退出时，session 末尾停在 `tool_result` 之类 AI 活动而无 ending 信号，活动栈会误判 ongoing；mtime 兜底将其纠正）。`list_sessions` 异步扫描路径与 `get_session_detail` 同步路径行为 MUST 一致；HTTP `GET /api/projects/{projectId}/sessions` 路径共用同一 `extract_session_metadata` 实现（详见本 spec §"HTTP `list_sessions` 复用 IPC 骨架 + push 实现"），自动适用。stat 失败时 SHALL 保守保留 messages_ongoing 判定（避免 fs 偶发错误把活跃 session 错判 dead）；时钟回拨导致 mtime > now 时 SHALL 判 not stale（避免未来 mtime 把活跃 session 误判 dead）。

序列化 SHALL 使用 camelCase（`isOngoing`、`messagesOmitted`、`headerModel`、`lastIsolatedTokens`、`isShutdownOnly`、`dataOmitted`、`contentOmitted`、`outputOmitted`、`outputBytes`）。例外：`ImageSource.media_type` 与 `ImageSource.type`（kind）保持 snake_case，与上游 Anthropic JSONL 格式一致——同 `TokenUsage` 例外。

**HTTP `GET /api/projects/{projectId}/sessions` 路径 SHALL 与 IPC `list_sessions` 共用同一 `DataApi::list_sessions(...)` 实现**——即 `cdt-api::http::routes::list_sessions` SHALL 调 `DataApi::list_sessions(project_id, pagination)`，由 trait 实现内部按 `pagination.cursor` 分叉到首页 eager 路径或翻页骨架+push 路径。HTTP 翻页路径上后台扫描产物 SHALL 通过 `cdt-api::http::bridge::forward_session_metadata` 桥接到 `/api/events` SSE，浏览器 client 按 `session-metadata-update` event 收到与 IPC 翻页路径同形的 patch；HTTP 首页路径上响应 body 已含真值，**不**触发 SSE `session_metadata_update` event。

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

#### Scenario: HTTP list_sessions 首页 cursor=None eager-await 前 EAGER_FIRST_PAGE_LIMIT 条

- **WHEN** 客户端发起 `GET /api/projects/{projectId}/sessions?pageSize=N`（不带 `cursor` query 参数，即 `cursor=None`）
- **THEN** axum handler `cdt-api::http::routes::list_sessions` SHALL 调 `DataApi::list_sessions(project_id, pagination)`（与 IPC 路径共用同一 trait 方法）
- **AND** 响应 body SHALL 是 `PaginatedResponse<SessionSummary>`：**前 `min(N, EAGER_FIRST_PAGE_LIMIT = 20)` 条** `SessionSummary` 的 `sessionId` / `projectId` / `timestamp` / `title` / `messageCount` / `isOngoing` / `gitBranch` SHALL 为真实值（`title` 可为 `null` 当 jsonl 文件未含 user message——这是真值不是占位；个别条目 metadata 解析超时 / 失败时降级为占位）
- **AND** 当 `N > EAGER_FIRST_PAGE_LIMIT` 时，**剩余 `N - 20` 条** SHALL 为骨架占位（`title=null` / `messageCount=0` / `isOngoing=false` / `gitBranch=null`），SHALL 通过 `/api/events` SSE 的 `session_metadata_update` 事件异步推送真实值（与翻页路径同语义）
- **AND** 该次响应 SHALL NOT 对前 20 条 eager 同步等到的 sessionId 触发 `/api/events` SSE 上的 `session_metadata_update` event；超时 / 失败条 deferred retry + remainder scan emit 的 update SHALL 通过 SSE 推送（兜底）

#### Scenario: HTTP list_sessions 翻页 cursor=Some 走骨架 + SSE

- **WHEN** 客户端发起 `GET /api/projects/{projectId}/sessions?pageSize=N&cursor=C`（`cursor=Some(C)`）
- **THEN** 响应 body SHALL 是骨架 `PaginatedResponse<SessionSummary>`：每条 `SessionSummary` 的 `sessionId` / `projectId` / `timestamp` SHALL 为真实值；`title` / `messageCount` / `isOngoing` / `gitBranch` SHALL 允许为占位值（除非 `try_lookup_cached_metadata` 命中可直接 inline 填回真值）
- **AND** 后台 `scan_metadata_for_page` 任务对 cache miss 的 session 完成扫描后调 `broadcast::Sender<SessionMetadataUpdate>::send(update)`，update SHALL 通过 `cdt-api::http::bridge::forward_session_metadata` 转换为 `PushEvent::SessionMetadataUpdate { projectId, sessionId, title, messageCount, isOngoing, gitBranch }` 推送到所有 `/api/events` 客户端
- **AND** 浏览器 client `transport.ts::BrowserTransport` SHALL 按既有归一化路径转交 `session-metadata-update` 事件给 listener，与 IPC 翻页路径行为一致

### Requirement: Emit session metadata updates

系统 SHALL 在 `LocalDataApi` 上暴露 in-process 订阅机制 `subscribe_session_metadata()`，返回一个 `broadcast::Receiver<SessionMetadataUpdate>`。`SessionMetadataUpdate` SHALL 携带 `projectId` / `sessionId` / `title` / `messageCount` / `isOngoing` / `gitBranch`（序列化时为 camelCase）。Tauri host SHALL 把该订阅桥接到 webview，向前端 emit `session-metadata-update` 事件。

`list_sessions` 的行为 SHALL 按 `pagination.cursor` 分叉：

- **首页路径（`pagination.cursor == None`）**：实现 SHALL 在调 `list_sessions_skeleton` 拿到当页 items 后，对前 `EAGER_FIRST_PAGE_LIMIT = 20` 条 items 用 `futures::future::join_all` 同步等待 `extract_session_metadata_cached(...)` 完成（每条包 `tokio::time::timeout(EAGER_PER_SESSION_TIMEOUT = 500ms)`，通过共享 `Semaphore(METADATA_SCAN_CONCURRENCY=8)` 限流），将真值字段（`title` / `messageCount` / `isOngoing` / `gitBranch`）填回 `SessionSummary`。`page_size > EAGER_FIRST_PAGE_LIMIT` 时剩余 items SHALL 走骨架 + `scan_metadata_for_page` 后台 spawn + broadcast push 路径（与翻页路径同模型；remainder scan 的 `active_scans` key SHALL 为 `format!("{project_id}|None|remainder")`，与翻页路径 key 命名空间隔离；同 key 的新 spawn SHALL abort 旧 entry——既有 generation token cleanup 模式继承，避免高频 silent refresh 反复 spawn 形成永远跑不完循环）。实现 SHALL NOT 通过 broadcast emit 已 eager 同步等到的 items 对应的 update（前端 IPC response 已含真值）；超时 / `extract_session_metadata_cached` 字段全占位（解析失败）的 items SHALL spawn deferred 单条 retry：500ms 后无 timeout 重新调 `extract_session_metadata_cached`，成功后 emit broadcast 让前端 patch；retry 仍失败保留占位（单条最多 retry 1 次避免 task 泄漏）。失败 metadata SHALL NOT 写入正向 `MetadataCache`（避免占位真值卡住后续 lookup），但 SHALL 写入 `negative_results: Map<sessionId, (FileSignature, Instant)>` 结构记录 60s negative TTL backoff —— 60s 内对同一 sessionId（`FileSignature` 等价）的重复请求 SHALL 跳过解析直接返占位，避免永久损坏 jsonl 在每次 silent refresh / 冷启动持续 spike CPU。**切 project 路径上**：当新 `projectB` 首页 eager 启动时，实现 SHALL 遍历 `active_scans` 并 abort 所有 `entry.project_id != projectB` 的 entry（projectId 由 key 头部 `key.split('|').next()` 解析；项目命名约定 projectId 不含 `|` 字符）。abort 多个旧 project 翻页扫描时让出 semaphore permits 给 `projectB` 首页 eager；abort 后已扫完的真值仍可通过 broadcast 推送（已经在飞的 send 不被 abort 影响），未扫完的取消。
- **翻页路径（`pagination.cursor == Some(_)`）**：行为不变。骨架阶段 SHALL 对每条 `(session_id, jsonl_path)` 先调用 `try_lookup_cached_metadata`（lookup-only fast-path：查 `MetadataCache` + `FileSignature` 等价校验 + `is_session_stale(mtime)` 实时合成 `isOngoing`，**不**触发扫描）。命中条 SHALL 在骨架阶段直接 inline 填回 `title` / `messageCount` / `isOngoing` / `gitBranch` 真实值，且 SHALL NOT 入 `page_jobs`（即不 spawn 后台扫描、不通过 `broadcast::Sender<SessionMetadataUpdate>` 推送对应 update）；未命中场景包括 cache miss、`tokio::fs::metadata` stat 失败、`FileSignature` 不等（mtime / size / identity 任一不等）—— 任一未命中条 SHALL 入 `page_jobs` 走原后台扫描路径，扫完通过 broadcast 推送 update。

骨架阶段的 lookup 并发度（翻页路径）SHALL 通过 `Semaphore(METADATA_SCAN_CONCURRENCY=8)` 限流，与后台扫描使用同一上限常量。后台扫描自身的并发度 SHALL 同样被限流（固定上限 8），避免 50+ 文件同时打开；每次翻页 `list_sessions(projectId, pagination)` 触发新扫描前 SHALL 取消同一 `(projectId, cursor)` 维度上一轮未完成的扫描，避免**同分页**的事件串扰；不同 `cursor` 的扫描 SHALL 并存而互不 abort（典型场景：page 1 与 page 2 的并发扫描相互独立——但首页 cursor=None 路径**不**入此并发模型，因为同步等待已自含等价语义）。扫描范围 SHALL 限定为本次 `list_sessions` 返回页中的**未命中** sessions；实现 MUST NOT 因为请求第一页而后台扫描完整项目历史。

翻页路径上 cache 全命中场景下 `page_jobs.is_empty()` 时 `list_sessions` SHALL 跳过 `tokio::spawn(scan_metadata_for_page(...))` 分支，**不**触碰 `active_scans` 注册表——既有的 abort + generation + insert race-free 抢占逻辑（详见本 spec `Session list pagination avoids duplicate full scans` 与历史 codex 二轮二审）由"cache miss 时进入 spawn 分支"路径自然继承。

`active_scans` 注册表的 key SHALL 为 `(projectId, cursor)` 组合编码字符串（实现以 `format!("{project_id}|{cursor_or_empty}")`，`|` 字符为 reserved 分隔符；当前 cursor 由 offset 数字字符串生成，不会冲突）。同 key 抢占 + per-key generation cleanup 的 race-free 语义不变。

#### Scenario: 首页路径同步等真值不 emit broadcast

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 随后调用 `list_sessions("projectA", { pageSize: 3, cursor: null })`，响应页含 3 个 session（`MetadataCache` cache miss / hit 任意组合）
- **THEN** IPC return 时响应中每条 `SessionSummary` 的 `title` / `messageCount` / `isOngoing` / `gitBranch` SHALL 为真值字段（解析失败的条目保留占位也算预期降级；不要求一定非 null）
- **AND** receiver SHALL 在该次 `list_sessions` 完成后短时间内（如 300 ms）**不**收到任何 `SessionMetadataUpdate`
- **AND** 实现 SHALL NOT 注册任何 `active_scans` entry 也不 spawn 后台 task

#### Scenario: 翻页路径未命中条订阅接收 metadata 更新

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 随后调用 `list_sessions("projectA", { pageSize: 3, cursor: "20" })`（翻页 cursor=Some），响应页包含 3 个 session，**所有** session 在 `MetadataCache` 中均为 miss（如冷启动场景）
- **THEN** receiver SHALL 在扫描完成后**最多**收到 3 条 `SessionMetadataUpdate`，每条携带对应 sessionId 的真实 `title` / `messageCount` / `isOngoing` / `gitBranch`

#### Scenario: Tauri host emit session-metadata-update

- **WHEN** Tauri host 在 `setup` 调用 `subscribe_session_metadata()` 并在后台 task 内订阅
- **AND** 后端产出 `SessionMetadataUpdate { project_id: "p", session_id: "s", title: Some("T"), message_count: 12, is_ongoing: false, git_branch: Some("main") }`
- **THEN** webview SHALL 通过 `listen("session-metadata-update", ...)` 收到 payload `{ projectId: "p", sessionId: "s", title: "T", messageCount: 12, isOngoing: false, gitBranch: "main" }`（camelCase）

#### Scenario: 翻页路径同 projectId 同 cursor 的新扫描取消旧扫描

- **WHEN** `list_sessions("projectA", { pageSize: 20, cursor: "20" })` 正在扫描中（后台有未完成任务，至少一条 cache miss 进入 spawn 分支）
- **AND** 调用方再次调用 `list_sessions("projectA", { pageSize: 20, cursor: "20" })`（**同 cursor**），新页中有 cache miss 条触发新扫描
- **THEN** 旧扫描任务 SHALL 被 abort，未完成的 session 元数据 SHALL NOT 再被推送；新扫描 SHALL 只扫描新响应页中的未命中 sessions

#### Scenario: 翻页路径同 projectId 不同 cursor 的扫描并存互不 abort

- **WHEN** `list_sessions("projectA", { pageSize: 20, cursor: "20" })`（page 2）正在扫描中
- **AND** 调用方紧接着调用 `list_sessions("projectA", { pageSize: 20, cursor: "40" })`（page 3，典型场景：用户连续翻页），新页中有 cache miss 条
- **THEN** page 2 扫描任务 SHALL **继续运行**，page 2 内未完成的 session 元数据 SHALL 通过 broadcast 正常推送；同时 page 3 SHALL 启动独立扫描任务推送其未命中 session 的 update

#### Scenario: 切 project 时新 project 首页 eager abort 所有非当前 project 的翻页扫描

- **WHEN** `list_sessions("projectA", { cursor: "20" })` 与 `list_sessions("projectC", { cursor: "40" })` 后台翻页扫描同时进行中（active_scans 含 key `"projectA|20"` 与 `"projectC|40"`），调用方紧接着调用 `list_sessions("projectB", { cursor: null })`（首页 eager 路径）
- **THEN** 实现 SHALL 在 projectB eager 路径开始 `join_all` 之前遍历 `active_scans`，对每个 entry 解析 key 头部 projectId（`key.split('|').next()`）并 abort 所有 `projectId != "projectB"` 的 entry —— 即 projectA 与 projectC 两个旧扫描都被 abort，让出 semaphore permits
- **AND** 已 emit 的 `SessionMetadataUpdate` 不受影响（已发送到 broadcast channel 的消息不会被 abort 撤回）；未扫完的 sessions SHALL NOT 再被推送
- **AND** 前端 listener 已按 `payload.projectId !== selectedProjectId` 过滤，已 emit 的旧 project update UI 不受影响

#### Scenario: 翻页路径切翻页路径不互相 abort（不同 project）

- **WHEN** `list_sessions("projectA", { cursor: "20" })` 后台翻页扫描进行中，调用方紧接着调用 `list_sessions("projectB", { cursor: "20" })`（**翻页路径**，不是首页）
- **THEN** projectA 的翻页扫描 SHALL **继续运行**至完成，projectA 的 `SessionMetadataUpdate` 仍会被 broadcast；projectB 翻页扫描独立启动；两者并存互不 abort（场景 D4b：abort 仅在新 project **首页** eager 启动时触发）

#### Scenario: 后台扫描并发度限制（翻页路径）

- **WHEN** 翻页路径扫描任务在并发处理某页 50 个 cache-miss session 文件
- **THEN** 同一时刻打开的 JSONL 文件句柄数 SHALL 不超过 8（通过 `tokio::sync::Semaphore` 或等价机制限流）

#### Scenario: 首页同步等真值并发度限制

- **WHEN** `list_sessions("projectA", { pageSize: 50, cursor: null })` 首页路径对 50 个 session 并发执行 `extract_session_metadata_cached`
- **THEN** 同一时刻进行 metadata 解析的 future 数 SHALL 不超过 8（通过与翻页路径共享的 `METADATA_SCAN_CONCURRENCY=8` 上限）

#### Scenario: 翻页路径骨架 lookup 并发度限制

- **WHEN** `list_sessions("projectA", { pageSize: 50, cursor: "50" })` 骨架阶段对 50 个 session 并发执行 `try_lookup_cached_metadata`
- **THEN** 同一时刻进行 `tokio::fs::metadata` stat 的 future 数 SHALL 不超过 8（通过与后台扫描共享的 `METADATA_SCAN_CONCURRENCY=8` 上限）

#### Scenario: 无 watcher 构造器下 subscribe 安全

- **WHEN** `LocalDataApi` 通过不带 watcher 的构造器实例化（集成测试路径）
- **AND** 调用方 `subscribe_session_metadata()`
- **THEN** 返回有效 `broadcast::Receiver`；翻页路径仍能正常推送（broadcast 不依赖 watcher）

#### Scenario: 翻页路径 Cache 命中时骨架直接带值且零 emit

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 已对 "projectA" 调用过若干次 `list_sessions`，期间 `MetadataCache` 已写入若干 session 的元数据
- **AND** 在 session jsonl 文件 mtime/size 未变化的前提下，调用 `list_sessions("projectA", { pageSize: 3, cursor: "20" })`（翻页）
- **THEN** 该次 `list_sessions` 返回的 `SessionSummary[]` SHALL 在骨架阶段直接携带每条的真实 `title` / `messageCount` / `isOngoing` / `gitBranch`（非占位）
- **AND** receiver SHALL 在该次调用后短时间内（如 300 ms）**不**收到任何新的 `SessionMetadataUpdate`

#### Scenario: 翻页路径 Cache 部分命中时未命中条仍走后台扫描

- **WHEN** `list_sessions("projectA", { cursor: "20" })` 骨架阶段对 3 个 session 调用 `try_lookup_cached_metadata`，其中 2 个命中（`FileSignature` 等价）、1 个 miss（jsonl 文件被追加新消息，size 与 mtime 已变更，`FileSignature` 不等）
- **THEN** 返回的 `SessionSummary[]` 中 2 个命中条骨架阶段 SHALL 已带真实元数据，1 个 miss 条骨架阶段 SHALL 仍为占位（`title=null` / `messageCount=0` / `isOngoing=false`）
- **AND** 该 miss 条 SHALL 入 `page_jobs` 走后台扫描，扫完通过 broadcast 推送 1 条 `SessionMetadataUpdate`；receiver 收到的 update 数 SHALL 为 1（只覆盖 miss 条）

#### Scenario: 翻页路径 Cache 全命中时不触发 spawn 不触碰 active_scans

- **WHEN** `list_sessions("projectA", { cursor: "20" })` 骨架阶段对所有 session 都 cache 命中（page_jobs 为空）
- **THEN** 实现 SHALL NOT 调用 `tokio::spawn(scan_metadata_for_page(...))`
- **AND** SHALL NOT 改动 `active_scans` 注册表（既不 abort 旧 entry 也不 insert 新 entry）
- **AND** receiver SHALL 不收到任何对应该次调用的 `SessionMetadataUpdate`

#### Scenario: 翻页路径 lookup stat 失败 fallback 到后台扫描

- **WHEN** `try_lookup_cached_metadata` 内 `tokio::fs::metadata(path).await` 返回 `Err`（罕见 IO 错误）
- **THEN** 函数 SHALL 返回 `None`
- **AND** 该 session SHALL 入 `page_jobs` 走后台扫描，由 `extract_session_metadata_cached` 内部的 uncached 路径处理（详见 `extract_session_metadata` 按 `FileSignature` 缓存 Requirement）

#### Scenario: 首页 eager 单条 metadata 解析超时降级 + deferred retry

- **WHEN** 首页 eager 路径调 `list_sessions("projectA", { pageSize: 20, cursor: null })`，其中某条 jsonl 极大或磁盘 IO 阻塞导致 `extract_session_metadata_cached` 在 `EAGER_PER_SESSION_TIMEOUT = 500ms` 内未完成
- **THEN** `tokio::time::timeout` 返回 `Err(_)`，该条 SHALL 在响应中保留骨架占位字段（`title=null` / `messageCount=0` / `isOngoing=false` / `gitBranch=null`）
- **AND** 实现 SHALL 在 IPC return 之后 spawn 一个**单条** deferred retry future：500ms 延迟后无 timeout 重新调 `extract_session_metadata_cached`，成功后通过 `broadcast::Sender<SessionMetadataUpdate>::send(update)` emit 让前端 patch；retry 失败保留占位
- **AND** 整页 wall 上限 SHALL 为 `ceil(min(page_size, EAGER_FIRST_PAGE_LIMIT) / METADATA_SCAN_CONCURRENCY) * EAGER_PER_SESSION_TIMEOUT + ε`（即 page_size=20 时最多 ~1500ms 含调度开销）

#### Scenario: 首页 eager 单条 metadata 解析失败不写 cache + retry

- **WHEN** 首页 eager 路径某条 jsonl 文件存在但解析失败（`extract_session_metadata_with_ongoing` 返回 `SessionMetadata { title: None, message_count: 0, is_ongoing: false, git_branch: None }` 全占位 fallback）
- **THEN** `extract_session_metadata_cached` SHALL NOT 把该条占位 metadata 写入 `MetadataCache`（避免下次 lookup 命中"占位真值"卡住直到 `FileSignature` 变化）
- **AND** 该条在响应中保留骨架占位 + spawn deferred 单条 retry（与超时降级共享同一 retry 路径）
- **AND** retry 仍失败时 SHALL NOT 反复无限 retry——单条最多 retry 1 次（避免后台 task 泄漏）

#### Scenario: 首页 eager Cache 全命中时同步等真值不 emit

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver，然后调 `list_sessions("projectA", { pageSize: 20, cursor: null })`，该 page 20 条 session 在 `MetadataCache` 中全命中（`FileSignature` 等价）
- **THEN** 响应 items 每条 SHALL 含真值 metadata 字段（cache 路径下解析时间近 0ms）
- **AND** receiver SHALL 在 IPC return 后 300ms 内**不**收到任何 `SessionMetadataUpdate`
- **AND** 整页 wall SHALL 远低于预算（cache 全命中近 instant）

#### Scenario: 首页 eager Cache 部分命中时仍同步等真值

- **WHEN** 首页 eager 路径 20 条 session 中 12 条 cache 命中、8 条 miss（jsonl 文件刚被追加 / mtime 变化 / 首次访问）
- **THEN** 12 条命中条 SHALL 通过 cache 路径瞬时返回真值；8 条 miss 条 SHALL 通过 `extract_session_metadata_cached` 同步解析（每条 timeout 500ms 兜底）
- **AND** receiver SHALL 在 IPC return 后**不**收到 12 条命中条对应的 update（已含真值无需 patch）
- **AND** 8 条 miss 条若解析成功 SHALL 写入 `MetadataCache` 但**不** emit broadcast（与命中条一致；前端 IPC response 已含真值）；解析失败的条目走 deferred retry 路径

#### Scenario: 永久损坏 jsonl 60s negative TTL backoff

- **WHEN** 首页 eager 路径某 sessionId 的 jsonl 文件长期损坏（无法解析 / 字段全占位）
- **AND** 用户连续在 60s 内多次 silent refresh / 切项目重访
- **THEN** 第一次访问失败时 SHALL 把该 sessionId + `FileSignature` + `Instant::now()` 写入 `negative_results` 结构（不写正向 cache）+ spawn 单条 deferred retry（最多 1 次，500ms 延迟后 `bypass_negative=true` 重新调 `extract_session_metadata_cached`）
- **AND** 后续 60s 内对同一 sessionId（`FileSignature` 等价）从**正常 list_sessions 路径**（`bypass_negative=false`）的访问 SHALL 命中 negative TTL backoff —— `extract_session_metadata_cached` 直接返回字段全占位，不再调 `extract_session_metadata_with_ongoing` 重新解析、不再 spawn deferred retry
- **AND** 60s 后 negative TTL 过期，下次正常访问 SHALL 重新尝试解析；如果 jsonl 已被修复，写入正向 cache + 移除 negative_results entry
- **AND** 如果 jsonl 文件被外部 mtime/size 修改（`FileSignature` 不等），SHALL 立即跳出 negative TTL 重新尝试解析（FileSignature 等价检查在 TTL 之前）

#### Scenario: deferred retry 通过 bypass_negative 不被自己写入的 negative TTL block

- **WHEN** 首页 eager 路径某条 metadata 解析失败（如短暂 IO 抖动 / partial write），写入 negative_results + spawn deferred retry
- **AND** 500ms 后 deferred retry 触发，调 `extract_session_metadata_cached(.., bypass_negative=true)`
- **THEN** retry 路径 SHALL **跳过** negative_results 检查直接调 `extract_session_metadata_with_ongoing` 重新解析；不会因为刚刚写入的 negative TTL 直接返占位（codex v3 复审 issue 3）
- **AND** 如果 retry 解析成功 → 移除 negative_results 中该 sessionId entry + 写入正向 cache + emit broadcast；前端 patch UI
- **AND** 如果 retry 仍失败 → 重写 negative_results（更新 `Instant::now()` 续 60s）；不再 spawn 第二次 retry
- **AND** 短暂故障（IO 抖动 / partial write 期间被读到）能通过 deferred retry 恢复；永久损坏 jsonl 在 retry 失败后才进入稳态 60s backoff

#### Scenario: page_size > 20 remainder scan 同 key dedupe 不反复 spawn

- **WHEN** 用户高频触发 silent refresh（典型场景：file-change 100ms debounce + 多次连续触发），每次都 `listSessions(projectId, pageSize=50, cursor=null)`
- **THEN** 后端 `active_scans` 中**始终**至多有 1 个 `format!("{project_id}|None|remainder")` key 的 entry —— 同 key 新 spawn SHALL abort 旧 entry（既有 generation token cleanup 模式继承）
- **AND** 最后一次 silent refresh 触发的 remainder scan 最终能完成；中间被 abort 的 scan SHALL NOT 永久挂起或泄漏 task
- **AND** receiver SHALL 收到最终 remainder scan emit 的 SessionMetadataUpdate；前端按既有 race buffer 兜底

### Requirement: Session list pagination avoids duplicate full scans

IPC clients that need all sessions for a project SHALL consume `list_sessions` through cursor pagination without re-requesting an already returned page as part of a larger full-list request.

The `list_sessions` response shape MUST follow the cursor-based dispatch defined in `Expose project and session queries`:

- **First page (`pagination.cursor == None`)**: response items SHALL contain real metadata values for the first `EAGER_FIRST_PAGE_LIMIT = 20` entries (synchronously awaited per-item with `tokio::time::timeout(EAGER_PER_SESSION_TIMEOUT = 500ms)`); per-item timeout / parse failure entries SHALL keep skeleton placeholders and be patched later via deferred broadcast retry. When `pagination.page_size > EAGER_FIRST_PAGE_LIMIT`, the remainder SHALL fall back to skeleton placeholders + `scan_metadata_for_page` background spawn + `session-metadata-update` broadcast (same model as paged path), so HTTP clients with large page_size still get real metadata for the visible 20 entries within the wall budget.
- **Subsequent pages (`pagination.cursor == Some(_)`)**: response items MAY remain skeleton with placeholder metadata; the existing skeleton-first contract holds — `SessionSummary` entries omit expensive metadata fields and `session-metadata-update` events fill them later via background scan + broadcast.

#### Scenario: Client accumulates pages without restarting from the first page

- **WHEN** a project has more sessions than the initial client page size and `list_sessions` returns a non-null `nextCursor`
- **THEN** the client requests the next page using that cursor and appends the new sessions to the already returned sessions
- **AND** the client does NOT issue a second request from the beginning with `pageSize = total`

#### Scenario: First page response carries real metadata for entries within EAGER_FIRST_PAGE_LIMIT

- **WHEN** caller invokes `list_sessions("projectA", { pageSize: 20, cursor: null })`
- **AND** all 20 sessions complete `extract_session_metadata_cached` within `EAGER_PER_SESSION_TIMEOUT` without parse failure
- **THEN** all 20 entries in the response SHALL contain real `title` / `messageCount` / `isOngoing` / `gitBranch` values
- **AND** no `session-metadata-update` broadcast SHALL be emitted for those 20 entries

#### Scenario: First page page_size > EAGER_FIRST_PAGE_LIMIT splits eager and skeleton

- **WHEN** caller invokes `list_sessions("projectA", { pageSize: 50, cursor: null })`
- **THEN** the first 20 entries (sorted by timestamp desc) SHALL contain real metadata values (eager-awaited);
- **AND** the remaining 30 entries SHALL contain skeleton placeholder metadata fields (`title=null` / `messageCount=0` / `isOngoing=false` / `gitBranch=null`)
- **AND** the skeleton 30 entries SHALL be patched later via `session-metadata-update` broadcast emitted from `scan_metadata_for_page` background task (same as `cursor=Some` path)

#### Scenario: Subsequent pages keep skeleton-first contract

- **WHEN** caller invokes `list_sessions("projectA", { pageSize: 20, cursor: "20" })`
- **THEN** response entries MAY remain skeleton `SessionSummary` with placeholder metadata fields
- **AND** later `session-metadata-update` events SHALL patch `title`, `messageCount`, `isOngoing`, and `gitBranch` in place

### Requirement: List sessions uses project-scoped light pagination

`list_sessions(projectId, pagination)` SHALL act as a project-scoped cursor pagination API for session list UI. The synchronous response shape MUST only require lightweight fields that can be obtained without parsing session contents at the framing level: `sessionId`, `projectId`, `timestamp`, and pagination metadata.

The metadata fields (`title`, `messageCount`, `isOngoing`, `gitBranch`) of each `SessionSummary` SHALL follow the cursor-based dispatch defined in `Expose project and session queries`:
- **First page (`pagination.cursor == None`)**: response items MUST already contain real metadata values (`title` may be `None` only when the jsonl genuinely has no user message — this is the real value, not a placeholder). Implementations SHALL synchronously await per-item `extract_session_metadata_cached` before IPC return.
- **Subsequent pages (`pagination.cursor == Some(_)`)**: response items MAY contain placeholder metadata fields and the real values SHALL be filled later through `session-metadata-update` broadcast events.

`list_sessions` SHALL NOT require callers to compute or consume an exact total count for the session list first page. Pagination continuation MUST be driven by `nextCursor` / equivalent `hasMore` semantics. If the response type keeps a `total` field for compatibility, callers SHALL treat it as informational and MUST NOT rely on it being a complete project count unless a future dedicated count API states so.

#### Scenario: first page returns real metadata

- **WHEN** caller invokes `list_sessions("projectA", { pageSize: 20, cursor: null })`
- **THEN** response SHALL contain at most 20 `SessionSummary` items for `projectA`
- **AND** each item SHALL contain real `sessionId`, `projectId`, and `timestamp`
- **AND** each item SHALL contain real `title` / `messageCount` / `isOngoing` / `gitBranch` values (not placeholder zeros / nulls), unless the underlying jsonl genuinely has no user message (`title` may legitimately be `null` in that case) or per-session metadata extraction failed (graceful degradation to placeholders for that single item)

#### Scenario: subsequent pages may return placeholder metadata

- **WHEN** caller invokes `list_sessions("projectA", { pageSize: 20, cursor: "20" })`
- **THEN** response SHALL contain at most 20 `SessionSummary` items
- **AND** each item SHALL contain real `sessionId`, `projectId`, and `timestamp`
- **AND** each item MAY contain placeholder `title = null`, `messageCount = 0`, `isOngoing = false`, and `gitBranch = null` — real metadata SHALL be pushed via `session-metadata-update` events

#### Scenario: continuation uses cursor not total

- **WHEN** first `list_sessions("projectA", { pageSize: 20, cursor: null })` response contains `nextCursor`
- **THEN** caller SHALL request the next page with that cursor
- **AND** caller SHALL NOT need an exact total count to continue pagination

#### Scenario: pageSize zero is rejected

- **WHEN** caller invokes `list_sessions("projectA", { pageSize: 0, cursor: null })`
- **THEN** API SHALL return a validation error instead of silently clamping the page size
