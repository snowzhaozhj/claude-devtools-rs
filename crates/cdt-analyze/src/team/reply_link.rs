//! Teammate 消息与触发它的 `SendMessage` `tool_use` 的反向配对。
//!
//! Spec：`openspec/specs/team-coordination-metadata/spec.md`
//! §`Link teammate messages to triggering SendMessage`。
//!
//! 算法（向后扫描，最近优先）：
//! 1. 先扫"新 flush 的 `AIChunk` 自身"的 `tool_executions`；
//! 2. 未命中再向"已 emit 的最近 N 个 `AIChunk`"回溯（[`LOOKBACK_LIMIT`]）；
//! 3. 仍未命中 → 返回 `None`（孤儿，UI 上追加到 turn 末尾）。
//!
//! 每条 `SendMessage` `tool_use_id` 在同一 chunk-building 跑批中至多被一条 teammate
//! 占用——`used_set` 跨 `AIChunk` 维护去重。

use std::collections::HashSet;
use std::hash::BuildHasher;

use cdt_core::AIChunk;

/// 跨 `AIChunk` 回溯上限：含"自身 + 已 emit 最近 N 个"。
///
/// 设为 3 兼顾常见多 turn 队友延迟回信场景与防无界扫描。
pub const LOOKBACK_LIMIT: usize = 3;

/// `SendMessage` 在 JSONL 中的 `tool_name`。
const SEND_MESSAGE_TOOL_NAME: &str = "SendMessage";

