//! `DetectedError` 类型 + 构建工具。
//!
//! 对应 TS `ErrorMessageBuilder.ts`。

use cdt_core::{ContentBlock, MessageContent, ToolResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 检测到的错误。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedError {
    pub id: String,
    pub timestamp: i64,
    pub session_id: String,
    pub project_id: String,
    pub file_path: String,
    pub source: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_name: Option<String>,
    pub context: DetectedErrorContext,
}

/// 错误上下文。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedErrorContext {
    pub project_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// 构建 `DetectedError` 的参数。
pub struct CreateDetectedErrorParams {
    pub session_id: String,
    pub project_id: String,
    pub file_path: String,
    pub project_name: String,
    pub line_number: usize,
    pub source: String,
    pub message: String,
    pub timestamp_ms: i64,
    pub cwd: Option<String>,
    pub tool_use_id: Option<String>,
    pub trigger_color: Option<String>,
    pub trigger_id: Option<String>,
    pub trigger_name: Option<String>,
}

const MAX_MESSAGE_LENGTH: usize = 500;

/// 截断消息到 `MAX_MESSAGE_LENGTH` 字符。
pub fn truncate_message(message: &str) -> String {
    if message.len() <= MAX_MESSAGE_LENGTH {
        message.to_owned()
    } else {
        let mut s = message[..MAX_MESSAGE_LENGTH].to_owned();
        s.push_str("...");
        s
    }
}

/// 从 `ToolResult` 提取错误消息文本。
pub fn extract_error_message(result: &ToolResult) -> String {
    if let Some(s) = result.content.as_str() {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return "Unknown error".into();
        }
        return trimmed.to_owned();
    }

    if let Some(arr) = result.content.as_array() {
        let mut texts = Vec::new();
        for item in arr {
            if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    texts.push(text);
                }
            }
        }
        let joined = texts.join("\n").trim().to_owned();
        if !joined.is_empty() {
            return joined;
        }
    }

    "Unknown error".into()
}

/// 从 `MessageContent` 提取纯文本。
pub fn extract_text_from_content(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut parts = Vec::new();
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    parts.push(text.as_str());
                }
            }
            parts.join("\n")
        }
    }
}

/// 构建 `DetectedError`，id 为 `(session_id, file_path, line_number, tool_use_id, trigger_id,
/// message)` 元组的 SHA-256 前 16 字节 hex。确定性 id 使同一错误重新检测时产出相同条目，
/// 配合 `NotificationManager::add_notification` 的 dedup 实现零副作用重扫。
pub fn create_detected_error(params: CreateDetectedErrorParams) -> DetectedError {
    let message = truncate_message(&params.message);
    let id = compute_detected_error_id(
        &params.session_id,
        &params.file_path,
        params.line_number,
        params.tool_use_id.as_deref(),
        params.trigger_id.as_deref(),
        &message,
    );
    DetectedError {
        id,
        timestamp: params.timestamp_ms,
        session_id: params.session_id,
        project_id: params.project_id,
        file_path: params.file_path,
        source: params.source,
        message,
        line_number: Some(params.line_number),
        tool_use_id: params.tool_use_id,
        trigger_color: params.trigger_color,
        trigger_id: params.trigger_id,
        trigger_name: params.trigger_name,
        context: DetectedErrorContext {
            project_name: params.project_name,
            cwd: params.cwd,
        },
    }
}

fn compute_detected_error_id(
    session_id: &str,
    file_path: &str,
    line_number: usize,
    tool_use_id: Option<&str>,
    trigger_id: Option<&str>,
    message: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(file_path.as_bytes());
    hasher.update(b"\0");
    hasher.update(line_number.to_le_bytes());
    hasher.update(b"\0");
    hasher.update(tool_use_id.unwrap_or("").as_bytes());
    hasher.update(b"\0");
    hasher.update(trigger_id.unwrap_or("").as_bytes());
    hasher.update(b"\0");
    hasher.update(message.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(32);
    for b in &digest[..16] {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_error_from_string_content() {
        let tr = ToolResult {
            tool_use_id: "t1".into(),
            content: serde_json::json!("some error message"),
            is_error: true,
        };
        assert_eq!(extract_error_message(&tr), "some error message");
    }

    #[test]
    fn extract_error_from_empty_string() {
        let tr = ToolResult {
            tool_use_id: "t1".into(),
            content: serde_json::json!("  "),
            is_error: true,
        };
        assert_eq!(extract_error_message(&tr), "Unknown error");
    }

    #[test]
    fn extract_error_from_array_blocks() {
        let tr = ToolResult {
            tool_use_id: "t1".into(),
            content: serde_json::json!([
                {"type": "text", "text": "line 1"},
                {"type": "text", "text": "line 2"}
            ]),
            is_error: true,
        };
        assert_eq!(extract_error_message(&tr), "line 1\nline 2");
    }

    #[test]
    fn extract_error_from_null() {
        let tr = ToolResult {
            tool_use_id: "t1".into(),
            content: serde_json::Value::Null,
            is_error: true,
        };
        assert_eq!(extract_error_message(&tr), "Unknown error");
    }

    #[test]
    fn truncate_short_message() {
        let msg = "short";
        assert_eq!(truncate_message(msg), "short");
    }

    #[test]
    fn truncate_long_message() {
        let msg = "x".repeat(600);
        let result = truncate_message(&msg);
        assert_eq!(result.len(), 503); // 500 + "..."
        assert!(result.ends_with("..."));
    }

    fn sample_params() -> CreateDetectedErrorParams {
        CreateDetectedErrorParams {
            session_id: "s1".into(),
            project_id: "p1".into(),
            file_path: "/tmp/test.jsonl".into(),
            project_name: "test".into(),
            line_number: 42,
            source: "Bash".into(),
            message: "fail".into(),
            timestamp_ms: 1000,
            cwd: None,
            tool_use_id: Some("tu1".into()),
            trigger_color: None,
            trigger_id: Some("t1".into()),
            trigger_name: Some("My Trigger".into()),
        }
    }

    #[test]
    fn create_detected_error_produces_deterministic_id() {
        let a = create_detected_error(sample_params());
        let b = create_detected_error(sample_params());
        assert_eq!(a.id, b.id);
        assert_eq!(a.id.len(), 32);
        assert!(a.id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(a.source, "Bash");
        assert_eq!(a.trigger_id, Some("t1".into()));
    }

    #[test]
    fn create_detected_error_different_sessions_different_ids() {
        let a = create_detected_error(sample_params());
        let mut p = sample_params();
        p.session_id = "s2".into();
        let b = create_detected_error(p);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn create_detected_error_different_triggers_different_ids() {
        let a = create_detected_error(sample_params());
        let mut p = sample_params();
        p.trigger_id = Some("t2".into());
        let b = create_detected_error(p);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn create_detected_error_different_messages_different_ids() {
        let a = create_detected_error(sample_params());
        let mut p = sample_params();
        p.message = "different failure".into();
        let b = create_detected_error(p);
        assert_ne!(a.id, b.id);
    }
}
