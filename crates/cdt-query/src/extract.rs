//! Item-level extraction from chunk sequences.
//!
//! Sits between raw `&[Chunk]` and session-level aggregation (`summary.rs`),
//! providing flat entry sequences that CLI `--extract` and MCP can consume directly.

use std::collections::HashMap;

use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

use cdt_core::tool_execution::ToolOutput;
use cdt_core::{Chunk, ToolExecution};

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkOverviewEntry {
    pub chunk_index: usize,
    pub kind: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    pub tool_count: usize,
    pub error_count: usize,
    pub tool_names: Vec<String>,
    pub response_count: usize,
    pub content_chars: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecEntry {
    pub chunk_index: usize,
    pub tool_index: usize,
    pub tool_name: String,
    pub tool_use_id: String,
    pub is_error: bool,
    pub input_summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
    pub output_chars: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Extraction functions
// ─────────────────────────────────────────────────────────────────────────────

pub fn extract_overview(indexed: &[(usize, &Chunk)]) -> Vec<ChunkOverviewEntry> {
    indexed
        .iter()
        .map(|(abs_idx, chunk)| {
            let (tool_count, error_count, tool_names, response_count) = match chunk {
                Chunk::Ai(ai) => {
                    let mut name_counts: HashMap<&str, usize> = HashMap::new();
                    let mut err = 0usize;
                    for te in &ai.tool_executions {
                        *name_counts.entry(&te.tool_name).or_default() += 1;
                        if te.is_error {
                            err += 1;
                        }
                    }
                    let mut names: Vec<(&str, usize)> = name_counts.into_iter().collect();
                    names.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
                    let names: Vec<String> =
                        names.into_iter().map(|(n, _)| n.to_string()).collect();
                    (ai.tool_executions.len(), err, names, ai.responses.len())
                }
                _ => (0, 0, Vec::new(), 0),
            };

            let content_chars = match chunk {
                Chunk::User(u) => content_char_count(&u.content),
                Chunk::System(s) => s.content_text.chars().count(),
                Chunk::Compact(c) => c.summary_text.chars().count(),
                Chunk::Ai(_) => 0,
            };

            ChunkOverviewEntry {
                chunk_index: *abs_idx,
                kind: chunk_kind_str(chunk).to_string(),
                timestamp: chunk.timestamp().to_rfc3339(),
                duration_ms: chunk_duration_ms(chunk),
                tool_count,
                error_count,
                tool_names,
                response_count,
                content_chars,
            }
        })
        .collect()
}

pub fn extract_tool_executions(indexed: &[(usize, &Chunk)]) -> Vec<ToolExecEntry> {
    let mut entries = Vec::new();
    for (abs_idx, chunk) in indexed {
        if let Chunk::Ai(ai) = chunk {
            for (ti, te) in ai.tool_executions.iter().enumerate() {
                entries.push(build_tool_exec_entry(*abs_idx, ti, te));
            }
        }
    }
    entries
}

pub fn extract_errors(indexed: &[(usize, &Chunk)]) -> Vec<ToolExecEntry> {
    let mut entries = Vec::new();
    for (abs_idx, chunk) in indexed {
        if let Chunk::Ai(ai) = chunk {
            for (ti, te) in ai.tool_executions.iter().enumerate() {
                if te.is_error {
                    entries.push(build_tool_exec_entry(*abs_idx, ti, te));
                }
            }
        }
    }
    entries
}

// ─────────────────────────────────────────────────────────────────────────────
// Error summary extraction
// ─────────────────────────────────────────────────────────────────────────────

static EXIT_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"exit (?:code|status) (\d+)").expect("valid regex"));

pub fn extract_error_summary(te: &ToolExecution) -> Option<String> {
    if let Some(ref msg) = te.error_message {
        if !msg.is_empty() {
            return Some(msg.clone());
        }
    }

    match &te.output {
        ToolOutput::Structured { value } => extract_from_structured(value),
        ToolOutput::Text { text } => extract_from_text(text),
        ToolOutput::Missing => None,
    }
}

