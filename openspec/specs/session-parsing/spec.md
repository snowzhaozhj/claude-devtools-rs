# session-parsing Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Stream JSONL session files line by line

The system SHALL parse Claude Code session files as newline-delimited JSON, processing one entry at a time without loading the whole file into memory.

#### Scenario: Large session file
- **WHEN** a session file is larger than 100MB
- **THEN** parsing SHALL complete without loading the entire file into memory and SHALL produce parsed messages in file order

#### Scenario: Malformed line in middle of file
- **WHEN** a single line contains invalid JSON
- **THEN** the system SHALL skip that line, log a parse warning with line number, and continue parsing subsequent lines

#### Scenario: Empty file
- **WHEN** the file exists but is empty
- **THEN** the system SHALL return an empty parsed message list without error

### Requirement: Produce ParsedMessage records

The system SHALL convert each JSONL entry into a ParsedMessage record containing at minimum: uuid, parentUuid, type, timestamp, content (string or block array), usage (if present), model (if present), cwd, gitBranch, isSidechain, isMeta, and extracted toolCalls / toolResults.

#### Scenario: Assistant message with tool_use blocks
- **WHEN** a JSONL entry has `type=assistant` and its content contains `tool_use` blocks
- **THEN** the ParsedMessage SHALL expose each tool call with id, name, input, and `isTask=true` only if the tool name is `Task`

#### Scenario: User message with tool_result blocks
- **WHEN** a JSONL entry has `type=user` with content containing `tool_result` blocks
- **THEN** the ParsedMessage SHALL populate toolResults with toolUseId, content, and isError fields, and SHALL be classified as internal user (isMeta semantics apply)

#### Scenario: Compact summary boundary
- **WHEN** a JSONL entry is a compact-summary boundary message
- **THEN** the ParsedMessage SHALL set `isCompactSummary=true`

### Requirement: Support both legacy and current content formats

The system SHALL accept user message content as either a plain string (older sessions) or an array of content blocks (newer sessions) and expose both via the same ParsedMessage.content field.

#### Scenario: Legacy string content
- **WHEN** a user entry's content is a plain string
- **THEN** the ParsedMessage.content SHALL be that string verbatim

#### Scenario: Current block array content
- **WHEN** a user entry's content is an array with text and image blocks
- **THEN** the ParsedMessage.content SHALL be the array, preserving block types and order

### Requirement: Deduplicate streaming entries by requestId

The system SHALL NOT deduplicate assistant messages by `requestId` in the main file-parsing path. Claude Code 的实际 JSONL 里，同一个 `requestId` 被用作"同一次 API response 的 grouping key"：一次响应的多个 content block（`thinking` / `text` / 各 `tool_use`）会被写成多条独立的 `assistant` 记录，**并非** streaming rewrite 的部分快照。在 parse 阶段按 `requestId` 合并或丢弃，会丢失有独立 `tool_use` 的记录（进而导致 subagent 匹配数变少）。

`dedupe_by_request_id` 函数 MAY 保留在 `cdt-parse` 中，但 SHALL 仅在需要避免 `usage` 字段重复计数的 metrics 计算路径中被手动调用，SHALL NOT 在 `parse_file` 的公开入口上自动运行。

#### Scenario: parse_file 保留同 requestId 的所有记录
- **WHEN** 一个 JSONL 文件包含两条或多条共享同一 `requestId` 的 assistant 记录，每条承载不同的 content block（例如独立的 `tool_use`）
- **THEN** `parse_file` SHALL 返回这些记录的全部 `ParsedMessage`，按文件顺序保留每一条

#### Scenario: 同 requestId 多条带 tool_use 的记录各自保留
- **WHEN** 同一 `requestId` 下有一条 `thinking` 记录、一条 `text` 记录、两条不同 `tool_use` 记录
- **THEN** `parse_file` 返回的 `ParsedMessage` 数量 SHALL 等于记录数；所有 `tool_use` 均被保留，便于下游 `chunk-building` 和 `tool-execution-linking` 正确匹配

#### Scenario: dedupe_by_request_id 仍作为 metrics 辅助函数可用
- **WHEN** 上层代码在计算 session metrics 时希望规避 usage 字段跨重复记录累加
- **THEN** 仍可调用 `cdt_parse::dedupe_by_request_id(&messages)`；该函数行为与旧实现一致（保留同 requestId 的最后一条 assistant 记录），但 `parse_file` 不再自动调用它

### Requirement: Emit parse warnings with line numbers on malformed input

The system SHALL, when encountering a line that fails JSON parsing, emit a warning containing the file path (when available) and the 1-based line number, then continue processing subsequent lines. The emitted `ParsedMessage` stream SHALL NOT include any placeholder for the malformed line.

#### Scenario: Single malformed line in the middle of a file
- **WHEN** a JSONL file has a malformed line at position N and valid lines before and after
- **THEN** the parser SHALL log a warning identifying line N as malformed, SHALL skip only that line, and SHALL emit parsed messages for every other line in original file order

#### Scenario: Two adjacent malformed lines
- **WHEN** lines N and N+1 are both malformed
- **THEN** both SHALL be skipped with one warning per line, and valid lines on either side SHALL still be emitted

### Requirement: Expose both a per-line and a per-file parsing API

The system SHALL expose a synchronous per-line entry point for parsing a single JSONL record and an asynchronous per-file entry point returning the full parsed message sequence. Both SHALL produce the same `ParsedMessage` shape and agree on `MessageCategory` classification for equivalent input.

#### Scenario: Per-line entry point parses a valid assistant message
- **WHEN** a caller passes a single well-formed JSONL assistant entry to the per-line entry point
- **THEN** the entry point SHALL return a `ParsedMessage` whose category reflects the assistant classification and whose tool calls match the block contents

#### Scenario: Per-file entry point agrees with per-line entry point
- **WHEN** the same byte sequence is parsed once through the per-file entry point and once line-by-line through the per-line entry point (excluding requestId deduplication)
- **THEN** the produced `ParsedMessage` values SHALL be equal field-for-field in the same order

### Requirement: Classify hard noise messages

The system SHALL mark messages that must never be rendered as hard noise, including: `system` / `summary` / `file-history-snapshot` / `queue-operation` entries, assistant messages with `model='<synthetic>'`, user messages wrapped solely in `<local-command-caveat>` or `<system-reminder>`, empty command-output messages, and interrupt markers.

#### Scenario: Synthetic assistant placeholder
- **WHEN** an assistant message has `model='<synthetic>'`
- **THEN** it SHALL be classified as hard noise and excluded from all downstream rendering

#### Scenario: Interrupt marker
- **WHEN** a user message content begins with `[Request interrupted by user`
- **THEN** it SHALL be classified as hard noise

