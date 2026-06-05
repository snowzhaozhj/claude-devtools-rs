## MODIFIED Requirements

### Requirement: Expose project and session queries

系统 SHALL 在请求 / 响应式 IPC 通道上暴露项目与会话相关数据查询，至少包括：列项目、列项目下 sessions（含分页）、取 session 详情、取 session metrics、取 waterfall 数据、取 subagent 详情。

`get_session_detail` SHALL 在返回 session 详情时集成 subagent 解析：**从主 session 所在 `projects_dir`（即 `~/.claude/projects/` 或 SSH 远端等价路径）下所有 project 目录扫描 `{rootSessionId}/subagents/agent-*.jsonl`（新结构）**，合并去重后填充 `AIChunk.subagents` 字段。旧结构（flat `{project_dir}/agent-*.jsonl`）SHALL 保持只扫描主 `project_dir` 并按首行 `parentUuid` / `sessionId` 字段过滤。若扫描失败或无候选，`subagents` SHALL 为空数组（不报错）。跨目录扫描开关设为 `false` 时 SHALL 退回"只扫主 `project_dir`"的原行为。

**`LocalDataApi::get_session_detail` SHALL 返回完整（未裁剪）的 `SessionDetail`——payload omission 是展示关注点（presentation concern），MUST NOT 在数据层执行**。`compact_derived` 字段填充（`phase_number` / `token_delta`）属于数据补全，SHALL 保留在 `get_session_detail` 内部。

**Tauri IPC `get_session_detail` command handler SHALL 在获得完整 `SessionDetailResponse` 后、序列化返回前端之前，调用 `apply_display_omissions` 执行以下裁剪**（行为与旧版 data layer 内裁剪完全一致）：

- `AIChunk.subagents[i].messages` 数组裁剪为空 Vec，且 `messagesOmitted=true` —— 用于把首屏 IPC payload 控制在原大小约 40%。`Process.headerModel` / `Process.lastIsolatedTokens` / `Process.isShutdownOnly` 三个 derived 字段 MUST 在候选转换阶段由 messages 预算后填充，让 SubagentCard header 不依赖完整 `messages` 也能正常渲染。subagent 消息裁剪开关设 false 时 SHALL 退回完整 payload（messages 不裁剪、`messagesOmitted=false`）。

- `ContentBlock::Image.source.data` 字段替换为空字符串 `""`，且同时设 `source.dataOmitted=true` —— 用于把首屏 IPC payload 中内联截图的 base64 字符串裁掉（行为契约见本 spec `Lazy load inline image asset` Requirement）。`source.kind` / `source.media_type` 字段 SHALL 保留，仅 `data` 字段被清空。图片数据裁剪开关设 false 时 SHALL 退回完整 base64 payload（`data` 保留原值、`dataOmitted=false`）。该裁剪 SHALL 应用于所有 chunk 类型（UserChunk / AIChunk responses / subagent.messages 内嵌套——但 subagent.messages 默认已被裁剪，仅在回滚 subagent 消息裁剪时才会触及嵌套层）。

- `AIChunk.responses[i].content` 字段替换为空文本内容，且同时设 `contentOmitted=true` —— 用于把首屏 IPC payload 中最大单一字段裁掉。该字段在前端任何代码路径中都不被读取（chunk 显示文本走 `semanticSteps` 的 thinking / text 步骤），裁剪后 UI 渲染零变化。response content 裁剪开关设 false 时 SHALL 退回完整 payload（`content` 携带原值、`contentOmitted=false`）。该裁剪 SHALL 应用于顶层 AIChunk 及 subagent.messages 嵌套层（与图片数据裁剪同模式）。

- `AIChunk.tool_executions[i].output` 内 `text` / `value` 字段替换为空（Text 变空字符串 / Structured 变 Null / Missing 不变），且同时设 `outputOmitted=true` —— 用于把首屏 IPC payload 中 tool 输出裁掉（行为契约见本 spec `Lazy load tool output` Requirement）。`output` enum 的 variant kind SHALL 保留，仅内层 `text` / `value` 被清空。tool output 裁剪开关设 false 时 SHALL 退回完整 payload（`output` 内字段保留原值、`outputOmitted=false`）。该裁剪 SHALL 应用于顶层 AIChunk 及 subagent.messages 嵌套层（与其它裁剪同模式）。