fn extract_from_structured(value: &serde_json::Value) -> Option<String> {
    if let Some(stderr) = value.get("stderr").and_then(|v| v.as_str()) {
        let trimmed = stderr.trim();
        if !trimmed.is_empty() {
            return Some(truncate_tail(trimmed, 200));
        }
    }

    if let Some(error) = value
        .get("error")
        .or_else(|| value.get("message"))
        .and_then(|v| v.as_str())
    {
        let trimmed = error.trim();
        if !trimmed.is_empty() {
            return Some(truncate_tail(trimmed, 200));
        }
    }

    let exit_code = value
        .get("exit_code")
        .or_else(|| value.get("exitCode"))
        .and_then(serde_json::Value::as_i64);
    if let Some(code) = exit_code {
        return Some(format!("exit code {code}"));
    }

    let serialized = serde_json::to_string(value).unwrap_or_default();
    if serialized.len() > 2 {
        return Some(truncate_tail(&serialized, 200));
    }

    None
}

fn extract_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(caps) = EXIT_CODE_RE.captures(trimmed) {
        return Some(format!("exit code {}", &caps[1]));
    }

    Some(truncate_tail(trimmed, 200))
}

// ─────────────────────────────────────────────────────────────────────────────
// Input summarization (shared with view.rs)
// ─────────────────────────────────────────────────────────────────────────────

const SUMMARY_MAX_FIELDS: usize = 3;
const SUMMARY_VALUE_MAX_CHARS: usize = 57;

pub fn summarize_input(input: &serde_json::Value) -> String {
    match input {
        serde_json::Value::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .take(SUMMARY_MAX_FIELDS)
                .map(|(k, v)| {
                    let val_str = match v {
                        serde_json::Value::String(s) => truncate_str(s, SUMMARY_VALUE_MAX_CHARS),
                        other => truncate_str(&other.to_string(), SUMMARY_VALUE_MAX_CHARS),
                    };
                    format!("{k}: {val_str}")
                })
                .collect();
            if map.len() > SUMMARY_MAX_FIELDS {
                format!(
                    "{} (+{} more)",
                    parts.join(", "),
                    map.len() - SUMMARY_MAX_FIELDS
                )
            } else {
                parts.join(", ")
            }
        }
        other => truncate_str(&other.to_string(), 117),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn build_tool_exec_entry(
    chunk_index: usize,
    tool_index: usize,
    te: &ToolExecution,
) -> ToolExecEntry {
    let output_chars = if te.output_omitted {
        te.output_bytes
            .map_or(0, |b| usize::try_from(b).unwrap_or(usize::MAX))
    } else {
        match &te.output {
            ToolOutput::Text { text } => text.chars().count(),
            ToolOutput::Structured { value } => serde_json::to_string(value).map_or(0, |s| s.len()),
            ToolOutput::Missing => 0,
        }
    };

    ToolExecEntry {
        chunk_index,
        tool_index,
        tool_name: te.tool_name.clone(),
        tool_use_id: te.tool_use_id.clone(),
        is_error: te.is_error,
        input_summary: summarize_input(&te.input),
        error_summary: if te.is_error {
            extract_error_summary(te)
        } else {
            None
        },
        output_chars,
    }
}

fn chunk_duration_ms(chunk: &Chunk) -> Option<i64> {
    match chunk {
        Chunk::Ai(ai) => ai.duration_ms,
        Chunk::User(u) => u.duration_ms,
        Chunk::System(s) => s.duration_ms,
        Chunk::Compact(c) => c.duration_ms,
    }
}

fn chunk_kind_str(chunk: &Chunk) -> &'static str {
    match chunk {
        Chunk::Ai(_) => "ai",
        Chunk::User(_) => "user",
        Chunk::System(_) => "system",
        Chunk::Compact(_) => "compact",
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}...")
}

fn truncate_tail(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        return s.to_string();
    }
    let skip = char_count - max_chars;
    let truncated: String = s.chars().skip(skip).collect();
    format!("...{truncated}")
}

