## MODIFIED Requirements

### Requirement: Expose project and session queries

系统 SHALL 在 `SessionSummary` 中包含会话活动摘要字段，让消费端在列表阶段即可掌握每个会话的用户意图、活动产出和关键指标，无需逐个拉取 chunk 级详情。

`SessionSummary` SHALL 新增以下字段（全部可选，`skip_serializing_if` 为空时不序列化）：

| 字段 | 类型 | 语义 |
|---|---|---|
| `userIntents` | `string[]` | 用户消息首行序列，上限 30 条，每条 ≤100 字符 |
| `lastActive` | `int64` | 最后一条消息的时间戳（epoch ms） |
| `durationMs` | `int64` | `lastActive - created`，会话跨度 |
| `totalCost` | `float64?` | 基于 token usage 的费用估算 |
| `toolErrorCount` | `int` | 工具执行错误计数 |
| `filesTouched` | `string[]` | 被编辑文件路径（去重），上限 20 条 |
| `gitSummary` | `string[]` | commit message 和 PR URL，上限 10 条 |

`userIntents` SHALL 过滤噪声确认词（≤3 字符的纯确认，如 `ok` / `yes` / `嗯` / `好` / `继续`），只保留有语义的用户输入。

`filesTouched` SHALL 从 `Edit` / `Write` / `MultiEdit` 工具调用的文件路径参数提取。

`gitSummary` SHALL 从 `Bash` 工具调用的 `command` 中提取 `git commit -m` 的 message，以及从工具输出中提取 GitHub PR URL。

所有新增字段 SHALL 在 metadata 扫描（`extract_session_metadata`）期间提取，不引入额外 I/O。

`SessionMetadataUpdate` event SHALL 同步包含新增字段，让前端 / SSE 消费端在 metadata push 时即可获得活动摘要。

#### Scenario: 列表包含用户意图序列

- **WHEN** 查询 sessions list 且某会话有 5 条用户消息
- **THEN** 返回的 `userIntents` SHALL 包含 5 条用户消息首行文本
- **AND** 每条截断至 100 字符

#### Scenario: 用户意图过滤噪声确认词

- **WHEN** 用户消息首行为 `ok` 或 `嗯` 或 `继续`
- **THEN** 该消息 SHALL NOT 出现在 `userIntents` 中

#### Scenario: 用户意图上限截断

- **WHEN** 会话有 50 条用户消息
- **THEN** `userIntents` SHALL 只包含前 30 条（保留时序，截断尾部）

#### Scenario: 文件编辑路径去重

- **WHEN** 同一文件被 `Edit` 工具修改 3 次
- **THEN** `filesTouched` SHALL 只包含该路径 1 次

#### Scenario: git commit message 提取

- **WHEN** Bash 工具执行了 `git commit -m "fix: session cache"`
- **THEN** `gitSummary` SHALL 包含 `fix: session cache`

#### Scenario: PR URL 提取

- **WHEN** Bash 工具输出包含 `https://github.com/user/repo/pull/42`
- **THEN** `gitSummary` SHALL 包含该 URL

#### Scenario: 无活动的会话

- **WHEN** 会话只有 1 条用户消息且无工具调用
- **THEN** `filesTouched` SHALL 为空数组，`gitSummary` SHALL 为空数组，`toolErrorCount` SHALL 为 0

#### Scenario: cost 估算

- **WHEN** 会话包含 assistant 消息的 token usage
- **THEN** `totalCost` SHALL 基于模型定价表估算费用

#### Scenario: 工具错误从 ToolResult 消息计数

- **WHEN** 用户消息中包含 `is_error=true` 的 ToolResult
- **THEN** `toolErrorCount` SHALL 递增

#### Scenario: PR URL 仅从 Bash 工具输出提取

- **WHEN** 非 Bash 工具的 ToolResult 输出包含 GitHub PR URL
- **THEN** 该 URL SHALL NOT 出现在 `gitSummary` 中
