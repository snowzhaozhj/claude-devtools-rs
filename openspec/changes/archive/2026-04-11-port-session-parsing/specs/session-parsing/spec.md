## MODIFIED Requirements

### Requirement: Deduplicate streaming entries by requestId

The system SHALL deduplicate assistant messages that share the same `requestId`, keeping the entry that appears **last in file order** (i.e. the most recent streaming rewrite), so that partial streaming rewrites are not counted twice. The deduplication function SHALL be invoked from the main file-parsing path; an unreachable dedup helper is not considered compliant.

#### Scenario: Two assistant entries with the same requestId
- **WHEN** two assistant entries share the same `requestId`
- **THEN** exactly one `ParsedMessage` SHALL be emitted for that `requestId`, and it SHALL be the entry that appeared **last** in the JSONL file

#### Scenario: Three entries with the same requestId, interleaved with other messages
- **WHEN** the JSONL file contains assistant entries `A1 (req=x)`, `U1`, `A2 (req=x)`, `U2`, `A3 (req=x)` in that order
- **THEN** the emitted stream SHALL contain exactly `A3`, `U1`, `U2` (with `A3` at the position it originally occupied), and `A1` / `A2` SHALL NOT be emitted

#### Scenario: Non-assistant messages with a requestId
- **WHEN** a non-assistant message (e.g. a user or system entry) carries a `requestId` field
- **THEN** deduplication SHALL NOT apply to that message and it SHALL pass through unchanged

#### Scenario: Dedup is actually wired into the parse pipeline
- **WHEN** a session file is parsed via the public file-parsing entry point
- **THEN** the deduplication pass SHALL run automatically before results are exposed to callers, without the caller having to invoke it explicitly

## ADDED Requirements

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
