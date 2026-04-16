//! 6 类 context injection 的纯函数聚合器。
//!
//! 每个 `aggregate_*` / `create_*` 函数都是"输入一段 chunk 子结构 + 一点
//! 元数据，输出 `Option<ContextInjection>`"，失败返回 `None`（表示本轮没
//! 有对应 injection），不报错、不 panic。
//!
//! 设计决策见 `openspec/changes/port-context-tracking/design.md` §决策 4。

use cdt_core::{
    AIChunk, ContextInjection, MessageContent, SemanticStep, TaskCoordinationBreakdown,
    TaskCoordinationInjection, TaskCoordinationKind, ThinkingTextBreakdown, ThinkingTextInjection,
    ThinkingTextKind, ToolExecution, ToolOutput, ToolOutputInjection, ToolTokenBreakdown,
    UserChunk, UserMessageInjection, estimate_content_tokens, estimate_tokens,
};

/// 作为 `task-coordination` 桶的 7 个工具名。
///
/// TS 侧常量的 Rust 对应，语义完全一致。
const TASK_COORDINATION_TOOL_NAMES: &[&str] = &[
    "SendMessage",
    "TeamCreate",
    "TeamDelete",
    "TaskCreate",
    "TaskUpdate",
    "TaskList",
    "TaskGet",
];

#[must_use]
pub(super) fn is_task_coordination_tool(name: &str) -> bool {
    TASK_COORDINATION_TOOL_NAMES.contains(&name)
}

/// 把单个 `ToolExecution` 的 call + result token 估算出来。
///
/// `call_tokens`：对 `tool.input` 做 `estimate_content_tokens`。
/// `result_tokens`：对 `tool.output`：`Text` 直接估字符串，`Structured` 估其
/// JSON 序列化结果，`Missing` 归 0。
fn estimate_tool_tokens(tool: &ToolExecution) -> u64 {
    let call = estimate_content_tokens(&tool.input) as u64;
    let result = match &tool.output {
        ToolOutput::Text { text } => estimate_tokens(text) as u64,
        ToolOutput::Structured { value } => estimate_content_tokens(value) as u64,
        ToolOutput::Missing => 0,
    };
    call + result
}

fn tool_output_id(turn_index: u32) -> String {
    format!("tool-output-ai-{turn_index}")
}

fn thinking_text_id(turn_index: u32) -> String {
    format!("thinking-text-ai-{turn_index}")
}

fn task_coord_id(turn_index: u32) -> String {
    format!("task-coord-ai-{turn_index}")
}

fn user_message_id(turn_index: u32) -> String {
    format!("user-msg-ai-{turn_index}")
}

/// 聚合 `tool-output` 桶 —— 不含 7 个 task coordination 工具。
#[must_use]
pub(super) fn aggregate_tool_outputs(
    ai_chunk: &AIChunk,
    turn_index: u32,
    ai_group_id: &str,
) -> Option<ContextInjection> {
    let mut breakdown = Vec::new();
    let mut total: u64 = 0;

    for tool in &ai_chunk.tool_executions {
        if is_task_coordination_tool(&tool.tool_name) {
            continue;
        }
        let tokens = estimate_tool_tokens(tool);
        if tokens == 0 {
            continue;
        }
        let display_name = if tool.tool_name == "Task" {
            "Task (Subagent)".to_string()
        } else {
            tool.tool_name.clone()
        };
        breakdown.push(ToolTokenBreakdown {
            tool_name: display_name,
            token_count: tokens,
            is_error: tool.is_error,
            tool_use_id: Some(tool.tool_use_id.clone()),
        });
        total += tokens;
    }

    if total == 0 {
        return None;
    }

    Some(ContextInjection::ToolOutput(ToolOutputInjection {
        id: tool_output_id(turn_index),
        turn_index,
        ai_group_id: ai_group_id.to_string(),
        estimated_tokens: total,
        tool_count: breakdown.len(),
        tool_breakdown: breakdown,
    }))
}

