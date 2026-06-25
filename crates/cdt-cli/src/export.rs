//! Session export to Markdown / JSON.
//!
//! Spec: `openspec/specs/session-export/spec.md` (CLI 导出路径).

use cdt_api::SessionDetail;
use cdt_core::message::{ContentBlock, MessageContent};
use cdt_core::tool_execution::ToolOutput;
use cdt_core::{AIChunk, Chunk, SemanticStep, ToolExecution};
use cdt_query::cost::SessionCost;
use cdt_query::summary::SessionSummaryOutput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolDetailMode {
    Full,
    Summary,
    NameOnly,
}

pub struct ExportOptions {
    pub format: ExportFormat,
    pub detail: ToolDetailMode,
    pub include_thinking: bool,
    pub include_subagents: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Markdown,
            detail: ToolDetailMode::Full,
            include_thinking: true,
            include_subagents: true,
        }
    }
}

const TOOL_OUTPUT_TRUNCATE_LEN: usize = 2000;

// ─────────────────────────────────────────────────────────────────────────────
// Public entry points
// ─────────────────────────────────────────────────────────────────────────────

pub fn export_session(
    detail: &SessionDetail,
    summary: &SessionSummaryOutput,
    cost: &SessionCost,
    options: &ExportOptions,
) -> Result<String, serde_json::Error> {
    match options.format {
        ExportFormat::Markdown => Ok(export_as_markdown(detail, summary, cost, options)),
        ExportFormat::Json => export_as_json(detail, options),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Markdown
// ─────────────────────────────────────────────────────────────────────────────

fn export_as_markdown(
    detail: &SessionDetail,
    summary: &SessionSummaryOutput,
    cost: &SessionCost,
    options: &ExportOptions,
) -> String {
    let mut parts = Vec::new();

    let title = detail.title.as_deref().unwrap_or(&detail.session_id);
    parts.push(format!("# {title}\n"));

    parts.push(build_metadata_table(detail, summary, cost));
    parts.push("---\n".to_string());

    let mut turn = 0;
    for chunk in &detail.chunks {
        turn += 1;
        parts.push(render_chunk_md(chunk, turn, options));
    }

    parts.join("\n")
}

fn build_metadata_table(
    detail: &SessionDetail,
    summary: &SessionSummaryOutput,
    cost: &SessionCost,
) -> String {
    let mut rows: Vec<(&str, String)> = Vec::new();
    rows.push(("Session ID", detail.session_id.clone()));
    if let Some(ref cwd) = detail.metadata.cwd {
        rows.push(("Working Directory", cwd.clone()));
    }
    rows.push(("Messages", summary.message_count.to_string()));
    rows.push((
        "Status",
        if detail.is_ongoing {
            "Ongoing".to_string()
        } else {
            "Completed".to_string()
        },
    ));
    rows.push(("Duration", format_duration_ms(summary.total_duration_ms)));
    rows.push(("Model", cost.model.clone()));
    rows.push(("Total Cost", format!("${:.4}", cost.total_cost)));
    rows.push(("Total Tokens", cost.total_tokens.to_string()));

    let mut table = String::from("| Field | Value |\n|-------|-------|\n");
    for (k, v) in &rows {
        use std::fmt::Write;
        let _ = writeln!(table, "| {k} | {v} |");
    }
    table
}

fn render_chunk_md(chunk: &Chunk, index: usize, options: &ExportOptions) -> String {
    match chunk {
        Chunk::User(u) => {
            let content = extract_user_text(&u.content);
            format!("## Turn {index} — User\n\n{content}\n\n---\n")
        }
        Chunk::Ai(ai) => render_ai_chunk_md(ai, index, options),
        Chunk::System(s) => {
            format!("## Turn {index} — System\n\n*{}*\n\n---\n", s.content_text)
        }
        Chunk::Compact(_) => {
            format!("## Turn {index} — Context Compacted\n\n*[Context compacted]*\n\n---\n")
        }
    }
}

fn render_ai_chunk_md(ai: &AIChunk, index: usize, options: &ExportOptions) -> String {
    let mut parts = Vec::new();
    parts.push(format!("## Turn {index} — Assistant\n"));

    // Build tool_use_id → ToolExecution map for inline rendering
    let tool_map: std::collections::HashMap<&str, &ToolExecution> = ai
        .tool_executions
        .iter()
        .map(|te| (te.tool_use_id.as_str(), te))
        .collect();
    let mut rendered_tools: std::collections::HashSet<&str> = std::collections::HashSet::new();

    // Render semantic_steps in order, inlining tool executions at their position
    for step in &ai.semantic_steps {
        match step {
            SemanticStep::Thinking { text, .. } => {
                if options.include_thinking {
                    parts.push(format!("> [thinking] {text}\n"));
                }
            }
            SemanticStep::Text { text, .. } => {
                if !text.is_empty() {
                    parts.push(format!("{text}\n"));
                }
            }
            SemanticStep::ToolExecution { tool_use_id, .. } => {
                if let Some(te) = tool_map.get(tool_use_id.as_str()) {
                    parts.push(render_tool_md(te, options));
                    rendered_tools.insert(tool_use_id.as_str());
                }
            }
            SemanticStep::Interruption { text, .. } => {
                parts.push(format!("*[interrupted]* {text}\n"));
            }
            SemanticStep::UserMessage { text, .. } => {
                if !text.is_empty() {
                    parts.push(format!("*[user]* {text}\n"));
                }
            }
            SemanticStep::SubagentSpawn { .. } => {}
        }
    }

    // Render any tool executions not referenced in semantic_steps
    for te in &ai.tool_executions {
        if !rendered_tools.contains(te.tool_use_id.as_str()) {
            parts.push(render_tool_md(te, options));
        }
    }

    if options.include_subagents {
        for sub in &ai.subagents {
            parts.push(render_subagent_md(sub));
        }
    }

    parts.push("---\n".to_string());
    parts.join("\n")
}

fn render_subagent_md(sub: &cdt_core::Process) -> String {
    let desc = sub
        .description
        .as_deref()
        .or(sub.root_task_description.as_deref())
        .unwrap_or("subagent");
    let agent_type = sub
        .subagent_type
        .as_deref()
        .map_or_else(String::new, |t| format!(" ({t})"));
    let duration = sub
        .duration_ms
        .map_or_else(String::new, |ms| format!(" — {}s", ms / 1000));
    format!("### Subagent: {desc}{agent_type}{duration}\n\n")
}

fn render_tool_md(te: &ToolExecution, options: &ExportOptions) -> String {
    match options.detail {
        ToolDetailMode::NameOnly => {
            format!("### Tool: {}\n\n", te.tool_name)
        }
        ToolDetailMode::Summary | ToolDetailMode::Full => {
            use std::fmt::Write;
            let mut md = format!("### Tool: {}\n\n", te.tool_name);

            let input_str = serde_json::to_string_pretty(&te.input).unwrap_or_default();
            if !input_str.is_empty() && input_str != "{}" && input_str != "null" {
                let _ = write!(md, "**Input:**\n```json\n{input_str}\n```\n\n");
            }

            let output_str = tool_output_text(&te.output);
            if !output_str.is_empty() {
                let display = if options.detail == ToolDetailMode::Summary {
                    truncate_chars(&output_str, TOOL_OUTPUT_TRUNCATE_LEN)
                } else {
                    output_str
                };
                let _ = write!(md, "**Output:**\n```\n{display}\n```\n\n");
            }

            if te.is_error {
                if let Some(ref err) = te.error_message {
                    let _ = write!(md, "**Error:** {err}\n\n");
                }
            }

            md
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JSON
// ─────────────────────────────────────────────────────────────────────────────

fn export_as_json(
    detail: &SessionDetail,
    options: &ExportOptions,
) -> Result<String, serde_json::Error> {
    let projected = project_detail(detail, options)?;
    serde_json::to_string_pretty(&projected)
}

fn project_detail(
    detail: &SessionDetail,
    options: &ExportOptions,
) -> Result<serde_json::Value, serde_json::Error> {
    let mut val = serde_json::to_value(detail)?;
    if let Some(obj) = val.as_object_mut() {
        if let Some(chunks) = obj.get_mut("chunks") {
            if let Some(arr) = chunks.as_array_mut() {
                for chunk in arr {
                    project_chunk_json(chunk, options);
                }
            }
        }
    }
    Ok(val)
}

fn project_chunk_json(chunk: &mut serde_json::Value, options: &ExportOptions) {
    let Some(obj) = chunk.as_object_mut() else {
        return;
    };

    // Only AI chunks need projection
    let is_ai = obj
        .get("kind")
        .and_then(|k| k.as_str())
        .is_some_and(|k| k == "ai");
    if !is_ai {
        return;
    }

    if !options.include_thinking {
        if let Some(steps) = obj.get_mut("semanticSteps") {
            if let Some(arr) = steps.as_array_mut() {
                arr.retain(|s| {
                    s.get("kind")
                        .and_then(|k| k.as_str())
                        .is_none_or(|k| k != "thinking")
                });
            }
        }
        // Also filter thinking blocks from response content
        if let Some(responses) = obj.get_mut("responses") {
            if let Some(resp_arr) = responses.as_array_mut() {
                for resp in resp_arr {
                    if let Some(content) = resp.get_mut("content") {
                        if let Some(blocks) = content.as_array_mut() {
                            blocks.retain(|b| {
                                b.get("type")
                                    .and_then(|t| t.as_str())
                                    .is_none_or(|t| t != "thinking")
                            });
                        }
                    }
                }
            }
        }
    }

    if !options.include_subagents {
        obj.insert(
            "subagents".to_string(),
            serde_json::Value::Array(Vec::new()),
        );
    }

    if options.detail == ToolDetailMode::NameOnly {
        if let Some(tools) = obj.get_mut("toolExecutions") {
            if let Some(arr) = tools.as_array_mut() {
                for tool in arr {
                    if let Some(t) = tool.as_object_mut() {
                        t.insert(
                            "input".to_string(),
                            serde_json::Value::Object(serde_json::Map::new()),
                        );
                        t.insert("output".to_string(), serde_json::json!({"kind": "missing"}));
                    }
                }
            }
        }
    } else if options.detail == ToolDetailMode::Summary {
        if let Some(tools) = obj.get_mut("toolExecutions") {
            if let Some(arr) = tools.as_array_mut() {
                for tool in arr {
                    truncate_tool_output_json(tool);
                }
            }
        }
    }
}

fn truncate_tool_output_json(tool: &mut serde_json::Value) {
    let Some(obj) = tool.as_object_mut() else {
        return;
    };
    let Some(output) = obj.get_mut("output") else {
        return;
    };
    let Some(out_obj) = output.as_object_mut() else {
        return;
    };

    // Truncate text output
    if let Some(text) = out_obj.get_mut("text") {
        if let Some(s) = text.as_str() {
            if s.chars().count() > TOOL_OUTPUT_TRUNCATE_LEN {
                *text = serde_json::Value::String(truncate_chars(s, TOOL_OUTPUT_TRUNCATE_LEN));
            }
        }
    }

    // Truncate structured output by serializing to string
    if let Some(value) = out_obj.get_mut("value") {
        let serialized = serde_json::to_string(value).unwrap_or_default();
        if serialized.chars().count() > TOOL_OUTPUT_TRUNCATE_LEN {
            *out_obj = serde_json::Map::from_iter([
                (
                    "kind".to_string(),
                    serde_json::Value::String("text".to_string()),
                ),
                (
                    "text".to_string(),
                    serde_json::Value::String(truncate_chars(
                        &serialized,
                        TOOL_OUTPUT_TRUNCATE_LEN,
                    )),
                ),
            ]);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn extract_user_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

fn tool_output_text(output: &ToolOutput) -> String {
    match output {
        ToolOutput::Text { text } => text.clone(),
        ToolOutput::Structured { value } => serde_json::to_string_pretty(value).unwrap_or_default(),
        ToolOutput::Missing => String::new(),
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{truncated}... (truncated)")
    }
}

fn format_duration_ms(ms: i64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::chunk::{CompactChunk, SystemChunk, UserChunk};
    use cdt_core::message::MessageContent;
    use chrono::Utc;

    fn default_metrics() -> cdt_core::chunk::ChunkMetrics {
        cdt_core::chunk::ChunkMetrics {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            tool_count: 0,
            cost_usd: None,
        }
    }

    fn make_user_chunk(text: &str) -> Chunk {
        Chunk::User(UserChunk {
            chunk_id: "u1".into(),
            uuid: "uuid-u1".into(),
            timestamp: Utc::now(),
            duration_ms: None,
            content: MessageContent::Text(text.into()),
            metrics: default_metrics(),
        })
    }

    fn make_system_chunk(text: &str) -> Chunk {
        Chunk::System(SystemChunk {
            chunk_id: "s1".into(),
            uuid: "uuid-s1".into(),
            timestamp: Utc::now(),
            duration_ms: None,
            content_text: text.into(),
            metrics: default_metrics(),
        })
    }

    fn make_compact_chunk() -> Chunk {
        Chunk::Compact(CompactChunk {
            chunk_id: "c1".into(),
            uuid: "uuid-c1".into(),
            timestamp: Utc::now(),
            duration_ms: None,
            summary_text: "compacted".into(),
            metrics: default_metrics(),
            phase_number: None,
            token_delta: None,
        })
    }

    fn make_ai_chunk_with_tool(tool_name: &str, output: &str) -> Chunk {
        let ts = Utc::now();
        Chunk::Ai(AIChunk {
            chunk_id: "a1".into(),
            timestamp: ts,
            duration_ms: None,
            responses: vec![],
            metrics: default_metrics(),
            semantic_steps: vec![
                SemanticStep::Text {
                    text: "assistant reply".into(),
                    timestamp: ts,
                },
                SemanticStep::ToolExecution {
                    tool_use_id: "tu1".into(),
                    tool_name: tool_name.into(),
                    timestamp: ts,
                },
            ],
            tool_executions: vec![ToolExecution {
                tool_use_id: "tu1".into(),
                tool_name: tool_name.into(),
                input: serde_json::json!({"command": "ls"}),
                output: ToolOutput::Text {
                    text: output.into(),
                },
                is_error: false,
                start_ts: ts,
                end_ts: None,
                source_assistant_uuid: "resp-1".into(),
                result_agent_id: None,
                error_message: None,
                output_omitted: false,
                output_bytes: None,
                teammate_spawn: None,
                workflow_run_id: None,
                workflow_script_path: None,
            }],
            subagents: vec![],
            slash_commands: vec![],
            teammate_messages: vec![],
        })
    }

    fn make_detail(chunks: Vec<Chunk>) -> SessionDetail {
        SessionDetail {
            session_id: "test-session-123".into(),
            project_id: "proj-1".into(),
            chunks,
            metrics: cdt_api::SessionDetailMetrics { message_count: 2 },
            metadata: cdt_api::SessionDetailMetadata {
                last_modified: None,
                size: None,
                cwd: Some("/home/user/project".into()),
            },
            context_injections: vec![],
            injections_by_phase: std::collections::BTreeMap::new(),
            phase_info: cdt_core::ContextPhaseInfo::default(),
            turn_context_stats: std::collections::HashMap::new(),
            is_ongoing: false,
            title: Some("Test Session".into()),
            workflow_items: vec![],
        }
    }

    fn make_summary() -> SessionSummaryOutput {
        SessionSummaryOutput {
            session_id: "test-session-123".into(),
            total_duration_ms: 120_000,
            message_count: 2,
            phases: vec![],
            tool_usage: vec![],
            top_files: vec![],
            error_count: 0,
            compaction_count: 0,
            idle_gaps: vec![],
            cost: SessionCost {
                input_tokens: 1000,
                output_tokens: 500,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                total_tokens: 1500,
                input_cost: 0.003,
                output_cost: 0.0075,
                cache_read_cost: 0.0,
                cache_creation_cost: 0.0,
                total_cost: 0.0105,
                model: "claude-sonnet-4-6".into(),
                model_pricing_used: "claude-sonnet-4-6".into(),
            },
            tool_activity: cdt_query::summary::ToolActivity {
                top_commands: vec![],
                top_files: vec![],
                git_ops: vec![],
                cli_tools: vec![],
                total_tool_executions: 0,
                omitted_count: 0,
            },
        }
    }

    fn make_cost() -> SessionCost {
        SessionCost {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            total_tokens: 1500,
            input_cost: 0.003,
            output_cost: 0.0075,
            cache_read_cost: 0.0,
            cache_creation_cost: 0.0,
            total_cost: 0.0105,
            model: "claude-sonnet-4-6".into(),
            model_pricing_used: "claude-sonnet-4-6".into(),
        }
    }

    #[test]
    fn markdown_contains_metadata_table_and_turn_structure() {
        let detail = make_detail(vec![
            make_user_chunk("Hello, what is Rust?"),
            make_ai_chunk_with_tool("Bash", "output here"),
        ]);
        let summary = make_summary();
        let cost = make_cost();
        let options = ExportOptions::default();
        let md = export_session(&detail, &summary, &cost, &options).unwrap();

        assert!(md.contains("# Test Session"));
        assert!(md.contains("| Session ID | test-session-123 |"));
        assert!(md.contains("| Model | claude-sonnet-4-6 |"));
        assert!(md.contains("## Turn 1 — User"));
        assert!(md.contains("Hello, what is Rust?"));
        assert!(md.contains("## Turn 2 — Assistant"));
        assert!(md.contains("### Tool: Bash"));
        assert!(md.contains("output here"));
    }

    #[test]
    fn json_no_thinking_filters_thinking_steps() {
        let detail = make_detail(vec![make_ai_chunk_with_tool("Read", "content")]);
        let options = ExportOptions {
            format: ExportFormat::Json,
            include_thinking: false,
            ..Default::default()
        };
        let json = export_session(&detail, &make_summary(), &make_cost(), &options).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let chunks = parsed["chunks"].as_array().unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn detail_name_only_omits_tool_input_output() {
        let detail = make_detail(vec![make_ai_chunk_with_tool("Bash", "long output")]);
        let options = ExportOptions {
            format: ExportFormat::Markdown,
            detail: ToolDetailMode::NameOnly,
            ..Default::default()
        };
        let md = export_session(&detail, &make_summary(), &make_cost(), &options).unwrap();

        assert!(md.contains("### Tool: Bash"));
        assert!(!md.contains("long output"));
        assert!(!md.contains("**Input:**"));
    }

    #[test]
    fn detail_summary_truncates_long_tool_output() {
        let long_output = "x".repeat(3000);
        let detail = make_detail(vec![make_ai_chunk_with_tool("Bash", &long_output)]);
        let options = ExportOptions {
            format: ExportFormat::Markdown,
            detail: ToolDetailMode::Summary,
            ..Default::default()
        };
        let md = export_session(&detail, &make_summary(), &make_cost(), &options).unwrap();

        assert!(md.contains("... (truncated)"));
        assert!(!md.contains(&long_output));
    }

    #[test]
    fn system_and_compact_chunks_render() {
        let detail = make_detail(vec![make_system_chunk("system msg"), make_compact_chunk()]);
        let options = ExportOptions::default();
        let md = export_session(&detail, &make_summary(), &make_cost(), &options).unwrap();

        assert!(md.contains("## Turn 1 — System"));
        assert!(md.contains("*system msg*"));
        assert!(md.contains("## Turn 2 — Context Compacted"));
    }

    fn make_ai_chunk_with_thinking() -> Chunk {
        let ts = Utc::now();
        Chunk::Ai(AIChunk {
            chunk_id: "a2".into(),
            timestamp: ts,
            duration_ms: None,
            responses: vec![],
            metrics: default_metrics(),
            semantic_steps: vec![
                SemanticStep::Thinking {
                    text: "internal reasoning here".into(),
                    timestamp: ts,
                },
                SemanticStep::Text {
                    text: "visible reply".into(),
                    timestamp: ts,
                },
            ],
            tool_executions: vec![],
            subagents: vec![cdt_core::Process {
                session_id: "sub-1".into(),
                root_task_description: Some("run tests".into()),
                spawn_ts: ts,
                end_ts: None,
                metrics: default_metrics(),
                team: None,
                subagent_type: Some("qa".into()),
                messages: vec![],
                main_session_impact: None,
                is_ongoing: false,
                duration_ms: Some(5000),
                parent_task_id: None,
                description: Some("QA agent".into()),
                header_model: None,
                last_isolated_tokens: 0,
                is_shutdown_only: false,
                messages_omitted: false,
                messages_total_count: 0,
            }],
            slash_commands: vec![],
            teammate_messages: vec![],
        })
    }

    #[test]
    fn no_thinking_excludes_thinking_in_markdown() {
        let detail = make_detail(vec![make_ai_chunk_with_thinking()]);
        let options = ExportOptions {
            include_thinking: false,
            ..Default::default()
        };
        let md = export_session(&detail, &make_summary(), &make_cost(), &options).unwrap();

        assert!(!md.contains("[thinking]"));
        assert!(!md.contains("internal reasoning here"));
        assert!(md.contains("visible reply"));
    }

    #[test]
    fn default_includes_thinking_in_markdown() {
        let detail = make_detail(vec![make_ai_chunk_with_thinking()]);
        let options = ExportOptions::default();
        let md = export_session(&detail, &make_summary(), &make_cost(), &options).unwrap();

        assert!(md.contains("> [thinking] internal reasoning here"));
        assert!(md.contains("visible reply"));
    }

    #[test]
    fn no_subagents_excludes_subagent_card() {
        let detail = make_detail(vec![make_ai_chunk_with_thinking()]);
        let with = ExportOptions::default();
        let md_with = export_session(&detail, &make_summary(), &make_cost(), &with).unwrap();
        assert!(md_with.contains("### Subagent: QA agent (qa)"));

        let without = ExportOptions {
            include_subagents: false,
            ..Default::default()
        };
        let md_without = export_session(&detail, &make_summary(), &make_cost(), &without).unwrap();
        assert!(!md_without.contains("### Subagent:"));
    }

    #[test]
    fn json_no_thinking_removes_thinking_from_steps_and_content() {
        let detail = make_detail(vec![make_ai_chunk_with_thinking()]);
        let options = ExportOptions {
            format: ExportFormat::Json,
            include_thinking: false,
            ..Default::default()
        };
        let json = export_session(&detail, &make_summary(), &make_cost(), &options).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let chunks = parsed["chunks"].as_array().unwrap();
        for chunk in chunks {
            if let Some(steps) = chunk.get("semanticSteps") {
                if let Some(arr) = steps.as_array() {
                    for step in arr {
                        let kind = step["kind"].as_str().unwrap_or("");
                        assert_ne!(kind, "thinking", "thinking step should be filtered");
                    }
                }
            }
        }
    }

    #[test]
    fn json_name_only_clears_tool_input_and_output() {
        let detail = make_detail(vec![make_ai_chunk_with_tool("Bash", "real output")]);
        let options = ExportOptions {
            format: ExportFormat::Json,
            detail: ToolDetailMode::NameOnly,
            ..Default::default()
        };
        let json = export_session(&detail, &make_summary(), &make_cost(), &options).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let chunks = parsed["chunks"].as_array().unwrap();
        for chunk in chunks {
            if let Some(tools) = chunk.get("toolExecutions") {
                if let Some(arr) = tools.as_array() {
                    for tool in arr {
                        let input = &tool["input"];
                        assert!(
                            input.as_object().is_none_or(serde_json::Map::is_empty),
                            "input should be empty object"
                        );
                        assert_eq!(
                            tool["output"]["kind"].as_str().unwrap_or(""),
                            "missing",
                            "output should be missing"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn markdown_full_detail_includes_tool_input() {
        let detail = make_detail(vec![make_ai_chunk_with_tool("Bash", "output")]);
        let options = ExportOptions::default();
        let md = export_session(&detail, &make_summary(), &make_cost(), &options).unwrap();

        assert!(md.contains("**Input:**"));
        assert!(md.contains("\"command\": \"ls\""));
        assert!(md.contains("**Output:**"));
        assert!(md.contains("output"));
    }

    #[test]
    fn truncate_chars_boundary() {
        assert_eq!(truncate_chars("hello", 10), "hello");
        assert_eq!(truncate_chars("hello world", 5), "hello... (truncated)");
        assert_eq!(truncate_chars("你好世界", 2), "你好... (truncated)");
    }

    #[test]
    fn json_summary_truncates_by_char_count_not_bytes() {
        let cjk_output = "中".repeat(2001);
        let detail = make_detail(vec![make_ai_chunk_with_tool("Bash", &cjk_output)]);
        let options = ExportOptions {
            format: ExportFormat::Json,
            detail: ToolDetailMode::Summary,
            ..Default::default()
        };
        let json = export_session(&detail, &make_summary(), &make_cost(), &options).unwrap();
        assert!(json.contains("truncated"));
    }
}
