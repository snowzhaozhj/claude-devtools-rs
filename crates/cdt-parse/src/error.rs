//! Error types for session-parsing.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("io error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A JSONL line could not be parsed as JSON. `line` is 1-based.
    #[error("malformed JSON at line {line}: {source}")]
    MalformedLine {
        line: usize,
        #[source]
        source: serde_json::Error,
    },

    /// JSON parsed but did not match the expected `ChatHistoryEntry`
    /// shape. `line` is 1-based when known, `0` if called via
    /// `parse_entry` with no file context.
    #[error("schema mismatch at line {line}: {reason}")]
    SchemaMismatch { line: usize, reason: String },
}
