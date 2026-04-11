//! JSONL session parsing.
//!
//! Owns the **session-parsing** capability (see
//! `openspec/specs/session-parsing/spec.md`). Responsibilities:
//! - Streaming JSONL reader (line-by-line, tolerant of malformed lines)
//! - Classification into `ParsedMessage` records via `cdt_core` types
//! - Hard-noise classification (synthetic placeholders, interrupt markers,
//!   `<local-command-caveat>`/`<system-reminder>` wrappers, etc.)
//! - `requestId` deduplication — the TS impl defined the function but
//!   never called it; this port wires it in unconditionally.

pub mod error;

pub(crate) mod dedupe;
pub(crate) mod file;
pub(crate) mod noise;
pub(crate) mod parser;

pub use dedupe::dedupe_by_request_id;
pub use error::ParseError;
pub use file::parse_file;
pub use parser::{parse_entry, parse_entry_at};
