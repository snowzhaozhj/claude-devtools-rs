## ADDED Requirements

### Requirement: Abstract filesystem access through a provider trait

The system SHALL perform all project / session file I/O through a single `FileSystemProvider` trait so that alternative backends (e.g. SSH remote) can be plugged in without modifying the project scanner, path resolver, or worktree grouper. The trait SHALL expose at minimum the operations required to: (a) check whether a path exists, (b) enumerate directory entries with their kind (file/dir), (c) stat a path for `size` and `mtime`, (d) read a file's full contents as a UTF-8 string, and (e) read the first N lines of a file without loading the rest.

#### Scenario: Local filesystem provider satisfies the scanner

- **WHEN** `ProjectScanner::scan` is invoked with a `LocalFileSystemProvider`
- **THEN** the scanner SHALL use only the trait's methods to enumerate projects and extract per-session metadata, and SHALL NOT call any platform-specific filesystem API directly

#### Scenario: Path resolver avoids full-file reads in remote mode

- **WHEN** the active provider reports `kind() == FsKind::Ssh` and the resolver needs to extract `cwd` from a session file
- **THEN** the resolver SHALL call `read_lines_head(path, N)` with a bounded `N` sufficient to capture the first user/summary entry, and SHALL NOT download the entire file

#### Scenario: Trait is the sole seam for alternative backends

- **WHEN** a new backend (e.g. SSH) is introduced in a later port
- **THEN** introducing it SHALL require only implementing `FileSystemProvider`, and SHALL NOT require changes to `ProjectScanner`, `ProjectPathResolver`, or `WorktreeGrouper`

### Requirement: Represent split subprojects with a stable composite identifier

The system SHALL, when two or more sessions inside the same encoded project directory have different `cwd` values, split that directory into multiple logical "subprojects" and identify each subproject by a composite ID of the form `{baseDir}::{hash8}`, where `baseDir` is the original encoded directory name and `hash8` is the lowercase hexadecimal representation of the first 8 characters of the SHA-256 digest of the subproject's canonical `cwd` string. The composite ID SHALL be deterministic — the same `baseDir` + `cwd` pair SHALL always produce the same ID.

#### Scenario: Single-cwd directory keeps its plain ID

- **WHEN** a project directory contains sessions that all share the same `cwd`
- **THEN** the system SHALL emit a single `Project` whose `id` equals the encoded directory name (no `::` suffix)

#### Scenario: Multi-cwd directory splits into composite IDs

- **WHEN** a project directory contains sessions with two distinct `cwd` values
- **THEN** the system SHALL emit two `Project` entries, each with a distinct composite ID of the form `{encodedDir}::{8-char-hex}`, and the `path` field of each SHALL be the respective `cwd`

#### Scenario: Composite ID is stable across scans

- **WHEN** the same directory is scanned twice with unchanged session contents
- **THEN** both scans SHALL produce the same composite IDs for the same subprojects

#### Scenario: Registry exposes session filter for a composite ID

- **WHEN** a caller queries the subproject registry with a composite ID
- **THEN** the registry SHALL return the set of session IDs that belong to that subproject (so that session listing can filter accordingly), and SHALL return `None` for any plain (non-composite) ID
