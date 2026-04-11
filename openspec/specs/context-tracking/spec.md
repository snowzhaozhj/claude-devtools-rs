# context-tracking Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Classify context injections into six categories

The system SHALL classify every piece of content that consumes Claude's context window into exactly one of six categories: `claude-md`, `mentioned-file`, `tool-output`, `thinking-text`, `team-coordination`, `user-message`.

#### Scenario: CLAUDE.md content injected at session start
- **WHEN** a session loads CLAUDE.md content (global, project, or directory scope)
- **THEN** the system SHALL record a `claude-md` injection with the file path, scope, and token count

#### Scenario: User references a file with @ mention
- **WHEN** a user message references a file via `@path/to/file`
- **THEN** the system SHALL record a `mentioned-file` injection once the file content has been loaded

#### Scenario: Read tool returns file content
- **WHEN** a Read tool produces output
- **THEN** the system SHALL record a `tool-output` injection sized by the output token count

#### Scenario: Extended thinking block appears
- **WHEN** an assistant response contains a thinking block
- **THEN** the system SHALL record a `thinking-text` injection for both the thinking and the subsequent text tokens

#### Scenario: TeamCreate or SendMessage invocation
- **WHEN** a team coordination tool is invoked
- **THEN** the system SHALL record a `team-coordination` injection for its argument and result tokens

#### Scenario: Real user prompt in a new turn
- **WHEN** a real user message creates a new turn
- **THEN** the system SHALL record a `user-message` injection sized by its token count

### Requirement: Compute cumulative context statistics per turn

The system SHALL compute, for every turn, the total tokens currently visible in the context window, broken down by the six categories.

#### Scenario: Turn with CLAUDE.md + two tool outputs + user message
- **WHEN** a turn contains those four injections
- **THEN** the per-turn stats SHALL sum their token counts into the matching category fields and expose a total

### Requirement: Reset accumulated context on compaction boundaries

The system SHALL treat compact items (derived from compact summary boundary messages via the chunk pipeline) as context phase boundaries and restart injection accumulation after each boundary, while preserving a record of the prior phase.

#### Scenario: Session with one compaction mid-way
- **WHEN** a compaction occurs in the middle of a session
- **THEN** injections after the boundary SHALL accumulate from zero, and a ContextPhaseInfo record SHALL capture the phase that ended

### Requirement: Expose context stats to display surfaces

The system SHALL expose per-turn context stats, per-category cumulative tokens, and phase history through a stable data structure consumable by UI badges, hover breakdowns, and full context panels.

#### Scenario: Query context stats for a specific turn
- **WHEN** a caller requests context stats at turn index N
- **THEN** the result SHALL include tokensByCategory, total tokens, active phase id, and the underlying injection list for that turn

