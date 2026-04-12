## MODIFIED Requirements

### Requirement: Validate inputs and return structured errors

The system SHALL validate IPC input parameters and return structured errors with an error code enum and a human-readable message, rather than propagating raw exceptions.

#### Scenario: Missing required field
- **WHEN** a caller invokes an operation missing a required field
- **THEN** the response SHALL contain a validation error with code `validation_error` and a description of the missing field

#### Scenario: Resource not found
- **WHEN** a caller requests a session or project that does not exist
- **THEN** the response SHALL contain an error with code `not_found` and the resource identifier
