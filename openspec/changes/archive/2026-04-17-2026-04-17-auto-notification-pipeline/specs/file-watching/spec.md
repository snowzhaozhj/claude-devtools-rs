## MODIFIED Requirements

### Requirement: Broadcast events to multiple subscribers

The system SHALL deliver each emitted event to all active subscribers (Electron renderer via IPC, HTTP clients via SSE, and in-process background services such as the notification pipeline) without duplication.

#### Scenario: Two subscribers present
- **WHEN** one file change triggers an event and two subscribers are active
- **THEN** both subscribers SHALL receive the event exactly once each

#### Scenario: Notification pipeline subscribes alongside IPC consumers
- **WHEN** the notification pipeline calls `subscribe_files()` at startup and the Tauri IPC layer also holds a subscription
- **THEN** both subscribers SHALL independently receive every debounced `FileChangeEvent`, and neither subscriber's lag SHALL delay delivery to the other
