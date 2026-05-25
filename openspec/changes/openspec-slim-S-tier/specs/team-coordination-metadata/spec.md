## MODIFIED Requirements

### Requirement: Link teammate messages to triggering SendMessage

系统 SHALL 把每条 teammate 消息回链到触发它的 `SendMessage` 工具调用，方式是填充 `TeammateMessage.reply_to_tool_use_id`。配对算法 MUST 按以下顺序执行（向后扫描越早的 SendMessage 越优先）：

1. 在**新 flush 的 AIChunk 自身**的 `tool_executions` 中按出现顺序扫描，寻找 `tool_name == "SendMessage"` 且 `input.recipient == teammate.teammate_id` 且 `tool_use_id` 未被同 batch 其它 teammate 占用的条目；命中即记录。
2. 若未命中则向**已 emit 的 AIChunks** 回溯（最多回溯 3 个最近 AIChunk），按相同条件扫描；命中即记录。
3. 仍未命中则 `reply_to_tool_use_id = None`（孤儿），UI 上展示为追加到 turn 末尾的卡片。

每条 SendMessage `tool_use_id` 在同一 chunk-building 跑批中 MUST 至多被一条 teammate 配对（去重 set 维护跨 AIChunk）；这样 SendMessage 给 alice 后 alice 多条 reply 时，第二条及以后会走孤儿路径而不抢配对。

配对算法 SHALL 实现为纯函数（无副作用、可独立单测覆盖）。

#### Scenario: Teammate reply matches preceding SendMessage in prior AIChunk

- **WHEN** `AIChunk1` 含 `SendMessage(tool_use_id=t1, recipient="alice")`，紧随其后的 user 消息为 `<teammate-message teammate_id="alice">...</teammate-message>`，再接 `AIChunk2`
- **THEN** `AIChunk2.teammate_messages[0].reply_to_tool_use_id` SHALL 为 `Some("t1")`

#### Scenario: Teammate reply matches SendMessage in same flushing AIChunk

- **WHEN** 同一 flush 周期内 buffer 含 `SendMessage(tool_use_id=t2, recipient="bob")`，pending teammate `bob` 在 buffer 内 SendMessage 之后到达（罕见，但 chunk 合并下可能出现）
- **THEN** 配对 SHALL 优先命中同 AIChunk 内的 `t2`

#### Scenario: Multiple replies from same teammate go orphan after first match

- **WHEN** alice 在同一 batch 收到一次 `SendMessage(tool_use_id=t1)`，但 alice 连发两条 teammate-message
- **THEN** 第一条 `teammate.reply_to_tool_use_id` SHALL 为 `Some("t1")`；第二条 SHALL 为 `None`（孤儿，因 t1 已被占用）

#### Scenario: Lookback window bounded

- **WHEN** teammate alice 的 reply 距 SendMessage 4 个 AIChunk 之外
- **THEN** 配对 SHALL 失败（`reply_to_tool_use_id = None`），不做无界回溯

#### Scenario: SendMessage to different recipient does not match

- **WHEN** 最近的 SendMessage tool_use 是 `recipient="charlie"`，pending teammate 是 `alice`
- **THEN** 该 SendMessage SHALL 被跳过，配对继续向更早回溯

### Requirement: Parse all teammate-message blocks from one user message

系统 SHALL 提供"解析所有 teammate-message 块"的纯函数能力：使用全局 regex `<teammate-message\s+teammate_id="([^"]+)"([^>]*)>([\s\S]*?)</teammate-message>` 从一条 user 消息中抽取**每个**独立 teammate-message 块。一条 user 消息含 N 个块时返回 N 条独立 `TeammateAttrs`。

为兼容历史调用方，系统 SHALL 同时保留"取首条"的兼容入口，行为等价于"取所有"再返回首条；新代码 SHALL 优先用多 block 入口。

chunk-building 在 teammate user 分支 MUST 把每个块都转成独立 `TeammateMessage` push 到 `pending_teammates`。多 block 时各条 uuid SHALL 加 `-{idx}` 后缀，避免下游 `{#each}` key 冲突；timestamp 共享原 user 消息时间戳。

