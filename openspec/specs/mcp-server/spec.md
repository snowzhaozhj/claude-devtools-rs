# mcp-server Specification

## Purpose
定义 MCP stdio server 的工具集、传输协议、安全策略（secret redaction）和输出契约。6 个 read-only 意图导向工具覆盖 session 列举、复合查看、chunk 取数、全文搜索、聚合统计。
## Requirements
### Requirement: MCP stdio transport

`cdt mcp serve` SHALL 启动 MCP stdio server（JSON-RPC over stdin/stdout），使用 rmcp SDK 的 `transport::stdio()` 机制。

Server SHALL 在 `get_info()` 中返回：
- `ServerCapabilities` 启用 `tools`
- `instructions` 字段引导 AI 按意图选择工具（决策树式）
- `Implementation` 带版本号

tracing/log 输出 SHALL 仅写入 stderr，stdout 专用于 JSON-RPC 通道。

#### Scenario: MCP server 启动并响应 initialize

- **GIVEN** `cdt mcp serve` 进程已启动
- **WHEN** 客户端发送 `initialize` JSON-RPC 请求
- **THEN** 服务端 SHALL 返回 `ServerInfo` 含 tools capability 和 instructions

#### Scenario: stdout 不含非 JSON-RPC 内容

- **GIVEN** MCP server 正在运行且有 tool 调用
- **THEN** stdout 输出的每一行 SHALL 是合法 JSON-RPC message

### Requirement: Read-only tool set

MCP server SHALL 暴露以下 7 个 tools，全部标注 `readOnlyHint=true`、`destructiveHint=false`、`idempotentHint=true`：`list_projects`、`list_sessions`、`get_session`、`get_turn`、`get_tool_output`、`search`、`get_stats`。

所有 tools SHALL 返回 JSON 结构化数据（不返回纯文本大段 dump）。turn/step 数据形状、截断规则、分页字段由 `[[session-turn-view]]` owner，本工具集引用其契约。CLI 与 MCP 的数据参数集 SHALL 完全一致（CLI 仅额外提供终端渲染 flags）。

#### Scenario: list_projects 返回项目列表

- **GIVEN** 用户有至少一个 Claude Code 项目
- **WHEN** AI 调用 `list_projects` tool
- **THEN** 返回 SHALL 包含 `name`、`path`、`sessions`、`lastActive` 字段的 JSON 数组

#### Scenario: list_sessions 全局跨项目查询

- **WHEN** AI 调用 `list_sessions` 不传 `project`，传 `since="yesterday"`
- **THEN** 返回 SHALL 含所有项目中时间范围内的 session 列表
- **AND** 每条 SHALL 含 `sessionId`、`projectName`、`title`、`messageCount`、`timestamp`、`gitBranch`、`isOngoing`、`filesTouched`
- **AND** 参数 SHALL 限于 `project`、`since`、`until`、`grep`、`pageSize`、`cursor`（不含 `branch`、`is_ongoing`、`group_by`——消费者从返回数据自行过滤/分组）

#### Scenario: get_session 返回 compact overview

- **GIVEN** 一个含多个 turn 的 session
- **WHEN** AI 调用 `get_session` 传 `session`
- **THEN** 返回 SHALL 含 session 级 `sessionId`、`model`、`totalCost`、`durationMs`、`filesTouched`、`userIntents`
- **AND** SHALL 含 `turns` 数组，每个 turn 含 `index`、`question`、`answer`、`tools`（按工具名聚合，每项 `name`/`count`/`errorCount`）、`stepsCount`、`metrics`
- **AND** SHALL 含统一分页字段 `total` 与（如有下一页）`nextCursor`
- **AND** 参数 SHALL 限于 `session`、`grep`、`pageSize`、`cursor`（不含 `project`——服务端自动解析；不含 `include`）

#### Scenario: get_session grep 命中标注

- **WHEN** AI 调用 `get_session` 传 `grep` 且某 turn 的 chunks 内容（含 AI 响应 / tool name / tool input / tool output / error / 用户消息）命中
- **THEN** 返回 SHALL 只含命中的 turn 且每个命中 turn 含 `matchedIn`
- **AND** `matchedIn` SHALL 标注命中来源：工具命中为 `"tool:<toolName>"`（如 `"tool:Read"`）、AI 响应文本为 `"answer"`、用户消息为 `"question"`、thinking 为 `"thinking"`、error 为 `"error"`；多处命中取优先级最高的（tool > error > thinking > answer > question）

