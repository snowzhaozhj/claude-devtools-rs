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

The system SHALL deduplicate assistant messages that share the same `requestId`, keeping the last complete entry, to avoid counting partial streaming rewrites twice.

#### Scenario: Two entries with same requestId
- **WHEN** two assistant entries share the same requestId
- **THEN** only one ParsedMessage SHALL be emitted for that requestId

### Requirement: Classify hard noise messages

The system SHALL mark messages that must never be rendered as hard noise, including: `system` / `summary` / `file-history-snapshot` / `queue-operation` entries, assistant messages with `model='<synthetic>'`, user messages wrapped solely in `<local-command-caveat>` or `<system-reminder>`, empty command-output messages, and interrupt markers.

#### Scenario: Synthetic assistant placeholder
- **WHEN** an assistant message has `model='<synthetic>'`
- **THEN** it SHALL be classified as hard noise and excluded from all downstream rendering

#### Scenario: Interrupt marker
- **WHEN** a user message content begins with `[Request interrupted by user`
- **THEN** it SHALL be classified as hard noise

