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

