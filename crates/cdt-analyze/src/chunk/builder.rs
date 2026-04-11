//! `build_chunks`：从已解析消息流构造 chunk 序列。
//!
//! Spec：`openspec/specs/chunk-building/spec.md`。
//!
//! 状态机：
//! 1. 过滤 `is_sidechain` 与 `HardNoise`；
//! 2. 顺序扫描剩余消息：
//!    - 遇 `MessageCategory::Assistant` → 累进 assistant buffer；
//!    - 遇 `MessageCategory::Compact` → 先 flush buffer，再产出 `CompactChunk`；
//!    - 遇 `MessageCategory::User` →
//!         - 若内容精确被 `<local-command-stdout>…</local-command-stdout>`
//!           包裹且非空 → flush buffer，产出 `SystemChunk`；
//!         - 若是"只含 `tool_result`"的回传 → 附加到 buffer 最后一条 assistant
//!           响应的 `tool_results`；buffer 为空则降级为普通 `UserChunk`；
//!         - 否则 → flush buffer，产出 `UserChunk`；
//! 3. 末尾 flush。

use cdt_core::{
    AIChunk, AssistantResponse, Chunk, ChunkMetrics, CompactChunk, ContentBlock, MessageCategory,
    MessageContent, ParsedMessage, SystemChunk, UserChunk,
};

use super::metrics::aggregate_metrics;
use super::semantic::extract_semantic_steps;

const STDOUT_OPEN: &str = "<local-command-stdout>";
const STDOUT_CLOSE: &str = "</local-command-stdout>";

pub fn build_chunks(messages: &[ParsedMessage]) -> Vec<Chunk> {
    let mut out: Vec<Chunk> = Vec::new();
    let mut buffer: Vec<AssistantResponse> = Vec::new();

    for msg in messages {
        if msg.is_sidechain || msg.category.is_hard_noise() {
            continue;
        }

        match &msg.category {
            MessageCategory::Assistant => {
                buffer.push(AssistantResponse {
                    uuid: msg.uuid.clone(),
                    timestamp: msg.timestamp,
                    content: msg.content.clone(),
                    tool_calls: msg.tool_calls.clone(),
                    usage: msg.usage.clone(),
                    model: msg.model.clone(),
                });
            }
            MessageCategory::Compact => {
                flush_buffer(&mut buffer, &mut out);
                out.push(Chunk::Compact(CompactChunk {
                    uuid: msg.uuid.clone(),
                    timestamp: msg.timestamp,
                    duration_ms: None,
                    summary_text: extract_plain_text(&msg.content),
                    metrics: ChunkMetrics::zero(),
                }));
            }
            MessageCategory::User => {
                if let Some(stdout) = extract_local_command_stdout(&msg.content) {
                    flush_buffer(&mut buffer, &mut out);
                    out.push(Chunk::System(SystemChunk {
                        uuid: msg.uuid.clone(),
                        timestamp: msg.timestamp,
                        duration_ms: None,
                        content_text: stdout,
                        metrics: ChunkMetrics::zero(),
                    }));
                } else if is_tool_result_only(&msg.content) {
                    if let Some(last) = buffer.last_mut() {
                        append_tool_results(last, &msg.content);
                    } else {
                        out.push(Chunk::User(UserChunk {
                            uuid: msg.uuid.clone(),
                            timestamp: msg.timestamp,
                            duration_ms: None,
                            content: msg.content.clone(),
                            metrics: ChunkMetrics::zero(),
                        }));
                    }
                } else {
                    flush_buffer(&mut buffer, &mut out);
                    out.push(Chunk::User(UserChunk {
                        uuid: msg.uuid.clone(),
                        timestamp: msg.timestamp,
                        duration_ms: None,
                        content: msg.content.clone(),
                        metrics: ChunkMetrics::zero(),
                    }));
                }
            }
            // `System` 这个 variant 在 parser 端被 hard-noise 前置拦截，
            // 实际不会走到这里；保留分支只是为了避免漏 match 告警。
            MessageCategory::System | MessageCategory::HardNoise(_) => {}
        }
    }

    flush_buffer(&mut buffer, &mut out);
    out
}

