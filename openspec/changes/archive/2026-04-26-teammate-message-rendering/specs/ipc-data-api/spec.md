## ADDED Requirements

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
- **WHEN** `get_session_detail` 返回的某 AIChunk 含一条 ToolExecution，对应 user msg `tool_use_result.status == "teammate_spawned"` 且 name=`"member-1"` color=`"blue"`
- **THEN** 该 toolExecution JSON SHALL 含 `"teammateSpawn":{"name":"member-1","color":"blue"}`

#### Scenario: Tool execution without spawn omits teammateSpawn
- **WHEN** `get_session_detail` 返回的某 ToolExecution 无 spawn 信息
- **THEN** 该 toolExecution JSON SHALL NOT 含 `teammateSpawn` 键

### Requirement: Strip teammate-message tags from session title

`extract_session_metadata` 提取的 `SessionSummary.title` MUST 在做长度截断之前剥除任何 `<teammate-message ...>...</teammate-message>` 包裹片段，避免 sidebar 标题吐出原始 XML。

实现 SHALL 在 `cdt-api::session_metadata::sanitize_for_title` 同函数内完成两步：

1. **Fast-path（teammate 主导消息）**：若 trim 后 text 以 `<teammate-message` 开头，先 regex 抽 `summary="..."` 属性内容；非空时 SHALL 直接返回 summary 内容作为标题候选（截断同既有 200 字符上限）。
2. **Fallback（剥标签）**：若 fast-path 未命中（无 summary 属性 / 文本含混合内容），SHALL 在既有标签剥除循环中追加 `teammate-message` 标签——把整段 `<teammate-message ...>body</teammate-message>` 从文本中删除（含 attributes 与 inner body）。剥除后若文本为空，SHALL 回退到 `command_fallback` 或 `None`，按既有路径处理。

`sanitize_for_title` MUST 不再在标题里输出任何 `<teammate-message` / `</teammate-message>` 字面量。

#### Scenario: Title takes summary attribute when message is wrapped solely by teammate-message
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice" summary="Set up project">body</teammate-message>`
- **THEN** `extract_session_metadata.title` SHALL 为 `Some("Set up project")`

#### Scenario: Title falls back when teammate-message has no summary
- **WHEN** session 第一条 user 消息 content 为 `<teammate-message teammate_id="alice">body</teammate-message>`（无 summary 属性）
- **THEN** `extract_session_metadata.title` SHALL NOT 含 `<teammate-message`，且 SHALL 退回 `None` 或 `command_fallback`

#### Scenario: Mixed content strips teammate-message tag
- **WHEN** 第一条 user 消息 content 为 `Hello team. <teammate-message teammate_id="alice">body</teammate-message> please continue.`
- **THEN** title SHALL 不含 `<teammate-message`，剥除后 SHALL 仅保留 `Hello team.  please continue.`（trim 后），整体走既有截断路径
