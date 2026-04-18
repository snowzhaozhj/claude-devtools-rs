//! Session "in progress" 状态检测。
//!
//! 端口自 `../claude-devtools/src/main/utils/sessionStateDetection.ts`。
//! Spec：`openspec/specs/session-display/spec.md` §"Ongoing banner at
//! session bottom" + `openspec/specs/sidebar-navigation/spec.md`
//! §"Ongoing indicator on session item"。
//!
//! 算法：按顺序记录活动栈（`Thinking` / `ToolUse` / `ToolResult` /
//! `TextOutput` / `Interruption` / `ExitPlanMode`），找最后一个 ending
//! 事件（`TextOutput` / `Interruption` / `ExitPlanMode`）；若其之后仍
//! 存在 AI 活动（`Thinking` / `ToolUse` / `ToolResult`）则判定 ongoing。
//! 若从未出现 ending 但有任意 AI 活动——同样 ongoing；完全没有 AI
//! 活动则非 ongoing。

use std::collections::HashSet;

use cdt_core::{ContentBlock, MessageCategory, MessageContent, MessageType, ParsedMessage};

const INTERRUPT_PREFIX: &str = "[Request interrupted by user";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Activity {
    Thinking,
    ToolUse,
    ToolResult,
    TextOutput,
    Interruption,
    ExitPlanMode,
}

impl Activity {
    fn is_ending(self) -> bool {
        matches!(
            self,
            Activity::TextOutput | Activity::Interruption | Activity::ExitPlanMode
        )
    }

    fn is_ai(self) -> bool {
        matches!(
            self,
            Activity::Thinking | Activity::ToolUse | Activity::ToolResult
        )
    }
}

/// 给定消息序列判定会话是否仍在进行。
///
/// 参见 module doc 的算法描述。空序列返回 `false`。
pub fn check_messages_ongoing(messages: &[ParsedMessage]) -> bool {
    let activities = build_activity_stack(messages);
    is_ongoing_from_activities(&activities)
}

fn build_activity_stack(messages: &[ParsedMessage]) -> Vec<Activity> {
    let mut acts: Vec<Activity> = Vec::new();
    let mut shutdown_tool_ids: HashSet<String> = HashSet::new();

    for msg in messages {
        match msg.message_type {
            MessageType::Assistant => process_assistant(msg, &mut acts, &mut shutdown_tool_ids),
            MessageType::User => process_user(msg, &mut acts, &shutdown_tool_ids),
            _ => {}
        }
    }

    acts
}

fn process_assistant(
    msg: &ParsedMessage,
    acts: &mut Vec<Activity>,
    shutdown_tool_ids: &mut HashSet<String>,
) {
    let MessageContent::Blocks(blocks) = &msg.content else {
        return;
    };
    for block in blocks {
        match block {
            ContentBlock::Thinking { thinking, .. } if !thinking.is_empty() => {
                acts.push(Activity::Thinking);
            }
            ContentBlock::Text { text } if !text.trim().is_empty() => {
                acts.push(Activity::TextOutput);
            }
            ContentBlock::ToolUse { id, name, input } => {
                if name == "ExitPlanMode" {
                    acts.push(Activity::ExitPlanMode);
                } else if is_shutdown_response(name, input) {
                    shutdown_tool_ids.insert(id.clone());
                    acts.push(Activity::Interruption);
                } else {
                    acts.push(Activity::ToolUse);
                }
            }
            _ => {}
        }
    }
}

fn is_shutdown_response(name: &str, input: &serde_json::Value) -> bool {
    if name != "SendMessage" {
        return false;
    }
    let Some(obj) = input.as_object() else {
        return false;
    };
    let is_shutdown = obj
        .get("type")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s == "shutdown_response");
    let approved = obj
        .get("approve")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    is_shutdown && approved
}

fn process_user(
    msg: &ParsedMessage,
    acts: &mut Vec<Activity>,
    shutdown_tool_ids: &HashSet<String>,
) {
    // parse 层已把 `[Request interrupted by user` 分类为 Interruption；
    // legacy string content 与 blocks 路径都走这个分支。
    if msg.category == MessageCategory::Interruption {
        acts.push(Activity::Interruption);
        return;
    }

    let MessageContent::Blocks(blocks) = &msg.content else {
        return;
    };

    let is_rejection = matches!(
        msg.tool_use_result.as_ref().and_then(|v| v.as_str()),
        Some("User rejected tool use")
    );

    for block in blocks {
        match block {
            ContentBlock::ToolResult { tool_use_id, .. } => {
                if shutdown_tool_ids.contains(tool_use_id) || is_rejection {
                    acts.push(Activity::Interruption);
                } else {
                    acts.push(Activity::ToolResult);
                }
            }
            ContentBlock::Text { text } if text.trim_start().starts_with(INTERRUPT_PREFIX) => {
                acts.push(Activity::Interruption);
            }
            _ => {}
        }
    }
}