fn flush_buffer(buffer: &mut Vec<AssistantResponse>, out: &mut Vec<Chunk>) {
    if buffer.is_empty() {
        return;
    }
    let responses = std::mem::take(buffer);
    let metrics = aggregate_metrics(&responses);
    let semantic_steps = extract_semantic_steps(&responses);
    let timestamp = responses.first().map(|r| r.timestamp).unwrap_or_default();
    let duration_ms = match (responses.first(), responses.last()) {
        (Some(a), Some(b)) if responses.len() > 1 => {
            Some((b.timestamp - a.timestamp).num_milliseconds())
        }
        _ => None,
    };
    out.push(Chunk::Ai(AIChunk {
        timestamp,
        duration_ms,
        responses,
        metrics,
        semantic_steps,
        tool_executions: Vec::new(),
        subagents: Vec::new(),
    }));
}

fn extract_plain_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut acc = String::new();
            for b in blocks {
                if let ContentBlock::Text { text } = b {
                    if !acc.is_empty() {
                        acc.push('\n');
                    }
                    acc.push_str(text);
                }
            }
            acc
        }
    }
}

fn extract_local_command_stdout(content: &MessageContent) -> Option<String> {
    let text = match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut acc = String::new();
            let mut saw_non_text = false;
            for b in blocks {
                if let ContentBlock::Text { text } = b {
                    if !acc.is_empty() {
                        acc.push('\n');
                    }
                    acc.push_str(text);
                } else {
                    saw_non_text = true;
                    break;
                }
            }
            if saw_non_text {
                return None;
            }
            acc
        }
    };
    let trimmed = text.trim();
    if !trimmed.starts_with(STDOUT_OPEN) || !trimmed.ends_with(STDOUT_CLOSE) {
        return None;
    }
    let inner = &trimmed[STDOUT_OPEN.len()..trimmed.len() - STDOUT_CLOSE.len()];
    if inner.is_empty() {
        return None;
    }
    Some(inner.to_owned())
}

fn is_tool_result_only(content: &MessageContent) -> bool {
    let MessageContent::Blocks(blocks) = content else {
        return false;
    };
    if blocks.is_empty() {
        return false;
    }
    blocks
        .iter()
        .all(|b| matches!(b, ContentBlock::ToolResult { .. }))
}

