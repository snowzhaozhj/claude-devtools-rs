# cli-output Specification

## Purpose
TBD - created by archiving change cli-output-optimization. Update Purpose after archive.
## Requirements
### Requirement: Content mode 控制 JSON/JSONL 输出粒度

`sessions detail` 命令 SHALL 支持 `--content <omit|full>` flag，控制 JSON/JSONL 输出中 chunk 内容的详细程度。

不指定 `--content` 时，JSON/JSONL 输出 SHALL 保持当前行为（raw `SessionDetail` 序列化）。

指定 `--content omit` 时，输出 SHALL 使用 `ChunkView` 包装格式：
- 每个 chunk 包含：`chunkIndex`、`chunkId`、`type`（kind）、`timestamp`、`durationMs`
- AI chunk 的每个 tool execution 包含：`toolName`、`toolUseId`、`isError`、`inputSummary`（摘要）、`outputChars`（大小）、`outputOmitted: true`、`errorMessage`
- AI chunk 的每个 response 包含：`model`、`contentChars`、`contentOmitted: true`
- User chunk：text ≤ 200 chars 时包含文本，>200 chars 时 `omitted: true` + `chars` 大小
- System chunk：同 User chunk 规则
- Compact chunk：始终包含 `compactSummary`

指定 `--content full` 时，输出 SHALL 使用 `ChunkView` 包装格式但包含完整内容。

`--content` flag SHALL 仅影响 `--format json` 和 `--format jsonl` 输出，不影响 `--format table`。

#### Scenario: 不指定 --content 保持兼容

- **WHEN** 运行 `cdt sessions detail <id> --format json` 不带 `--content` flag
- **THEN** 输出 SHALL 与当前行为一致（raw `SessionDetail` JSON）

#### Scenario: --content omit 输出 chunk 结构概览

- **WHEN** 运行 `cdt sessions detail <id> --format json --content omit`
- **THEN** 每个 AI chunk 的 tool execution SHALL 包含 `inputSummary` 和 `outputChars`，不含完整 `input`/`output`
- **THEN** 每个 AI chunk 的 response SHALL 包含 `contentChars`，不含完整 `content`

#### Scenario: --content full 输出完整内容

- **WHEN** 运行 `cdt sessions detail <id> --format json --content full`
- **THEN** 每个 tool execution SHALL 包含完整 `input` 和 `output`
- **THEN** 每个 response SHALL 包含完整 `content`

#### Scenario: --content omit 与 grep 的交互

- **WHEN** 运行 `cdt sessions detail <id> --content omit --grep <keyword>`
- **THEN** grep 直接命中的 chunk SHALL auto-expand 为 `content full`
- **THEN** grep context chunk（非直接命中但在 context 窗口内）SHALL 保持 `content omit`

#### Scenario: --content 不影响 table 输出

- **WHEN** 运行 `cdt sessions detail <id> --format table --content omit`
- **THEN** table 输出 SHALL 与不指定 `--content` 时一致

#### Scenario: --content 非法值报错

- **WHEN** 运行 `cdt sessions detail <id> --content invalid`
- **THEN** SHALL 报错，提示合法值为 `omit` 或 `full`

### Requirement: grep 应用顺序统一

`sessions detail` 的 `--grep` SHALL 在 `kind_filter` 之后、`range/tail` 之前应用。

即顺序为：kind_filter → grep + context expansion → range/tail。

#### Scenario: grep 在全集上搜索

- **WHEN** 运行 `cdt sessions detail <id> --grep <keyword>`（不指定 `--range` 或 `--tail`）
- **THEN** grep SHALL 在所有 chunk（经 kind_filter 后）中搜索，然后应用默认 tail
- **THEN** 结果不限于最后 20 个 chunk 内的命中

#### Scenario: grep 与 tail 组合

- **WHEN** 运行 `cdt sessions detail <id> --grep <keyword> --tail 5`
- **THEN** grep SHALL 先在全集中搜索，context 展开后取最后 5 个可见 chunk

### Requirement: --all flag 替代 --full

`sessions detail` 的 `--full` flag SHALL 重命名为 `--all`，原 `--full` 保留为 alias。

`--all` 的含义 SHALL 是"返回全部 chunk，禁用默认 tail=20"，help 文本 SHALL 明确此语义。

#### Scenario: --all 返回全部 chunk

- **WHEN** 运行 `cdt sessions detail <id> --all`
- **THEN** SHALL 返回所有 chunk，不应用默认 tail=20

