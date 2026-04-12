## MODIFIED Requirements

### Requirement: Return safe defaults on lookup failures (current baseline)

The system SHALL return structured error responses for lookup failures: `404` with `{"code":"not_found","message":"..."}` for missing resources, `400` with `{"code":"validation_error","message":"..."}` for invalid input. This is an intentional improvement over the TS baseline which returns `200` with `null`/empty arrays.

#### Scenario: GET nonexistent session
- **WHEN** a client requests a session id that does not exist
- **THEN** the response SHALL be `404` with a JSON body containing `code: "not_found"`

#### Scenario: GET sessions for unknown project
- **WHEN** a client requests sessions for a project id that cannot be resolved
- **THEN** the response SHALL be `404` with a JSON body containing `code: "not_found"`

#### Scenario: Unhandled server exception
- **WHEN** an unexpected exception is thrown while serving a request
- **THEN** the response SHALL be `500` with a JSON body containing `code: "internal"`
