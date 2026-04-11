# ssh-remote-context Specification

## Purpose
TBD - created by archiving change rust-rewrite-baseline. Update Purpose after archive.
## Requirements
### Requirement: Manage local and SSH contexts

The system SHALL expose a notion of "context" representing where session data is read from, with two kinds: `local` (host filesystem) and `ssh` (remote host). The system SHALL allow listing contexts, switching the active context, and querying the currently active one.

#### Scenario: Default local context
- **WHEN** the app starts with no prior SSH state
- **THEN** the active context SHALL be `local` with the local filesystem provider bound

#### Scenario: Switch to SSH context
- **WHEN** a caller requests to switch the active context to an established SSH context
- **THEN** subsequent session discovery and reads SHALL use the SSH filesystem provider

### Requirement: Establish and tear down SSH connections

The system SHALL connect to a remote host using SSH, reading host metadata from `~/.ssh/config` when available, and SHALL cleanly disconnect on request or on shutdown.

#### Scenario: Connect by host alias from ssh config
- **WHEN** a caller requests to connect to an alias defined in `~/.ssh/config`
- **THEN** the system SHALL resolve hostname, user, port, and identity file from the ssh config and establish the connection

#### Scenario: Test connection without persisting
- **WHEN** a caller requests a connection test
- **THEN** the system SHALL attempt to authenticate, return success or error detail, and SHALL NOT register the connection as active

#### Scenario: Disconnect
- **WHEN** a caller disconnects an active SSH context
- **THEN** the connection SHALL be closed and any subsequent read from that context SHALL fail with a clear error

### Requirement: Read sessions and files over SSH with same contract

The system SHALL provide the same project-discovery, session-parsing, and file-read operations over an SSH context as over the local context, so that downstream consumers observe identical data shapes.

#### Scenario: List projects on a remote host
- **WHEN** the active context is SSH and a caller requests project list
- **THEN** the result SHALL have the same shape as the local project list, sourced from the remote `~/.claude/projects/` directory

#### Scenario: Read a remote session
- **WHEN** the active context is SSH and a caller requests a session detail
- **THEN** the system SHALL stream the remote JSONL file and return parsed chunks identical in shape to local output

### Requirement: Report SSH connection status

The system SHALL expose the current connection status of every configured SSH context (disconnected, connecting, connected, error) with a human-readable error message when applicable.

#### Scenario: Query status of a failed context
- **WHEN** an SSH context has failed to connect
- **THEN** the status query SHALL return `error` with the underlying error message

