## ADDED Requirements

### Requirement: Stream detected errors to subscribers

The system SHALL expose an in-process subscription mechanism on `LocalDataApi` that lets host runtimes (such as the Tauri application) receive newly detected errors emitted by the automatic notification pipeline, without polling the persistent notifications store.

#### Scenario: Tauri runtime subscribes and forwards to renderer
- **WHEN** the Tauri runtime calls `subscribe_detected_errors()` during application setup
- **AND** a new `DetectedError` is produced by the notification pipeline
- **THEN** the subscriber's `broadcast::Receiver` SHALL yield the `DetectedError`, allowing the host to emit a frontend event (e.g. `notification-added`)

#### Scenario: Subscription without a watcher attached
- **WHEN** `LocalDataApi` is constructed via the non-watcher constructor (used in integration tests or HTTP-only hosts)
- **AND** a caller calls `subscribe_detected_errors()`
- **THEN** the call SHALL return a valid `broadcast::Receiver` that never yields (silent no-op), not an error

#### Scenario: Multiple subscribers receive the same error
- **WHEN** two independent subscribers call `subscribe_detected_errors()`
- **AND** the pipeline produces one `DetectedError`
- **THEN** both subscribers SHALL independently receive the same `DetectedError` exactly once
