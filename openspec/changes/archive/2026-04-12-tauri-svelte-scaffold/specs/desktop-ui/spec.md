## ADDED Requirements

### Requirement: Display project list

The system SHALL display a list of discovered Claude Code projects with their name, path, and session count in a desktop window.

#### Scenario: Launch with projects
- **WHEN** the app launches and projects exist in `~/.claude/projects/`
- **THEN** the UI SHALL display each project with name, path, and session count

### Requirement: Display session list for a project

The system SHALL display a paginated list of sessions for a selected project, showing session ID, last modified time, and file size.

#### Scenario: Click a project
- **WHEN** the user clicks a project in the list
- **THEN** the UI SHALL show that project's sessions sorted by most recent first
