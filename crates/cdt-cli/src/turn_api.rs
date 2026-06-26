//! Serializable view types for the turn-model CLI/MCP API.

use serde::Serialize;

use cdt_core::ToolOutput;
use cdt_query::step::Step;
use cdt_query::turn_view::{TurnMetrics, TurnOverview, measure_output};

// ─────────────────────────────────────────────────────────────────────────────
// get_session response
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOverviewResponse {
    pub session_id: String,
    pub model: Option<String>,
    pub total_cost: f64,
    pub duration_ms: i64,
    pub files_touched: Vec<String>,
    pub user_intents: Vec<String>,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub turns: Vec<TurnCompactView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCompactView {
    pub index: u32,
    pub question: Option<String>,
    pub answer: Option<String>,
    pub tools: Vec<ToolAggView>,
    pub steps_count: usize,
    pub metrics: MetricsView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_in: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAggView {
    pub name: String,
    pub count: usize,
    pub error_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsView {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost: f64,
    pub duration_ms: i64,
    pub model: Option<String>,
}

impl From<&TurnMetrics> for MetricsView {
    fn from(m: &TurnMetrics) -> Self {
        Self {
            input_tokens: m.input_tokens,
            output_tokens: m.output_tokens,
            cache_read_tokens: m.cache_read_tokens,
            cache_creation_tokens: m.cache_creation_tokens,
            cost: m.cost,
            duration_ms: m.duration_ms,
            model: m.model.clone(),
        }
    }
}

impl TurnCompactView {
    #[must_use]
    pub fn from_overview(o: &TurnOverview, matched_in: Option<String>) -> Self {
        Self {
            index: o.index,
            question: o.question.clone(),
            answer: o.answer.clone(),
            tools: o
                .tools
                .iter()
                .map(|t| ToolAggView {
                    name: t.name.clone(),
                    count: t.count,
                    error_count: t.error_count,
                })
                .collect(),
            steps_count: o.steps_count,
            metrics: MetricsView::from(&o.metrics),
            matched_in,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// get_turn response
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnDetailResponse {
    pub session_id: String,
    pub turn_index: u32,
    pub question: Option<String>,
    pub answer: Option<String>,
    pub steps_total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub metrics: MetricsView,
    pub steps: Vec<StepView>,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum StepView {
    Thinking {
        index: usize,
        text: String,
    },
    Text {
        index: usize,
        text: String,
    },
    Tool {
        index: usize,
        tool_use_id: String,
        name: String,
        input: serde_json::Value,
        output: ToolOutputView,
        is_error: bool,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        output_truncated: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        output_bytes: Option<u64>,
    },
    Subagent {
        index: usize,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        subagent_session_id: Option<String>,
        steps_count: usize,
    },
    TeammateSpawn {
        index: usize,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        tool_use_id: String,
    },
    Workflow {
        index: usize,
        tool_use_id: String,
        name: String,
        run_id: String,
    },
    Interruption {
        index: usize,
        text: String,
    },
    UserMessage {
        index: usize,
        text: String,
    },
    Slash {
        index: usize,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<String>,
    },
    TeammateMessage {
        index: usize,
        teammate_id: String,
        body: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        color: Option<String>,
    },
    Compaction {
        index: usize,
        summary: String,
    },
    System {
        index: usize,
        content: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ToolOutputView {
    Text { text: String },
    Structured { value: serde_json::Value },
    Missing,
}

impl From<&ToolOutput> for ToolOutputView {
    fn from(o: &ToolOutput) -> Self {
        match o {
            ToolOutput::Text { text } => Self::Text { text: text.clone() },
            ToolOutput::Structured { value } => Self::Structured {
                value: value.clone(),
            },
            ToolOutput::Missing => Self::Missing,
        }
    }
}

impl StepView {
    #[must_use]
    pub fn from_step(step: &Step, index: usize) -> Self {
        match step {
            Step::Thinking { text, .. } => Self::Thinking {
                index,
                text: text.clone(),
            },
            Step::Text { text, .. } => Self::Text {
                index,
                text: text.clone(),
            },
            Step::Tool {
                tool_use_id,
                name,
                input,
                output,
                is_error,
                ..
            } => {
                let (output_truncated, output_bytes) = measure_output(output);
                let output_view = if output_truncated {
                    truncated_output_view(output)
                } else {
                    ToolOutputView::from(output)
                };
                Self::Tool {
                    index,
                    tool_use_id: tool_use_id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                    output: output_view,
                    is_error: *is_error,
                    output_truncated,
                    output_bytes,
                }
            }
            Step::Subagent {
                name,
                description,
                subagent_session_id,
                steps_count,
                ..
            } => Self::Subagent {
                index,
                name: name.clone(),
                description: description.clone(),
                subagent_session_id: subagent_session_id.clone(),
                steps_count: *steps_count,
            },
            Step::TeammateSpawn {
                name,
                color,
                tool_use_id,
                ..
            } => Self::TeammateSpawn {
                index,
                name: name.clone(),
                color: color.clone(),
                tool_use_id: tool_use_id.clone(),
            },
            Step::Workflow {
                tool_use_id,
                name,
                run_id,
                ..
            } => Self::Workflow {
                index,
                tool_use_id: tool_use_id.clone(),
                name: name.clone(),
                run_id: run_id.clone(),
            },
            Step::Interruption { text, .. } => Self::Interruption {
                index,
                text: text.clone(),
            },
            Step::UserMessage { text, .. } => Self::UserMessage {
                index,
                text: text.clone(),
            },
            Step::Slash {
                name,
                message,
                args,
                ..
            } => Self::Slash {
                index,
                name: name.clone(),
                message: message.clone(),
                args: args.clone(),
            },
            Step::TeammateMsg {
                teammate_id,
                body,
                color,
                ..
            } => Self::TeammateMessage {
                index,
                teammate_id: teammate_id.clone(),
                body: body.clone(),
                color: color.clone(),
            },
            Step::Compaction { summary, .. } => Self::Compaction {
                index,
                summary: summary.clone(),
            },
            Step::System { content, .. } => Self::System {
                index,
                content: content.clone(),
            },
        }
    }
}

fn truncated_output_view(output: &ToolOutput) -> ToolOutputView {
    const PREFIX_CHARS: usize = 2000;
    match output {
        ToolOutput::Text { text } => ToolOutputView::Text {
            text: text.chars().take(PREFIX_CHARS).collect(),
        },
        ToolOutput::Structured { value } => {
            let s = serde_json::to_string(value).unwrap_or_default();
            ToolOutputView::Text {
                text: s.chars().take(PREFIX_CHARS).collect(),
            }
        }
        ToolOutput::Missing => ToolOutputView::Missing,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// get_tool_output response
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolOutputFullResponse {
    pub session_id: String,
    pub tool_use_id: String,
    pub tool_name: String,
    pub output_bytes: u64,
    pub output: ToolOutputView,
}

// ─────────────────────────────────────────────────────────────────────────────
// search response
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSearchResponse {
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub results: Vec<TurnSearchHit>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSearchHit {
    pub session_id: String,
    pub turn_index: u32,
    pub question: Option<String>,
    pub match_snippet: String,
    pub timestamp: i64,
    pub project_name: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// pagination helpers
// ─────────────────────────────────────────────────────────────────────────────

pub fn paginate_cursor(cursor: Option<&str>) -> usize {
    cursor.and_then(|c| c.parse::<usize>().ok()).unwrap_or(0)
}

pub fn next_cursor(offset: usize, page_size: usize, total: usize) -> Option<String> {
    let next = offset + page_size;
    if next < total {
        Some(next.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_view_tool_serializes_tagged() {
        let view = StepView::Tool {
            index: 0,
            tool_use_id: "tu1".into(),
            name: "Read".into(),
            input: serde_json::json!({"path": "/foo"}),
            output: ToolOutputView::Text {
                text: "content".into(),
            },
            is_error: false,
            output_truncated: false,
            output_bytes: None,
        };
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["type"], "tool");
        assert_eq!(json["toolUseId"], "tu1");
        assert_eq!(json["name"], "Read");
    }

    #[test]
    fn step_view_thinking_serializes_tagged() {
        let view = StepView::Thinking {
            index: 0,
            text: "hmm".into(),
        };
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["type"], "thinking");
        assert_eq!(json["text"], "hmm");
    }

    #[test]
    fn tool_output_view_structured() {
        let view = ToolOutputView::Structured {
            value: serde_json::json!({"stdout": "ok"}),
        };
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["kind"], "structured");
        assert_eq!(json["value"]["stdout"], "ok");
    }

    #[test]
    fn pagination_cursor_roundtrip() {
        assert_eq!(paginate_cursor(None), 0);
        assert_eq!(paginate_cursor(Some("20")), 20);
        assert_eq!(paginate_cursor(Some("invalid")), 0);
    }

    #[test]
    fn next_cursor_has_more() {
        assert_eq!(next_cursor(0, 20, 50), Some("20".into()));
        assert_eq!(next_cursor(40, 20, 50), None);
        assert_eq!(next_cursor(0, 20, 20), None);
    }

    #[test]
    fn truncated_output_large_text() {
        let large = "x".repeat(10_000);
        let output = ToolOutput::Text { text: large };
        let (truncated, bytes) = measure_output(&output);
        assert!(truncated);
        assert_eq!(bytes, Some(10_000));

        let view = truncated_output_view(&output);
        if let ToolOutputView::Text { text } = &view {
            assert_eq!(text.len(), 2000);
        } else {
            panic!("expected Text");
        }
    }

    #[test]
    fn small_output_not_truncated() {
        let output = ToolOutput::Text {
            text: "small".into(),
        };
        let (truncated, bytes) = measure_output(&output);
        assert!(!truncated);
        assert!(bytes.is_none());
    }
}
