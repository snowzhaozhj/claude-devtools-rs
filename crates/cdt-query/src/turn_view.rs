//! Turn-level views for CLI/MCP consumers.
//!
//! Builds compact overviews and per-turn detail from session chunks,
//! consuming `cdt_analyze::derive_turns` as the single authority for
//! turn boundaries.

use std::collections::{HashMap, HashSet};

use cdt_analyze::{TurnDriver, derive_turns};
use cdt_core::Chunk;
use chrono::{DateTime, Utc};

use crate::cost::lookup_pricing;
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
    pub step_counts: HashMap<String, usize>,
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
            let step_counts = count_steps_by_type(&steps);
            let metrics = compute_turn_metrics(&turn.member_chunk_ids, &chunk_map);

            TurnOverview {
                index: turn.index,
                question,
                answer,
                tools,
                step_counts,
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
            ..
        } = step
        {
            let tool_text = format!("{name} {} {}", input, output_text_for_grep(output));
            if tool_text.to_lowercase().contains(&needle_lower) {
                return Some(format!("tool:{name}"));
            }
        }
    }

    for step in steps {
        if let Step::Tool {
            is_error: true,
            error_message: Some(msg),
            ..
        } = step
        {
            if msg.to_lowercase().contains(&needle_lower) {
                return Some("error".to_string());
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

/// Extract file paths modified in a session from tool execution inputs.
#[must_use]
pub fn extract_files_modified(chunks: &[Chunk]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    for chunk in chunks {
        let Chunk::Ai(ai) = chunk else { continue };
        for exec in &ai.tool_executions {
            match exec.tool_name.as_str() {
                "Edit" | "Write" => {
                    if let Some(fp) = exec.input.get("file_path").and_then(|v| v.as_str()) {
                        if seen.insert(fp.to_string()) {
                            files.push(fp.to_string());
                        }
                    }
                }
                "MultiEdit" => {
                    if let Some(arr) = exec.input.get("files").and_then(|v| v.as_array()) {
                        for f in arr {
                            if let Some(fp) = f.get("file_path").and_then(|v| v.as_str()) {
                                if seen.insert(fp.to_string()) {
                                    files.push(fp.to_string());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    files
}

/// Compute wall-clock session duration from first to last chunk boundary.
#[must_use]
pub fn compute_session_duration_ms(chunks: &[Chunk]) -> i64 {
    let mut first_ts: Option<DateTime<Utc>> = None;
    let mut last_ts: Option<DateTime<Utc>> = None;
    for chunk in chunks {
        let ts = chunk.timestamp();
        if first_ts.is_none() || ts < first_ts.unwrap() {
            first_ts = Some(ts);
        }
        if let Chunk::Ai(ai) = chunk {
            let end = ai
                .duration_ms
                .map_or(ts, |d| ts + chrono::Duration::milliseconds(d));
            if last_ts.is_none() || end > last_ts.unwrap() {
                last_ts = Some(end);
            }
        } else if last_ts.is_none() || ts > last_ts.unwrap() {
            last_ts = Some(ts);
        }
    }
    match (first_ts, last_ts) {
        (Some(f), Some(l)) => (l - f).num_milliseconds(),
        _ => 0,
    }
}

/// Compute total session cost from all AI response usage + pricing table.
#[must_use]
pub fn compute_session_cost_from_chunks(chunks: &[Chunk]) -> f64 {
    let mut total = 0.0_f64;
    for chunk in chunks {
        let Chunk::Ai(ai) = chunk else { continue };
        for resp in &ai.responses {
            let Some(ref usage) = resp.usage else {
                continue;
            };
            let model_id = resp.model.as_deref().unwrap_or("unknown");
            let pricing = lookup_pricing(model_id);
            #[allow(clippy::cast_precision_loss)]
            {
                total += usage.input_tokens as f64 * pricing.input_per_mtok / 1_000_000.0;
                total += usage.output_tokens as f64 * pricing.output_per_mtok / 1_000_000.0;
                total += usage.cache_read_input_tokens as f64 * pricing.cache_read_per_mtok
                    / 1_000_000.0;
                total += usage.cache_creation_input_tokens as f64 * pricing.cache_write_per_mtok
                    / 1_000_000.0;
            }
        }
    }
    total
}

/// Count steps by type within a turn.
#[must_use]
pub fn count_steps_by_type(steps: &[Step]) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for step in steps {
        let key = match step {
            Step::Thinking { .. } => "thinking",
            Step::Text { .. } => "text",
            Step::Tool { .. } => "tool",
            Step::Subagent { .. } => "subagent",
            Step::TeammateSpawn { .. } => "teammate_spawn",
            Step::Workflow { .. } => "workflow",
            Step::Interruption { .. } => "interruption",
            Step::UserMessage { .. } => "user_message",
            Step::Slash { .. } => "slash",
            Step::TeammateMsg { .. } => "teammate_message",
            Step::Compaction { .. } => "compaction",
            Step::System { .. } => "system",
        };
        *counts.entry(key.to_string()).or_insert(0) += 1;
    }
    counts
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
        let cm = chunk.metrics();
        let ts = chunk.timestamp();

        metrics.input_tokens += cm.input_tokens;
        metrics.output_tokens += cm.output_tokens;
        metrics.cache_read_tokens += cm.cache_read_tokens;
        metrics.cache_creation_tokens += cm.cache_creation_tokens;

        if let Chunk::Ai(ai) = chunk {
            for resp in &ai.responses {
                if let Some(ref usage) = resp.usage {
                    let model_id = resp.model.as_deref().unwrap_or("unknown");
                    let pricing = lookup_pricing(model_id);
                    #[allow(clippy::cast_precision_loss)]
                    {
                        metrics.cost +=
                            usage.input_tokens as f64 * pricing.input_per_mtok / 1_000_000.0;
                        metrics.cost +=
                            usage.output_tokens as f64 * pricing.output_per_mtok / 1_000_000.0;
                        metrics.cost += usage.cache_read_input_tokens as f64
                            * pricing.cache_read_per_mtok
                            / 1_000_000.0;
                        metrics.cost += usage.cache_creation_input_tokens as f64
                            * pricing.cache_write_per_mtok
                            / 1_000_000.0;
                    }
                }
            }

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

        if first_ts.is_none() || ts < first_ts.unwrap() {
            first_ts = Some(ts);
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
    use cdt_core::{AIChunk, AssistantResponse, SemanticStep, SystemChunk, TokenUsage, UserChunk};

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
            responses: vec![AssistantResponse {
                uuid: format!("resp-{id}"),
                timestamp: ts(timestamp),
                content: cdt_core::MessageContent::Text(String::new()),
                tool_calls: vec![],
                usage: Some(TokenUsage {
                    input_tokens: 100,
                    output_tokens: 50,
                    cache_read_input_tokens: 20,
                    cache_creation_input_tokens: 0,
                }),
                model: Some("claude-sonnet-4-20250514".to_string()),
                content_omitted: false,
            }],
            metrics: ChunkMetrics {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_tokens: 0,
                cache_read_tokens: 20,
                tool_count: 0,
                cost_usd: None,
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
        assert_eq!(overviews[0].step_counts.get("text"), Some(&1));
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
        let total: usize = overviews[0].step_counts.values().sum();
        assert!(total >= 2);
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
    fn metrics_cost_computed_from_pricing_table() {
        let chunks = vec![
            make_user_chunk("u1", "Q", 100),
            make_ai_chunk("a1", 101, Some("A")),
        ];
        let overviews = build_turn_overviews(&chunks);
        let m = &overviews[0].metrics;
        assert_eq!(m.input_tokens, 100);
        assert_eq!(m.output_tokens, 50);
        assert!(
            m.cost > 0.0,
            "cost should be positive (from pricing table), got {}",
            m.cost
        );
        assert!(m.duration_ms > 0);
        assert_eq!(m.model.as_deref(), Some("claude-sonnet-4-20250514"));
    }

    #[test]
    fn session_cost_from_chunks_matches_turn_sum() {
        let chunks = vec![
            make_user_chunk("u1", "Q1", 100),
            make_ai_chunk("a1", 101, Some("A1")),
            make_user_chunk("u2", "Q2", 200),
            make_ai_chunk("a2", 201, Some("A2")),
        ];
        let overviews = build_turn_overviews(&chunks);
        let sum_turn_cost: f64 = overviews.iter().map(|o| o.metrics.cost).sum();
        let session_cost = compute_session_cost_from_chunks(&chunks);
        let diff = (sum_turn_cost - session_cost).abs();
        assert!(
            diff < 1e-10,
            "session cost {session_cost} should match sum of turn costs {sum_turn_cost}"
        );
    }

    #[test]
    fn session_wall_clock_duration() {
        let chunks = vec![
            make_user_chunk("u1", "Q", 100),
            make_ai_chunk("a1", 101, Some("A")),
        ];
        let dur = compute_session_duration_ms(&chunks);
        assert_eq!(dur, 6000);
    }

    #[test]
    fn extract_files_modified_from_tool_executions() {
        use cdt_core::ToolExecution;
        let chunks = vec![Chunk::Ai(AIChunk {
            chunk_id: "a1".into(),
            timestamp: ts(100),
            duration_ms: None,
            responses: vec![],
            metrics: ChunkMetrics::default(),
            semantic_steps: vec![],
            tool_executions: vec![
                ToolExecution {
                    tool_use_id: "tu1".into(),
                    tool_name: "Edit".into(),
                    input: serde_json::json!({"file_path": "/src/main.rs"}),
                    output: cdt_core::ToolOutput::Missing,
                    is_error: false,
                    start_ts: ts(100),
                    end_ts: None,
                    source_assistant_uuid: "r1".into(),
                    result_agent_id: None,
                    error_message: None,
                    output_omitted: false,
                    output_bytes: None,
                    teammate_spawn: None,
                    workflow_run_id: None,
                    workflow_script_path: None,
                },
                ToolExecution {
                    tool_use_id: "tu2".into(),
                    tool_name: "Write".into(),
                    input: serde_json::json!({"file_path": "/src/lib.rs"}),
                    output: cdt_core::ToolOutput::Missing,
                    is_error: false,
                    start_ts: ts(101),
                    end_ts: None,
                    source_assistant_uuid: "r1".into(),
                    result_agent_id: None,
                    error_message: None,
                    output_omitted: false,
                    output_bytes: None,
                    teammate_spawn: None,
                    workflow_run_id: None,
                    workflow_script_path: None,
                },
                ToolExecution {
                    tool_use_id: "tu3".into(),
                    tool_name: "Edit".into(),
                    input: serde_json::json!({"file_path": "/src/main.rs"}),
                    output: cdt_core::ToolOutput::Missing,
                    is_error: false,
                    start_ts: ts(102),
                    end_ts: None,
                    source_assistant_uuid: "r1".into(),
                    result_agent_id: None,
                    error_message: None,
                    output_omitted: false,
                    output_bytes: None,
                    teammate_spawn: None,
                    workflow_run_id: None,
                    workflow_script_path: None,
                },
                ToolExecution {
                    tool_use_id: "tu4".into(),
                    tool_name: "Bash".into(),
                    input: serde_json::json!({"command": "ls"}),
                    output: cdt_core::ToolOutput::Missing,
                    is_error: false,
                    start_ts: ts(103),
                    end_ts: None,
                    source_assistant_uuid: "r1".into(),
                    result_agent_id: None,
                    error_message: None,
                    output_omitted: false,
                    output_bytes: None,
                    teammate_spawn: None,
                    workflow_run_id: None,
                    workflow_script_path: None,
                },
            ],
            subagents: vec![],
            slash_commands: vec![],
            teammate_messages: vec![],
        })];
        let files = extract_files_modified(&chunks);
        assert_eq!(files, vec!["/src/main.rs", "/src/lib.rs"]);
    }

    #[test]
    fn step_counts_by_type() {
        let chunks = vec![
            make_user_chunk("u1", "Q", 100),
            make_ai_chunk("a1", 101, Some("A")),
        ];
        let overviews = build_turn_overviews(&chunks);
        let sc = &overviews[0].step_counts;
        assert_eq!(sc.get("text"), Some(&1));
        assert!(sc.get("tool").is_none());
    }

    #[test]
    fn count_steps_by_type_covers_all_variants() {
        use crate::step::Step;
        let steps = vec![
            Step::Thinking {
                text: "hmm".into(),
                timestamp: ts(1),
            },
            Step::Text {
                text: "hi".into(),
                timestamp: ts(2),
            },
            Step::Text {
                text: "bye".into(),
                timestamp: ts(3),
            },
            Step::Compaction {
                summary: "s".into(),
                timestamp: ts(4),
            },
        ];
        let counts = count_steps_by_type(&steps);
        assert_eq!(counts.get("thinking"), Some(&1));
        assert_eq!(counts.get("text"), Some(&2));
        assert_eq!(counts.get("compaction"), Some(&1));
        assert_eq!(counts.len(), 3);
    }
}