#### Scenario: get_turn 返回单 turn 完整 steps

- **WHEN** AI 调用 `get_turn` 传 `session` 与 `turn`
- **THEN** 返回 SHALL 含该 turn 的 `question`、`answer`、`metrics` 与有序 `steps`（thinking/text/tool/subagent 等）
- **AND** tool step 的 output ≥5KB 时 SHALL 截断并标 `outputTruncated`/`outputBytes`
- **AND** steps 超过单页上限时 SHALL 用 `total`+`nextCursor` 分页，支持 `pageSize` 参数

#### Scenario: get_tool_output 取完整原文

- **WHEN** AI 对某被截断的 tool step 调用 `get_tool_output` 传 `session` 与 `toolUseId`
- **THEN** 返回 SHALL 含该工具调用的完整未截断 output

#### Scenario: search 返回 turn 级命中

- **WHEN** AI 调用 `search` 传 `query`
- **THEN** 返回 SHALL 为 turn 级命中列表，每条含 `sessionId`、`turnIndex`、`question`、`matchSnippet`、`timestamp`、`projectName`
- **AND** AI SHALL 能用返回的 `turnIndex` 直接调 `get_turn` 钻取

#### Scenario: session='latest' 解析

- **WHEN** AI 调用 `get_session` 或 `get_turn` 传 `session="latest"`
- **THEN** 服务端 SHALL 解析为最近一次 session（按 timestamp 降序第一条）

### Requirement: Secret redaction

MCP server SHALL 默认对 tool 返回内容进行 secret pattern 脱敏——匹配的 secret 替换为 `[REDACTED]`，返回体附加 `redacted: true` 和 `redactedCount: N`。

支持 `--allow-sensitive` 启动参数跳过 redaction。

脱敏 SHALL 作用于响应的结构化表示（对 JSON 字符串叶子值与对象 key 内的 secret 做替换），SHALL NOT 破坏响应的 JSON 结构：脱敏后返回体始终是合法 JSON，未命中 secret 的字段完整保留，仅命中的 secret 子串被替换为 `[REDACTED]`。

#### Scenario: API key 被自动脱敏

- **GIVEN** session 内容包含 `sk-ant-api03-xxxxxxxxxxxx`
- **WHEN** AI 调用 `get_session_chunks` 获取该段内容
- **THEN** 返回中该 key SHALL 被替换为 `[REDACTED]`
- **AND** 返回体 SHALL 含 `redacted: true`

#### Scenario: allow-sensitive 跳过脱敏

- **GIVEN** MCP server 以 `--allow-sensitive` 启动
- **WHEN** AI 调用任意 tool
- **THEN** 返回内容 SHALL 不做脱敏处理

#### Scenario: 脱敏不破坏响应 JSON 结构

- **GIVEN** 某字符串字段的值包含 secret（如 `password=hunter2`），且其后紧邻其他字段
- **WHEN** AI 调用返回该内容的任意 tool（默认脱敏开启）
- **THEN** 返回体（含 `{data, redacted: true, redactedCount: N}` 包裹）SHALL 是合法 JSON
- **AND** 命中的 secret 子串 SHALL 被替换为 `[REDACTED]`
- **AND** 同一响应中其余未命中 secret 的字段 SHALL 完整保留、值不被截断

### Requirement: MCP 注册

`cdt setup mcp` SHALL 调用 `claude mcp add -s <scope> cdt-devtools -- <cdt-path> mcp serve` 注册 MCP server 到 Claude Code。

`--scope` 支持 `local`（默认）/ `project` / `user`。`--dry-run` 时仅打印将执行的命令。

#### Scenario: setup mcp 注册到 Claude Code

- **WHEN** 用户运行 `cdt setup mcp`
- **THEN** SHALL 执行 `claude mcp add -s local cdt-devtools -- <cdt-path> mcp serve`
- **AND** 注册成功时 SHALL 输出确认信息

#### Scenario: setup mcp --dry-run 仅打印

- **WHEN** 用户运行 `cdt setup mcp --dry-run`
- **THEN** SHALL 仅打印将执行的命令，不实际注册

### Requirement: search_sessions 支持 session 参数

`search_sessions` MCP tool SHALL 接受可选的 `session` 参数。当提供时，搜索范围 SHALL 限定到该 session（委托给 `[[session-search]]` 的 intra-session search 能力）。

