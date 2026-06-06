## ADDED Requirements

### Requirement: --extract flag 提供 item 级展平输出

`sessions detail` 命令 SHALL 支持 `--extract <mode>` flag，将 chunk 级数据展平为 item 级条目序列。

支持的 mode SHALL 为：
- `overview`：每个 chunk 一条概览（类型、工具数、错误数、工具名列表）
- `errors`：每条失败的 tool execution 一条（含统一提取的错误信息）
- `tools`：每条 tool execution 一条（含 chunk 索引、工具名、状态、input 摘要）

`--extract` SHALL 与 `--filter`、`--grep`、`--range`、`--tail`、`--all` 正交组合。数据管道顺序 SHALL 为：kind_filter → grep → range/tail → extract 展平。

`--extract` 与 `--content` SHALL 互斥——同时指定 SHALL 报错。

非法 mode 值 SHALL 报错并提示合法值。

#### Scenario: --extract overview 输出每 chunk 一行概览

- **WHEN** 运行 `cdt sessions detail <id> --extract overview --all`
- **THEN** 输出 SHALL 为每个 chunk 一行，包含 chunkIndex、kind（user/ai/system/compact）、toolCount、errorCount、toolNames（去重按频率排序）
- **AND** 输出行数 SHALL 等于 chunk 总数

#### Scenario: --extract errors 输出每条错误一行

- **WHEN** 运行 `cdt sessions detail <id> --extract errors --all`
- **THEN** 输出 SHALL 为每条 `isError=true` 的 tool execution 一行
- **AND** 每行 SHALL 包含 chunkIndex、toolName、errorSummary（统一提取的有意义错误信息）
- **AND** 无错误时 SHALL 输出空（text 模式无输出，JSON 模式输出 `[]`）

#### Scenario: --extract tools 输出每条工具调用一行

- **WHEN** 运行 `cdt sessions detail <id> --extract tools --all`
- **THEN** 输出 SHALL 为每条 tool execution 一行，跨 chunk 展平
- **AND** 每行 SHALL 包含 chunkIndex、toolIndex、toolName、isError 状态、inputSummary

#### Scenario: --extract 与 --filter 组合

- **WHEN** 运行 `cdt sessions detail <id> --extract tools --filter errors_only --all`
- **THEN** 先按 `errors_only` 选出含错误的 chunk，再展平这些 chunk 的所有 tool executions
- **AND** 输出 SHALL 包含这些 chunk 中成功和失败的 tool executions

#### Scenario: --extract 与 --format json 组合

- **WHEN** 运行 `cdt sessions detail <id> --extract errors --format json --all`
- **THEN** 输出 SHALL 为扁平 JSON array，每个元素是一条 error entry
- **AND** JSON 字段名 SHALL 使用 camelCase

#### Scenario: --extract 默认 text 格式

- **WHEN** 运行 `cdt sessions detail <id> --extract overview --all`（不指定 `--format`）
- **THEN** 输出 SHALL 为 text 格式，每行一条，适合 AI 助手直接消费

#### Scenario: --extract 非法 mode 报错

- **WHEN** 运行 `cdt sessions detail <id> --extract invalid`
- **THEN** SHALL 报错并提示合法值为 `overview`、`errors`、`tools`

#### Scenario: --extract 与 --content 互斥报错

- **WHEN** 运行 `cdt sessions detail <id> --extract overview --content omit`
- **THEN** SHALL 报错，提示 `--extract` 和 `--content` 不能同时使用

### Requirement: 统一 error message 提取

所有 error 相关输出（`sessions errors` 命令、`--extract errors`）SHALL 使用统一的错误信息提取逻辑。

提取优先级 SHALL 为：
1. `errorMessage` 字段（如有）
2. `ToolOutput::Structured` 时：读取 value 中的 `stderr` / `error` / `message` 字段；读取 `exit_code` 或 `exitCode` 构造 "exit code N"
3. `ToolOutput::Text` 时：regex 匹配 `exit code \d+` 或 `exit status \d+`
4. output 最后 200 字符作为 fallback
5. 以上均无内容时返回 `None`

#### Scenario: Bash 工具 Structured output 提取 exit code

- **WHEN** 一条 Bash tool execution 的 `isError=true` 且 `errorMessage` 为 `None`
- **AND** tool output 为 `Structured` 类型，value 含 `{"exit_code": 1, "stderr": "command not found"}`
- **THEN** errorSummary SHALL 包含 "command not found"（优先 stderr）

#### Scenario: Bash 工具 Text output 提取 exit code

- **WHEN** 一条 Bash tool execution 的 `isError=true` 且 `errorMessage` 为 `None`
- **AND** tool output 为 `Text` 类型，包含 "exit code 1"
- **THEN** errorSummary SHALL 包含 "exit code 1"

#### Scenario: errorMessage 存在时优先使用

- **WHEN** 一条 tool execution 的 `isError=true` 且 `errorMessage` 为 "file not found"
- **THEN** errorSummary SHALL 为 "file not found"

#### Scenario: sessions errors 命令使用统一提取

- **WHEN** 运行 `cdt sessions errors <id>`
- **THEN** error message 列 SHALL 使用统一提取逻辑，不再显示 `(no message)`（除非 tool output 确实为空）

## MODIFIED Requirements

### Requirement: --json fields 字段选择

所有输出 JSON 的命令 SHALL 支持 `--json <fields>` flag（逗号分隔字段名）。

`--json` 隐含 `--format json` + 字段过滤 + 紧凑输出（无 pretty-print）。

`--json` 无参数时 SHALL 列出该命令可用的字段名。

`--extract` 模式下，`--json` 的可用字段 SHALL 为 extract 输出的扁平字段（如 `chunkIndex`、`toolName`、`isError` 等），而非原 chunk 级字段。

#### Scenario: 字段选择输出

- **WHEN** 运行 `cdt sessions list -p X --json sessionId,title,messageCount`
- **THEN** 输出 SHALL 只含指定字段，紧凑 JSON 格式

#### Scenario: 列出可用字段

- **WHEN** 运行 `cdt sessions list --json`（无参数）
- **THEN** SHALL 列出所有可用字段名

#### Scenario: 字段选择作用于数组元素

- **WHEN** 运行 `cdt sessions list -p X --json sessionId,title`
- **THEN** 输出 SHALL 是 JSON 数组，每个元素只含 `sessionId` 和 `title` 两个字段

#### Scenario: 未知字段静默忽略

- **WHEN** 运行 `cdt sessions list -p X --json sessionId,nonExistentField`
- **THEN** 输出 SHALL 只含 `sessionId` 字段，`nonExistentField` 不出现，不报错

#### Scenario: --extract 模式下 --json 列出扁平字段

- **WHEN** 运行 `cdt sessions detail <id> --extract tools --json`（无参数）
- **THEN** SHALL 列出 extract tools 的可用字段名（如 `chunkIndex`、`toolIndex`、`toolName`、`isError`、`inputSummary`）
