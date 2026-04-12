## ADDED Requirements

### Requirement: Provide a default local implementation of the data API

The system SHALL provide a concrete `LocalDataApi` that implements the `DataApi` trait by composing the local filesystem provider, project scanner, session parser, chunk builder, config manager, notification manager, and SSH connection manager.

#### Scenario: List projects on local filesystem
- **WHEN** `LocalDataApi.list_projects()` is called
- **THEN** it SHALL delegate to `ProjectScanner::scan()` and map results to `ProjectInfo`
