## MODIFIED Requirements

### Requirement: Detect teammate messages

The system SHALL detect user messages that carry a teammate payload wrapped in `<teammate-message teammate_id="..." ...>content</teammate-message>` and classify them as teammate messages rather than real user input.

#### Scenario: Teammate message in string content
- **WHEN** a user message's string content starts with `<teammate-message teammate_id="alice"`
- **THEN** the message SHALL be flagged as a teammate message and SHALL NOT create a `UserChunk`

#### Scenario: Teammate message in block content
- **WHEN** a user message has a single text block containing the teammate tag
- **THEN** the message SHALL be flagged as a teammate message
