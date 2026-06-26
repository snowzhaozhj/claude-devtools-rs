//! Turn-level views for CLI/MCP consumers.
//!
//! Builds compact overviews and per-turn detail from session chunks,
//! consuming `cdt_analyze::derive_turns` as the single authority for
//! turn boundaries.

use cdt_analyze::{TurnDriver, derive_turns};
use cdt_core::{Chunk, ChunkMetrics};
use chrono::{DateTime, Utc};

use crate::step::{
    Step, ToolAggregation, aggregate_tools, build_chunk_map, build_steps_for_turn, detect_answer,
};

/// Compact overview of a single turn (for `get_session`).
#[derive(Debug, Clone)]
pub struct TurnOverview {
    pub index: u32,
    pub question: Option<String>,
    pub answer: Option<String>,
    pub tools: Vec<ToolAggregation>,
    pub steps_count: usize,
    pub metrics: TurnMetrics,
}

/// Detailed view of a single turn (for `get_turn`).
#[derive(Debug, Clone)]
pub struct TurnDetail {
    pub index: u32,
    pub question: Option<String>,
    pub answer: Option<String>,
    pub steps: Vec<Step>,
    pub metrics: TurnMetrics,
}

/// Per-turn metrics computed from chunk data.
#[derive(Debug, Clone, Default)]
pub struct TurnMetrics {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost: f64,
    pub duration_ms: i64,
    pub model: Option<String>,
}

/// Build compact turn overviews for all turns in a session.
#[must_use]
pub fn build_turn_overviews(chunks: &[Chunk]) -> Vec<TurnOverview> {
    let turns = derive_turns(chunks);
    let chunk_map = build_chunk_map(chunks);

    turns
        .iter()
        .map(|turn| {
            let question = extract_question(&turn.driver, chunks);
            let steps = build_steps_for_turn(&turn.member_chunk_ids, &chunk_map);
            let answer = detect_answer(&steps);
            let tools = aggregate_tools(&steps);
            let steps_count = steps.len();
            let metrics = compute_turn_metrics(&turn.member_chunk_ids, &chunk_map);

            TurnOverview {
                index: turn.index,
                question,
                answer,
                tools,
                steps_count,
                metrics,
            }
        })
        .collect()
}

/// Build detailed view for a single turn by index.
#[must_use]
pub fn build_turn_detail(chunks: &[Chunk], turn_index: u32) -> Option<TurnDetail> {
    let turns = derive_turns(chunks);
    let chunk_map = build_chunk_map(chunks);

    let turn = turns.iter().find(|t| t.index == turn_index)?;
    let question = extract_question(&turn.driver, chunks);
    let steps = build_steps_for_turn(&turn.member_chunk_ids, &chunk_map);
    let answer = detect_answer(&steps);
    let metrics = compute_turn_metrics(&turn.member_chunk_ids, &chunk_map);

    Some(TurnDetail {
        index: turn.index,
        question,
        answer,
        steps,
        metrics,
    })
}

/// Truncation threshold for tool output (bytes).
pub const TOOL_OUTPUT_TRUNCATE_THRESHOLD: usize = 5 * 1024;

/// Prefix length to keep when truncating tool output (chars).
const TOOL_OUTPUT_TRUNCATE_PREFIX_CHARS: usize = 2000;

/// Apply server-side truncation to tool step outputs in place.
///
/// For `tool` steps whose output text is >= `TOOL_OUTPUT_TRUNCATE_THRESHOLD`
/// bytes, truncates to `TOOL_OUTPUT_TRUNCATE_PREFIX_CHARS` characters and
/// sets `output_truncated=true` + records `output_bytes`.
pub fn truncate_tool_outputs(steps: &mut [Step]) {
    for step in steps.iter_mut() {
        if let Step::Tool { output, .. } = step {
            let bytes = output_byte_len(output);
            if bytes >= TOOL_OUTPUT_TRUNCATE_THRESHOLD {
                truncate_output(output, bytes);
            }
        }
    }
}

/// Returns `(output_truncated, output_bytes)` for a tool step's output.
#[must_use]
pub fn measure_output(output: &cdt_core::ToolOutput) -> (bool, Option<u64>) {
    let bytes = output_byte_len(output);
    if bytes >= TOOL_OUTPUT_TRUNCATE_THRESHOLD {
        (true, Some(bytes as u64))
    } else {
        (false, None)
    }
}

fn output_byte_len(output: &cdt_core::ToolOutput) -> usize {
    match output {
        cdt_core::ToolOutput::Text { text } => text.len(),
        cdt_core::ToolOutput::Structured { value } => {
            serde_json::to_string(value).map_or(0, |s| s.len())
        }
        cdt_core::ToolOutput::Missing => 0,
    }
}

fn truncate_output(output: &mut cdt_core::ToolOutput, _original_bytes: usize) {
    match output {
        cdt_core::ToolOutput::Text { text } => {
            let truncated: String = text
                .chars()
                .take(TOOL_OUTPUT_TRUNCATE_PREFIX_CHARS)
                .collect();
            *text = truncated;
        }
        cdt_core::ToolOutput::Structured { value } => {
            let s = serde_json::to_string(value).unwrap_or_default();
            let truncated: String = s.chars().take(TOOL_OUTPUT_TRUNCATE_PREFIX_CHARS).collect();
            *value = serde_json::Value::String(truncated);
        }
        cdt_core::ToolOutput::Missing => {}
    }
}

