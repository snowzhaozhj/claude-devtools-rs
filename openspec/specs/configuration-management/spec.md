# configuration-management Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Persist application configuration

The system SHALL persist application configuration (triggers, UI preferences, pinned sessions, HTTP server port, SSH hosts, feature toggles) to a user-scoped configuration file and load it on startup.

#### Scenario: First launch with no config file
- **WHEN** the configuration file does not exist on startup
- **THEN** the system SHALL materialize a default configuration, persist it, and continue

#### Scenario: Corrupted config file
- **WHEN** the configuration file exists but cannot be parsed
- **THEN** the system SHALL back up the corrupted file, load defaults, log the error, and continue

### Requirement: Expose config read and update operations

The system SHALL expose operations to read the current configuration, update a field, add a trigger, remove a trigger, pin/unpin a session, and open the config file in an external editor.

#### Scenario: Update a single config field
- **WHEN** a caller updates the HTTP port to a new value
- **THEN** the new value SHALL be persisted and returned on the next read

#### Scenario: Add a new trigger
- **WHEN** a caller adds a trigger via the add-trigger operation
- **THEN** the trigger SHALL be persisted with a generated id and appear in subsequent reads

### Requirement: Read CLAUDE.md files

The system SHALL read `CLAUDE.md` files from three scopes — user global (`~/.claude/CLAUDE.md`), project (project directory), and current working directory — returning each file's path, content, and scope tag.

#### Scenario: Only global CLAUDE.md exists
- **WHEN** the user has a global CLAUDE.md but the project has none
- **THEN** the result SHALL contain one entry with scope `global`

#### Scenario: All three scopes present
- **WHEN** global, project, and cwd CLAUDE.md all exist
- **THEN** the result SHALL contain three entries with scope `global`, `project`, `directory`

### Requirement: Resolve and read mentioned files safely

The system SHALL resolve `@path` mentions relative to a session's cwd and read file contents, rejecting paths that escape the allowed roots.

#### Scenario: Valid in-project mention
- **WHEN** a mention `@src/foo.ts` resolves inside the session's project root
- **THEN** the file SHALL be read and returned with its resolved absolute path

#### Scenario: Path traversal attempt
- **WHEN** a mention resolves outside the allowed roots (e.g., `@../../etc/passwd`)
- **THEN** the read SHALL be rejected with a validation error

### Requirement: Validate configuration fields before persistence

The system SHALL validate incoming configuration updates (e.g., HTTP port range, regex patterns, file paths) and reject invalid updates with a descriptive error rather than persisting bad state.

#### Scenario: Invalid port number
- **WHEN** a caller attempts to set the HTTP port to a value outside 1–65535
- **THEN** the update SHALL be rejected with a validation error and the stored value SHALL remain unchanged

