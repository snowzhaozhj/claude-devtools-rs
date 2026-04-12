## MODIFIED Requirements

### Requirement: Manage local and SSH contexts

The system SHALL expose a notion of "context" representing where session data is read from, with two kinds: `local` (host filesystem) and `ssh` (remote host). The system SHALL allow listing contexts, switching the active context, and querying the currently active one.

#### Scenario: Default local context
- **WHEN** the app starts with no prior SSH state
- **THEN** the active context SHALL be `local` with the local filesystem provider bound

#### Scenario: Switch to SSH context
- **WHEN** a caller requests to switch the active context to an established SSH context
- **THEN** subsequent session discovery and reads SHALL use the SSH filesystem provider

#### Scenario: Switch back to local
- **WHEN** a caller switches from SSH back to local context
- **THEN** the SSH connection SHALL remain open but reads SHALL use the local filesystem provider
