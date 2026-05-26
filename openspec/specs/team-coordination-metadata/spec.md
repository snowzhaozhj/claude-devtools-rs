# team-coordination-metadata Specification

## Purpose

定义团队协作工具与 teammate 消息的识别、解析、配对、富化规则：识别 `<teammate-message>` 包裹的 user 消息为 teammate 而非真实 user 输入；把 teammate 消息按 `SendMessage(recipient=...)` 配对回触发 tool_use；为 subagent Process 富化 `team` 元数据；分类 operational noise 与 resend 关键词。本 capability 让 UI 能为 teammate 渲染独立卡片、过滤运维噪声、给 SendMessage 与 reply 画连线。
## Requirements
### Requirement: Detect teammate messages

系统 SHALL 检测被 `<teammate-message teammate_id="..." ...>content</teammate-message>` 包裹的 user 消息，把它分类为 teammate 消息而非真实 user 输入。

#### Scenario: Teammate message in string content
- **WHEN** 一条 user 消息的字符串 content 以 `<teammate-message teammate_id="alice"` 起首
- **THEN** 该消息 SHALL 被标记为 teammate，且 SHALL NOT 产 `UserChunk`

#### Scenario: Teammate message in block content
- **WHEN** 一条 user 消息含单个 text block，块文本含 teammate 标签
- **THEN** 该消息 SHALL 被标记为 teammate

### Requirement: Render teammate messages as dedicated items

系统 SHALL 把 teammate 消息以独立 display 记录形式暴露，携带 `teammate_id`、`color`、`summary`、`body`，与普通 user 与 AI 项区分。当 chunk-building 启用 teammate 嵌入（`EMBED_TEAMMATES=true`）时，display 记录 MUST 落点为 `AIChunk.teammate_messages: Vec<TeammateMessage>`，**不再**作为顶层 `UserChunk` 或独立 `Chunk` variant。

`TeammateMessage` 结构 MUST 同时携带以下派生字段，由数据层在 chunk-building 阶段预算并直接落到 IPC payload，让 UI 端无须重复实现检测逻辑：

- `reply_to_tool_use_id: Option<String>` —— 若该 teammate 消息是回复某条 SendMessage tool_use，记录其 `tool_use_id`；否则为 `None`（详见 `Link teammate messages to triggering SendMessage`）。
- `is_noise: bool` —— 是否运维噪声（idle / shutdown / terminated 类）（详见 `Detect operational noise and resend in teammate messages`）。
- `is_resend: bool` —— 是否检测到重复发送关键词。
- `token_count: Option<u64>` —— 该 teammate body 灌入主 session 的 token 估算（优先取 `usage.input_tokens` 真实值；缺失时 fallback 到 body 字符数 ÷ 4 启发式）。
- `uuid: String` —— 来自原始 user 消息 uuid，让前端可按 uuid 反向定位 / 搜索命中。
- `timestamp: DateTime<Utc>` —— 来自原始 user 消息 timestamp。

#### Scenario: Teammate id and color present
- **WHEN** 一条 teammate 消息携带 `teammate_id="alice"` 与 `color="blue"`
- **THEN** 产出的 `TeammateMessage` SHALL 暴露 `teammate_id="alice"` 与 `color=Some("blue")`，使消费者可渲染独立卡片

#### Scenario: Teammate message lands in AIChunk.teammate_messages
- **WHEN** chunk-building 启用 `EMBED_TEAMMATES=true`，输入流含一条 teammate user 消息（在 SendMessage 之后）
- **THEN** 解析得到的 `TeammateMessage` SHALL 出现在下一个 flush 出的 `AIChunk.teammate_messages` 中，且 SHALL 携带 `uuid` / `teammate_id` / `color` / `summary` / `body` / `timestamp` / `reply_to_tool_use_id` / `is_noise` / `is_resend` / `token_count` 全部字段

### Requirement: Recognize team coordination tools

系统 SHALL 把以下 tool name 识别为团队协作工具，并把它们的 summary 经 team-specific 格式化器输出：`TeamCreate`、`TaskCreate`、`TaskUpdate`、`TaskList`、`TaskGet`、`SendMessage`、`TeamDelete`。

#### Scenario: TaskCreate invocation
- **WHEN** 出现一次 `TaskCreate` 工具调用
- **THEN** 其 summary SHALL 含 task name 与 assignee

#### Scenario: SendMessage with shutdown_response
- **WHEN** 一次 `SendMessage` 工具调用是 `approve=true` 的 shutdown_response
- **THEN** 系统 SHALL 把它视作 session 结束信号，而非进行中活动

