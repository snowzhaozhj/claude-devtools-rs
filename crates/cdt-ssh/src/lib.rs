//! SSH-backed remote context.
//!
//! Owns the **ssh-remote-context** capability. Implements the
//! `FileSystemProvider` trait (defined in `cdt-core`) for remote hosts,
//! exposing the exact same read API as the local provider so downstream
//! consumers observe identical data shapes regardless of transport.
//!
//! Responsibilities:
//! - Parse `~/.ssh/config` for host aliases (hostname, user, port, identity)
//! - Connect / disconnect / test / query status
//! - Stream remote JSONL files through an async `Read` adapter
//! - Report structured connection status: disconnected / connecting /
//!   connected / error with human-readable message
//!
//! Port status: **stub**.

/// Placeholder — replaced during `port-ssh-remote-context`.
pub fn stub() {}
