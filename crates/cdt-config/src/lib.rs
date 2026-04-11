//! Configuration, CLAUDE.md reading, notification triggers.
//!
//! Owns two baseline capabilities:
//! - **configuration-management** — persisted app config, CLAUDE.md reads
//!   (global / project / directory scopes), @mention path resolution with
//!   sandboxing, corruption backup on load failure. Port note: the TS
//!   implementation skips the backup step — Rust MUST implement it per spec.
//! - **notification-triggers** — error detection over tool executions,
//!   trigger evaluation (literal + regex with RE2-style safety validation),
//!   historical preview without side effects, persisted read/unread state.
//!
//! Port status: **stub**.

pub mod config {
    //! configuration-management capability.
}

pub mod triggers {
    //! notification-triggers capability.
}
