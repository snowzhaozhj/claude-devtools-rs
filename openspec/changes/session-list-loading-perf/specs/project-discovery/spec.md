## ADDED Requirements

### Requirement: Project session enumeration minimizes per-file overhead

Project session enumeration SHALL preserve sorted, paginated results while avoiding unnecessary repeated per-file filesystem metadata work during a single list operation. The implementation MUST keep `total`, `nextCursor`, and descending recency order consistent with the files present in the project directory at scan time.

#### Scenario: Listing many sessions preserves recency order

- **WHEN** a project directory contains many `.jsonl` session files with different modification times
- **THEN** session enumeration returns sessions in descending recency order
- **AND** the order is identical whether the caller requests all sessions at once or consumes them through cursor pagination

#### Scenario: Pagination reports complete directory total

- **WHEN** a caller requests a limited page of sessions from a project directory
- **THEN** the response reports the total number of session files in that directory
- **AND** `nextCursor` points to the next page only when more sessions remain
