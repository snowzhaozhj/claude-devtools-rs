# project-discovery Spec Delta

## MODIFIED Requirements

### Requirement: Scan Claude projects directory

The system SHALL enumerate all Claude Code projects by scanning the configured projects root directory (default `~/.claude/projects/` on Unix, `%USERPROFILE%\.claude\projects\` on Windows), treating each first-level subdirectory as one project.

The system SHALL resolve the user's home directory by checking environment variables in this priority order: `HOME` → `USERPROFILE` → `HOMEDRIVE`+`HOMEPATH` concatenation → platform default (`dirs::home_dir()`). This matches the TS 原版 `pathDecoder.ts::getHomeDir` fallback chain and lets WSL / Git Bash / Cygwin users override via `HOME` while still locating `%USERPROFILE%\.claude\` on Windows native shells.

#### Scenario: Empty root directory

- **WHEN** the projects root directory exists but contains no subdirectories
- **THEN** the system SHALL return an empty project list without error

#### Scenario: Root directory missing

- **WHEN** the projects root directory does not exist
- **THEN** the system SHALL return an empty project list and log a warning, not throw

#### Scenario: Multiple project directories present

- **WHEN** the projects root contains N subdirectories
- **THEN** the system SHALL return N project entries, each exposing its decoded filesystem path, display name, and session count

#### Scenario: Home directory resolution on Windows native

- **WHEN** running on Windows with `HOME` unset but `USERPROFILE` set to `C:\Users\alice`
- **THEN** the system SHALL resolve the projects root to `C:\Users\alice\.claude\projects\`

#### Scenario: Home directory resolution via HOMEDRIVE/HOMEPATH fallback

- **WHEN** running on Windows with both `HOME` and `USERPROFILE` unset but `HOMEDRIVE=C:` and `HOMEPATH=\Users\alice` set
- **THEN** the system SHALL resolve the home directory to `C:\Users\alice` and the projects root to `C:\Users\alice\.claude\projects\`

#### Scenario: HOME env variable takes priority over USERPROFILE

- **WHEN** running with `HOME=/home/user` and `USERPROFILE=C:\Users\alice` both set (e.g. WSL / Git Bash on Windows)
- **THEN** the system SHALL prefer `HOME` and resolve the projects root to `/home/user/.claude/projects/`

### Requirement: Decode encoded project paths

The system SHALL convert Claude Code's encoded directory names back to original filesystem paths. The decoder SHALL recognize three formats in order:

1. **Legacy Windows format** `^([A-Za-z])--(.+)$` (e.g. `C--Users-alice-app`) SHALL decode to `<drive_upper>:/<rest_with_slashes>` (e.g. `C:/Users/alice/app`).
2. **New Windows format** (post-legacy, `-C:-Users-alice-app`): after stripping the leading `-` and replacing remaining `-` with `/`, a string matching `^[A-Za-z]:/` SHALL be returned as-is (e.g. `C:/Users/alice/app`) without adding a POSIX `/` prefix.
3. **POSIX format** (`-Users-alice-app`): the decoder SHALL strip the leading `-`, replace remaining `-` with `/`, and ensure a leading `/` to produce an absolute path (e.g. `/Users/alice/app`).

When the target platform is Windows, the decoder SHALL additionally translate WSL mount paths: any decoded path matching `^/mnt/([A-Za-z])(/.*)?$` SHALL be rewritten to `<drive_upper>:<rest>` (e.g. `/mnt/c/code` → `C:/code`).

On non-Windows platforms, WSL mount paths SHALL be returned as-is without rewriting (already covered by existing scenario "WSL-style path").

#### Scenario: Standard encoded name

- **WHEN** a project directory is named `-Users-alice-code-app`
- **THEN** the decoded path SHALL be `/Users/alice/code/app`

#### Scenario: Path containing legitimate hyphens

- **WHEN** a project directory is named `-Users-alice-my-app` (ambiguous between `/Users/alice/my-app` and `/Users/alice/my/app`)
- **THEN** the decoder SHALL return a best-effort replacement (every leading hyphen becomes a slash) and the authoritative cwd SHALL be recovered from the `cwd` field of the session entries inside the project directory when available

#### Scenario: WSL-style path on non-Windows platforms

- **WHEN** the decoded path refers to a WSL mount (e.g., `/mnt/c/...`) and the current platform is not Windows
- **THEN** the system SHALL return the path as-is without platform rewriting

#### Scenario: New Windows format decodes to drive-letter path

- **WHEN** a project directory is named `-C:-Users-alice-app`
- **THEN** the decoded path SHALL be `C:/Users/alice/app` without a POSIX leading `/`

#### Scenario: Legacy Windows format decodes to drive-letter path

- **WHEN** a project directory is named `C--Users-alice-app` (no leading `-`, colon encoded as `--`)
- **THEN** the decoded path SHALL be `C:/Users/alice/app`; the drive letter SHALL be uppercased even if the source had a lowercase letter

#### Scenario: WSL mount translation on Windows

- **WHEN** running on Windows and the decoded path is `/mnt/c/code`
- **THEN** the system SHALL rewrite it to `C:/code`

#### Scenario: is_valid_encoded_path accepts legacy Windows format

- **WHEN** testing `is_valid_encoded_path("C--Users-alice-app")`
- **THEN** the result SHALL be `true`; similarly for any input matching `^[A-Za-z]--[A-Za-z0-9_.\s-]+$`

## ADDED Requirements

### Requirement: Encode absolute paths into directory names

The system SHALL expose a single canonical `encode_path(absolute_path: &str) -> String` function (in `cdt-discover::path_decoder`) that converts any absolute filesystem path into the directory name used under `~/.claude/projects/`. The encoding rule SHALL:

1. Replace **every** occurrence of `/` **and** `\` with `-` (both separators handled in one pass to support Windows paths containing either slash form).
2. Preserve drive-letter colons (e.g. `C:`) in-place — no escaping, no duplication — so that Windows paths round-trip with the new format decoder described in `Decode encoded project paths`.
3. Ensure the result begins with a single leading `-`; if the raw input started with `/` or `\` (thus becoming `-...` after replacement) no extra prefix is added, otherwise one `-` SHALL be prepended.

This function SHALL be the sole implementation of path encoding across the workspace. All other crates needing to encode a path (e.g. `cdt-config::claude_md` for auto-memory path calculation) SHALL import and call `cdt_discover::path_decoder::encode_path` rather than implementing a private copy. This keeps encode / decode in the same module under one test suite and prevents divergence such as the one that caused Windows auto-memory lookup to fail prior to this change.

#### Scenario: POSIX absolute path encoding

- **WHEN** `encode_path("/Users/alice/code/app")` is called
- **THEN** the result SHALL be `-Users-alice-code-app`

#### Scenario: Windows absolute path with backslashes

- **WHEN** `encode_path("C:\\Users\\alice\\app")` is called
- **THEN** the result SHALL be `-C:-Users-alice-app`

#### Scenario: Windows absolute path with forward slashes

- **WHEN** `encode_path("C:/Users/alice/app")` is called
- **THEN** the result SHALL also be `-C:-Users-alice-app` (same as backslash form)

#### Scenario: Mixed separators encoding

- **WHEN** `encode_path("C:\\a/b\\c")` is called
- **THEN** the result SHALL be `-C:-a-b-c`

#### Scenario: Round-trip with decode_path for Windows paths

- **WHEN** a Windows path `C:/Users/alice/app` is encoded then decoded
- **THEN** `decode_path(encode_path("C:/Users/alice/app"))` SHALL equal `C:/Users/alice/app`

#### Scenario: Round-trip with decode_path for POSIX paths

- **WHEN** a POSIX path `/Users/alice/app` is encoded then decoded
- **THEN** `decode_path(encode_path("/Users/alice/app"))` SHALL equal `/Users/alice/app`

#### Scenario: Empty input produces empty string

- **WHEN** `encode_path("")` is called
- **THEN** the result SHALL be `""`
