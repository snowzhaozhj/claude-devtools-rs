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

pub fn build_chunk_view(
    abs_index: usize,
    chunk: &Chunk,
    mode: &ContentMode,
    grep_hit: Option<bool>,
) -> ChunkView {
    match chunk {
        Chunk::Ai(ai) => {
            let tool_execs: Vec<ToolExecView> = ai
                .tool_executions
                .iter()
                .map(|te| build_tool_exec_view(te, mode))
                .collect();

            let responses: Vec<ResponseView> = ai
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
                chunk_index: abs_index,
                chunk_id: ai.chunk_id.clone(),
                kind: "ai".to_string(),
                timestamp: ai.timestamp.to_rfc3339(),
                duration_ms: ai.duration_ms,
                tool_executions: tool_execs,
                responses,
                user_content: None,
                system_content: None,
                compact_summary: None,
                grep_hit,
            }
        }
        Chunk::User(user) => {
            let text = message_content_text(&user.content);
            let chars = text.chars().count();
            let user_content = match mode {
                ContentMode::Omit => ContentField {
                    text: if chars <= 200 { Some(text) } else { None },
                    omitted: chars > 200,
                    chars,
                },
                ContentMode::Full => ContentField {
                    text: Some(text),
                    omitted: false,
                    chars,
                },
            };
            ChunkView {
                chunk_index: abs_index,
                chunk_id: user.chunk_id.clone(),
                kind: "user".to_string(),
                timestamp: user.timestamp.to_rfc3339(),
                duration_ms: user.duration_ms,
                tool_executions: vec![],
                responses: vec![],
                user_content: Some(user_content),
                system_content: None,
                compact_summary: None,
                grep_hit,
            }
        }
        Chunk::System(sys) => {
            let chars = sys.content_text.chars().count();
            let system_content = match mode {
                ContentMode::Omit => ContentField {
                    text: if chars <= 200 {
                        Some(sys.content_text.clone())
                    } else {
                        None
                    },
                    omitted: chars > 200,
                    chars,
                },
                ContentMode::Full => ContentField {
                    text: Some(sys.content_text.clone()),
                    omitted: false,
                    chars,
                },
            };
            ChunkView {
                chunk_index: abs_index,
                chunk_id: sys.chunk_id.clone(),
                kind: "system".to_string(),
                timestamp: sys.timestamp.to_rfc3339(),
                duration_ms: sys.duration_ms,
                tool_executions: vec![],
                responses: vec![],
                user_content: None,
                system_content: Some(system_content),
                compact_summary: None,
                grep_hit,
            }
        }
        Chunk::Compact(compact) => ChunkView {
            chunk_index: abs_index,
            chunk_id: compact.chunk_id.clone(),
            kind: "compact".to_string(),
            timestamp: compact.timestamp.to_rfc3339(),
            duration_ms: compact.duration_ms,
            tool_executions: vec![],
            responses: vec![],
            user_content: None,
            system_content: None,
            compact_summary: Some(compact.summary_text.clone()),
            grep_hit,
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
    if max_width == 0 {
        return String::new();
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

pub fn summarize_input(input: &serde_json::Value) -> String {
    match input {
        serde_json::Value::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .take(3)
                .map(|(k, v)| {
                    let val_str = match v {
                        serde_json::Value::String(s) => truncate_str(s, 57),
                        other => truncate_str(&other.to_string(), 57),
                    };
                    format!("{k}: {val_str}")
                })
                .collect();
            if map.len() > 3 {
                format!("{} (+{} more)", parts.join(", "), map.len() - 3)
            } else {
                parts.join(", ")
            }
        }
        other => truncate_str(&other.to_string(), 117),
    }
}
