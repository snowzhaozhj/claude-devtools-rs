//! `ErrorDetector` orchestrator。
//!
//! 对应 TS `ErrorDetector.ts`。遍历 messages × triggers 产出 `DetectedError`。

use cdt_core::ParsedMessage;

use crate::detected_error::DetectedError;
use crate::error_trigger_checker::{
    check_token_threshold_trigger, check_tool_result_trigger, check_tool_use_trigger,
    matches_repository_scope,
};
use crate::types::{NotificationTrigger, TriggerContentType, TriggerMode};

/// 从一组消息中检测错误。
///
/// 遍历每条消息和每个 enabled trigger，路由到对应的 checker，收集所有匹配。
pub fn detect_errors(
    messages: &[ParsedMessage],
    triggers: &[NotificationTrigger],
    session_id: &str,
    project_id: &str,
    file_path: &str,
) -> Vec<DetectedError> {
    if triggers.is_empty() {
        return Vec::new();
    }

    let enabled: Vec<&NotificationTrigger> = triggers.iter().filter(|t| t.enabled).collect();
    if enabled.is_empty() {
        return Vec::new();
    }

    let mut errors = Vec::new();

    for (i, message) in messages.iter().enumerate() {
        let line_number = i + 1; // 1-based

        for trigger in &enabled {
            let trigger_errors = check_trigger(
                message,
                trigger,
                session_id,
                project_id,
                file_path,
                line_number,
            );
            errors.extend(trigger_errors);
        }
    }

    errors
}

/// 用单个 trigger 检测错误（用于历史预览）。
pub fn detect_errors_with_trigger(
    messages: &[ParsedMessage],
    trigger: &NotificationTrigger,
    session_id: &str,
    project_id: &str,
    file_path: &str,
) -> Vec<DetectedError> {
    let mut errors = Vec::new();

    for (i, message) in messages.iter().enumerate() {
        let line_number = i + 1;
        let trigger_errors = check_trigger(
            message,
            trigger,
            session_id,
            project_id,
            file_path,
            line_number,
        );
        errors.extend(trigger_errors);
    }

    errors
}

/// 路由到对应的 checker。
fn check_trigger(
    message: &ParsedMessage,
    trigger: &NotificationTrigger,
    session_id: &str,
    project_id: &str,
    file_path: &str,
    line_number: usize,
) -> Vec<DetectedError> {
    // 检查仓库范围
    if !matches_repository_scope(project_id, trigger.repository_ids.as_deref()) {
        return Vec::new();
    }

    // token_threshold 模式
    if trigger.mode == TriggerMode::TokenThreshold {
        return check_token_threshold_trigger(
            message,
            trigger,
            session_id,
            project_id,
            file_path,
            line_number,
        );
    }

    // tool_result trigger
    if trigger.content_type == TriggerContentType::ToolResult {
        return check_tool_result_trigger(
            message,
            trigger,
            session_id,
            project_id,
            file_path,
            line_number,
        )
        .into_iter()
        .collect();
    }

    // tool_use trigger
    if trigger.content_type == TriggerContentType::ToolUse {
        return check_tool_use_trigger(
            message,
            trigger,
            session_id,
            project_id,
            file_path,
            line_number,
        )
        .into_iter()
        .collect();
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::*;
    use chrono::Utc;

    fn make_msg_with_error() -> ParsedMessage {
        ParsedMessage {
            uuid: "m1".into(),
            parent_uuid: None,
            message_type: MessageType::Assistant,
            category: MessageCategory::Assistant,
            timestamp: Utc::now(),
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
            tool_calls: vec![],
            tool_results: vec![ToolResult {
                tool_use_id: "tu1".into(),
                content: serde_json::json!("error happened"),
                is_error: true,
            }],
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            is_compact_summary: false,
            request_id: None,
            tool_use_result: None,
        }
    }

    fn make_error_trigger() -> NotificationTrigger {
        NotificationTrigger {
            id: "t1".into(),
            name: "Error".into(),
            enabled: true,
            content_type: TriggerContentType::ToolResult,
            mode: TriggerMode::ErrorStatus,
            require_error: Some(true),
            is_builtin: None,
            tool_name: None,
            ignore_patterns: None,
            match_field: None,
            match_pattern: None,
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        }
    }

    #[test]
    fn detect_errors_finds_error() {
        let messages = vec![make_msg_with_error()];
        let triggers = vec![make_error_trigger()];
        let errors = detect_errors(&messages, &triggers, "s1", "p1", "/tmp/f.jsonl");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "error happened");
    }

    #[test]
    fn detect_errors_empty_triggers() {
        let messages = vec![make_msg_with_error()];
        let errors = detect_errors(&messages, &[], "s1", "p1", "/tmp/f.jsonl");
        assert!(errors.is_empty());
    }

    #[test]
    fn detect_errors_disabled_trigger() {
        let messages = vec![make_msg_with_error()];
        let mut trigger = make_error_trigger();
        trigger.enabled = false;
        let errors = detect_errors(&messages, &[trigger], "s1", "p1", "/tmp/f.jsonl");
        assert!(errors.is_empty());
    }

    #[test]
    fn detect_errors_with_single_trigger() {
        let messages = vec![make_msg_with_error()];
        let trigger = make_error_trigger();
        let errors = detect_errors_with_trigger(&messages, &trigger, "s1", "p1", "/tmp/f.jsonl");
        assert_eq!(errors.len(), 1);
    }
}