/// Grep `matchedIn` attribution — determines which part of a turn matched.
///
/// Priority: `tool:<name>` > `error` > `thinking` > `answer` > `question`.
#[must_use]
pub fn attribute_grep_match(
    needle: &str,
    question: Option<&str>,
    answer: Option<&str>,
    steps: &[Step],
) -> Option<String> {
    let needle_lower = needle.to_lowercase();

    for step in steps {
        if let Step::Tool {
            name,
            input,
            output,
            is_error,
            error_message,
            ..
        } = step
        {
            if *is_error {
                if let Some(msg) = error_message {
                    if msg.to_lowercase().contains(&needle_lower) {
                        return Some("error".to_string());
                    }
                }
            }
            let tool_text = format!("{name} {} {}", input, output_text_for_grep(output));
            if tool_text.to_lowercase().contains(&needle_lower) {
                return Some(format!("tool:{name}"));
            }
        }
    }

    for step in steps {
        if let Step::Thinking { text, .. } = step {
            if text.to_lowercase().contains(&needle_lower) {
                return Some("thinking".to_string());
            }
        }
    }

    if let Some(a) = answer {
        if a.to_lowercase().contains(&needle_lower) {
            return Some("answer".to_string());
        }
    }

    if let Some(q) = question {
        if q.to_lowercase().contains(&needle_lower) {
            return Some("question".to_string());
        }
    }

    None
}

fn output_text_for_grep(output: &cdt_core::ToolOutput) -> String {
    match output {
        cdt_core::ToolOutput::Text { text } => text.clone(),
        cdt_core::ToolOutput::Structured { value } => {
            serde_json::to_string(value).unwrap_or_default()
        }
        cdt_core::ToolOutput::Missing => String::new(),
    }
}

/// Check if a turn's content matches a grep pattern (case-insensitive substring).
#[must_use]
pub fn turn_matches_grep(
    needle: &str,
    question: Option<&str>,
    answer: Option<&str>,
    steps: &[Step],
) -> bool {
    attribute_grep_match(needle, question, answer, steps).is_some()
}

/// Extract the question text based on the turn driver.
fn extract_question(driver: &TurnDriver, chunks: &[Chunk]) -> Option<String> {
    match driver {
        TurnDriver::User(chunk_id) => chunks.iter().find_map(|c| {
            if let Chunk::User(u) = c {
                if u.chunk_id == *chunk_id {
                    return Some(extract_message_text(&u.content));
                }
            }
            None
        }),
        TurnDriver::Teammate(uuids) => {
            for chunk in chunks {
                if let Chunk::Ai(ai) = chunk {
                    for tm in &ai.teammate_messages {
                        if uuids.contains(&tm.uuid) {
                            return Some(tm.body.clone());
                        }
                    }
                }
            }
            None
        }
        TurnDriver::Headless => None,
    }
}