#### Scenario: --full 作为 alias

- **WHEN** 运行 `cdt sessions detail <id> --full`
- **THEN** 行为 SHALL 与 `--all` 一致

### Requirement: range 与 tail 互斥

`sessions detail` 的 `--range` 和 `--tail` SHALL 互斥，同时指定 SHALL 报错。

#### Scenario: 同时指定 range 和 tail 报错

- **WHEN** 运行 `cdt sessions detail <id> --range 0:10 --tail 5`
- **THEN** SHALL 报错，提示 `--range` 和 `--tail` 互斥

### Requirement: jsonl 格式契约

`--format jsonl` 输出 SHALL 符合 NDJSON 规范：每行一个紧凑 JSON 对象（无缩进、无 pretty-print）。

对于 `sessions summary`、`sessions cost`、`stats` 等单对象输出，jsonl SHALL 输出紧凑单行 JSON。

#### Scenario: summary jsonl 输出紧凑 JSON

- **WHEN** 运行 `cdt sessions summary <id> --format jsonl`
- **THEN** 输出 SHALL 是单行紧凑 JSON（不含换行和缩进）

#### Scenario: sessions list jsonl 逐行输出

- **WHEN** 运行 `cdt sessions list -p <project> --format jsonl`
- **THEN** 每个 session 占一行，每行是紧凑 JSON

### Requirement: 空结果返回 exit 0

所有命令在查询结果为空时 SHALL 以 exit code 0 退出。

JSON 模式 SHALL 输出 `[]`（空数组）或 `{}`（空对象）到 stdout。table 模式 SHALL 输出提示信息到 stderr。

#### Scenario: sessions list 无结果

- **WHEN** 运行 `cdt sessions list -p <project> --format json` 且无匹配 session
- **THEN** stdout SHALL 输出 `[]`，exit code SHALL 为 0

#### Scenario: search 无结果

- **WHEN** 运行 `cdt search <query> --format json` 且无匹配
- **THEN** stdout SHALL 输出 `[]`，exit code SHALL 为 0

#### Scenario: sessions errors 无结果

- **WHEN** 运行 `cdt sessions errors <id> --format json` 且无错误
- **THEN** stdout SHALL 输出 `[]`，exit code SHALL 为 0

#### Scenario: stats 无结果

- **WHEN** 运行 `cdt stats 7d --format json` 且时间范围内无 session
- **THEN** exit code SHALL 为 0

### Requirement: unicode-width-aware 截断

table 模式的所有文本截断 SHALL 使用 Unicode display width 计算（中文字符占 2 列宽度）。

截断 SHALL 统一使用 `…`（U+2026）作为省略符号。

#### Scenario: 中文标题截断对齐

- **WHEN** table 中有中文标题和 ASCII 标题混排
- **THEN** 列对齐 SHALL 基于 display width 而非 char count，中文和 ASCII 行的列位置一致

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

### Requirement: --no-truncate flag

table 模式 SHALL 支持 `--no-truncate` flag，指定时不截断任何字段。

#### Scenario: 不截断 table 输出

- **WHEN** 运行 `cdt sessions list -p X --no-truncate`
- **THEN** 所有字段 SHALL 完整显示，不截断

### Requirement: table 显示优化

table 模式 SHALL 具备以下能力：
- PATH 字段 SHALL 将 home 目录前缀替换为 `~/`
- 列宽 SHALL 根据终端宽度动态分配（pipe 时 fallback 120 列）
- `sessions detail` 的 chunk 内容截断宽度 SHALL 跟随终端宽度（非固定 60 字符）

#### Scenario: PATH 缩写

- **WHEN** table 中显示用户 home 目录下的路径
- **THEN** PATH SHALL 以 `~/` 开头而非完整 home 路径

#### Scenario: 终端宽度自适应

- **WHEN** 终端宽度为 200 列
- **THEN** 弹性列 SHALL 扩展以利用额外空间

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

### Requirement: CLI 命令结构

CLI binary `cdt` SHALL 提供以下顶级命令结构：

- `cdt projects` — 列出所有项目
- `cdt sessions` — 列出 session（支持全局或按项目）
- `cdt session <id>` — 单 session 复合视图（summary + cost + errors）
- `cdt session <id> --chunks` — chunk 级内容取数
- `cdt search <query>` — 全文搜索
- `cdt stats [period]` — 聚合统计
- `cdt serve` — HTTP API server
- `cdt mcp serve` — MCP stdio server
- `cdt setup` — 安装配置
- `cdt completions <shell>` — shell 补全脚本生成
- `cdt self-update` — 自更新

