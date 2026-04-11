## ADDED Requirements

### Requirement: Expose a pure synchronous API driven by chunk output

The system SHALL expose context-tracking as a pure synchronous API that consumes an in-memory chunk sequence and an externally injected dictionary of per-file token counts, and SHALL NOT perform any filesystem I/O, network calls, or other side effects during context computation. The API SHALL be callable from non-async code paths and SHALL NOT require a runtime such as `tokio`.

#### Scenario: Library consumer calls the API from a sync context

- **WHEN** a caller on a non-async thread invokes `process_session_context_with_phases(chunks, params)` with a borrowed chunk slice and a `ProcessSessionParams` whose token dictionaries are already populated
- **THEN** the function SHALL return a `SessionContextResult` without spawning tasks, awaiting futures, or touching the filesystem

#### Scenario: Missing token data falls back to zero without error

- **WHEN** the injected `claude_md_token_data` / `mentioned_file_token_data` / `directory_token_data` maps do not contain a key referenced by a chunk
- **THEN** the corresponding injection SHALL still be produced with `estimated_tokens = 0`, and the function SHALL NOT return an error, panic, or log higher than `debug`

#### Scenario: Empty chunk slice yields empty result

- **WHEN** the chunk slice is empty
- **THEN** the returned `SessionContextResult` SHALL have an empty `stats_map`, an empty `phase_info.phases` list, and `phase_info.compaction_count == 0`

### Requirement: Estimate token counts with a Unicode-scalar heuristic

The system SHALL provide a single canonical token estimation function `estimate_tokens(text)` whose result equals `⌈scalar_count(text) / 4⌉`, where `scalar_count` counts Unicode scalar values (not UTF-8 bytes, not grapheme clusters). The function SHALL return `0` for empty or missing input. All context-tracking code paths —and any other crate needing a rough token estimate— SHALL use this function rather than rolling their own heuristic.

#### Scenario: ASCII text of length 16 estimates to 4 tokens

- **WHEN** `estimate_tokens("abcdefghijklmnop")` is called
- **THEN** the result SHALL be `4`

#### Scenario: Empty and whitespace-only inputs

- **WHEN** the input is an empty string
- **THEN** the result SHALL be `0`

- **WHEN** the input is `"   "` (three spaces)
- **THEN** the result SHALL be `1` (⌈3/4⌉)

#### Scenario: Multi-byte scalar counts by scalar, not byte

- **WHEN** `estimate_tokens("你好世界")` is called (4 Han characters)
- **THEN** the result SHALL be `1` (⌈4/4⌉), not `3` (⌈12/4⌉)

#### Scenario: JSON-valued content is stringified before estimating

- **WHEN** `estimate_content_tokens(value)` is called with a JSON array `[1, 2, 3]`
- **THEN** the function SHALL stringify the value and return `estimate_tokens("[1,2,3]")` (which is `2`)

## MODIFIED Requirements

### Requirement: Compute cumulative context statistics per turn

The system SHALL compute, for every turn, the total tokens currently visible in the context window, broken down by the six categories. An empty AI group (no steps, no responses, no preceding user group) SHALL still produce a `ContextStats` record with all six category token counts equal to zero and a total of zero, rather than being skipped.

#### Scenario: Turn with CLAUDE.md + two tool outputs + user message

- **WHEN** a turn contains those four injections
- **THEN** the per-turn stats SHALL sum their token counts into the matching category fields and expose a total

#### Scenario: Empty AI group still produces a zeroed stats record

- **WHEN** a turn contains an AI group with no steps, no responses, and no preceding user message
- **THEN** the per-turn stats SHALL be produced with `tokens_by_category.*` all equal to `0`, `total_estimated_tokens == 0`, and an empty `new_injections` list, rather than being absent from the stats map

### Requirement: Reset accumulated context on compaction boundaries

The system SHALL treat compact items (derived from compact summary boundary messages via the chunk pipeline) as context phase boundaries and restart injection accumulation after each boundary, while preserving a record of the prior phase. When a compaction boundary is followed by at least one subsequent AI group, the system SHALL additionally compute a `CompactionTokenDelta` that records the pre-compaction and post-compaction total token counts derived from the assistant `usage` of the last AI group before the boundary and the first AI group after it.

#### Scenario: Session with one compaction mid-way

- **WHEN** a compaction occurs in the middle of a session
- **THEN** injections after the boundary SHALL accumulate from zero, and a `ContextPhaseInfo` record SHALL capture the phase that ended

#### Scenario: First AI group after compaction records a compaction token delta

- **WHEN** the session contains `[AI_1, compact, AI_2]` where `AI_1.last_assistant.usage.total == 1000` and `AI_2.first_assistant.usage.total == 600`
- **THEN** `phase_info.compaction_token_deltas` SHALL contain exactly one entry keyed by the compact chunk id, with `pre_compaction_tokens == 1000`, `post_compaction_tokens == 600`, and `delta == -400`

#### Scenario: Compaction at the very end of a session does not produce a delta

- **WHEN** the session contains `[AI_1, compact]` with no AI group after the compact chunk
- **THEN** `phase_info.compaction_token_deltas` SHALL NOT contain any entry for that compact chunk, and the phase that ended SHALL still be finalized in `phase_info.phases`
