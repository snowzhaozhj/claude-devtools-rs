//! `requestId` deduplication.
//!
//! Spec: `openspec/specs/session-parsing/spec.md` §"Deduplicate streaming
//! entries by requestId". The TS impl defined `deduplicateByRequestId`
//! but never called it from the main parse path (see
//! `openspec/followups.md`). The Rust port wires it in unconditionally.

use std::collections::HashSet;

use cdt_core::{MessageCategory, ParsedMessage};

/// Drop all but the **last** assistant entry for each `requestId`.
///
/// Non-assistant messages pass through unchanged. Relative ordering of
/// kept messages is preserved. Assistant entries without a `requestId`
/// are always kept.
pub fn dedupe_by_request_id(messages: Vec<ParsedMessage>) -> Vec<ParsedMessage> {
    // Scan from the end, remember which requestIds we've already seen,
    // and skip any assistant entry whose requestId appears later in the
    // file. Build the kept indices, then rebuild the output in the
    // original order.
    let mut seen: HashSet<String> = HashSet::new();
    let mut keep = vec![true; messages.len()];

    for (idx, msg) in messages.iter().enumerate().rev() {
        if !matches!(msg.category, MessageCategory::Assistant) {
            continue;
        }
        let Some(rid) = msg.request_id.as_deref() else {
            continue;
        };
        if seen.contains(rid) {
            keep[idx] = false;
        } else {
            seen.insert(rid.to_owned());
        }
    }

    messages
        .into_iter()
        .enumerate()
        .filter_map(|(i, m)| if keep[i] { Some(m) } else { None })
        .collect()
}
