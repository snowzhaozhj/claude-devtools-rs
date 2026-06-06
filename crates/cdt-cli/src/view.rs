//! Shared view layer for CLI and MCP output.
//!
//! Protocol-agnostic types and builders extracted from MCP;
//! both `main.rs` (CLI) and `mcp/mod.rs` reference this module.

use serde::Serialize;

use cdt_core::message::MessageContent;
use cdt_core::tool_execution::ToolOutput;
use cdt_core::{Chunk, ToolExecution};

// ─────────────────────────────────────────────────────────────────────────────
// Content mode
// ─────────────────────────────────────────────────────────────────────────────

pub enum ContentMode {
    Omit,
    Full,
}

// ─────────────────────────────────────────────────────────────────────────────
// View types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkView {
    pub chunk_index: usize,
    pub chunk_id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_executions: Vec<ToolExecView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub responses: Vec<ResponseView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_content: Option<ContentField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_content: Option<ContentField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grep_hit: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecView {
    pub tool_name: String,
    pub tool_use_id: String,
    pub is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    pub output_omitted: bool,
    pub output_chars: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub content_omitted: bool,
    pub content_chars: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentField {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub omitted: bool,
    pub chars: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Builders
// ─────────────────────────────────────────────────────────────────────────────

/// 构造 `ContentField`，User 和 System chunk 共用。
/// Omit 模式下短文本（<= 200 chars）仍内联，长文本省略。
fn build_content_field(text: String, mode: &ContentMode) -> ContentField {
    let chars = text.chars().count();
    match mode {
        ContentMode::Omit => ContentField {
            omitted: chars > 200,
            text: if chars <= 200 { Some(text) } else { None },
            chars,
        },
        ContentMode::Full => ContentField {
            text: Some(text),
            omitted: false,
            chars,
        },
    }
}

/// 公共字段全置默认值的 `ChunkView` 骨架，各 variant 用 struct update 覆盖差异字段。
fn base_chunk_view(
    chunk_index: usize,
    chunk_id: String,
    kind: &str,
    timestamp: String,
    duration_ms: Option<i64>,
    grep_hit: Option<bool>,
) -> ChunkView {
    ChunkView {
        chunk_index,
        chunk_id,
        kind: kind.to_string(),
        timestamp,
        duration_ms,
        tool_executions: vec![],
        responses: vec![],
        user_content: None,
        system_content: None,
        compact_summary: None,
        grep_hit,
    }
}

pub fn build_chunk_view(
    abs_index: usize,
    chunk: &Chunk,
    mode: &ContentMode,
    grep_hit: Option<bool>,
) -> ChunkView {
    match chunk {
        Chunk::Ai(ai) => {
            let tool_execs = ai
                .tool_executions
                .iter()
                .map(|te| build_tool_exec_view(te, mode))
                .collect();

            let responses = ai
                .responses
                .iter()
                .map(|r| {
                    let text = message_content_text(&r.content);
                    let content_chars = text.chars().count();
                    let upstream_omitted = r.content_omitted;
                    match mode {
                        ContentMode::Omit => ResponseView {
                            model: r.model.clone(),
                            content: None,
                            content_omitted: true,
                            content_chars,
                        },
                        ContentMode::Full => ResponseView {
                            model: r.model.clone(),
                            content: if upstream_omitted { None } else { Some(text) },
                            content_omitted: upstream_omitted,
                            content_chars,
                        },
                    }
                })
                .collect();

            ChunkView {
                tool_executions: tool_execs,
                responses,
                ..base_chunk_view(
                    abs_index,
                    ai.chunk_id.clone(),
                    "ai",
                    ai.timestamp.to_rfc3339(),
                    ai.duration_ms,
                    grep_hit,
                )
            }
        }
        Chunk::User(user) => ChunkView {
            user_content: Some(build_content_field(
                message_content_text(&user.content),
                mode,
            )),
            ..base_chunk_view(
                abs_index,
                user.chunk_id.clone(),
                "user",
                user.timestamp.to_rfc3339(),
                user.duration_ms,
                grep_hit,
            )
        },
        Chunk::System(sys) => ChunkView {
            system_content: Some(build_content_field(sys.content_text.clone(), mode)),
            ..base_chunk_view(
                abs_index,
                sys.chunk_id.clone(),
                "system",
                sys.timestamp.to_rfc3339(),
                sys.duration_ms,
                grep_hit,
            )
        },
        Chunk::Compact(compact) => ChunkView {
            compact_summary: Some(compact.summary_text.clone()),
            ..base_chunk_view(
                abs_index,
                compact.chunk_id.clone(),
                "compact",
                compact.timestamp.to_rfc3339(),
                compact.duration_ms,
                grep_hit,
            )
        },
    }
}

pub fn build_tool_exec_view(te: &ToolExecution, mode: &ContentMode) -> ToolExecView {
    let output_text = tool_output_text(&te.output);
    let upstream_omitted = te.output_omitted;
    let output_chars = if upstream_omitted {
        te.output_bytes
            .map_or(0, |b| usize::try_from(b).unwrap_or(usize::MAX))
    } else {
        output_text.chars().count()
    };

    match mode {
        ContentMode::Omit => ToolExecView {
            tool_name: te.tool_name.clone(),
            tool_use_id: te.tool_use_id.clone(),
            is_error: te.is_error,
            input_summary: Some(summarize_input(&te.input)),
            input: None,
            output: None,
            output_omitted: true,
            output_chars,
            error_message: te.error_message.clone(),
        },
        ContentMode::Full => ToolExecView {
            tool_name: te.tool_name.clone(),
            tool_use_id: te.tool_use_id.clone(),
            is_error: te.is_error,
            input_summary: None,
            input: Some(te.input.clone()),
            output: if upstream_omitted {
                None
            } else {
                Some(tool_output_to_value(&te.output))
            },
            output_omitted: upstream_omitted,
            output_chars,
            error_message: te.error_message.clone(),
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Text extraction helpers
// ─────────────────────────────────────────────────────────────────────────────

pub fn message_content_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut parts = Vec::new();
            for block in blocks {
                match block {
                    cdt_core::message::ContentBlock::Text { text } => parts.push(text.as_str()),
                    cdt_core::message::ContentBlock::Thinking { thinking, .. } => {
                        parts.push(thinking.as_str());
                    }
                    _ => {}
                }
            }
            parts.join("\n")
        }
    }
}

pub fn tool_output_text(output: &ToolOutput) -> String {
    match output {
        ToolOutput::Text { text } => text.clone(),
        ToolOutput::Structured { value } => serde_json::to_string(value).unwrap_or_default(),
        ToolOutput::Missing => String::new(),
    }
}

pub fn tool_output_to_value(output: &ToolOutput) -> serde_json::Value {
    match output {
        ToolOutput::Text { text } => serde_json::Value::String(text.clone()),
        ToolOutput::Structured { value } => value.clone(),
        ToolOutput::Missing => serde_json::Value::Null,
    }
}

pub fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}...")
}