fn extract_message_text(content: &cdt_core::MessageContent) -> String {
    match content {
        cdt_core::MessageContent::Text(s) => s.clone(),
        cdt_core::MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| {
                if let cdt_core::ContentBlock::Text { text } = b {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// Compute metrics for a turn from its member chunks.
fn compute_turn_metrics(
    member_chunk_ids: &[String],
    chunk_map: &std::collections::HashMap<&str, &Chunk>,
) -> TurnMetrics {
    let mut metrics = TurnMetrics::default();
    let mut first_ts: Option<DateTime<Utc>> = None;
    let mut last_ts: Option<DateTime<Utc>> = None;
    let mut model: Option<String> = None;

    for chunk_id in member_chunk_ids {
        let Some(chunk) = chunk_map.get(chunk_id.as_str()) else {
            continue;
        };
        let cm: &ChunkMetrics = chunk.metrics();
        let ts = chunk.timestamp();

        metrics.input_tokens += cm.input_tokens;
        metrics.output_tokens += cm.output_tokens;
        metrics.cache_read_tokens += cm.cache_read_tokens;
        metrics.cache_creation_tokens += cm.cache_creation_tokens;
        if let Some(cost) = cm.cost_usd {
            metrics.cost += cost;
        }

        if first_ts.is_none() || ts < first_ts.unwrap() {
            first_ts = Some(ts);
        }

        if let Chunk::Ai(ai) = chunk {
            let end_ts = ai
                .duration_ms
                .map_or(ts, |d| ts + chrono::Duration::milliseconds(d));
            if last_ts.is_none() || end_ts > last_ts.unwrap() {
                last_ts = Some(end_ts);
            }
            if model.is_none() {
                if let Some(resp) = ai.responses.first() {
                    model.clone_from(&resp.model);
                }
            }
        } else if last_ts.is_none() || ts > last_ts.unwrap() {
            last_ts = Some(ts);
        }
    }

    if let (Some(first), Some(last)) = (first_ts, last_ts) {
        metrics.duration_ms = (last - first).num_milliseconds();
    }
    metrics.model = model;
    metrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::chunk::ChunkMetrics;
    use cdt_core::{AIChunk, SemanticStep, SystemChunk, UserChunk};

    fn ts(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).unwrap()
    }

    fn make_user_chunk(id: &str, text: &str, timestamp: i64) -> Chunk {
        Chunk::User(UserChunk {
            chunk_id: id.to_string(),
            uuid: format!("uuid-{id}"),
            timestamp: ts(timestamp),
            duration_ms: None,
            content: cdt_core::MessageContent::Text(text.to_string()),
            metrics: ChunkMetrics::default(),
        })
    }

    fn make_ai_chunk(id: &str, timestamp: i64, text: Option<&str>) -> Chunk {
        let mut semantic_steps = Vec::new();
        if let Some(t) = text {
            semantic_steps.push(SemanticStep::Text {
                text: t.to_string(),
                timestamp: ts(timestamp + 1),
            });
        }
        Chunk::Ai(AIChunk {
            chunk_id: id.to_string(),
            timestamp: ts(timestamp),
            duration_ms: Some(5000),
            responses: vec![],
            metrics: ChunkMetrics {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_tokens: 0,
                cache_read_tokens: 20,
                tool_count: 0,
                cost_usd: Some(0.01),
            },
            semantic_steps,
            tool_executions: vec![],
            subagents: vec![],
            slash_commands: vec![],
            teammate_messages: vec![],
        })
    }

    #[test]
    fn single_turn_overview() {
        let chunks = vec![
            make_user_chunk("u1", "What is 2+2?", 100),
            make_ai_chunk("a1", 101, Some("4")),
        ];
        let overviews = build_turn_overviews(&chunks);
        assert_eq!(overviews.len(), 1);
        assert_eq!(overviews[0].index, 0);
        assert_eq!(overviews[0].question.as_deref(), Some("What is 2+2?"));
        assert_eq!(overviews[0].answer.as_deref(), Some("4"));
        assert_eq!(overviews[0].steps_count, 1);
    }

    #[test]
    fn multiple_turns() {
        let chunks = vec![
            make_user_chunk("u1", "Q1", 100),
            make_ai_chunk("a1", 101, Some("A1")),
            make_user_chunk("u2", "Q2", 200),
            make_ai_chunk("a2", 201, Some("A2")),
        ];
        let overviews = build_turn_overviews(&chunks);
        assert_eq!(overviews.len(), 2);
        assert_eq!(overviews[0].question.as_deref(), Some("Q1"));
        assert_eq!(overviews[0].answer.as_deref(), Some("A1"));
        assert_eq!(overviews[1].question.as_deref(), Some("Q2"));
        assert_eq!(overviews[1].answer.as_deref(), Some("A2"));
    }

    #[test]
    fn headless_turn_null_question() {
        let chunks = vec![make_ai_chunk("a1", 100, Some("boot text"))];
        let overviews = build_turn_overviews(&chunks);
        assert_eq!(overviews.len(), 1);
        assert!(overviews[0].question.is_none());
        assert_eq!(overviews[0].answer.as_deref(), Some("boot text"));
    }

    #[test]
    fn system_chunk_folded_into_turn() {
        let chunks = vec![
            Chunk::System(SystemChunk {
                chunk_id: "sys-1".into(),
                uuid: "u-sys".into(),
                timestamp: ts(50),
                duration_ms: None,
                content_text: "system prompt".into(),
                metrics: ChunkMetrics::default(),
            }),
            make_user_chunk("u1", "Hi", 100),
            make_ai_chunk("a1", 101, Some("Hello")),
        ];
        let overviews = build_turn_overviews(&chunks);
        assert_eq!(overviews.len(), 1);
        assert!(overviews[0].steps_count >= 2);
    }

    #[test]
    fn turn_detail_returns_steps() {
        let chunks = vec![
            make_user_chunk("u1", "Explain Rust", 100),
            make_ai_chunk("a1", 101, Some("Rust is a systems language")),
        ];
        let detail = build_turn_detail(&chunks, 0).unwrap();
        assert_eq!(detail.question.as_deref(), Some("Explain Rust"));
        assert_eq!(detail.answer.as_deref(), Some("Rust is a systems language"));
        assert!(!detail.steps.is_empty());
    }

    #[test]
    fn turn_detail_invalid_index_returns_none() {
        let chunks = vec![
            make_user_chunk("u1", "Q", 100),
            make_ai_chunk("a1", 101, Some("A")),
        ];
        assert!(build_turn_detail(&chunks, 99).is_none());
    }

    #[test]
    fn metrics_accumulate_across_member_chunks() {
        let chunks = vec![
            make_user_chunk("u1", "Q", 100),
            make_ai_chunk("a1", 101, Some("A")),
        ];
        let overviews = build_turn_overviews(&chunks);
        let m = &overviews[0].metrics;
        assert_eq!(m.input_tokens, 100);
        assert_eq!(m.output_tokens, 50);
        assert!(m.cost > 0.0);
        assert!(m.duration_ms > 0);
    }
}
