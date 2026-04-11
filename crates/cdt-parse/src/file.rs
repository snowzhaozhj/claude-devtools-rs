//! Async file-level parsing with dedup and malformed-line tolerance.

use std::path::{Path, PathBuf};

use cdt_core::ParsedMessage;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::dedupe::dedupe_by_request_id;
use crate::error::ParseError;
use crate::parser::parse_entry_at;

/// Parse an entire JSONL session file.
///
/// - Streams the file line by line via `tokio::fs` to avoid slurping the
///   raw bytes into memory.
/// - Collects every successfully parsed `ParsedMessage` (malformed lines
///   emit a `tracing::warn!` and are skipped; empty files yield an empty
///   Vec with no error).
/// - Runs `dedupe_by_request_id` on the collected list before returning,
///   fixing the TS impl-bug where the dedup function existed but was
///   never called.
///
/// Returns the parsed messages in file order (post-dedup).
pub async fn parse_file(path: impl AsRef<Path>) -> Result<Vec<ParsedMessage>, ParseError> {
    let path: PathBuf = path.as_ref().to_path_buf();
    let file = File::open(&path).await.map_err(|e| ParseError::Io {
        path: path.clone(),
        source: e,
    })?;

    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut out = Vec::new();
    let mut line_no: usize = 0;

    loop {
        let next = lines.next_line().await.map_err(|e| ParseError::Io {
            path: path.clone(),
            source: e,
        })?;
        let Some(line) = next else { break };
        line_no += 1;
        if line.trim().is_empty() {
            continue;
        }
        match parse_entry_at(&line, line_no) {
            Ok(Some(msg)) => out.push(msg),
            Ok(None) => {}
            Err(ParseError::MalformedLine { line, source }) => {
                tracing::warn!(
                    file = %path.display(),
                    line,
                    error = %source,
                    "skipping malformed JSONL line"
                );
            }
            Err(ParseError::SchemaMismatch { line, reason }) => {
                tracing::warn!(
                    file = %path.display(),
                    line,
                    reason = %reason,
                    "skipping entry with schema mismatch"
                );
            }
            Err(e @ ParseError::Io { .. }) => return Err(e),
        }
    }

    Ok(dedupe_by_request_id(out))
}
