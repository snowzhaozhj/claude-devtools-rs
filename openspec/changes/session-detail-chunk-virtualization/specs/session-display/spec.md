## ADDED Requirements

### Requirement: Virtualized chunk stream for long SessionDetail

SessionDetail SHALL virtualize the main `detail.chunks` conversation stream for long sessions: when virtualization is enabled and search is not active, only chunks intersecting the visible viewport plus overscan SHALL be mounted in the DOM, while spacer elements preserve the full scrollable height. The virtualizer MUST support variable-height chunks by combining estimated heights with `ResizeObserver` measurements of rendered rows.

#### Scenario: Long session mounts only visible chunk window
- **WHEN** SessionDetail renders a session with 200 chunks and virtualization enabled
- **THEN** the `.conversation` DOM SHALL contain only the visible chunks plus overscan rows, not all 200 chunk rows
- **AND** the scroll container SHALL preserve a scrollable height equivalent to the full session through top and bottom spacer elements

#### Scenario: Chunk height measurement updates offsets
- **WHEN** a virtualized chunk row is mounted and its measured height differs from the estimated height
- **THEN** the virtualizer SHALL store the measured height for that chunk key
- **AND** subsequent visible range and spacer calculations SHALL use the measured height instead of the estimate

#### Scenario: Tool expansion updates virtual row height
- **WHEN** the user expands or collapses an AIChunk tool section inside a virtualized row
- **THEN** `ResizeObserver` SHALL observe the row height change and update the virtualizer measurement
- **AND** neighboring rows SHALL keep their relative order without overlapping or leaving persistent blank gaps

#### Scenario: Lazy markdown height changes are measured
- **WHEN** a lazy markdown placeholder inside a virtualized chunk enters the viewport and renders real markdown or Mermaid content
- **THEN** the row height change SHALL be measured and reflected in spacer offsets
- **AND** markdown SHALL still render through the existing lazy markdown pipeline with XSS sanitization and syntax highlighting

#### Scenario: Search preserves full-session results
- **WHEN** SessionDetail search UI is active or a search query is being evaluated
- **THEN** SessionDetail SHALL render the full chunk stream or otherwise make all chunks searchable
- **AND** existing DOM-based highlight and navigation behavior SHALL still find matches in chunks that were previously outside the virtualized window

#### Scenario: Auto refresh keeps pinned-to-bottom with virtualization
- **WHEN** a file-change refresh starts while the user is pinned to the bottom of a virtualized conversation
- **AND** `getSessionDetail` returns appended content
- **THEN** SessionDetail SHALL scroll to the virtualized stream end after render so the newest chunk remains visible

#### Scenario: Auto refresh does not steal scroll when user is reading history
- **WHEN** a file-change refresh starts while the user is not pinned to the bottom of a virtualized conversation
- **AND** `getSessionDetail` returns new content
- **THEN** SessionDetail SHALL NOT force the scroll position to the bottom
- **AND** the user SHALL remain near the same historical content rather than losing reading position

#### Scenario: Per-tab scroll restore remains isolated
- **WHEN** the user switches away from a session tab and later returns to it
- **THEN** SessionDetail SHALL restore that tab's saved `scrollTop` in the virtualized conversation
- **AND** another tab or pane showing the same session SHALL NOT receive this scroll position

#### Scenario: OpenOrReplace resets stale virtual state
- **WHEN** `openOrReplaceTab` reuses an existing tab id for a different `sessionId`
- **THEN** SessionDetail SHALL NOT reuse the previous session's virtualizer measurements, scroll offset, or expanded row measurements
- **AND** the new session SHALL render from its own chunks and per-tab UI state

#### Scenario: Virtualization rollback switch
- **WHEN** the SessionDetail chunk virtualization rollback constant is disabled
- **THEN** SessionDetail SHALL render the full chunk stream using the pre-virtualization behavior
- **AND** search, lazy markdown, tool expansion, and auto refresh SHALL continue to work without requiring callers to branch