**tool output 裁剪路径下 `ToolExecution.outputBytes: Option<u64>` MUST 在清空 output 之前按 variant 记录原始字节长度**（Text → 文本字节长度、Structured → JSON 序列化后字节长度、Missing → 不填保持 `None`），让前端在懒加载之前即可估算 output token 数（按 `outputBytes / 4` 启发式），从而 BaseItem 头部 token 显示 SHALL **在懒加载展开前后保持一致**。tool output 裁剪关闭时 `outputBytes` SHALL 保持 `None`（前端 fallback 到直接读文本长度）。解析层 SHALL **不**主动填充 `outputBytes`——该字段仅在 omission 裁剪层语义有意义。

**非 Tauri IPC 消费者（MCP server / CLI / HTTP route）SHALL 获得完整未裁剪数据**——它们直接消费 `LocalDataApi::get_session_detail` 返回值，无需额外处理。HTTP 路径同样 SHALL NOT 应用裁剪（HTTP 当前无活跃用户、且无对应 asset 协议端点 / 懒拉接口，保留完整 payload 传输）。

**`apply_display_omissions` SHALL 作为 `cdt_api` crate 的 public API 导出**（路径 `cdt_api::ipc::apply_display_omissions`），签名为 `pub fn apply_display_omissions(chunks: &mut Vec<Chunk>)`，供 `src-tauri` Tauri command handler 调用。

`list_sessions` 返回的每个 `SessionSummary` MUST 携带 `sessionId` / `projectId` / `timestamp` 的真实值（可直接从目录扫描得出），但 `title` / `messageCount` / `isOngoing` SHALL 允许为占位值（`null` / `0` / `false`）——这些元数据字段的真实值由后端异步扫描后通过 `session-metadata-update` push event 逐条推送（见本 spec `Emit session metadata updates` Requirement）。`get_session_detail` 返回的 `SessionDetail.isOngoing` 仍 MUST 为同步计算后的真实值（因为 detail 已在调用链内完成全文件解析）。

**`isOngoing` 真实值 SHALL 由两路 AND 计算**：(a) 结构性活动栈五信号判定返回 `true`，**且** (b) session JSONL 文件 mtime 距当前时刻 `< 5 分钟`。任一条件不满足时 `isOngoing` MUST 为 `false`。stale 阈值为 5 分钟（对齐原版 TS 实现的同名常量，覆盖 CLI 异常退出时活动栈误判 ongoing 的场景——mtime 兜底纠正）。`list_sessions` 异步扫描路径与 `get_session_detail` 同步路径行为 MUST 一致；HTTP `GET /api/projects/{projectId}/sessions` 路径共用同一元数据提取实现（详见本 spec §"HTTP `list_sessions` 复用 IPC 骨架 + push 实现"），自动适用。stat 失败时 SHALL 保守保留活动栈判定（避免 fs 偶发错误把活跃 session 错判 dead）；时钟回拨导致 mtime > now 时 SHALL 判 not stale（避免未来 mtime 把活跃 session 误判 dead）。

序列化 SHALL 使用 camelCase（`isOngoing`、`messagesOmitted`、`headerModel`、`lastIsolatedTokens`、`isShutdownOnly`、`dataOmitted`、`contentOmitted`、`outputOmitted`、`outputBytes`）。例外：`ImageSource.media_type` 与 `ImageSource.type`（kind）保持 snake_case，与上游 Anthropic JSONL 格式一致——同 `TokenUsage` 例外。

**HTTP `GET /api/projects/{projectId}/sessions` 路径 SHALL 与 IPC `list_sessions` 共用骨架 + push 实现**——即 HTTP handler SHALL 调会话列表查询接口（骨架快返 + 缓存元数据快速路径 + spawn 后台扫描 + 事件广播通道 emit），**不**得调同步全扫描接口。后台扫描产物 SHALL 通过 HTTP 桥接层转换为 SSE 推送到 `/api/events`，浏览器 client 按 `session-metadata-update` event 收到与 IPC 路径同形的 patch。

同步全扫描 trait method SHALL 保留作为 trait 默认 fallback（供未来非 SSE-aware HTTP client 或 CLI 直接 trait 调用使用），但 HTTP route 实现 **不**得再调用它。

#### Scenario: get_session_detail 返回完整未裁剪数据

- **WHEN** 任意消费者调 `LocalDataApi::get_session_detail(project_id, session_id, None)`
- **AND** session 包含含 tool output text 的 ToolExecution
- **THEN** 返回的 `SessionDetail.chunks` 中该 ToolExecution 的 `output.text` SHALL 为完整原始内容
- **AND** `outputOmitted` SHALL 为 `false`
- **AND** `outputBytes` SHALL 为 `None`

#### Scenario: Tauri IPC handler 裁剪后返回 omitted 数据