/// Display-width-aware truncation using Unicode width.
pub fn truncate_display(s: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    use unicode_width::UnicodeWidthStr;
    if max_width == 0 {
        return String::new();
    }
    if s.width() <= max_width {
        return s.to_string();
    }
    let mut width = 0;
    let mut result = String::new();
    for c in s.chars() {
        let w = c.width().unwrap_or(0);
        if width + w > max_width.saturating_sub(1) {
            result.push('…');
            return result;
        }
        width += w;
        result.push(c);
    }
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Field projection for --json
// ─────────────────────────────────────────────────────────────────────────────

pub fn project_fields(value: serde_json::Value, fields: &[&str]) -> serde_json::Value {
    match value {
        serde_json::Value::Array(arr) => {
            let projected: Vec<serde_json::Value> = arr
                .into_iter()
                .map(|item| project_object_fields(item, fields))
                .collect();
            serde_json::Value::Array(projected)
        }
        other => project_object_fields(other, fields),
    }
}

fn project_object_fields(value: serde_json::Value, fields: &[&str]) -> serde_json::Value {
    if let serde_json::Value::Object(map) = value {
        let filtered: serde_json::Map<String, serde_json::Value> = map
            .into_iter()
            .filter(|(k, _)| fields.contains(&k.as_str()))
            .collect();
        serde_json::Value::Object(filtered)
    } else {
        value
    }
}

pub use cdt_query::extract::summarize_input;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_display_ascii_within_limit() {
        assert_eq!(truncate_display("hello", 10), "hello");
    }

    #[test]
    fn truncate_display_ascii_exact_fit() {
        assert_eq!(truncate_display("hello", 5), "hello");
    }

    #[test]
    fn truncate_display_ascii_over_limit() {
        assert_eq!(truncate_display("hello world", 5), "hell…");
    }

    #[test]
    fn truncate_display_cjk_exact_fit() {
        assert_eq!(truncate_display("你好", 4), "你好");
    }

    #[test]
    fn truncate_display_cjk_over_limit() {
        assert_eq!(truncate_display("你好世界", 5), "你好…");
    }

    #[test]
    fn truncate_display_mixed_ascii_cjk() {
        assert_eq!(truncate_display("hi你好", 6), "hi你好");
        // "hi你好world" width=11 > 6, truncate: content budget=5, "hi你"=4, "好" would be 6 > 5
        assert_eq!(truncate_display("hi你好world", 6), "hi你…");
    }

    #[test]
    fn truncate_display_empty_string() {
        assert_eq!(truncate_display("", 10), "");
    }

    #[test]
    fn truncate_display_max_width_zero() {
        assert_eq!(truncate_display("hello", 0), "");
    }

    #[test]
    fn truncate_display_max_width_one() {
        assert_eq!(truncate_display("hello", 1), "…");
    }

    #[test]
    fn project_fields_array_projection() {
        let input = serde_json::json!([
            {"sessionId": "abc", "title": "test", "count": 5},
            {"sessionId": "def", "title": "other", "count": 3},
        ]);
        let result = project_fields(input, &["sessionId", "title"]);
        let expected = serde_json::json!([
            {"sessionId": "abc", "title": "test"},
            {"sessionId": "def", "title": "other"},
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn project_fields_single_object() {
        let input = serde_json::json!({"model": "claude", "cost": 1.5, "tokens": 100});
        let result = project_fields(input, &["model", "cost"]);
        let expected = serde_json::json!({"model": "claude", "cost": 1.5});
        assert_eq!(result, expected);
    }

    #[test]
    fn project_fields_unknown_fields_ignored() {
        let input = serde_json::json!({"sessionId": "abc", "title": "test"});
        let result = project_fields(input, &["sessionId", "nonExistent"]);
        let expected = serde_json::json!({"sessionId": "abc"});
        assert_eq!(result, expected);
    }

    #[test]
    fn project_fields_non_object_passthrough() {
        let input = serde_json::json!("just a string");
        let result = project_fields(input.clone(), &["field"]);
        assert_eq!(result, input);
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
}