### Requirement: Enrich subagent processes with team metadata

系统 SHALL 在 spawn 上下文中含 team info（来自 Task 调用 input 或匹配的 `teammate_spawned` 工具结果）时，把 subagent Process 的 `team` 字段填上 `{ teamName, memberName, memberColor }`。

#### Scenario: Task call carries team metadata
- **WHEN** spawn 用的 Task 调用 input 含 team name 与 member name
- **THEN** `Process.team` SHALL 被填入相应字段

### Requirement: Distinguish teammates from regular subagents via Process metadata

系统 SHALL 通过 `Process.team` 字段是否填充使调用方区分 teammate 进程与普通 subagent：填充了 `team` 的 subagent 是 teammate，未填充的是普通 subagent。计数关注点归调用方——数据层 SHALL 仅暴露原始字段，不预算分类汇总。

#### Scenario: Inspect Process.team to classify
- **WHEN** 一个 AIChunk spawn 了 3 个 subagent，其中 2 个携带 team 元数据
- **THEN** 调用方 SHALL 能基于 `Process.team` 是否存在过滤出 "1 普通 subagent + 2 teammate"

#### Scenario: No team metadata available
- **WHEN** 任何 spawn 出的 subagent 都不带 team 元数据
- **THEN** 所有条目 SHALL 以 `Process.team = undefined` 出现，调用方 SHALL 视它们为普通 subagent

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

### Requirement: Expose teammate messages on AIChunk

`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk` MUST 暴露新字段 `teammateMessages: TeammateMessage[]`（camelCase 序列化）。无 teammate 嵌入的 AIChunk MUST 通过 `#[serde(skip_serializing_if = "Vec::is_empty")]` 在 IPC payload 中省略该字段，保持老前端 / 老 fixture 兼容。

`TeammateMessage` IPC schema MUST 含以下字段（camelCase 序列化、字段语义详见 `team-coordination-metadata::Render teammate messages as dedicated items`）：

| 字段                 | 类型             | 说明                                                  |
| -------------------- | ---------------- | ----------------------------------------------------- |
| `uuid`               | `string`         | 来自原始 user 消息 uuid                              |
| `teammateId`         | `string`         | 队友标识                                              |
| `color`              | `string \| null` | 队友色（teamColors 调色板键）                        |
| `summary`            | `string \| null` | 队友自填主题                                          |
| `body`               | `string`         | 队友消息正文（已 trim 标签）                         |
| `timestamp`          | `string`         | ISO8601                                               |
| `replyToToolUseId`   | `string \| null` | 配对的 SendMessage tool_use_id；orphan 时为 null      |
| `tokenCount`         | `number \| null` | body 灌入主 session 的 token 估算                    |
| `isNoise`            | `boolean`        | 运维噪声（idle / shutdown / terminated）             |
| `isResend`           | `boolean`        | 是否检测到重复发送关键词                              |

序列化约定（与本 spec `Expose project and session queries` 既有 camelCase 约定一致）：`teammateMessages` / `teammateId` / `replyToToolUseId` / `tokenCount` / `isNoise` / `isResend` 全部 camelCase。

HTTP API 路径（`GET /projects/:id/sessions/:sid`）SHALL 同步暴露 `teammateMessages` 字段——与 IPC 路径共享 `LocalDataApi::get_session_detail` 实现，自动适用。

回滚开关：`cdt-analyze::chunk::builder` 顶部 `const EMBED_TEAMMATES: bool = true;`；为 `false` 时所有 `AIChunk.teammateMessages` SHALL 为 `[]`（IPC 序列化省略字段），等价于本 change 落地前的 payload 形态。

#### Scenario: AIChunk with teammate replies serializes teammateMessages
- **WHEN** `get_session_detail` 返回的某 AIChunk 含 2 条 teammate 嵌入
- **THEN** 该 chunk 的 JSON SHALL 含 `"teammateMessages": [{...}, {...}]`，每条 object SHALL 含全部 10 个 camelCase 字段

#### Scenario: AIChunk without teammate omits the field
- **WHEN** `get_session_detail` 返回的某 AIChunk 无 teammate 嵌入
- **THEN** 该 chunk 的 JSON SHALL NOT 含 `teammateMessages` 键（由 `skip_serializing_if = "Vec::is_empty"` 控制），与本 change 落地前 payload 形态一致

