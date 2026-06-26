# session-turn-view Specification

## Purpose
TBD - created by archiving change redesign-cli-mcp-api. Update Purpose after archive.
## Requirements
### Requirement: Turn 数据模型

系统 SHALL 把会话呈现为 turn 序列。turn 边界由 `cdt-analyze::derive_turns` 统一派生（单一权威），驱动来源含用户消息、队友消息或无驱动前缀。每个 turn SHALL 含 `index`（从 0 起，连续无空洞）、`question`（驱动输入文本，无驱动时为 `null`）、`answer`（AI 最终文本响应，被打断时为 `null`）、`metrics`。`SystemChunk` 不单独产 turn；`CompactChunk` 不产 turn 而作为 phase 边界。

本能力是 turn/step 数据模型的单一 owner，`mcp-server` / `cli-output` / `session-search` 引用本契约，不各自重复定义 turn/step 字段。

#### Scenario: 用户提问与 AI 响应配对为一个 turn
- **WHEN** 会话含一条真实用户消息及其后的 AI 响应
- **THEN** 系统 SHALL 产出一个 turn，`question` 为用户消息文本，`answer` 为 AI 最终文本，`index` 较前一 turn 递增 1

#### Scenario: AI 响应被打断
- **WHEN** 某 turn 的 AI 响应被用户打断、无最终文本
- **THEN** 该 turn 的 `answer` SHALL 为 `null`，steps 保留到中断点

#### Scenario: 队友消息驱动的 turn
- **WHEN** 某 AI 响应由队友消息驱动（agent team 场景），无用户消息
- **THEN** 系统 SHALL 产出一个 turn，`question` 为队友消息文本，`answer` 为 AI 最终文本

#### Scenario: 无驱动前缀的 headless turn
- **WHEN** 会话起始有 AI 响应但无任何驱动输入（resumed/fork session）
- **THEN** 系统 SHALL 产出 index=0 的 turn，`question` 为 `null`

### Requirement: Turn 内 step 序列

每个 turn SHALL 暴露有序 `steps`，类型集合为：`thinking`、`text`、`tool`、`subagent`、`teammate_spawn`、`workflow`、`interruption`、`user_message`、`slash`、`teammate_message`、`compaction`、`system`。`tool` step SHALL 含 `toolUseId`、`name`、`input`、`output`。一个工具仅在结构上非「普通 input→output」时升格为独立 step 类型（Task/Agent → `subagent`、派生队友的 SendMessage → `teammate_spawn`、Workflow → `workflow`）；其余工具（Read/Edit/Write/Skill/Bash/MCP 工具等）SHALL 为 `tool` step 并以 `name` 区分。`compaction` step SHALL 含 `summary`（压缩摘要）；`system` step SHALL 含 `content`（系统提示内容）。

`tool` step 的 `output` SHALL 保留三态：文本（`text`）、结构化 JSON（`structured`，如命令的 stdout/stderr/exitCode）、缺失（`missing`，工具调用无对应结果）。

#### Scenario: 工具调用呈现为 tool step 并保留 input/output
- **WHEN** 某 turn 含一次文件读取工具调用
- **THEN** 对应 step `type` 为 `tool`、`name` 为该工具名，且含完整 `input` 与 `output`

#### Scenario: 结构化工具输出不被扁平化
- **WHEN** 某工具返回结构化结果（含 stdout / stderr / 退出码）
- **THEN** step 的 `output` SHALL 以结构化形态保留各字段，不拼接为单一字符串

#### Scenario: 被打断标记呈现为 interruption step
- **WHEN** 某 turn 内 AI 响应被用户打断
- **THEN** steps SHALL 含一个 `interruption` step 标记中断位置

#### Scenario: 上下文压缩呈现为 compaction step
- **WHEN** 某 turn 内发生上下文压缩
- **THEN** steps SHALL 含一个 `compaction` step，包含 `summary`（压缩摘要文本）

#### Scenario: 系统提示呈现为 system step
- **WHEN** 某 turn 内有系统提示注入
- **THEN** steps SHALL 含一个 `system` step，包含 `content`（系统提示内容）

### Requirement: 服务端内置截断

系统 SHALL 在服务端对大字段内置截断，不暴露内容裁剪参数。`question` / `answer` / `thinking` / `text` SHALL 全量返回。`tool` step 的 `output` 大小达到阈值（≥ 5KB）时 SHALL 截断为前缀并附 `outputTruncated: true`、`outputBytes`（原始字节数）与 `toolUseId`。被截断的完整 output SHALL 可通过 `get_tool_output` + `toolUseId` 取回（API 自闭环）。

#### Scenario: 大工具输出被截断并标注
- **WHEN** 某 tool step 的原始 output ≥ 5KB
- **THEN** 返回 SHALL 含截断后的前缀、`outputTruncated: true`、原始 `outputBytes` 与 `toolUseId`

#### Scenario: 小工具输出全量返回
- **WHEN** 某 tool step 的原始 output < 5KB
- **THEN** 返回 SHALL 含完整 output 且 `outputTruncated` 为 false

### Requirement: Subagent 作为独立 session 暴露

派生 subagent 的工具调用 SHALL 呈现为 `subagent` step，含 `name`、`description`、`subagentSessionId`、`stepsCount`。`subagentSessionId` 字段预留供后续通过同一组会话/turn 接口递归钻取（本次不实现递归钻取，defer 至后续 change）。

#### Scenario: subagent step 暴露 session id
- **WHEN** 某 turn 含一次派生 subagent 的工具调用
- **THEN** 对应 step `type` 为 `subagent` 且 SHALL 含 `subagentSessionId`

### Requirement: Turn metrics

每个 turn SHALL 含 `metrics` 字段，包含 `inputTokens`、`outputTokens`、`cacheReadTokens`、`cacheCreationTokens`（token usage）、`cost`（美元）、`durationMs`（turn 耗时毫秒）、`model`（模型名）。所有字段 SHALL 从 turn 内 chunk 数据无额外 I/O 算出。

#### Scenario: turn metrics 含 token usage 和 cost
- **WHEN** 某 turn 含 AI 响应
- **THEN** 该 turn 的 `metrics` SHALL 含 `inputTokens`、`outputTokens`、`cacheReadTokens`、`cacheCreationTokens`、`cost`、`durationMs`、`model`

### Requirement: 统一分页契约

返回列表的接口 SHALL 使用统一分页字段：`total`（总数）与 `nextCursor`（不透明游标）。`nextCursor` 存在表示还有下一页、不存在表示到底；系统 SHALL NOT 额外暴露 `hasMore` / `returned` 等可由 `nextCursor` 与列表长度推导的冗余字段。

#### Scenario: 还有更多结果时返回游标
- **WHEN** 某分页接口的结果超过单页上限
- **THEN** 返回 SHALL 含 `total` 与非空 `nextCursor`

#### Scenario: 最后一页不含游标
- **WHEN** 某分页接口的结果已到最后一页
- **THEN** 返回 SHALL 含 `total` 且不含 `nextCursor`

