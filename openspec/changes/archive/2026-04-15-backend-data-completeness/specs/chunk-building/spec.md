## ADDED Requirements

### Requirement: Extract slash commands from isMeta messages

The system SHALL 在构建 chunks 时从 `isMeta=true` 的 user 消息中提取 slash 命令信息。Slash 命令通过 `<command-name>/xxx</command-name>` XML 标签识别，提取 name、message（`<command-message>`）和 args（`<command-args>`）。

提取的 slash 命令 SHALL 附加到紧随其后的 `AIChunk` 的 `slash_commands` 字段中。若 isMeta 消息不含 slash 命令格式，SHALL 静默跳过。

#### Scenario: isMeta message with slash command
- **WHEN** an isMeta user message contains `<command-name>/commit</command-name>`
- **THEN** the system SHALL extract a `SlashCommand` with `name="commit"` and attach it to the next `AIChunk.slash_commands`

#### Scenario: isMeta message with slash command including message and args
- **WHEN** an isMeta user message contains `<command-name>/review-pr</command-name><command-message>review-pr</command-message><command-args>123</command-args>`
- **THEN** the extracted `SlashCommand` SHALL have `name="review-pr"`, `message=Some("review-pr")`, `args=Some("123")`

#### Scenario: isMeta message without slash format
- **WHEN** an isMeta user message contains a system-reminder injection without `<command-name>` tags
- **THEN** no `SlashCommand` SHALL be extracted and the message SHALL be handled as before (tool_result merge or skip)

#### Scenario: Slash command with no following AIChunk
- **WHEN** a slash command is extracted from an isMeta message but no subsequent AIChunk exists
- **THEN** the slash command SHALL be discarded without error

## MODIFIED Requirements

### Requirement: Build independent chunks from classified messages

The system SHALL convert a sequence of `ParsedMessage` into a sequence of independent chunks of four types: `UserChunk`, `AIChunk`, `SystemChunk`, `CompactChunk`. Chunks SHALL NOT be paired — a `UserChunk` does not "own" the following `AIChunk`. 连续的 assistant 消息 SHALL 被合并到同一个 `AIChunk.responses` 中，直到遇到真实用户消息、`SystemChunk` 对应的 `<local-command-stdout>` 消息、`CompactChunk` 对应的 compact summary 消息或输入末尾时 flush。

`AIChunk` SHALL 暴露 `slash_commands: Vec<SlashCommand>` 字段，包含由前述 isMeta 消息中提取的 slash 命令。默认为空数组。

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

#### Scenario: AIChunk includes slash commands from preceding isMeta message
- **WHEN** an isMeta user message with a slash command precedes an assistant response
- **THEN** the resulting `AIChunk` SHALL have the extracted slash command in its `slash_commands` field
