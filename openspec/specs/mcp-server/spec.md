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

