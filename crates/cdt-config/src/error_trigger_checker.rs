//! 3 种 trigger mode 的 checker 纯函数。
//!
//! 对应 TS `ErrorTriggerChecker.ts`。

use cdt_core::{ContentBlock, MessageType, ParsedMessage};

use crate::detected_error::{
    CreateDetectedErrorParams, DetectedError, create_detected_error, extract_error_message,
};
use crate::trigger_matcher::{
    extract_tool_use_field, get_content_blocks, matches_ignore_patterns, matches_pattern,
};
use crate::types::{NotificationTrigger, TriggerMode};

/// `repository_ids` 范围检查（stub）。
///
/// 无 `repository_ids` → 全匹配。有 `repository_ids` → 暂返回 `false`
/// （完整实现依赖 `cdt-discover::GitIdentityResolver`，留给后续接入）。
pub fn matches_repository_scope(_project_id: &str, repository_ids: Option<&[String]>) -> bool {
    let Some(ids) = repository_ids else {
        return true;
    };
    if ids.is_empty() {
        return true;
    }
    tracing::debug!(
        "repository_ids scope check stubbed — returning false for {} ids",
        ids.len()
    );
    false
}

/// 检查 `tool_result` trigger。
pub fn check_tool_result_trigger(
    message: &ParsedMessage,
    trigger: &NotificationTrigger,
    session_id: &str,
    project_id: &str,
    file_path: &str,
    line_number: usize,
) -> Option<DetectedError> {
    for result in &message.tool_results {
        // error_status mode：检查 `is_error` flag
        if trigger.mode == TriggerMode::ErrorStatus && trigger.require_error == Some(true) {
            if !result.is_error {
                continue;
            }

            let error_msg = extract_error_message(result);

            if matches_ignore_patterns(&error_msg, trigger.ignore_patterns.as_deref()) {
                continue;
            }

            let tool_name = find_tool_name_for_result(message, &result.tool_use_id);

            return Some(create_detected_error(CreateDetectedErrorParams {
                session_id: session_id.to_owned(),
                project_id: project_id.to_owned(),
                file_path: file_path.to_owned(),
                project_name: project_id.to_owned(),
                line_number,
                source: tool_name.unwrap_or_else(|| "tool_result".into()),
                message: error_msg,
                timestamp_ms: message.timestamp.timestamp_millis(),
                cwd: message.cwd.clone(),
                tool_use_id: Some(result.tool_use_id.clone()),
                trigger_color: trigger.color.clone(),
                trigger_id: Some(trigger.id.clone()),
                trigger_name: Some(trigger.name.clone()),
            }));
        }

        // content_match mode
        if trigger.mode == TriggerMode::ContentMatch {
            if let Some(ref tool_name_filter) = trigger.tool_name {
                let actual_name = find_tool_name_for_result(message, &result.tool_use_id);
                if actual_name.as_deref() != Some(tool_name_filter.as_str()) {
                    continue;
                }
            }

            if trigger.match_field.as_deref() == Some("content") {
                if let Some(ref pat) = trigger.match_pattern {
                    let content_str = content_value_to_string(&result.content);
                    if !matches_pattern(&content_str, pat) {
                        continue;
                    }
                    if matches_ignore_patterns(&content_str, trigger.ignore_patterns.as_deref()) {
                        continue;
                    }

                    let preview = if content_str.len() > 200 {
                        format!("{}...", &content_str[..200])
                    } else {
                        content_str
                    };

                    return Some(create_detected_error(CreateDetectedErrorParams {
                        session_id: session_id.to_owned(),
                        project_id: project_id.to_owned(),
                        file_path: file_path.to_owned(),
                        project_name: project_id.to_owned(),
                        line_number,
                        source: trigger
                            .tool_name
                            .clone()
                            .unwrap_or_else(|| "tool_result".into()),
                        message: format!("Tool result matched: {preview}"),
                        timestamp_ms: message.timestamp.timestamp_millis(),
                        cwd: message.cwd.clone(),
                        tool_use_id: Some(result.tool_use_id.clone()),
                        trigger_color: trigger.color.clone(),
                        trigger_id: Some(trigger.id.clone()),
                        trigger_name: Some(trigger.name.clone()),
                    }));
                }
            }
        }
    }

    None
}