#### Scenario: Orphan teammate has null replyToToolUseId
- **WHEN** 某 teammate 嵌入未配对到任何 SendMessage（orphan）
- **THEN** 其 IPC JSON 字段 `replyToToolUseId` SHALL 为 `null`

#### Scenario: EMBED_TEAMMATES=false reverts payload shape
- **WHEN** 编译期常量 `EMBED_TEAMMATES = false`
- **THEN** 所有 AIChunk 的 IPC JSON SHALL NOT 含 `teammateMessages` 键，与本 change 落地前的 payload 形态等价

### Requirement: Expose teammate spawn metadata on ToolExecution

`get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.toolExecutions[i]` MUST 暴露新字段 `teammateSpawn?: TeammateSpawnInfo | null`（camelCase 序列化）。无 spawn 信息的 ToolExecution MUST 通过 `#[serde(skip_serializing_if = "Option::is_none")]` 在 IPC payload 中省略该字段，保持老前端 / 老 fixture 兼容。

`TeammateSpawnInfo` IPC schema MUST 含以下字段：

| 字段     | 类型                | 说明                                  |
| -------- | ------------------- | ------------------------------------- |
| `name`   | `string`            | 队友成员名（如 `"member-1"`）        |
| `color`  | `string \| null`    | 队友色（teamColors 调色板键）        |

字段语义详见 `tool-execution-linking::Detect teammate-spawned tool results`。

#### Scenario: Tool execution with teammate spawn populates teammateSpawn
- **WHEN** `get_session_detail` 返回的某 AIChunk 含一条 ToolExecution，对应 user msg `tool_use_result.status == "teammate_spawned"`、`name="member-1"`、`color="blue"`
- **THEN** 该 toolExecution JSON SHALL 含 `"teammateSpawn":{"name":"member-1","color":"blue"}`

#### Scenario: Tool execution without spawn omits teammateSpawn
- **WHEN** `get_session_detail` 返回的某 ToolExecution 无 spawn 信息
- **THEN** 该 toolExecution JSON SHALL NOT 含 `teammateSpawn` 键

### Requirement: Strip teammate-message tags from session title

`extract_session_metadata` 提取的 `SessionSummary.title` MUST 在做长度截断之前剥除任何 `<teammate-message ...>...</teammate-message>` 包裹片段，避免 sidebar 标题吐出原始 XML。

实现 SHALL 在 `cdt-api::session_metadata` 标题提取路径中完成两步——先调 `extract_teammate_summary_title` 跑 fast-path；未命中（非 teammate 主导，或主导但 summary + body 都空）再走 `sanitize_for_title` 跑 fallback 整段剥标签。两个 helper 是独立函数，调用顺序由 `extract_session_metadata_with_ongoing` / `extract_session_metadata_from_parsed` 统一编排：

1. **Fast-path（teammate 主导消息）**：若 trim 后 text 以 `<teammate-message` 开头，SHALL 按以下优先级提取标题候选：
   - **优先 `summary` 属性**：regex 抽 `summary="..."` 属性内容，非空时 SHALL 直接返回作为标题候选（截断长度由常量 `TITLE_MAX_CHARS` 控制）
   - **fallback 到 body 文本**（2026-05-21 修订）：`summary` 属性缺失或值为空时，SHALL 提取开标签 `>` 与闭标签 `</teammate-message>` 之间的 body 文本（无闭标签时取剩余全部），trim 后非空则作为标题候选。**只有** body 也为空时才退回到下一步。
   理由：用户实测发现"teammate-message 主导消息但作者忘写 `summary` 属性"是常见场景（如 `<teammate-message teammate_id="team-lead">用户在 Claude Code auto mode 下调用 codex:codex-rescue 被拦截...</teammate-message>`），body 才是真实对话内容；初版 spec 直接剥除整段会让 title 永久 null，UI 列表项 fallback 到 sessionId 前缀让用户无法识别 session。

2. **Fallback（剥标签）**：若 fast-path 完全未命中（非 teammate 主导，或主导但 summary + body 都空），SHALL 在既有标签剥除循环中追加 `teammate-message` 标签——把整段 `<teammate-message ...>body</teammate-message>` 从文本中删除（含 attributes 与 inner body）。剥除后若文本为空，SHALL 回退到 `command_fallback` 或 `None`，按既有路径处理。

   注意：此 fallback 仅在 **非 teammate 主导消息**（混合内容中嵌入 teammate 块）时触发；teammate 主导消息已在 fast-path 由 summary 或 body 兜底，不会落到这一步剥除 body。