fn is_ongoing_from_activities(activities: &[Activity]) -> bool {
    if activities.is_empty() {
        return false;
    }

    let last_ending = activities.iter().rposition(|a| a.is_ending());

    match last_ending {
        None => activities.iter().any(|a| a.is_ai()),
        Some(idx) => activities[idx + 1..].iter().any(|a| a.is_ai()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{ContentBlock, MessageCategory, MessageContent, MessageType};
    use chrono::{DateTime, Duration, Utc};

    fn ts(n: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::seconds(n)
    }

    fn blank(uuid: &str, n: i64, ty: MessageType, cat: MessageCategory) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.into(),
            parent_uuid: None,
            message_type: ty,
            category: cat,
            timestamp: ts(n),
            role: None,
            content: MessageContent::Blocks(Vec::new()),
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
            tool_use_result: None,
        }
    }

    fn assistant_blocks(uuid: &str, n: i64, blocks: Vec<ContentBlock>) -> ParsedMessage {
        ParsedMessage {
            content: MessageContent::Blocks(blocks),
            ..blank(uuid, n, MessageType::Assistant, MessageCategory::Assistant)
        }
    }

    fn user_blocks(uuid: &str, n: i64, blocks: Vec<ContentBlock>) -> ParsedMessage {
        ParsedMessage {
            content: MessageContent::Blocks(blocks),
            ..blank(uuid, n, MessageType::User, MessageCategory::User)
        }
    }

    #[test]
    fn empty_messages_is_not_ongoing() {
        assert!(!check_messages_ongoing(&[]));
    }

    #[test]
    fn plain_text_output_ends_session() {
        let msgs = vec![assistant_blocks(
            "a1",
            1,
            vec![ContentBlock::Text {
                text: "done".into(),
            }],
        )];
        assert!(!check_messages_ongoing(&msgs));
    }

    #[test]
    fn tool_use_after_text_means_ongoing() {
        let msgs = vec![
            assistant_blocks(
                "a1",
                1,
                vec![ContentBlock::Text {
                    text: "working".into(),
                }],
            ),
            assistant_blocks(
                "a2",
                2,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
        ];
        assert!(check_messages_ongoing(&msgs));
    }

    #[test]
    fn interrupt_marker_user_text_ends_session() {
        let msgs = vec![
            assistant_blocks(
                "a1",
                1,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
            ParsedMessage {
                category: MessageCategory::Interruption,
                content: MessageContent::Text("[Request interrupted by user for tool use]".into()),
                ..blank("u1", 2, MessageType::User, MessageCategory::Interruption)
            },
        ];
        assert!(!check_messages_ongoing(&msgs));
    }

    #[test]
    fn tool_rejection_ends_session() {
        let mut rejection = user_blocks(
            "u1",
            2,
            vec![ContentBlock::ToolResult {
                tool_use_id: "t1".into(),
                content: serde_json::json!("..."),
                is_error: false,
            }],
        );
        rejection.tool_use_result =
            Some(serde_json::Value::String("User rejected tool use".into()));
        let msgs = vec![
            assistant_blocks(
                "a1",
                1,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
            rejection,
        ];
        assert!(!check_messages_ongoing(&msgs));
    }

    #[test]
    fn exit_plan_mode_is_ending() {
        let msgs = vec![assistant_blocks(
            "a1",
            1,
            vec![ContentBlock::ToolUse {
                id: "t1".into(),
                name: "ExitPlanMode".into(),
                input: serde_json::json!({}),
            }],
        )];
        assert!(!check_messages_ongoing(&msgs));
    }

    #[test]
    fn shutdown_response_ends_session_with_matching_result() {
        let shutdown_assistant = assistant_blocks(
            "a1",
            1,
            vec![ContentBlock::ToolUse {
                id: "t-shutdown".into(),
                name: "SendMessage".into(),
                input: serde_json::json!({"type": "shutdown_response", "approve": true}),
            }],
        );
        let shutdown_result = user_blocks(
            "u1",
            2,
            vec![ContentBlock::ToolResult {
                tool_use_id: "t-shutdown".into(),
                content: serde_json::json!("ok"),
                is_error: false,
            }],
        );
        let msgs = vec![shutdown_assistant, shutdown_result];
        assert!(!check_messages_ongoing(&msgs));
    }

    #[test]
    fn ongoing_when_only_ai_activity() {
        let msgs = vec![assistant_blocks(
            "a1",
            1,
            vec![ContentBlock::ToolUse {
                id: "t1".into(),
                name: "Bash".into(),
                input: serde_json::json!({}),
            }],
        )];
        assert!(check_messages_ongoing(&msgs));
    }

    #[test]
    fn ai_activity_after_interrupt_resumes_ongoing() {
        let msgs = vec![
            assistant_blocks(
                "a1",
                1,
                vec![ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
            ParsedMessage {
                category: MessageCategory::Interruption,
                content: MessageContent::Text("[Request interrupted by user]".into()),
                ..blank("u1", 2, MessageType::User, MessageCategory::Interruption)
            },
            assistant_blocks(
                "a2",
                3,
                vec![ContentBlock::ToolUse {
                    id: "t2".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
        ];
        assert!(check_messages_ongoing(&msgs));
    }
}