`cdt session <id>` 和 `cdt session <id> --chunks` 共用同一子命令入口，通过 `--chunks` flag 区分模式。

#### Scenario: cdt session 默认返回复合视图

- **WHEN** 用户运行 `cdt session abc123`
- **THEN** SHALL 输出 summary + cost + errors 的合并视图
- **AND** table 格式 SHALL 紧凑展示核心指标

#### Scenario: cdt session --chunks 进入 chunk 模式

- **WHEN** 用户运行 `cdt session abc123 --chunks --tail 5 --content full`
- **THEN** SHALL 输出最后 5 条 chunk 的完整内容

#### Scenario: cdt sessions 支持全局查询

- **WHEN** 用户运行 `cdt sessions --since yesterday`（不带 --project）
- **THEN** SHALL 输出所有项目中昨天的 session 列表

#### Scenario: cdt sessions 支持 group-by

- **WHEN** 用户运行 `cdt sessions --since 7d --group-by project`
- **THEN** table 输出 SHALL 按项目分组显示

#### Scenario: cdt sessions 支持 branch 过滤

- **WHEN** 用户运行 `cdt sessions --branch feat/auth`
- **THEN** SHALL 只输出 gitBranch 含 "feat/auth" 的 session

#### Scenario: cdt session latest 解析

- **WHEN** 用户运行 `cdt session latest`
- **THEN** SHALL 解析为最近一次 session 并输出其复合视图

### Requirement: CLI 自动补全

`cdt completions <shell>` SHALL 生成包含新命令结构的 shell 补全脚本（bash/zsh/fish/powershell）。

自动补全 SHALL 覆盖：
- 顶级命令名（projects/sessions/session/search/stats/serve/mcp/setup/completions/self-update）
- `cdt session` 的位置参数 SHALL 提供 session ID 补全（基于最近 session 列表）
- `cdt sessions --project` 的参数值 SHALL 提供项目名补全
- `cdt sessions --since` 的参数值 SHALL 提供常用时间表达式补全（today/yesterday/7d/24h/30d）
- `cdt sessions --group-by` 的参数值 SHALL 提供枚举补全（none/project/day）
- `cdt session <id> --include` 的参数值 SHALL 提供 facet 枚举补全（phases/tools/activity/idle_gaps/files）
- `cdt session <id> --chunks --content` 的参数值 SHALL 提供模式补全（compact/overview/full）
- `cdt session <id> --chunks --filter` 的参数值 SHALL 提供枚举补全（errors_only/tool_calls）

#### Scenario: zsh 补全 session ID

- **GIVEN** 用户已 source 了 `cdt completions zsh` 的输出
- **WHEN** 用户输入 `cdt session ` 后按 Tab
- **THEN** SHALL 展示最近 session 的 ID 列表（通过 `SessionCompleter`）

#### Scenario: bash 补全 --since 值

- **GIVEN** 用户已 eval 了 `cdt completions bash` 的输出
- **WHEN** 用户输入 `cdt sessions --since ` 后按 Tab
- **THEN** SHALL 展示 today/yesterday/7d/24h/30d 等候选值

#### Scenario: zsh 补全 --include facets

- **WHEN** 用户输入 `cdt session abc --include ` 后按 Tab
- **THEN** SHALL 展示 phases/tools/activity/idle_gaps/files 候选值

### Requirement: 时间参数格式统一

CLI 的 `--since` 和 `--until` 参数 SHALL 与 MCP 的 `since`/`until` 接受完全相同的格式集：

- 相对时长：`7d`/`24h`/`1h`/`30m`
- 命名周期：`today`/`yesterday`/`week`
- 绝对日期：`2026-06-06`/ISO 8601

`--until` SHALL 作为新 flag 添加到 `cdt sessions` 命令。

#### Scenario: --since yesterday 与 MCP 行为一致

- **WHEN** 用户运行 `cdt sessions --since yesterday --format json`
- **THEN** 输出结果 SHALL 与 MCP `list_sessions({since: "yesterday"})` 返回的 items 一致

#### Scenario: --until 配合 --since 限定范围

- **WHEN** 用户运行 `cdt sessions --since 2026-06-01 --until 2026-06-03`
- **THEN** SHALL 只输出 [6月1日, 6月3日) 范围内的 session

