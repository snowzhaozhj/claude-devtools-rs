## MODIFIED Requirements

### Requirement: Persist application configuration

The system SHALL persist application configuration (triggers, UI preferences, pinned sessions, HTTP server port, SSH hosts, feature toggles) to a user-scoped configuration file (`~/.claude/claude-devtools-config.json`) and load it on startup.

#### Scenario: First launch with no config file
- **WHEN** the configuration file does not exist on startup
- **THEN** the system SHALL materialize a default configuration, persist it, and continue

#### Scenario: Corrupted config file
- **WHEN** the configuration file exists but cannot be parsed
- **THEN** the system SHALL rename the corrupted file to `<path>.bak.<unix_timestamp_ms>`, log a warning with the backup path, load defaults, persist the fresh config, and continue

#### Scenario: Partial config with missing fields
- **WHEN** the configuration file parses successfully but is missing some fields
- **THEN** the system SHALL merge with defaults to fill missing fields, preserving existing values

### Requirement: Read CLAUDE.md files

The system SHALL read CLAUDE.md files from eight scopes and return each file's path, existence flag, character count, and estimated token count (`char_count / 4`).

#### Scenario: All eight scopes enumerated
- **WHEN** a caller requests CLAUDE.md files for a given project root
- **THEN** the system SHALL check these scopes in order:
  1. `enterprise` — platform-specific path (macOS: `/Library/Application Support/ClaudeCode/CLAUDE.md`)
  2. `user` — `<claude_base>/CLAUDE.md`
  3. `project` — `<project_root>/CLAUDE.md`
  4. `project-alt` — `<project_root>/.claude/CLAUDE.md`
  5. `project-rules` — `<project_root>/.claude/rules/**/*.md`（递归收集，合并统计）
  6. `project-local` — `<project_root>/CLAUDE.local.md`
  7. `user-rules` — `<claude_base>/rules/**/*.md`（递归收集，合并统计）
  8. `auto-memory` — `<claude_base>/projects/<encoded_project_root>/memory/MEMORY.md`（仅前 200 行）

#### Scenario: Only global CLAUDE.md exists
- **WHEN** the user has a global CLAUDE.md but the project has none
- **THEN** the result SHALL contain one entry with scope `user` marked as exists, and all other scopes marked as not exists

#### Scenario: All three original scopes present
- **WHEN** global, project, and cwd CLAUDE.md all exist
- **THEN** the result SHALL contain entries for `user`, `project`, and `project-alt` (if present) all marked as exists

#### Scenario: File not readable
- **WHEN** a CLAUDE.md file exists but cannot be read (permission denied)
- **THEN** the system SHALL return that scope with `exists: false` and zero counts, and log the error

### Requirement: Resolve and read mentioned files safely

The system SHALL resolve `@path` mentions relative to a session's cwd and read file contents, rejecting paths that escape the allowed roots.

#### Scenario: Valid in-project mention
- **WHEN** a mention `@src/foo.ts` resolves inside the session's project root
- **THEN** the file SHALL be read and returned with its resolved absolute path, character count, and estimated token count

#### Scenario: Path traversal attempt
- **WHEN** a mention resolves outside the allowed roots (e.g., `@../../etc/passwd`)
- **THEN** the read SHALL be rejected with a validation error

#### Scenario: Sensitive file blocked
- **WHEN** a mention resolves to a path matching a sensitive file pattern (`.ssh/`, `.env`, `.aws/`, private keys, etc.)
- **THEN** the read SHALL be rejected even if within allowed directories

#### Scenario: Symlink escape
- **WHEN** a mention resolves within the project root but the symlink target is outside
- **THEN** the system SHALL canonicalize the path and reject if the real path is outside allowed roots

#### Scenario: Token limit exceeded
- **WHEN** a mentioned file's estimated token count exceeds the caller-specified maximum
- **THEN** the read SHALL return null/None

### Requirement: Validate configuration fields before persistence

The system SHALL validate incoming configuration updates (e.g., HTTP port range, regex patterns, file paths) and reject invalid updates with a descriptive error rather than persisting bad state.

#### Scenario: Invalid port number
- **WHEN** a caller attempts to set the HTTP port to a value outside 1024–65535
- **THEN** the update SHALL be rejected with a validation error and the stored value SHALL remain unchanged

#### Scenario: Invalid regex pattern
- **WHEN** a caller provides a regex pattern longer than 100 characters or containing dangerous constructs (nested quantifiers, etc.)
- **THEN** the pattern SHALL be rejected with a descriptive error

#### Scenario: Invalid `claude_root_path`
- **WHEN** a caller sets `claude_root_path` to a non-absolute or empty path
- **THEN** the value SHALL be normalized to `None`