`sanitize_for_title` MUST 不再在标题里输出任何 `<teammate-message` / `</teammate-message>` 字面量。

#### Scenario: Title takes summary attribute when message is wrapped solely by teammate-message
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice" summary="Set up project">body</teammate-message>`
- **THEN** `extract_session_metadata.title` SHALL 为 `Some("Set up project")`

#### Scenario: Title falls back to body when teammate-message has no summary
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice">用户在 auto mode 下调用 codex 被拦截</teammate-message>`（无 summary 属性，body 非空）
- **THEN** `extract_session_metadata.title` SHALL 为 `Some("用户在 auto mode 下调用 codex 被拦截")`
- **AND** title SHALL NOT 含 `<teammate-message` / `</teammate-message>` 字面量

#### Scenario: Title returns None when teammate-message has no summary and empty body
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice"></teammate-message>`（无 summary 属性，body 也空）
- **THEN** `extract_session_metadata.title` SHALL 为 `None` 或退回到 `command_fallback`

#### Scenario: Mixed content strips teammate-message tag
- **WHEN** 第一条 user 消息 content 为 `Hello team. <teammate-message teammate_id="alice">body</teammate-message> please continue.`
- **THEN** title SHALL 不含 `<teammate-message`，剥除后 SHALL 仅保留 `Hello team.  please continue.`（trim 后），整体走既有截断路径
- **AND** 此场景下 teammate 块按 fallback 路径整段剥除（含 body），与 teammate 主导消息的 body fallback 行为不冲突

### Requirement: Expose subagent messages total count

`Process` / `SubagentProcess` 序列化 IPC payload MUST 含 `messagesTotalCount: u32` 字段（Rust 端字段名 `messages_total_count`，`#[serde(rename = "messagesTotalCount")]`），记录 subagent JSONL 内**裁剪前**的完整 `Vec<Chunk>` 长度（`cand.messages.len()`）。该字段 SHALL 在 `OMIT_SUBAGENT_MESSAGES=true`（默认裁剪路径）与 `OMIT_SUBAGENT_MESSAGES=false`（回滚路径）下行为一致——始终等于 subagent session build_chunks 后的 chunk 数。

该字段是前端 SubagentCard 在 `messagesOmitted=true` 下的唯一"messages 数量是否变化"的版本指纹来源；前端 SHALL 用 `(isOngoing, endTs, messagesTotalCount)` 三元组判定 trace 版本，版本递增即代表 subagent 内部有新 chunk 写入。

`messages_total_count` MUST 在 `candidate_to_process` 阶段（`cdt-analyze::tool_linking::resolver`）由 `cand.messages.len() as u32` 填充——与 `header_model` / `last_isolated_tokens` / `is_shutdown_only` 同阶段。IPC 层在 `apply_subagent_messages_omit` 之前 SHALL 保证该字段已填，避免裁剪 messages 后再读 length 永远是 0。

#### Scenario: messagesTotalCount in OMIT default path

- **WHEN** `OMIT_SUBAGENT_MESSAGES=true`，`Process` 由 subagent session 含 7 个 chunk 的 candidate 构造
- **THEN** IPC 序列化 JSON SHALL 含 `"messagesTotalCount": 7`、`"messagesOmitted": true`、`"messages": []`

#### Scenario: messagesTotalCount in rollback path

- **WHEN** `OMIT_SUBAGENT_MESSAGES=false`，同一 candidate 构造 `Process`
- **THEN** IPC 序列化 JSON SHALL 含 `"messagesTotalCount": 7`、`"messagesOmitted": false`、`"messages": <length=7>`

#### Scenario: messagesTotalCount 反映 ongoing subagent 内部增长

- **WHEN** 同一 subagent session 经两次 `get_session_detail`：第一次扫描时含 5 chunk，第二次扫描时（中间有 file-change 触发）含 8 chunk
- **THEN** 两次 IPC 响应中对应 `Process.messagesTotalCount` SHALL 分别为 `5` 与 `8`；前端可据此版本递增判定需要重拉 trace

#### Scenario: 嵌套 subagent 各自暴露 messagesTotalCount

- **WHEN** subagent A 的 messages 内嵌套含一条 subagent B 的引用，`get_subagent_trace` 返回 A 的 trace 含 B 的 `Process` 占位
- **THEN** A 与 B 的 `Process` MUST 各自携带独立的 `messagesTotalCount`，B 的值等于其自身 JSONL build_chunks 后的 chunk 数

