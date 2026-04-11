# claude-devtools-rs

Rust port of [claude-devtools](https://github.com/matt1398/claude-devtools),
an Electron app that visualizes Claude Code session execution. This repo
ports the data/logic layer per the OpenSpec baseline frozen in
`openspec/specs/`. UI technology is deferred.

## Status

Bootstrap-only: the workspace compiles cleanly but no capability has been
ported yet. All 13 capabilities are in `not started` state — see `CLAUDE.md`
for the capability → crate map and recommended port order.

## Getting started

```bash
cargo build --workspace
cargo run -p cdt-cli      # prints "claude-devtools-rs bootstrap OK"
cargo clippy --workspace --all-targets
cargo test --workspace
```

## Project layout

```
crates/
├── cdt-core       # shared types (no runtime deps)
├── cdt-parse      # session-parsing
├── cdt-analyze    # chunk-building, tool-linking, context-tracking, team-metadata
├── cdt-discover   # project-discovery, session-search
├── cdt-watch      # file-watching
├── cdt-config     # configuration-management, notification-triggers
├── cdt-ssh        # ssh-remote-context
├── cdt-api        # ipc-data-api, http-data-api
└── cdt-cli        # binary entrypoint
openspec/
├── specs/         # 13 capability specs (authoritative)
├── followups.md   # TS impl-bugs to fix, not replicate
└── config.yaml
```

Full contributor brief in `CLAUDE.md`; Rust conventions in
`.claude/rules/rust.md`.
