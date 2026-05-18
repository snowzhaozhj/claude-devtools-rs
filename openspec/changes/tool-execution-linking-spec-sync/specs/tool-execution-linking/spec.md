## MODIFIED Requirements

### Requirement: Pair tool_use with tool_result by id

系统 SHALL 把每个 `tool_use` 块与同 `tool_use_id` 的 `tool_result` 块配对，无视两者间隔多少条消息。配对算法 SHALL 为纯同步函数，对输入消息单次遍历即可完成；无匹配的 `tool_use` SHALL 作为 orphan 保留，标记 `output = Missing` 且 `end_ts = None`，不抛错。

#### Scenario: Immediate result
- **WHEN** 一个 `tool_use` 紧接的下一条 user 消息含同 id 的 `tool_result`
- **THEN** 这对 SHALL 被链接，并产出含起止时间戳的 `ToolExecution` 记录

#### Scenario: Delayed result
- **WHEN** `tool_use` 后还间隔了若干消息才出现其 `tool_result`
- **THEN** 一旦匹配到对应 result，配对 SHALL 仍然成立

#### Scenario: Duplicate tool_use ids
- **WHEN** assistant 侧两个 `tool_use` 块共享同一 `tool_use_id`（例如流式 rewrite / retry 写入同 id 的两份 tool_use）
- **THEN** 系统 SHALL 保留首条遇到的 `tool_use` 进入 pending 配对队列，跳过重复者，把返回的 `ToolLinkingResult` 中 `duplicates_dropped` 计数加 1，并以 `tracing::warn!` 上报这个 `tool_use_id`

#### Scenario: Duplicate result ids
- **WHEN** 两个 `tool_result` 块共享同一 `tool_use_id`
- **THEN** 系统 SHALL 链接首条遇到的 result，把返回的 `ToolLinkingResult` 中 `duplicates_dropped` 计数加 1，并以 `tracing::warn!` 上报这个 id

#### Scenario: Orphan tool_use has no matching result
- **WHEN** 一条 assistant `tool_use` 在整个 session 中无任何 `tool_result` 与之匹配
- **THEN** 系统 SHALL 产出一条 `ToolExecution` 记录，`output = Missing`、`end_ts = None`、`is_error = false`，SHALL NOT panic

### Requirement: Format readable summaries for team coordination tools

系统 SHALL 为每个团队协作工具（`TeamCreate`、`TaskCreate`、`TaskUpdate`、`TaskList`、`TaskGet`、`SendMessage`、`TeamDelete`）产出一条简短可读的 summary 字符串，捕捉最显著的参数。

`SendMessage` 工具的 summary SHALL 按 `input.type` 字段分四个 branch 处理：

1. `type == "shutdown_response"` —— 读 `input.approve`（bool），`true` 时输出 `"Shutdown approved"`，否则（含缺失 / `false`）输出 `"Shutdown denied"`，**不**追加 recipient / message body。
2. `type == "broadcast"` —— 输出 `"Broadcast: <truncated message>"`，message 截断长度 SHALL ≤ 50 字符。
3. `type` 为其它值（含 `"message"` / 缺失 / 非字符串）且 `input.to` 存在 —— 输出 `"To <recipient>: <truncated message>"`，message 截断 SHALL ≤ 50 字符。
4. `type` 为其它值且 `input.to` 缺失 —— 退回到 `truncate(type, 50)` 文本，避免 summary 退化为单纯的工具名字面量。

#### Scenario: SendMessage with recipient and body
- **WHEN** 一次 `SendMessage` 工具调用 `input` 含 `to` 与 `message` 参数（`type` 缺失或为 `"message"` 等非特殊值）
- **THEN** summary SHALL 同时含 recipient 与截断后的 message 预览，形如 `To <recipient>: <truncated>`，message 部分截断长度 ≤ 50 字符

#### Scenario: SendMessage shutdown_response approve true
- **WHEN** 一次 `SendMessage` 调用 `input.type == "shutdown_response"` 且 `input.approve == true`
- **THEN** summary SHALL 为字面量 `"Shutdown approved"`，**不**追加 recipient 或 message body

#### Scenario: SendMessage shutdown_response approve false or missing
- **WHEN** 一次 `SendMessage` 调用 `input.type == "shutdown_response"` 且 `input.approve == false` 或缺失
- **THEN** summary SHALL 为字面量 `"Shutdown denied"`，**不**追加 recipient 或 message body

#### Scenario: SendMessage broadcast type
- **WHEN** 一次 `SendMessage` 调用 `input.type == "broadcast"` 含 `input.message`
- **THEN** summary SHALL 形如 `Broadcast: <truncated message>`，message 部分截断长度 ≤ 50 字符；recipient 字段在 broadcast branch 不参与渲染（即使 `input.to` 存在也忽略）

#### Scenario: SendMessage default type without recipient
- **WHEN** 一次 `SendMessage` 调用 `input.type` 为非特殊值且 `input.to` 缺失
- **THEN** summary SHALL 退回到 `truncate(type, 50)` 形式（如 `type == "message"` 时输出 `"message"`），避免空 recipient 路径下 summary 退化为单纯的 `"SendMessage"` 字面量