fn append_tool_results(target: &mut AssistantResponse, incoming: &MessageContent) {
    let MessageContent::Blocks(blocks) = incoming else {
        return;
    };
    let MessageContent::Blocks(existing) = &mut target.content else {
        let mut merged = Vec::new();
        merged.extend(blocks.iter().cloned());
        target.content = MessageContent::Blocks(merged);
        return;
    };
    for b in blocks {
        if matches!(b, ContentBlock::ToolResult { .. }) {
            existing.push(b.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{
        HardNoiseReason, MessageContent, MessageType, SemanticStep, TokenUsage, ToolCall,
        ToolResult,
    };
    use chrono::{DateTime, Duration, Utc};

    fn ts(n: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::seconds(n)
    }

    fn blank_message(uuid: &str, n: i64) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.into(),
            parent_uuid: None,
            message_type: MessageType::User,
            category: MessageCategory::User,
            timestamp: ts(n),
            role: None,
            content: MessageContent::Text(String::new()),
            usage: None,
            model: None,
            cwd: None,
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta: false,
            user_type: None,
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            is_compact_summary: false,
            request_id: None,
        }
    }

    fn user(uuid: &str, n: i64, text: &str) -> ParsedMessage {
        ParsedMessage {
            content: MessageContent::Text(text.into()),
            ..blank_message(uuid, n)
        }
    }

    fn assistant(uuid: &str, n: i64, blocks: &[ContentBlock]) -> ParsedMessage {
        ParsedMessage {
            message_type: MessageType::Assistant,
            category: MessageCategory::Assistant,
            content: MessageContent::Blocks(blocks.to_vec()),
            tool_calls: blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { id, name, input } => Some(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                        is_task: name == "Task",
                        task_description: None,
                        task_subagent_type: None,
                    }),
                    _ => None,
                })
                .collect(),
            tool_results: Vec::new(),
            ..blank_message(uuid, n)
        }
    }

    #[test]
    fn user_question_then_ai_response_emits_two_chunks() {
        let msgs = vec![
            user("u1", 0, "hi"),
            assistant(
                "a1",
                1,
                &[ContentBlock::Text {
                    text: "hello".into(),
                }],
            ),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 2);
        assert!(matches!(chunks[0], Chunk::User(_)));
        assert!(matches!(chunks[1], Chunk::Ai(_)));
    }

    #[test]
    fn multiple_assistant_turns_coalesce_into_one_ai_chunk() {
        let msgs = vec![
            assistant("a1", 1, &[ContentBlock::Text { text: "1".into() }]),
            assistant("a2", 2, &[ContentBlock::Text { text: "2".into() }]),
            assistant("a3", 3, &[ContentBlock::Text { text: "3".into() }]),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 1);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.responses.len(), 3);
    }

    #[test]
    fn assistant_buffer_flushes_before_new_user() {
        let msgs = vec![
            assistant("a1", 1, &[ContentBlock::Text { text: "1".into() }]),
            assistant("a2", 2, &[ContentBlock::Text { text: "2".into() }]),
            user("u1", 3, "next?"),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 2);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.responses.len(), 2);
        assert!(matches!(chunks[1], Chunk::User(_)));
    }

    #[test]
    fn local_command_stdout_becomes_system_chunk() {
        let msgs = vec![
            assistant("a1", 1, &[ContentBlock::Text { text: "1".into() }]),
            user(
                "u1",
                2,
                "<local-command-stdout>ls output</local-command-stdout>",
            ),
            assistant("a2", 3, &[ContentBlock::Text { text: "2".into() }]),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 3);
        assert!(matches!(chunks[0], Chunk::Ai(_)));
        let Chunk::System(sys) = &chunks[1] else {
            panic!("expected SystemChunk");
        };
        assert_eq!(sys.content_text, "ls output");
        assert!(matches!(chunks[2], Chunk::Ai(_)));
    }

    #[test]
    fn sidechain_messages_are_dropped() {
        let mut side = assistant("a1", 1, &[ContentBlock::Text { text: "x".into() }]);
        side.is_sidechain = true;
        let msgs = vec![
            side,
            assistant("a2", 2, &[ContentBlock::Text { text: "y".into() }]),
        ];
        let chunks = build_chunks(&msgs);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.responses.len(), 1);
        assert_eq!(ai.responses[0].uuid, "a2");
    }

    #[test]
    fn hard_noise_messages_are_dropped() {
        let mut synthetic = assistant("a1", 1, &[ContentBlock::Text { text: "x".into() }]);
        synthetic.category = MessageCategory::HardNoise(HardNoiseReason::SyntheticAssistant);
        let msgs = vec![
            synthetic,
            assistant("a2", 2, &[ContentBlock::Text { text: "y".into() }]),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 1);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.responses.len(), 1);
    }

    #[test]
    fn ai_chunk_metrics_sum_tool_calls() {
        let msgs = vec![assistant(
            "a1",
            1,
            &[
                ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                },
                ContentBlock::ToolUse {
                    id: "t2".into(),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                },
                ContentBlock::ToolUse {
                    id: "t3".into(),
                    name: "Grep".into(),
                    input: serde_json::json!({}),
                },
            ],
        )];
        let chunks = build_chunks(&msgs);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.metrics.tool_count, 3);
    }

    #[test]
    fn user_chunk_metrics_all_zero_and_duration_none() {
        let msgs = vec![user("u1", 0, "hi")];
        let chunks = build_chunks(&msgs);
        let Chunk::User(u) = &chunks[0] else {
            panic!("expected UserChunk");
        };
        assert_eq!(u.metrics, ChunkMetrics::zero());
        assert_eq!(u.duration_ms, None);
    }

    #[test]
    fn compact_summary_emits_compact_chunk_and_flushes_buffer() {
        let mut compact = user("c1", 3, "summary text");
        compact.category = MessageCategory::Compact;
        compact.is_compact_summary = true;
        let msgs = vec![
            assistant("a1", 1, &[ContentBlock::Text { text: "1".into() }]),
            assistant("a2", 2, &[ContentBlock::Text { text: "2".into() }]),
            compact,
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 2);
        assert!(matches!(chunks[0], Chunk::Ai(_)));
        let Chunk::Compact(c) = &chunks[1] else {
            panic!("expected CompactChunk");
        };
        assert_eq!(c.summary_text, "summary text");
    }

    #[test]
    fn semantic_steps_follow_block_order() {
        let msgs = vec![assistant(
            "a1",
            1,
            &[
                ContentBlock::Thinking {
                    thinking: "reason".into(),
                    signature: String::new(),
                },
                ContentBlock::Text {
                    text: "hello".into(),
                },
                ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                },
            ],
        )];
        let chunks = build_chunks(&msgs);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.semantic_steps.len(), 3);
        assert!(matches!(
            ai.semantic_steps[0],
            SemanticStep::Thinking { .. }
        ));
        assert!(matches!(ai.semantic_steps[1], SemanticStep::Text { .. }));
        assert!(matches!(
            ai.semantic_steps[2],
            SemanticStep::ToolExecution { .. }
        ));
    }

    #[test]
    fn subagent_spawn_variant_not_emitted_yet() {
        let msgs = vec![assistant(
            "a1",
            1,
            &[ContentBlock::ToolUse {
                id: "t1".into(),
                name: "Task".into(),
                input: serde_json::json!({"description": "find things"}),
            }],
        )];
        let chunks = build_chunks(&msgs);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert!(
            !ai.semantic_steps
                .iter()
                .any(|s| matches!(s, SemanticStep::SubagentSpawn { .. }))
        );
    }

    #[test]
    fn tool_execution_list_is_empty_placeholder() {
        let msgs = vec![assistant(
            "a1",
            1,
            &[ContentBlock::ToolUse {
                id: "t1".into(),
                name: "Bash".into(),
                input: serde_json::json!({}),
            }],
        )];
        let chunks = build_chunks(&msgs);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert!(ai.tool_executions.is_empty());
        assert!(ai.subagents.is_empty());
    }

    #[test]
    fn tool_result_only_user_message_attaches_to_last_assistant() {
        let tool_result_block = ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: serde_json::json!("ok"),
            is_error: false,
        };
        let mut tool_result_user = blank_message("u2", 2);
        tool_result_user.content = MessageContent::Blocks(vec![tool_result_block]);
        let msgs = vec![
            assistant(
                "a1",
                1,
                &[ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
            tool_result_user,
            assistant(
                "a2",
                3,
                &[ContentBlock::Text {
                    text: "done".into(),
                }],
            ),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 1);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.responses.len(), 2);
    }

    #[test]
    fn metrics_sum_token_usage_across_responses() {
        let mut a1 = assistant("a1", 1, &[ContentBlock::Text { text: "1".into() }]);
        a1.usage = Some(TokenUsage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_input_tokens: 2,
            cache_creation_input_tokens: 1,
        });
        let mut a2 = assistant("a2", 2, &[ContentBlock::Text { text: "2".into() }]);
        a2.usage = Some(TokenUsage {
            input_tokens: 3,
            output_tokens: 4,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        });
        let chunks = build_chunks(&[a1, a2]);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.metrics.input_tokens, 13);
        assert_eq!(ai.metrics.output_tokens, 9);
        assert_eq!(ai.metrics.cache_read_tokens, 2);
        assert_eq!(ai.metrics.cache_creation_tokens, 1);
        assert_eq!(ai.metrics.cost_usd, None);
    }

    #[test]
    fn unused_tool_result_import_sanity() {
        let _ = ToolResult {
            tool_use_id: "x".into(),
            content: serde_json::json!(null),
            is_error: false,
        };
    }
}
