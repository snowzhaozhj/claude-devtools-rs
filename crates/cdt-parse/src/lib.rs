//! JSONL session parsing.
//!
//! Owns the **session-parsing** capability (see
//! `openspec/specs/session-parsing/spec.md`). Responsibilities:
//! - Streaming JSONL reader (line-by-line, tolerant of malformed lines)
//! - Classification into `ParsedMessage` records
//! - Hard-noise filtering (synthetic placeholders, interrupt markers, etc.)
//! - `requestId` deduplication (baseline says SHALL, TS impl has bug: function
//!   exists but is not called — the Rust port MUST wire it up).
//!
//! Port status: **stub**.

/// Placeholder entry point — replaced during `port-session-parsing`.
pub fn stub() {}