- **WHEN** 前端通过 Tauri IPC 调用 `get_session_detail` command
- **THEN** Tauri command handler SHALL 先调 `LocalDataApi::get_session_detail` 获取完整数据
- **AND** 再调 `apply_display_omissions` 裁剪 chunks
- **AND** 返回给前端的 payload 中 `outputOmitted` / `contentOmitted` / `dataOmitted` / `messagesOmitted` SHALL 为 `true`（与旧版行为一致）

#### Scenario: MCP consumer 获得完整 tool output

- **WHEN** MCP server 调 `get_session_detail` 获取 session 数据用于 grep
- **THEN** 返回的 chunks 中 `ToolExecution.output.text` SHALL 为完整原始内容（未裁剪）
- **AND** grep 匹配 SHALL 能命中 tool output 中的内容

#### Scenario: outputBytes filled before trim under OMIT_TOOL_OUTPUT

- **WHEN** Tauri IPC handler 的 `apply_display_omissions` 裁剪路径触发处理一个 `ToolExecution`
- **AND** 该 `ToolExecution.output` 是 `Text { text: "abcde" }`（5 字节）
- **THEN** 处理后 `output.text` SHALL 为 `""`、`outputOmitted` SHALL 为 `true`、`outputBytes` SHALL 为 `Some(5)`

#### Scenario: outputBytes for structured uses serialized length

- **WHEN** Tauri IPC handler 裁剪处理 `Structured { value: {"stdout": "ok", "exit": 0} }`
- **THEN** `outputBytes` SHALL 为 `Some(JSON 序列化后的字节长度)`，`output.value` SHALL 为 `Null`

#### Scenario: outputBytes none for missing variant

- **WHEN** Tauri IPC handler 裁剪处理 `output: Missing`
- **THEN** `outputBytes` SHALL 保持 `None`、`output` 不变

#### Scenario: BaseItem token count stable across expand

- **WHEN** 前端 `BaseItem` 渲染一条 `outputOmitted=true` 的 tool 行
- **AND** 用户点击展开触发懒加载，展开后 `output.text` 替换为完整原始内容
- **THEN** 头部 token badge 显示的数字 SHALL **在展开前后相等**（前端在懒加载前从 `outputBytes` 估算、懒加载后从 `outputBytes` 读取——两次结果一致）

#### Scenario: get_session_detail 跨 project_dir 装载 subagent
- **WHEN** caller 调 `get_session_detail(A, S)`，A 是主 `project_id`，S 是 root session id
- **AND** subagent JSONL 物理位于 `project_dir = B`（`B/S/subagents/agent-<subUuid>.jsonl`）
- **THEN** 返回 `SessionDetail.chunks` 内对应 Task tool_use 的 `AIChunk.subagents` SHALL 含 `Process { session_id: <subUuid>, ... }`
- **AND** subagent 关联三阶段 fallback SHALL 正常评估，与"主 project_dir 自带 subagent"等价

#### Scenario: 跨目录扫描开关=false 回滚到原行为
- **WHEN** 跨目录 subagent 扫描开关设为 false
- **AND** subagent JSONL 位于非主 `project_dir`
- **THEN** `get_session_detail` SHALL NOT 装载该 candidate，对应 Task SHALL 保留为未解析（原行为）

#### Scenario: HTTP list_sessions 走骨架而非 sync

- **WHEN** 客户端发起 `GET /api/projects/{projectId}/sessions?pageSize=N&cursor=C`
- **THEN** HTTP handler SHALL 调会话列表查询接口（**不**得调同步全扫描接口）
- **AND** 响应 body SHALL 是骨架 `PaginatedResponse<SessionSummary>`：每条 `SessionSummary` 的 `sessionId` / `projectId` / `timestamp` SHALL 为真实值；`title` / `messageCount` / `isOngoing` / `gitBranch` SHALL 允许为占位值（除非缓存元数据快速路径命中可直接 inline 填回真值）

#### Scenario: HTTP list_sessions 后台扫描产物经 SSE 推送

- **WHEN** HTTP `list_sessions` 返回骨架后，后台元数据扫描任务对 cache miss 的 session 完成扫描并通过事件广播通道发送 update
- **THEN** 该 update SHALL 通过 HTTP 桥接层转换为 `PushEvent::SessionMetadataUpdate { projectId, sessionId, title, messageCount, isOngoing, gitBranch }` 推送到所有 `/api/events` 客户端
- **AND** 浏览器 client 传输层 SHALL 按既有归一化路径转交 `session-metadata-update` 事件给 listener，与 IPC 路径行为一致