/// 检查 `tool_use` trigger。
pub fn check_tool_use_trigger(
    message: &ParsedMessage,
    trigger: &NotificationTrigger,
    session_id: &str,
    project_id: &str,
    file_path: &str,
    line_number: usize,
) -> Option<DetectedError> {
    if message.message_type != MessageType::Assistant {
        return None;
    }

    let blocks = get_content_blocks(&message.content);

    for block in blocks {
        let ContentBlock::ToolUse { id, name, input } = block else {
            continue;
        };

        if let Some(ref filter_name) = trigger.tool_name {
            if name != filter_name {
                continue;
            }
        }

        // 提取待匹配字段
        let field_value = if let Some(ref mf) = trigger.match_field {
            extract_tool_use_field(input, mf)
        } else {
            // 无 matchField 时匹配整个 input JSON
            Some(input.to_string())
        };

        let Some(ref val) = field_value else {
            continue;
        };

        if let Some(ref pat) = trigger.match_pattern {
            if !matches_pattern(val, pat) {
                continue;
            }
        }

        if matches_ignore_patterns(val, trigger.ignore_patterns.as_deref()) {
            continue;
        }

        let preview = if val.len() > 200 {
            format!("{}...", &val[..200])
        } else {
            val.clone()
        };
        let field_label = trigger.match_field.as_deref().unwrap_or("tool_use");

        return Some(create_detected_error(CreateDetectedErrorParams {
            session_id: session_id.to_owned(),
            project_id: project_id.to_owned(),
            file_path: file_path.to_owned(),
            project_name: project_id.to_owned(),
            line_number,
            source: name.clone(),
            message: format!("{field_label}: {preview}"),
            timestamp_ms: message.timestamp.timestamp_millis(),
            cwd: message.cwd.clone(),
            tool_use_id: Some(id.clone()),
            trigger_color: trigger.color.clone(),
            trigger_id: Some(trigger.id.clone()),
            trigger_name: Some(trigger.name.clone()),
        }));
    }

    None
}

/// 检查 `token_threshold` trigger。
pub fn check_token_threshold_trigger(
    message: &ParsedMessage,
    trigger: &NotificationTrigger,
    session_id: &str,
    project_id: &str,
    file_path: &str,
    line_number: usize,
) -> Vec<DetectedError> {
    let mut errors = Vec::new();

    if trigger.mode != TriggerMode::TokenThreshold {
        return errors;
    }

    let Some(threshold) = trigger.token_threshold else {
        return errors;
    };

    if message.message_type != MessageType::Assistant {
        return errors;
    }

    let token_type = trigger
        .token_type
        .unwrap_or(crate::types::TriggerTokenType::Total);

    // 收集 tool_use blocks
    let blocks = get_content_blocks(&message.content);
    let mut tool_uses: Vec<(&str, &str, &serde_json::Value)> = Vec::new();

    for block in &blocks {
        if let ContentBlock::ToolUse { id, name, input } = block {
            tool_uses.push((id, name, input));
        }
    }

    // 也检查 tool_calls
    for tc in &message.tool_calls {
        if !tool_uses.iter().any(|(id, _, _)| *id == tc.id) {
            tool_uses.push((&tc.id, &tc.name, &tc.input));
        }
    }

    // 对每个 tool_use 检查 token 阈值
    for (tu_id, tu_name, tu_input) in &tool_uses {
        if let Some(ref filter_name) = trigger.tool_name {
            if *tu_name != filter_name.as_str() {
                continue;
            }
        }

        let call_text = format!(
            "{tu_name}{}",
            serde_json::to_string(tu_input).unwrap_or_default()
        );
        let call_tokens = call_text.len() as u64 / 4;

        // 查找对应的 tool_result
        let result_tokens = message
            .tool_results
            .iter()
            .find(|r| r.tool_use_id == *tu_id)
            .map_or(0, |r| content_value_to_string(&r.content).len() as u64 / 4);

        let token_count = match token_type {
            crate::types::TriggerTokenType::Input => call_tokens,
            crate::types::TriggerTokenType::Output => result_tokens,
            crate::types::TriggerTokenType::Total => call_tokens + result_tokens,
        };

        if token_count <= threshold {
            continue;
        }

        let type_label = match token_type {
            crate::types::TriggerTokenType::Total => "",
            crate::types::TriggerTokenType::Input => " input",
            crate::types::TriggerTokenType::Output => " output",
        };
        let token_msg =
            format!("{tu_name}: ~{token_count}{type_label} tokens (threshold: {threshold})");

        if matches_ignore_patterns(&token_msg, trigger.ignore_patterns.as_deref()) {
            continue;
        }

        errors.push(create_detected_error(CreateDetectedErrorParams {
            session_id: session_id.to_owned(),
            project_id: project_id.to_owned(),
            file_path: file_path.to_owned(),
            project_name: project_id.to_owned(),
            line_number,
            source: (*tu_name).to_owned(),
            message: token_msg,
            timestamp_ms: message.timestamp.timestamp_millis(),
            cwd: message.cwd.clone(),
            tool_use_id: Some((*tu_id).to_owned()),
            trigger_color: trigger.color.clone(),
            trigger_id: Some(trigger.id.clone()),
            trigger_name: Some(trigger.name.clone()),
        }));
    }

    errors
}

