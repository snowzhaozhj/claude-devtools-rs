## ADDED Requirements

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