/// 聚合 `task-coordination` 桶 —— 仅含 7 个 task coordination 工具。
///
/// `teammate_message` display item 本 port 不实现（需要
/// `team-coordination-metadata` 的身份识别），TS 里的那一路先留空。
#[must_use]
pub(super) fn aggregate_task_coordination(
    ai_chunk: &AIChunk,
    turn_index: u32,
    ai_group_id: &str,
) -> Option<ContextInjection> {
    let mut breakdown = Vec::new();
    let mut total: u64 = 0;

    for tool in &ai_chunk.tool_executions {
        if !is_task_coordination_tool(&tool.tool_name) {
            continue;
        }
        let tokens = estimate_tool_tokens(tool);
        if tokens == 0 {
            continue;
        }

        let (kind, label) = if tool.tool_name == "SendMessage" {
            let recipient = tool
                .input
                .get("recipient")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let label = if recipient.is_empty() {
                "SendMessage".to_string()
            } else {
                format!("SendMessage → {recipient}")
            };
            (TaskCoordinationKind::SendMessage, label)
        } else {
            (TaskCoordinationKind::TaskTool, tool.tool_name.clone())
        };

        breakdown.push(TaskCoordinationBreakdown {
            kind,
            token_count: tokens,
            label,
            tool_name: Some(tool.tool_name.clone()),
        });
        total += tokens;
    }

    if total == 0 {
        return None;
    }

    Some(ContextInjection::TaskCoordination(
        TaskCoordinationInjection {
            id: task_coord_id(turn_index),
            turn_index,
            ai_group_id: ai_group_id.to_string(),
            estimated_tokens: total,
            breakdown,
        },
    ))
}

/// 聚合 `thinking-text` 桶 —— `SemanticStep::Thinking` + `SemanticStep::Text`。
#[must_use]
pub(super) fn aggregate_thinking_text(
    ai_chunk: &AIChunk,
    turn_index: u32,
    ai_group_id: &str,
) -> Option<ContextInjection> {
    let mut thinking_tokens: u64 = 0;
    let mut text_tokens: u64 = 0;

    for step in &ai_chunk.semantic_steps {
        match step {
            SemanticStep::Thinking { text, .. } => {
                thinking_tokens += estimate_tokens(text) as u64;
            }
            SemanticStep::Text { text, .. } => {
                text_tokens += estimate_tokens(text) as u64;
            }
            _ => {}
        }
    }

    let total = thinking_tokens + text_tokens;
    if total == 0 {
        return None;
    }

    let mut breakdown = Vec::new();
    if thinking_tokens > 0 {
        breakdown.push(ThinkingTextBreakdown {
            kind: ThinkingTextKind::Thinking,
            token_count: thinking_tokens,
        });
    }
    if text_tokens > 0 {
        breakdown.push(ThinkingTextBreakdown {
            kind: ThinkingTextKind::Text,
            token_count: text_tokens,
        });
    }

    Some(ContextInjection::ThinkingText(ThinkingTextInjection {
        id: thinking_text_id(turn_index),
        turn_index,
        ai_group_id: ai_group_id.to_string(),
        estimated_tokens: total,
        breakdown,
    }))
}

/// 从 `UserChunk.content` 提取纯文本用于 token 估计。
#[must_use]
fn user_chunk_text(user: &UserChunk) -> String {
    match &user.content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut out = String::new();
            for b in blocks {
                if let cdt_core::ContentBlock::Text { text } = b {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(text);
                }
            }
            out
        }
    }
}

