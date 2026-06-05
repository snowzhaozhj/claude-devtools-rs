# mcp-server Specification

## Purpose
TBD - created by archiving change mcp-server-stdio. Update Purpose after archive.
## Requirements
### Requirement: MCP stdio transport

`cdt mcp serve` SHALL 启动 MCP stdio server（JSON-RPC over stdin/stdout），使用 rmcp SDK 的 `transport::stdio()` 机制。

Server SHALL 在 `get_info()` 中返回：
- `ServerCapabilities` 启用 `tools`
- `instructions` 字段引导 AI 先调 summary 再按需 detail
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

MCP server SHALL 暴露以下 8 个 tools，全部标注 `readOnlyHint=true`、`destructiveHint=false`、`idempotentHint=true`：`list_projects`、`list_sessions`、`get_session_summary`、`get_session_detail`、`get_session_errors`、`search_sessions`、`get_session_cost`、`get_stats`。

所有 tools SHALL 返回 JSON 结构化数据（不返回纯文本大段 dump）。

#### Scenario: list_projects 返回项目列表

- **GIVEN** 用户有至少一个 Claude Code 项目
- **WHEN** AI 调用 `list_projects` tool
- **THEN** 返回 SHALL 包含 `name`、`path`、`sessions`、`lastActive` 字段的 JSON 数组

#### Scenario: get_session_summary 返回紧凑诊断

- **GIVEN** 一个含 50+ 消息的 session
- **WHEN** AI 调用 `get_session_summary`
- **THEN** 返回 SHALL 包含 phases、toolUsage、errorCount、cost 等结构化摘要
- **AND** 序列化后 SHALL < 4K tokens

#### Scenario: get_session_detail 支持 range 和 tail

- **GIVEN** 一个含 200 条 chunks 的 session
- **WHEN** AI 调用 `get_session_detail` 带 `tail=20`
- **THEN** SHALL 只返回最后 20 条 chunks
- **WHEN** AI 调用 `get_session_detail` 带 `range="50:70"`
- **THEN** SHALL 只返回第 50-70 条 chunks

#### Scenario: project 参数可省略

- **GIVEN** AI 知道 session_id 但不知道 project
- **WHEN** AI 调用 `get_session_summary` 只传 `session` 不传 `project`
- **THEN** 服务端 SHALL 自动通过 `find_session_project` 解析所属项目

### Requirement: Secret redaction

MCP server SHALL 默认对 tool 返回内容进行 secret pattern 脱敏——匹配的 secret 替换为 `[REDACTED]`，返回体附加 `redacted: true` 和 `redactedCount: N`。

支持 `--allow-sensitive` 启动参数跳过 redaction。

#### Scenario: API key 被自动脱敏

- **GIVEN** session 内容包含 `sk-ant-api03-xxxxxxxxxxxx`
- **WHEN** AI 调用 `get_session_detail` 获取该段内容
- **THEN** 返回中该 key SHALL 被替换为 `[REDACTED]`
- **AND** 返回体 SHALL 含 `redacted: true`

#### Scenario: allow-sensitive 跳过脱敏

- **GIVEN** MCP server 以 `--allow-sensitive` 启动
- **WHEN** AI 调用任意 tool
- **THEN** 返回内容 SHALL 不做脱敏处理

### Requirement: Context budget truncation

`get_session_detail` tool SHALL 支持 `max_tokens` 参数（可选）。当序列化结果超出 budget 时，SHALL 按 chunk 粒度截断（不切半条 JSON），返回 `truncated: true`、`totalChunks: N`、`nextRange: "<offset>:"`。

#### Scenario: max_tokens 触发截断

- **GIVEN** session 有 500 条 chunks，序列化后约 50K tokens
- **WHEN** AI 调用 `get_session_detail` 带 `max_tokens=8000`
- **THEN** SHALL 返回前 N 条完整 chunks（总估算 ≤ 8000 tokens）
- **AND** 返回 SHALL 含 `truncated: true` 和 `nextRange`

### Requirement: MCP 注册

`cdt setup mcp` SHALL 打印注册命令提示。`cdt setup mcp --apply` SHALL 调用 `claude mcp add cdt-devtools -- cdt mcp serve` 注册 MCP server 到 Claude Code。

#### Scenario: setup mcp 打印注册命令

- **WHEN** 用户运行 `cdt setup mcp`
- **THEN** SHALL 输出 `claude mcp add` 注册命令到 stdout

#### Scenario: setup mcp --apply 通过 claude CLI 注册

- **WHEN** 用户运行 `cdt setup mcp --apply`
- **THEN** SHALL 执行 `claude mcp add cdt-devtools -- cdt mcp serve`
- **AND** 注册成功时 SHALL 输出确认信息

### Requirement: grep 过滤 session detail chunks

`get_session_detail` SHALL 支持可选的 `grep` 参数（case-insensitive literal substring）。当提供 grep 时：

- 系统 SHALL 在完整 chunk 数据上执行匹配（omit envelope 构建之前），匹配范围包括 assistant 文本、user 文本、tool_use input（递归 JSON string leaf）、tool output、tool name、error message
- 系统 SHALL 只返回匹配的 chunks 及其 context window（由 `grep_context` 参数控制，默认 1）
- 匹配的 chunks SHALL 自动 promote 到 full content mode（无论 `content_mode` 参数设定），context-only chunks SHALL 遵循用户设定的 `content_mode`
- grep 模式下所有返回的 chunk envelope SHALL 包含 `grepHit` boolean 字段：匹配 chunk 为 true，context-only chunk 为 false。非 grep 模式下 SHALL 省略该字段

#### Scenario: grep 过滤返回匹配的 chunks
- **WHEN** 调用 `get_session_detail` 时 grep 为 "mw switch" 且 session 的第 5、12 个 chunk 含匹配内容
- **THEN** 返回的 chunks SHALL 包含第 4-6 和 11-13 个 chunk（±1 context）
- **AND** 第 5 和 12 个 chunk 的 `grepHit` SHALL 为 true

#### Scenario: grep 匹配的 chunks 自动展开内容
- **WHEN** 调用 `get_session_detail` 时 grep 为 "mw switch" 且 `content_mode` 为 "omit"
- **THEN** 匹配的 chunks SHALL 返回完整内容（等效 full mode）
- **AND** context-only chunks SHALL 保持 omit mode

#### Scenario: grep 匹配 tool_use input 中的内容
- **WHEN** 调用 `get_session_detail` 时 grep 为 "switch" 且某 AI chunk 的 tool_execution input 含 `{"command": "mw switch get ..."}`
- **THEN** 该 chunk SHALL 出现在返回结果中

#### Scenario: grep 无匹配时返回空
- **WHEN** grep 关键词不在任何 chunk 中出现
- **THEN** 返回的 chunks SHALL 为空数组

### Requirement: search_sessions 支持 session 参数

`search_sessions` MCP tool SHALL 接受可选的 `session` 参数。当提供时，搜索范围 SHALL 限定到该 session（委托给 `[[session-search]]` 的 intra-session search 能力）。

#### Scenario: MCP search_sessions 带 session 参数
- **WHEN** 调用 `search_sessions` 时 session 为 "908b77f7" 且 query 为 "mw switch"
- **THEN** 返回结果 SHALL 只包含该 session 的命中
- **AND** sessions_searched SHALL 为 1

### Requirement: session summary 包含 toolActivity

`get_session_summary` 的返回 SHALL 包含 `toolActivity` 字段，结构化展示该会话中工具执行的确定性摘要：

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

