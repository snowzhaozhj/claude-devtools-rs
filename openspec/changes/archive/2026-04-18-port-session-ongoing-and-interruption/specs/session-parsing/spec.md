## MODIFIED Requirements

### Requirement: Classify hard noise messages

The system SHALL mark messages that must never be rendered as hard noise, including: `system` / `summary` / `file-history-snapshot` / `queue-operation` entries, assistant messages with `model='<synthetic>'`, user messages wrapped solely in `<local-command-caveat>` or `<system-reminder>`, and empty command-output messages. 与原版的"interrupt marker 是 hard noise"约定相反，本 port 不再把
interrupt marker 归入 hard noise——interrupt 需保留以供 chunk-building
生成语义步骤以及 session-state 检测使用（详见下一条 Requirement）。

#### Scenario: Synthetic assistant placeholder
- **WHEN** an assistant message has `model='<synthetic>'`
- **THEN** it SHALL be classified as hard noise and excluded from all downstream rendering

#### Scenario: Interrupt marker is NOT hard noise
- **WHEN** a user message content begins with `[Request interrupted by user`
- **THEN** it SHALL NOT be classified as hard noise; it SHALL be classified as `MessageCategory::Interruption` per the next Requirement

## ADDED Requirements

### Requirement: Classify interrupt marker messages

The system SHALL classify any user message whose visible text begins with
`[Request interrupted by user` as `MessageCategory::Interruption`. Interrupt
消息与 hard noise 不同：MUST 保留在 `ParsedMessage` 流中，下游
chunk-building 以其为依据往 `AIChunk.semantic_steps` 追加
`SemanticStep::Interruption`，同时 session-state 检测基于其存在把会话
标记为已结束。

#### Scenario: Interrupt marker in plain text content
- **WHEN** a user JSONL entry's content is the string `[Request interrupted by user for tool use]`
- **THEN** the resulting `ParsedMessage.category` SHALL equal `MessageCategory::Interruption` and the message SHALL NOT be dropped before chunk-building

#### Scenario: Interrupt marker in block content
- **WHEN** a user JSONL entry's content is an array with a single text block whose text begins with `[Request interrupted by user`
- **THEN** the resulting `ParsedMessage.category` SHALL equal `MessageCategory::Interruption`

#### Scenario: Non-interrupt user text is unaffected
- **WHEN** a user JSONL entry's content is plain text like `hello` without the interrupt prefix
- **THEN** the resulting `ParsedMessage.category` SHALL equal `MessageCategory::User` and classification SHALL remain unchanged compared to current behavior