fn content_char_count(content: &cdt_core::message::MessageContent) -> usize {
    match content {
        cdt_core::message::MessageContent::Text(s) => s.chars().count(),
        cdt_core::message::MessageContent::Blocks(blocks) => blocks
            .iter()
            .map(|b| match b {
                cdt_core::message::ContentBlock::Text { text } => text.chars().count(),
                cdt_core::message::ContentBlock::Thinking { thinking, .. } => {
                    thinking.chars().count()
                }
                _ => 0,
            })
            .sum(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::message::MessageContent;
    use cdt_core::{AIChunk, ChunkMetrics, UserChunk, chunk::AssistantResponse};
    use chrono::{TimeZone, Utc};

    fn ts(minutes: i64) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap() + chrono::Duration::minutes(minutes)
    }

    fn make_tool_exec(
        name: &str,
        is_error: bool,
        error_message: Option<&str>,
        output: ToolOutput,
    ) -> ToolExecution {
        ToolExecution {
            tool_use_id: format!("tu-{name}"),
            tool_name: name.to_string(),
            input: serde_json::json!({"command": "test"}),
            output,
            is_error,
            start_ts: ts(0),
            end_ts: None,
            source_assistant_uuid: String::new(),
            result_agent_id: None,
            error_message: error_message.map(ToString::to_string),
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
            workflow_run_id: None,
            workflow_script_path: None,
        }
    }

    fn make_ai_chunk(minute: i64, tools: Vec<ToolExecution>) -> Chunk {
        Chunk::Ai(AIChunk {
            chunk_id: format!("ai-{minute}"),
            timestamp: ts(minute),
            duration_ms: Some(1000),
            responses: vec![AssistantResponse {
                uuid: format!("resp-{minute}"),
                timestamp: ts(minute),
                content: MessageContent::Text("response".into()),
                tool_calls: vec![],
                usage: None,
                model: Some("claude-sonnet-4-6".into()),
                content_omitted: false,
            }],
            metrics: ChunkMetrics {
                tool_count: tools.len() as u64,
                ..Default::default()
            },
            semantic_steps: vec![],
            tool_executions: tools,
            subagents: vec![],
            slash_commands: vec![],
            teammate_messages: vec![],
        })
    }

    fn make_user_chunk(minute: i64) -> Chunk {
        Chunk::User(UserChunk {
            chunk_id: format!("user-{minute}"),
            uuid: format!("u-{minute}"),
            timestamp: ts(minute),
            duration_ms: None,
            content: MessageContent::Text("hello world".into()),
            metrics: ChunkMetrics::default(),
        })
    }

    #[test]
    fn extract_overview_empty_chunks() {
        let result = extract_overview(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_overview_preserves_absolute_index() {
        let user = make_user_chunk(0);
        let ai = make_ai_chunk(
            1,
            vec![
                make_tool_exec("Bash", false, None, ToolOutput::Text { text: "ok".into() }),
                make_tool_exec("Read", false, None, ToolOutput::Text { text: "ok".into() }),
                make_tool_exec(
                    "Bash",
                    true,
                    Some("fail"),
                    ToolOutput::Text { text: "err".into() },
                ),
            ],
        );
        let indexed: Vec<(usize, &Chunk)> = vec![(5, &user), (10, &ai)];
        let result = extract_overview(&indexed);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].chunk_index, 5);
        assert_eq!(result[0].kind, "user");
        assert_eq!(result[0].tool_count, 0);
        assert_eq!(result[1].chunk_index, 10);
        assert_eq!(result[1].kind, "ai");
        assert_eq!(result[1].tool_count, 3);
        assert_eq!(result[1].error_count, 1);
        assert_eq!(result[1].tool_names, vec!["Bash", "Read"]);
    }

    #[test]
    fn extract_tool_executions_flat_across_chunks() {
        let ai1 = make_ai_chunk(
            0,
            vec![make_tool_exec(
                "Read",
                false,
                None,
                ToolOutput::Text { text: "ok".into() },
            )],
        );
        let ai2 = make_ai_chunk(
            1,
            vec![
                make_tool_exec("Bash", false, None, ToolOutput::Text { text: "ok".into() }),
                make_tool_exec(
                    "Edit",
                    true,
                    Some("not found"),
                    ToolOutput::Text { text: "err".into() },
                ),
            ],
        );
        let indexed: Vec<(usize, &Chunk)> = vec![(2, &ai1), (4, &ai2)];
        let result = extract_tool_executions(&indexed);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].chunk_index, 2);
        assert_eq!(result[0].tool_index, 0);
        assert_eq!(result[0].tool_name, "Read");
        assert_eq!(result[1].chunk_index, 4);
        assert_eq!(result[1].tool_index, 0);
        assert_eq!(result[2].chunk_index, 4);
        assert_eq!(result[2].tool_index, 1);
        assert_eq!(result[2].tool_name, "Edit");
        assert!(result[2].is_error);
    }

    #[test]
    fn extract_errors_only_errors() {
        let ai = make_ai_chunk(
            0,
            vec![
                make_tool_exec("Read", false, None, ToolOutput::Text { text: "ok".into() }),
                make_tool_exec(
                    "Bash",
                    true,
                    Some("fail"),
                    ToolOutput::Text { text: "err".into() },
                ),
                make_tool_exec("Edit", false, None, ToolOutput::Text { text: "ok".into() }),
            ],
        );
        let indexed: Vec<(usize, &Chunk)> = vec![(0, &ai)];
        let result = extract_errors(&indexed);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tool_name, "Bash");
        assert_eq!(result[0].error_summary.as_deref(), Some("fail"));
    }

    #[test]
    fn error_summary_prefers_error_message() {
        let te = make_tool_exec(
            "Bash",
            true,
            Some("explicit error"),
            ToolOutput::Structured {
                value: serde_json::json!({"stderr": "should not appear", "exit_code": 1}),
            },
        );
        assert_eq!(
            extract_error_summary(&te).as_deref(),
            Some("explicit error")
        );
    }

    #[test]
    fn error_summary_structured_stderr() {
        let te = make_tool_exec(
            "Bash",
            true,
            None,
            ToolOutput::Structured {
                value: serde_json::json!({"stderr": "command not found", "exit_code": 127}),
            },
        );
        assert_eq!(
            extract_error_summary(&te).as_deref(),
            Some("command not found")
        );
    }

    #[test]
    fn error_summary_structured_exit_code_no_stderr() {
        let te = make_tool_exec(
            "Bash",
            true,
            None,
            ToolOutput::Structured {
                value: serde_json::json!({"stdout": "some output", "exit_code": 1}),
            },
        );
        assert_eq!(extract_error_summary(&te).as_deref(), Some("exit code 1"));
    }

    #[test]
    fn error_summary_text_exit_code() {
        let te = make_tool_exec(
            "Bash",
            true,
            None,
            ToolOutput::Text {
                text: "some output\nexit code 42\n".into(),
            },
        );
        assert_eq!(extract_error_summary(&te).as_deref(), Some("exit code 42"));
    }

    #[test]
    fn error_summary_text_fallback() {
        let te = make_tool_exec(
            "Bash",
            true,
            None,
            ToolOutput::Text {
                text: "some random error output".into(),
            },
        );
        assert_eq!(
            extract_error_summary(&te).as_deref(),
            Some("some random error output")
        );
    }

    #[test]
    fn error_summary_missing_output() {
        let te = make_tool_exec("Bash", true, None, ToolOutput::Missing);
        assert_eq!(extract_error_summary(&te), None);
    }

    #[test]
    fn summarize_input_small_object() {
        let input = serde_json::json!({"file_path": "/tmp/test.rs", "command": "ls"});
        let result = summarize_input(&input);
        assert!(result.contains("file_path"));
        assert!(result.contains("command"));
    }

    #[test]
    fn summarize_input_large_object_truncated() {
        let input = serde_json::json!({"a": 1, "b": 2, "c": 3, "d": 4, "e": 5});
        let result = summarize_input(&input);
        assert!(result.contains("(+2 more)"));
    }

    #[test]
    fn overview_user_chunk_content_chars() {
        let user = make_user_chunk(0);
        let indexed: Vec<(usize, &Chunk)> = vec![(0, &user)];
        let result = extract_overview(&indexed);
        assert_eq!(result[0].content_chars, "hello world".len());
    }
}
