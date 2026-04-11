//! Data API facade.
//!
//! Owns two baseline capabilities:
//! - **ipc-data-api** — trait surface exposing the full operation set the
//!   TypeScript renderer consumes (projects, sessions, search, config,
//!   notifications, ssh, context, updater, validation, plus auxiliary
//!   reads: agent configs, CLAUDE.md, worktree sessions, etc.).
//! - **http-data-api** — HTTP/SSE server under the `/api` prefix that
//!   mirrors the IPC operation set for web/remote clients. Current
//!   baseline returns safe defaults on lookup failures; status-code-based
//!   error taxonomy is an intentional improvement opportunity for the Rust
//!   port tracked as a separate spec delta.
//!
//! The IPC facade is a **trait-based façade**, not a concrete IPC transport.
//! If a Tauri shell is added later, it can implement the trait directly; if
//! we stay headless, only the HTTP server is exposed.
//!
//! Port status: **stub**.

pub mod ipc {
    //! ipc-data-api capability — trait surface only, no transport binding.
}

pub mod http {
    //! http-data-api capability — axum-based HTTP/SSE server (not yet wired).
}
