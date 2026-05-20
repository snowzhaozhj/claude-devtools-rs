## MODIFIED Requirements

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