/// 在 `candidate_chunks` 中向后扫描，找出最近一条 `tool_name == "SendMessage"`、
/// `input.recipient == teammate_id` 且未被 `used_set` 占用的 `SendMessage` `tool_use_id`。
///
/// `candidate_chunks` 调用方传入顺序约定：**最近优先**——通常是
/// `[&about_to_push_chunk, &out[len-1], &out[len-2], ...]`。
///
/// 命中时把对应 `tool_use_id` 写入 `used_set` 并返回 `Some(...)`；未命中返回 `None`。
pub fn link_teammate_to_send_message<S: BuildHasher>(
    teammate_id: &str,
    candidate_chunks: &[&AIChunk],
    used_set: &mut HashSet<String, S>,
) -> Option<String> {
    for chunk in candidate_chunks.iter().take(LOOKBACK_LIMIT) {
        for exec in &chunk.tool_executions {
            if exec.tool_name != SEND_MESSAGE_TOOL_NAME {
                continue;
            }
            if used_set.contains(&exec.tool_use_id) {
                continue;
            }
            let Some(recipient) = exec.input.get("recipient").and_then(|v| v.as_str()) else {
                continue;
            };
            if recipient == teammate_id {
                used_set.insert(exec.tool_use_id.clone());
                return Some(exec.tool_use_id.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{AssistantResponse, ChunkMetrics, ToolExecution, ToolOutput};
    use chrono::{DateTime, Utc};

    fn ts() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-26T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn empty_ai() -> AIChunk {
        AIChunk {
            timestamp: ts(),
            duration_ms: None,
            responses: Vec::<AssistantResponse>::new(),
            metrics: ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: Vec::new(),
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        }
    }

    fn send_msg_exec(tool_use_id: &str, recipient: &str) -> ToolExecution {
        ToolExecution {
            tool_use_id: tool_use_id.into(),
            tool_name: SEND_MESSAGE_TOOL_NAME.into(),
            input: serde_json::json!({ "recipient": recipient, "content": "do work" }),
            output: ToolOutput::Missing,
            is_error: false,
            start_ts: ts(),
            end_ts: None,
            source_assistant_uuid: "a".into(),
            result_agent_id: None,
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
        }
    }

    fn other_tool_exec(tool_use_id: &str, name: &str) -> ToolExecution {
        ToolExecution {
            tool_use_id: tool_use_id.into(),
            tool_name: name.into(),
            input: serde_json::json!({}),
            output: ToolOutput::Missing,
            is_error: false,
            start_ts: ts(),
            end_ts: None,
            source_assistant_uuid: "a".into(),
            result_agent_id: None,
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
        }
    }

    #[test]
    fn matches_send_message_in_same_ai_chunk() {
        let mut chunk = empty_ai();
        chunk.tool_executions = vec![send_msg_exec("t1", "alice")];
        let mut used = HashSet::new();
        let got = link_teammate_to_send_message("alice", &[&chunk], &mut used);
        assert_eq!(got.as_deref(), Some("t1"));
        assert!(used.contains("t1"));
    }

    #[test]
    fn matches_send_message_across_prior_ai_chunk() {
        let mut prior = empty_ai();
        prior.tool_executions = vec![send_msg_exec("t1", "alice")];
        let cur = empty_ai();
        let mut used = HashSet::new();
        // 调用方约定：最近优先 → 当前 chunk 在前
        let got = link_teammate_to_send_message("alice", &[&cur, &prior], &mut used);
        assert_eq!(got.as_deref(), Some("t1"));
    }

    #[test]
    fn second_teammate_to_same_send_message_goes_orphan() {
        let mut chunk = empty_ai();
        chunk.tool_executions = vec![send_msg_exec("t1", "alice")];
        let mut used = HashSet::new();
        let first = link_teammate_to_send_message("alice", &[&chunk], &mut used);
        assert_eq!(first.as_deref(), Some("t1"));
        // 第二条 alice reply 必须 orphan（t1 已被占）
        let second = link_teammate_to_send_message("alice", &[&chunk], &mut used);
        assert!(second.is_none());
    }

    #[test]
    fn different_recipient_is_skipped() {
        let mut chunk = empty_ai();
        chunk.tool_executions = vec![send_msg_exec("t1", "charlie")];
        let mut used = HashSet::new();
        let got = link_teammate_to_send_message("alice", &[&chunk], &mut used);
        assert!(got.is_none());
        assert!(!used.contains("t1"));
    }

    #[test]
    fn lookback_limit_excludes_far_chunks() {
        // chain：当前 + 4 个历史 chunk，SendMessage 在最远那个 → 不命中
        let cur = empty_ai();
        let near1 = empty_ai();
        let near2 = empty_ai();
        let near3 = empty_ai();
        let mut far = empty_ai();
        far.tool_executions = vec![send_msg_exec("t-far", "alice")];
        let mut used = HashSet::new();
        let got = link_teammate_to_send_message(
            "alice",
            &[&cur, &near1, &near2, &near3, &far],
            &mut used,
        );
        assert!(
            got.is_none(),
            "回溯应在 LOOKBACK_LIMIT={LOOKBACK_LIMIT} 之内截断，far chunk 不应被扫描"
        );
    }

    #[test]
    fn empty_tool_executions_returns_none() {
        let cur = empty_ai();
        let mut used = HashSet::new();
        assert!(link_teammate_to_send_message("alice", &[&cur], &mut used).is_none());
    }

    #[test]
    fn used_set_skips_already_consumed_tool_use_id() {
        let mut chunk = empty_ai();
        chunk.tool_executions = vec![send_msg_exec("t1", "alice")];
        let mut used = HashSet::new();
        used.insert("t1".to_string());
        let got = link_teammate_to_send_message("alice", &[&chunk], &mut used);
        assert!(got.is_none(), "已 used 的 t1 不应被再配对");
    }

    #[test]
    fn send_message_input_without_recipient_is_skipped() {
        let mut chunk = empty_ai();
        let mut exec = send_msg_exec("t1", "alice");
        // 把 recipient 字段干掉，只剩 content
        exec.input = serde_json::json!({ "content": "hi" });
        chunk.tool_executions = vec![exec];
        let mut used = HashSet::new();
        let got = link_teammate_to_send_message("alice", &[&chunk], &mut used);
        assert!(got.is_none());
    }

    #[test]
    fn non_send_message_tools_are_ignored() {
        let mut chunk = empty_ai();
        chunk.tool_executions = vec![other_tool_exec("t1", "Bash"), other_tool_exec("t2", "Read")];
        let mut used = HashSet::new();
        assert!(link_teammate_to_send_message("alice", &[&chunk], &mut used).is_none());
    }

    #[test]
    fn nearest_send_message_wins_over_older() {
        // 当前 chunk 与历史 chunk 都有给 alice 的 SendMessage → 命中"最近"那条
        let mut cur = empty_ai();
        cur.tool_executions = vec![send_msg_exec("t-new", "alice")];
        let mut prior = empty_ai();
        prior.tool_executions = vec![send_msg_exec("t-old", "alice")];
        let mut used = HashSet::new();
        let got = link_teammate_to_send_message("alice", &[&cur, &prior], &mut used);
        assert_eq!(got.as_deref(), Some("t-new"));
    }
}