#### Scenario: MCP search_sessions 带 session 参数
- **WHEN** 调用 `search_sessions` 时 session 为 "908b77f7" 且 query 为 "mw switch"
- **THEN** 返回结果 SHALL 只包含该 session 的命中
- **AND** sessions_searched SHALL 为 1

### Requirement: session summary 包含 toolActivity

`get_session` 的返回 SHALL 包含 `toolActivity` 字段，结构化展示该会话中工具执行的确定性摘要：

- `topCommands`：出现次数最多的 Bash 命令（top 20），每条截断到 200 字符首行，附执行次数
- `topFiles`：被 Edit/Write/Read 操作的文件路径（top 20），附操作次数
- `gitOps`：git 相关操作摘要（top 10），附次数
- `cliTools`：检测到的 CLI 工具名列表（从 Bash command 第一个 token 提取，去重）
- `totalToolExecutions`：工具执行总数
- `omittedCount`：被截断未列出的条目数

#### Scenario: summary 包含 Bash 命令摘要
- **WHEN** 会话包含 50 次 Bash tool 调用
- **THEN** `toolActivity.topCommands` SHALL 包含出现最频繁的命令（最多 20 条），每条附 count

#### Scenario: summary 包含操作文件列表
- **WHEN** 会话包含 Edit/Write/Read 操作
- **THEN** `toolActivity.topFiles` SHALL 包含被操作的文件路径（最多 20 条），附操作次数

#### Scenario: summary 包含 CLI 工具检测
- **WHEN** 会话中 Bash 命令含 "mw switch get" 和 "git commit" 和 "a1 deploy"
- **THEN** `toolActivity.cliTools` SHALL 包含 "mw"、"git"、"a1"

### Requirement: Content mode 视图层

MCP server 的 `get_session_chunks` tool 的 `content_mode` 行为 SHALL 保持不变。

视图层实现（`ChunkView`/`ToolExecView`/`ResponseView`/`ContentField`/`ContentMode`/`build_chunk_view()`/`summarize_input()`）SHALL 提取到共享模块 `crate::view`，MCP handler 和 CLI 通过引用共享模块使用。

提取后 MCP output 的 JSON 结构 SHALL 与提取前完全一致（字段名、字段顺序、omit/full 行为不变）。

#### Scenario: MCP get_session_chunks content_mode=omit 行为不变

- **WHEN** MCP client 调用 `get_session_chunks` with `content_mode: "omit"`
- **THEN** 返回的 ChunkView 结构 SHALL 与重构前一致
- **THEN** tool execution 的 `inputSummary`、`outputChars`、`outputOmitted` 字段 SHALL 保持不变

#### Scenario: MCP get_session_chunks content_mode=full 行为不变

- **WHEN** MCP client 调用 `get_session_chunks` with `content_mode: "full"`
- **THEN** 返回的 ChunkView 结构 SHALL 与重构前一致

#### Scenario: grep hit auto-expand 行为不变

- **WHEN** MCP client 调用 `get_session_chunks` with `content_mode: "omit"` 和 `grep: "keyword"`
- **THEN** grep 命中 chunk SHALL auto-expand 为 full，context chunk 保持 omit

### Requirement: 时间表达式解析

MCP server 所有接受时间参数（`since`/`until`）的 tools SHALL 支持以下三类格式：

1. **相对时长**：`'7d'`/`'24h'`/`'1h'`/`'30m'` — 以 UTC 当前时间为基准向前偏移
2. **命名周期**：`'today'`/`'yesterday'`/`'week'` — 以本地时区（`chrono::Local`）解析日历边界
3. **绝对日期**：`'2026-06-06'`（NaiveDate，按本地时区转 epoch）/ ISO 8601 完整格式

解析失败时 SHALL 返回 `invalid_params` 错误，含提示文本列出合法格式。

`until` 参数 SHALL 暴露给 `list_sessions` 和 `search_sessions`（底层 `SessionListFilter.until` 已实现）。

#### Scenario: since='yesterday' 解析为本地时区昨日

- **WHEN** 本地时区为 UTC+8，当前时间为 2026-06-07 10:00 CST
- **AND** AI 调用 `list_sessions` 传 `since="yesterday"`
- **THEN** 系统 SHALL 将 since 解析为 2026-06-06 00:00:00 CST（即 2026-06-05T16:00:00Z）