/// 从 `UserChunk` 产出 `user-message` injection。
#[must_use]
pub(super) fn create_user_message_injection(
    user: &UserChunk,
    turn_index: u32,
    ai_group_id: &str,
) -> Option<ContextInjection> {
    let text = user_chunk_text(user);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let tokens = estimate_tokens(&text) as u64;
    if tokens == 0 {
        return None;
    }
    let preview: String = text.chars().take(80).collect();
    let preview = if text.chars().count() > 80 {
        format!("{preview}…")
    } else {
        preview
    };

    Some(ContextInjection::UserMessage(UserMessageInjection {
        id: user_message_id(turn_index),
        turn_index,
        ai_group_id: ai_group_id.to_string(),
        estimated_tokens: tokens,
        text_preview: preview,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{
        AssistantResponse, ChunkMetrics, MessageContent, SemanticStep, ToolExecution, ToolOutput,
    };
    use chrono::{DateTime, Utc};

    fn ts() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
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
        }
    }

    fn make_tool(name: &str, input: serde_json::Value, output: &str) -> ToolExecution {
        ToolExecution {
            tool_use_id: format!("tu-{name}"),
            tool_name: name.to_string(),
            input,
            output: ToolOutput::Text {
                text: output.to_string(),
            },
            is_error: false,
            start_ts: ts(),
            end_ts: Some(ts()),
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
        }
    }

    #[test]
    fn aggregate_tool_outputs_skips_task_coordination_tools() {
        let mut ai = empty_ai();
        ai.tool_executions.push(make_tool(
            "Bash",
            serde_json::json!({"cmd":"ls -la"}),
            "file1\nfile2\nfile3",
        ));
        ai.tool_executions.push(make_tool(
            "SendMessage",
            serde_json::json!({"recipient":"bob","body":"hi"}),
            "ack",
        ));
        let inj = aggregate_tool_outputs(&ai, 0, "ai-0").expect("has tool output");
        match inj {
            ContextInjection::ToolOutput(x) => {
                assert_eq!(x.tool_count, 1);
                assert_eq!(x.tool_breakdown[0].tool_name, "Bash");
                assert!(x.estimated_tokens > 0);
            }
            _ => panic!("expected ToolOutput"),
        }
    }

    #[test]
    fn aggregate_tool_outputs_returns_none_when_only_task_tools() {
        let mut ai = empty_ai();
        ai.tool_executions.push(make_tool(
            "TaskCreate",
            serde_json::json!({"subject":"x"}),
            "ok",
        ));
        assert!(aggregate_tool_outputs(&ai, 0, "ai-0").is_none());
    }

    #[test]
    fn aggregate_task_coordination_labels_send_message_with_recipient() {
        let mut ai = empty_ai();
        ai.tool_executions.push(make_tool(
            "SendMessage",
            serde_json::json!({"recipient":"alice","body":"hi"}),
            "ack",
        ));
        let inj = aggregate_task_coordination(&ai, 1, "ai-1").unwrap();
        match inj {
            ContextInjection::TaskCoordination(x) => {
                assert_eq!(x.breakdown.len(), 1);
                assert_eq!(x.breakdown[0].label, "SendMessage → alice");
                assert!(matches!(
                    x.breakdown[0].kind,
                    TaskCoordinationKind::SendMessage
                ));
            }
            _ => panic!("expected TaskCoordination"),
        }
    }

    #[test]
    fn aggregate_thinking_text_sums_both_kinds() {
        let mut ai = empty_ai();
        ai.semantic_steps.push(SemanticStep::Thinking {
            text: "abcdefghijklmnop".into(), // 16 chars → 4 tokens
            timestamp: ts(),
        });
        ai.semantic_steps.push(SemanticStep::Text {
            text: "abcdefgh".into(), // 8 chars → 2 tokens
            timestamp: ts(),
        });
        let inj = aggregate_thinking_text(&ai, 2, "ai-2").unwrap();
        match inj {
            ContextInjection::ThinkingText(x) => {
                assert_eq!(x.estimated_tokens, 6);
                assert_eq!(x.breakdown.len(), 2);
            }
            _ => panic!("expected ThinkingText"),
        }
    }

    #[test]
    fn aggregate_thinking_text_returns_none_when_empty() {
        let ai = empty_ai();
        assert!(aggregate_thinking_text(&ai, 0, "ai-0").is_none());
    }

    #[test]
    fn create_user_message_injection_handles_text_and_trims_preview() {
        let user = UserChunk {
            uuid: "u1".into(),
            timestamp: ts(),
            duration_ms: None,
            content: MessageContent::Text("a".repeat(120)),
            metrics: ChunkMetrics::zero(),
        };
        let inj = create_user_message_injection(&user, 0, "ai-0").unwrap();
        match inj {
            ContextInjection::UserMessage(x) => {
                assert!(x.estimated_tokens > 0);
                assert!(x.text_preview.chars().count() <= 81); // 80 + …
                assert!(x.text_preview.ends_with('…'));
            }
            _ => panic!("expected UserMessage"),
        }
    }

    #[test]
    fn create_user_message_injection_returns_none_on_empty() {
        let user = UserChunk {
            uuid: "u1".into(),
            timestamp: ts(),
            duration_ms: None,
            content: MessageContent::Text("   ".into()),
            metrics: ChunkMetrics::zero(),
        };
        assert!(create_user_message_injection(&user, 0, "ai-0").is_none());
    }
}
