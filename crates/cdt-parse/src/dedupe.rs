//! `requestId` 去重。
//!
//! Spec：`openspec/specs/session-parsing/spec.md` §"Deduplicate streaming
//! entries by requestId"。TS 版定义了 `deduplicateByRequestId` 但在主解析
//! 路径上从未被调用（见 `openspec/followups.md`）。Rust port 在 `parse_file`
//! 里无条件把它接进来。

use std::collections::HashSet;

use cdt_core::{MessageCategory, ParsedMessage};

/// 对同一 `requestId` 的 assistant 条目只保留**最后一次出现**的那一条。
///
/// 非 assistant 消息一律放行；被保留的消息之间相对顺序不变；
/// 没有 `requestId` 的 assistant 条目也一律保留。
pub fn dedupe_by_request_id(messages: Vec<ParsedMessage>) -> Vec<ParsedMessage> {
    // 从后往前扫，记录已经见过的 requestId；再遇到同 id 的 assistant 条目
    // 就标记不保留。最后按原顺序把 `keep[i] == true` 的条目收集出来。
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
