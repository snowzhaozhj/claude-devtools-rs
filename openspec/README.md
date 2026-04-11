# OpenSpec in claude-devtools-rs

This directory mirrors the spec baseline established in the parent
TypeScript project (`../claude-devtools`) on bootstrap day and is now
**owned independently** by this Rust repo. Future changes happen here
first; the TS repo is frozen for reference purposes.

## Layout

```
openspec/
├── config.yaml       # Schema = spec-driven, project context
├── specs/            # 13 capability specs (baseline snapshot)
├── changes/          # Active and archived changes for the Rust port
└── followups.md      # TS impl-bugs the Rust port MUST fix (not replicate)
```

## Source of truth

- `specs/` is the authoritative behavioral contract for the Rust port.
- `followups.md` lists places where the **TypeScript implementation deviated
  from its own spec**. When porting, implement the spec — do **not** copy the
  TS bug.

## Workflow

1. Use `/opsx:propose <name>` for a new port change (one capability per change is ideal)
2. Use `/opsx:apply` to work through tasks
3. Use `/opsx:archive` when a change is merged and its delta should move into `specs/`

## Capability → crate map

| Capability                     | Owning crate    |
|--------------------------------|-----------------|
| project-discovery              | `cdt-discover`  |
| session-parsing                | `cdt-parse`     |
| chunk-building                 | `cdt-analyze`   |
| tool-execution-linking         | `cdt-analyze`   |
| context-tracking               | `cdt-analyze`   |
| team-coordination-metadata     | `cdt-analyze`   |
| session-search                 | `cdt-discover`  |
| file-watching                  | `cdt-watch`     |
| configuration-management       | `cdt-config`    |
| notification-triggers          | `cdt-config`    |
| ssh-remote-context             | `cdt-ssh`       |
| ipc-data-api                   | `cdt-api`       |
| http-data-api                  | `cdt-api`       |
