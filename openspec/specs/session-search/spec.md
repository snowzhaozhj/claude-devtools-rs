# session-search Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Search within a single session

The system SHALL search the textual content of one session for a given query, returning ordered hits with message uuid, offset within content, and a short context preview.

#### Scenario: Query matches text in multiple messages
- **WHEN** the query matches in 3 different messages of one session
- **THEN** the result SHALL contain 3 hits ordered by message timestamp, each with a preview snippet

#### Scenario: Query matches nothing
- **WHEN** the query does not appear in any message
- **THEN** the result SHALL be an empty hit list without error

#### Scenario: Case-insensitive match
- **WHEN** the query is lowercase and message content contains the same word in mixed case
- **THEN** the match SHALL still be found

### Requirement: Search across all sessions of a project

The system SHALL search all sessions within a given project for a query, returning one result entry per matching session with the hit count and the first few preview snippets.

#### Scenario: Project with 100 sessions and query matching 5
- **WHEN** the query matches in 5 of the 100 sessions
- **THEN** the result SHALL contain 5 session entries sorted by most-recently-modified

### Requirement: Search across all projects

The system SHALL support global search across every project's sessions, returning entries grouped by project with per-session hit counts and previews.

#### Scenario: Global search with query appearing in two projects
- **WHEN** the query matches sessions in two distinct projects
- **THEN** the result SHALL contain two project groups, each listing their matching sessions

### Requirement: Exclude filtered content from search index

The system SHALL exclude hard-noise messages, tool_result internal payloads, and sidechain messages from search matching, so that search results reflect visible conversation text only.

#### Scenario: Search term appears only inside a hard-noise system-reminder
- **WHEN** the only match is inside a message classified as hard noise
- **THEN** the result SHALL NOT include that match

### Requirement: Support staged-limit search over SSH contexts

The system SHALL, when searching in an SSH context, apply per-stage result limits to avoid long round-trip delays over the network, returning early when enough results have been collected for the current stage.

#### Scenario: Global search over SSH with many matches
- **WHEN** the active context is SSH and a global search query matches many sessions
- **THEN** the search SHALL return a partial but ordered result set once the configured SSH fast-search stage limit is reached, and the result SHALL indicate whether more results are available

### Requirement: Cache extracted search text

The system SHALL cache the extracted searchable text for each session so that repeated searches do not re-parse the full JSONL file when the file has not changed.

#### Scenario: Second search on same session after first
- **WHEN** a search is issued on a session that has not been modified since the last search
- **THEN** the system SHALL reuse the cached search text instead of re-parsing JSONL

