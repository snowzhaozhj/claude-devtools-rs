## MODIFIED Requirements

### Requirement: Build independent chunks from classified messages

The system SHALL convert a sequence of `ParsedMessage` into a sequence of independent chunks of four types: `UserChunk`, `AIChunk`, `SystemChunk`, `CompactChunk`. Chunks SHALL NOT be paired — a `UserChunk` does not "own" the following `AIChunk`. 连续的 assistant 消息 SHALL 被合并到同一个 `AIChunk.responses` 中，直到遇到真实用户消息、`SystemChunk` 对应的 `<local-command-stdout>` 消息、`CompactChunk` 对应的 compact summary 消息或输入末尾时 flush。

`AIChunk` SHALL 暴露 `slash_commands: Vec<SlashCommand>` 字段，包含紧邻前一条 slash user 消息提取的 slash 命令（详见 `Emit slash commands as both UserChunk and AIChunk.slash_commands` Requirement）。默认为空数组。

`AIChunk` SHALL 同时暴露 `teammate_messages: Vec<TeammateMessage>` 字段，包含被嵌入到该 turn 的 teammate user 消息（详见 `Embed teammate messages into AIChunk` Requirement）。默认为空数组——序列化时通过 `skip_serializing_if = "Vec::is_empty"` 在无 teammate 时省略字段，保持老快照与老 IPC payload 兼容。

#### Scenario: User question followed by AI response
- **WHEN** the input is a real user message followed by one assistant message
- **THEN** the output SHALL be one `UserChunk` and one `AIChunk` as independent entries, in input order

#### Scenario: Multiple assistant turns before next user input
- **WHEN** several assistant messages appear consecutively without intervening real user input
- **THEN** they SHALL be coalesced into a single `AIChunk` whose `responses` field holds all assistant messages in chronological order

#### Scenario: Assistant buffer flushed by following user message
- **WHEN** an assistant buffer of N responses is followed by a real user message
- **THEN** the system SHALL emit the accumulated `AIChunk` before the new `UserChunk`

#### Scenario: Command output appears inline
- **WHEN** a user message whose content is exactly wrapped by `<local-command-stdout>...</local-command-stdout>` appears in the stream
- **THEN** a `SystemChunk` SHALL be emitted for it, not absorbed into a surrounding `AIChunk`, and any in-progress assistant buffer SHALL be flushed first

#### Scenario: AIChunk includes slash commands from preceding slash user message
- **WHEN** a slash user message (content starting with `<command-name>/xxx</command-name>`, `is_meta=false`) immediately precedes an assistant response
- **THEN** the resulting `AIChunk` SHALL have the extracted slash command in its `slash_commands` field

#### Scenario: AIChunk teammate_messages defaults to empty
- **WHEN** an `AIChunk` is built from a turn that contains no teammate reply
- **THEN** its `teammate_messages` field SHALL be an empty `Vec`，且 IPC 序列化结果 SHALL 不包含 `teammateMessages` 键（由 `skip_serializing_if = "Vec::is_empty"` 控制）

## ADDED Requirements

### Requirement: Embed teammate messages into AIChunk

The system SHALL detect user messages classified as teammate replies (per `team-coordination-metadata::Detect teammate messages`) and MUST NOT emit `UserChunk` for them. 检测到的 teammate 消息 SHALL 被解析为 `TeammateMessage` 并嵌入到下一个 flush 出的 `AIChunk.teammate_messages` 中——而不是丢弃（旧行为）。

实现 SHALL 在 chunk-building 主循环中维护 `pending_teammates: Vec<TeammateMessage>` 缓冲区：

1. 遇到 teammate user 消息时，调用 `parse_teammate_attrs` 解析 `teammate_id / color / summary / body`，叠加 `detect_noise` / `detect_resend` 预算结果与 `token_count` 估算，封装成 `TeammateMessage` 并 push 到 `pending_teammates`；**SHALL NOT** flush 当前 assistant buffer，**SHALL NOT** 产 `UserChunk`，主循环继续。
2. 在 `flush_buffer` 即将产出 `AIChunk` 之前，对 `pending_teammates` 内每条 teammate 调用 `link_teammate_to_send_message(&teammate, &chain)`（详见 `team-coordination-metadata::Link teammate messages to triggering SendMessage`），把 `reply_to_tool_use_id` 字段填好。
3. 把 pending teammates 整批 move 到新构造的 `AIChunk.teammate_messages` 字段；清空 `pending_teammates`。
4. 若主循环结束时 `pending_teammates` 非空（无后续 AIChunk 接收），SHALL 把它们追加到最后一个已 emit 的 AIChunk 的 `teammate_messages`；若整个 session 没有任何 AIChunk，SHALL 静默丢弃这些 pending teammate（罕见边界）。

回滚开关：`cdt-analyze::chunk::builder` 顶部 `const EMBED_TEAMMATES: bool = true;`；为 `false` 时 SHALL 退回旧行为——teammate user 消息直接 `continue`，`AIChunk.teammate_messages` 永远为空。该常量 MUST 与 enrichment 函数（`apply_teammate_embed`）和调用点同一轮 Edit 落地，不得分批提交（避免 clippy `dead_code`）。

`TeammateMessage` 结构体定义在 `cdt-core::chunk` 模块（与 `AIChunk` 共生命周期），字段含义详见 `ipc-data-api::Expose teammate messages on AIChunk`。

#### Scenario: Teammate message does not produce UserChunk
- **WHEN** the input is a real user message followed by an assistant turn whose tail user message is `<teammate-message teammate_id="alice" summary="hi">body</teammate-message>`
- **THEN** the output chunks SHALL contain exactly one `UserChunk` (the real user) and one `AIChunk`，且 SHALL NOT 包含任何代表 teammate 消息的 `UserChunk`

#### Scenario: Teammate message embedded into next AIChunk
- **WHEN** the message stream is `assistant(SendMessage→alice) → user(<teammate-message teammate_id="alice" summary="ok">body</teammate-message>) → assistant("got it")`
- **THEN** the resulting chunk list SHALL contain two `AIChunk` entries: 第一个含 SendMessage tool execution；第二个的 `teammate_messages` SHALL 含一条 `TeammateMessage { teammate_id: "alice", summary: Some("ok"), body: "body", reply_to_tool_use_id: Some(<sendmessage_tool_use_id>) }`

#### Scenario: Trailing teammate message attaches to last AIChunk
- **WHEN** the stream ends with a teammate user message after the last AIChunk has been emitted, with no further assistant message
- **THEN** the teammate SHALL be appended to the last emitted `AIChunk.teammate_messages` (instead of being dropped)

#### Scenario: Orphan teammate when no AIChunk exists
- **WHEN** the entire session contains only teammate user messages and no assistant message
- **THEN** the chunk list SHALL be empty, and the teammate messages SHALL be silently discarded; the system SHALL NOT panic

#### Scenario: EMBED_TEAMMATES=false reverts to legacy drop behavior
- **WHEN** the constant `EMBED_TEAMMATES` is set to `false` and chunk-building runs over a session with teammate messages
- **THEN** every teammate user message SHALL be skipped (legacy `continue` behavior), and every produced `AIChunk.teammate_messages` SHALL be empty