/// 在 message 的 `tool_calls` 里查找 `tool_use_id` 对应的 tool name。
fn find_tool_name_for_result(message: &ParsedMessage, tool_use_id: &str) -> Option<String> {
    message
        .tool_calls
        .iter()
        .find(|tc| tc.id == tool_use_id)
        .map(|tc| tc.name.clone())
}

/// 将 `serde_json::Value` 转为字符串（用于 content 匹配）。
fn content_value_to_string(val: &serde_json::Value) -> String {
    if let Some(s) = val.as_str() {
        s.to_owned()
    } else {
        val.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TriggerContentType;
    use cdt_core::*;
    use chrono::Utc;

    fn make_message(
        msg_type: MessageType,
        content: MessageContent,
        tool_results: Vec<ToolResult>,
        tool_calls: Vec<ToolCall>,
    ) -> ParsedMessage {
        ParsedMessage {
            uuid: "msg-1".into(),
            parent_uuid: None,
            message_type: msg_type,
            category: MessageCategory::Assistant,
            timestamp: Utc::now(),
            role: None,
            content,
            usage: None,
            model: None,
            cwd: None,
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta: false,
            user_type: None,
            tool_calls,
            tool_results,
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            is_compact_summary: false,
            request_id: None,
        }
    }

    fn make_error_status_trigger() -> NotificationTrigger {
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
            color: Some("red".into()),
        }
    }

    #[test]
    fn detect_is_error_true() {
        let msg = make_message(
            MessageType::Assistant,
            MessageContent::Text(String::new()),
            vec![ToolResult {
                tool_use_id: "tu1".into(),
                content: serde_json::json!("command failed"),
                is_error: true,
            }],
            vec![],
        );

        let trigger = make_error_status_trigger();
        let result = check_tool_result_trigger(&msg, &trigger, "s1", "p1", "/tmp/f.jsonl", 1);
        assert!(result.is_some());
        assert_eq!(result.unwrap().message, "command failed");
    }

    #[test]
    fn skip_non_error_result() {
        let msg = make_message(
            MessageType::Assistant,
            MessageContent::Text(String::new()),
            vec![ToolResult {
                tool_use_id: "tu1".into(),
                content: serde_json::json!("success"),
                is_error: false,
            }],
            vec![],
        );

        let trigger = make_error_status_trigger();
        let result = check_tool_result_trigger(&msg, &trigger, "s1", "p1", "/tmp/f.jsonl", 1);
        assert!(result.is_none());
    }

    #[test]
    fn ignore_pattern_suppresses() {
        let msg = make_message(
            MessageType::Assistant,
            MessageContent::Text(String::new()),
            vec![ToolResult {
                tool_use_id: "tu1".into(),
                content: serde_json::json!("The user doesn't want to proceed with this tool use."),
                is_error: true,
            }],
            vec![],
        );

        let mut trigger = make_error_status_trigger();
        trigger.ignore_patterns = Some(vec![r"user doesn't want to proceed".into()]);

        let result = check_tool_result_trigger(&msg, &trigger, "s1", "p1", "/tmp/f.jsonl", 1);
        assert!(result.is_none());
    }

    #[test]
    fn tool_use_trigger_matches() {
        let msg = make_message(
            MessageType::Assistant,
            MessageContent::Blocks(vec![ContentBlock::ToolUse {
                id: "tu1".into(),
                name: "Bash".into(),
                input: serde_json::json!({"command": "cat .env"}),
            }]),
            vec![],
            vec![],
        );

        let trigger = NotificationTrigger {
            id: "t2".into(),
            name: "Env access".into(),
            enabled: true,
            content_type: TriggerContentType::ToolUse,
            mode: TriggerMode::ContentMatch,
            match_field: Some("command".into()),
            match_pattern: Some("\\.env".into()),
            is_builtin: None,
            tool_name: Some("Bash".into()),
            ignore_patterns: None,
            require_error: None,
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        };

        let result = check_tool_use_trigger(&msg, &trigger, "s1", "p1", "/tmp/f.jsonl", 1);
        assert!(result.is_some());
    }

    #[test]
    fn tool_name_filter_skips_mismatch() {
        let msg = make_message(
            MessageType::Assistant,
            MessageContent::Blocks(vec![ContentBlock::ToolUse {
                id: "tu1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": ".env"}),
            }]),
            vec![],
            vec![],
        );

        let trigger = NotificationTrigger {
            id: "t2".into(),
            name: "Env".into(),
            enabled: true,
            content_type: TriggerContentType::ToolUse,
            mode: TriggerMode::ContentMatch,
            match_field: Some("file_path".into()),
            match_pattern: Some("\\.env".into()),
            tool_name: Some("Bash".into()), // 不匹配 "Read"
            is_builtin: None,
            ignore_patterns: None,
            require_error: None,
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        };

        let result = check_tool_use_trigger(&msg, &trigger, "s1", "p1", "/tmp/f.jsonl", 1);
        assert!(result.is_none());
    }

    #[test]
    fn token_threshold_trigger() {
        let big_input = serde_json::json!({"data": "x".repeat(40_000)});
        let msg = make_message(
            MessageType::Assistant,
            MessageContent::Blocks(vec![ContentBlock::ToolUse {
                id: "tu1".into(),
                name: "Read".into(),
                input: big_input,
            }]),
            vec![],
            vec![],
        );

        let trigger = NotificationTrigger {
            id: "t3".into(),
            name: "High tokens".into(),
            enabled: true,
            content_type: TriggerContentType::ToolResult,
            mode: TriggerMode::TokenThreshold,
            token_threshold: Some(1000),
            token_type: Some(crate::types::TriggerTokenType::Total),
            is_builtin: None,
            tool_name: None,
            ignore_patterns: None,
            require_error: None,
            match_field: None,
            match_pattern: None,
            repository_ids: None,
            color: None,
        };

        let errors = check_token_threshold_trigger(&msg, &trigger, "s1", "p1", "/tmp/f.jsonl", 1);
        assert!(!errors.is_empty());
    }

    #[test]
    fn repository_scope_no_ids_matches() {
        assert!(matches_repository_scope("p1", None));
        assert!(matches_repository_scope("p1", Some(&[])));
    }

    #[test]
    fn repository_scope_with_ids_stub_false() {
        let ids = vec!["repo-1".into()];
        assert!(!matches_repository_scope("p1", Some(&ids)));
    }
}
