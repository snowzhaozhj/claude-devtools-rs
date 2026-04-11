//! Shared types and traits for claude-devtools-rs.
//!
//! This crate is the foundation of the workspace. It owns the types that
//! cross capability boundaries (`ParsedMessage`, `Chunk`, `Process`,
//! `TokenUsage`, `FileSystemProvider` trait, etc.) and is the ONLY crate
//! other crates may depend on without introducing runtime infrastructure.
//!
//! Invariants (see `openspec/specs/rust-workspace-layout/spec.md`):
//! - MUST NOT depend on `tokio`, `axum`, `notify`, `ssh2`, `reqwest`, or any
//!   other runtime infrastructure crate.
//! - MUST stay usable from synchronous unit tests without async runtime setup.
//! - Any type needed by two or more capability crates belongs here, not
//!   duplicated.
//!
//! Port status: **stub** — types will be introduced as each capability is ported.

pub mod prelude {
    //! Re-exports for consumers. Empty until the first capability is ported.
}
