//! Shared types and traits for claude-devtools-rs.
//!
//! This crate is the foundation of the workspace. It owns the types that
//! cross capability boundaries (`ParsedMessage`, `ContentBlock`,
//! `ToolCall`, `TokenUsage`, etc.) and is the ONLY crate other crates may
//! depend on without introducing runtime infrastructure.
//!
//! Invariants (see `openspec/specs/rust-workspace-layout/spec.md`):
//! - MUST NOT depend on `tokio`, `axum`, `notify`, `ssh2`, `reqwest`, or any
//!   other runtime infrastructure crate.
//! - MUST stay usable from synchronous unit tests without async runtime setup.
//! - Any type needed by two or more capability crates belongs here, not
//!   duplicated.

pub mod message;

pub use message::{
    ContentBlock, HardNoiseReason, ImageSource, MessageCategory, MessageContent, MessageType,
    ParsedMessage, TokenUsage, ToolCall, ToolResult,
};

pub mod prelude {
    //! Re-exports for consumers.
    pub use super::message::{
        ContentBlock, HardNoiseReason, ImageSource, MessageCategory, MessageContent, MessageType,
        ParsedMessage, TokenUsage, ToolCall, ToolResult,
    };
}
