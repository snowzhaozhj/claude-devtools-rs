# project-discovery Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Scan Claude projects directory

The system SHALL enumerate all Claude Code projects by scanning the configured projects root directory (default `~/.claude/projects/`), treating each first-level subdirectory as one project.

#### Scenario: Empty root directory
- **WHEN** the projects root directory exists but contains no subdirectories
- **THEN** the system SHALL return an empty project list without error

#### Scenario: Root directory missing
- **WHEN** the projects root directory does not exist
- **THEN** the system SHALL return an empty project list and log a warning, not throw

#### Scenario: Multiple project directories present
- **WHEN** the projects root contains N subdirectories
- **THEN** the system SHALL return N project entries, each exposing its decoded filesystem path, display name, and session count

### Requirement: Decode encoded project paths

The system SHALL convert Claude Code's encoded directory names back to original filesystem paths by replacing leading hyphens with slashes.

#### Scenario: Standard encoded name
- **WHEN** a project directory is named `-Users-alice-code-app`
- **THEN** the decoded path SHALL be `/Users/alice/code/app`

#### Scenario: Path containing legitimate hyphens
- **WHEN** a project directory is named `-Users-alice-my-app` (ambiguous between `/Users/alice/my-app` and `/Users/alice/my/app`)
- **THEN** the decoder SHALL return a best-effort replacement (every leading hyphen becomes a slash) and the authoritative cwd SHALL be recovered from the `cwd` field of the session entries inside the project directory when available

#### Scenario: WSL-style path
- **WHEN** the decoded path refers to a WSL mount (e.g., `/mnt/c/...`)
- **THEN** the system SHALL return the path as-is without platform rewriting

### Requirement: List sessions per project

The system SHALL list all `*.jsonl` session files inside a given project directory, returning each session's id (basename), last-modified timestamp, and file size.

#### Scenario: Project with multiple sessions
- **WHEN** a project directory contains 5 `.jsonl` files
- **THEN** the session list SHALL contain 5 entries sorted by last-modified time descending

#### Scenario: Project with non-jsonl files
- **WHEN** a project directory contains `.jsonl` files mixed with other files
- **THEN** the session list SHALL include only the `.jsonl` files

### Requirement: Group projects by git worktree

The system SHALL group project directories that belong to the same git repository's worktrees under a single logical repository entry, preserving each worktree as a distinct member.

#### Scenario: Two worktrees of one repo
- **WHEN** two project paths resolve to different worktrees of the same repository (same `git common dir`)
- **THEN** the system SHALL emit one repository group containing both worktrees as members

#### Scenario: Standalone project not in a worktree
- **WHEN** a project path has no git metadata
- **THEN** the system SHALL emit the project as its own single-member group

### Requirement: Resolve subprojects and pinned sessions

The system SHALL track subproject associations and user-pinned sessions as configuration state, surfacing them alongside scanned projects.

#### Scenario: Pinned session exists
- **WHEN** a session has been pinned via configuration
- **THEN** the system SHALL mark it as pinned in the session list regardless of its modification time

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