#### Scenario: Multiple blocks in one user message produce separate attrs

- **WHEN** user 消息 content 为 `<teammate-message teammate_id="alice">A</teammate-message><teammate-message teammate_id="bob">B</teammate-message><teammate-message teammate_id="charlie">C</teammate-message>`
- **THEN** "取所有"入口 SHALL 返回 3 条 `TeammateAttrs`，分别为 `{teammate_id: "alice", body: "A"}`、`{teammate_id: "bob", body: "B"}`、`{teammate_id: "charlie", body: "C"}`

#### Scenario: Blocks separated by noise text are still parsed independently

- **WHEN** user 消息 content 为 `<teammate-message teammate_id="alice">A</teammate-message>some noise<teammate-message teammate_id="bob">B</teammate-message>`
- **THEN** "取所有"入口 SHALL 返回 2 条 `TeammateAttrs`，body 分别为 `"A"` 与 `"B"`，noise 文本被忽略

#### Scenario: Multi-block teammate user message produces N TeammateMessage in chunk

- **WHEN** AIChunk flush 时 `pending_teammates` 含来自一条 user 消息的 3 条 teammate（每条对应一个 block）
- **THEN** flush 后 `AIChunk.teammate_messages` SHALL 含 3 条 `TeammateMessage`，每条 `uuid` SHALL 为 `<原 msg.uuid>-0` / `-1` / `-2`

### Requirement: Detect operational noise and resend in teammate messages

系统 SHALL 通过填充 `TeammateMessage.is_noise` 与 `TeammateMessage.is_resend` 来分类运维噪声 teammate 消息以及重复 / 转发 teammate 消息，让前端可据此采用极简单行 / 半透明 RefreshCw 渲染，避免噪声干扰主对话流。

`is_noise = true` MUST 满足以下任一条件：

- `teammate_id == "system"` 且 body trim 后是 JSON 且 `type` 字段属于 `{ idle_notification, shutdown_request, shutdown_approved, teammate_terminated }` 集合；
- `teammate_id == "system"` 且 body trim 后**不**是 JSON 且长度 < 200；
- `teammate_id != "system"` 但 body trim 后是 JSON 且 `type` 字段在上述集合内。

`is_resend = true` MUST 满足以下任一条件：

- `summary` 命中 `/\bresend/i`、`/\bre-send/i`、`/\bsent\b.{0,20}\bearlier/i`、`/\balready\s+sent/i`、`/\bsent\s+in\s+my\s+previous/i` 任一；
- 或 body 前 300 字符命中上述正则任一。

noise / resend 检测 SHALL 实现为纯函数（无副作用、可独立单测覆盖）。

#### Scenario: System idle_notification is noise

- **WHEN** teammate `teammate_id="system"`，body 为 `{"type":"idle_notification","message":"Idle"}`
- **THEN** `is_noise` SHALL 为 `true`

#### Scenario: System short text is noise

- **WHEN** teammate `teammate_id="system"`，body trim 后为 `Heartbeat ack`（< 200 字符，非 JSON）
- **THEN** `is_noise` SHALL 为 `true`

#### Scenario: System long text is not noise

- **WHEN** teammate `teammate_id="system"`，body 长度 ≥ 200 字符
- **THEN** `is_noise` SHALL 为 `false`

#### Scenario: Non-system teammate idle JSON is noise

- **WHEN** teammate `teammate_id="alice"`，body 为 `{"type":"shutdown_request"}`
- **THEN** `is_noise` SHALL 为 `true`

#### Scenario: Resend keyword in summary

- **WHEN** teammate summary 含 `"Resending the previous message"`
- **THEN** `is_resend` SHALL 为 `true`

#### Scenario: Resend keyword in body prefix

- **WHEN** teammate summary 为空，body 前 100 字符含 `"sent earlier in my previous reply"`
- **THEN** `is_resend` SHALL 为 `true`

#### Scenario: No resend signal

- **WHEN** teammate summary 与 body 均无 resend 关键词
- **THEN** `is_resend` SHALL 为 `false`
