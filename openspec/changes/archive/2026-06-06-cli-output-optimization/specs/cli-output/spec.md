## ADDED Requirements

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