#### Scenario: since 绝对日期

- **WHEN** AI 调用 `list_sessions` 传 `since="2026-06-01"`
- **THEN** 系统 SHALL 将 since 解析为 2026-06-01 00:00:00 本地时区

#### Scenario: until 参数过滤

- **WHEN** AI 调用 `list_sessions` 传 `since="2026-06-01"` 和 `until="2026-06-03"`
- **THEN** 返回 SHALL 只包含 timestamp 在 [6月1日 00:00, 6月3日 00:00) 本地时区范围内的 session

#### Scenario: 非法时间格式报错

- **WHEN** AI 调用 `list_sessions` 传 `since="last month"`
- **THEN** SHALL 返回 `invalid_params` 错误
- **AND** 错误信息 SHALL 包含合法格式示例

### Requirement: 数据完整性

MCP server 所有 tools SHALL 遵循以下数据完整性规则：

1. **列表截断**：通过 `limit` + `hasMore` + `total` 控制返回条数，每条记录本身 SHALL 完整返回
2. **超长文本摘要**：当文本字段超过实现定义的阈值时，SHALL 使用 head + "…" + tail 格式保留首尾内容（不做硬截断丢弃）；SHALL 标记 `messageSummarized: true`；SHALL 保留 `chunkIndex` 供 agent 通过 `get_session_chunks` 取全文
3. **单条记录不切半**：任何返回的 JSON 对象/数组元素 SHALL 是结构完整的，不出现 field 被切到一半的情况

#### Scenario: 超长 errorMessage 使用 head+tail 摘要

- **GIVEN** 某 tool 执行的 errorMessage 超过阈值（如含 5000 字符 stack trace）
- **WHEN** AI 通过 `get_session` 获取该 session 的 errors
- **THEN** 该 error 的 `errorMessage` SHALL 包含开头部分 + "…" + 结尾部分
- **AND** SHALL 标记 `messageSummarized: true`
- **AND** SHALL 包含 `chunkIndex` 字段

#### Scenario: agent 通过 chunkIndex 获取全文

- **GIVEN** get_session 返回的某 error 标记 `messageSummarized: true` 且 `chunkIndex=42`
- **WHEN** AI 调用 `get_session_chunks(session=X, range="42:43", content_mode="full")`
- **THEN** 返回 SHALL 包含该 chunk 的完整 errorMessage（不做摘要）

### Requirement: project 参数可省略

所有接受 `project` 参数的 tools（`list_sessions`、`get_session`、`get_session_chunks`、`search_sessions`、`get_stats`）SHALL 将 `project` 视为可选参数。当 `project` 省略时：

- `list_sessions`/`get_stats`：SHALL 跨所有项目执行查询
- `get_session`/`get_session_chunks`：SHALL 自动通过 `find_session_project` 解析所属项目
- `search_sessions`：SHALL 跨所有项目搜索（现有行为）

#### Scenario: list_sessions 不传 project 全局查询

- **GIVEN** 用户有 3 个项目各含若干 session
- **WHEN** AI 调用 `list_sessions` 不传 `project`，传 `since="today"`
- **THEN** 返回 SHALL 包含所有 3 个项目中今天的 session
- **AND** 每条 SHALL 含 `projectName` 字段标识来源项目

#### Scenario: get_stats 不传 project 全局聚合

- **WHEN** AI 调用 `get_stats` 不传 `project`
- **THEN** 返回 SHALL 聚合所有项目的统计数据

### Requirement: list_sessions 活动摘要字段

MCP `list_sessions` tool 返回的 session 对象 SHALL 包含活动摘要字段（`userIntents`、`lastActive`、`durationMs`、`totalCost`、`toolErrorCount`、`filesTouched`、`gitSummary`），与 CLI `sessions list --format json` 输出的字段一致。

新增字段通过 `SessionSummary` 的 serde 序列化自动透传。

#### Scenario: list_sessions 返回活动摘要

- **WHEN** MCP client 调用 `list_sessions` tool
- **THEN** 返回的每个 session 对象 SHALL 包含 `userIntents` 数组和 `filesTouched` 数组

#### Scenario: 新字段为空时不序列化

- **WHEN** 某会话的 `filesTouched` 为空数组
- **THEN** MCP 返回的 JSON 中该字段 SHALL NOT 出现（`skip_serializing_if` 生效）

