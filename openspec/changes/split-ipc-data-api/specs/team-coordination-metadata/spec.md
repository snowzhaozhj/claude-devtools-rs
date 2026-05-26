# team-coordination-metadata Specification (delta)

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

