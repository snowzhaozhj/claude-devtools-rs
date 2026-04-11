//! Parsed message types shared across all capability crates.
//!
//! These types are the Rust counterpart of the TS `ParsedMessage` shape
//! (see `../claude-devtools/src/main/types/messages.ts`). They are the
//! contract between `cdt-parse` and every downstream consumer.
//!
//! Spec: `openspec/specs/session-parsing/spec.md`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Raw JSONL entry `type` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MessageType {
    User,
    Assistant,
    System,
    Summary,
    FileHistorySnapshot,
    QueueOperation,
}

/// Post-classification category used by downstream filters.
///
/// `HardNoise` messages MUST be excluded from any user-facing rendering
/// but are still emitted by the parser so that analytics / debug views
/// can observe them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageCategory {
    User,
    Assistant,
    System,
    Compact,
    HardNoise(HardNoiseReason),
}

impl MessageCategory {
    pub fn is_hard_noise(&self) -> bool {
        matches!(self, MessageCategory::HardNoise(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardNoiseReason {
    /// `system`, `summary`, `file-history-snapshot`, `queue-operation`.
    NonConversationalEntry,
    /// Assistant message with `model == "<synthetic>"`.
    SyntheticAssistant,
    /// User message wrapped solely in `<local-command-caveat>`.
    LocalCommandCaveatOnly,
    /// User message wrapped solely in `<system-reminder>`.
    SystemReminderOnly,
    /// Empty `<local-command-stdout></local-command-stdout>` or stderr.
    EmptyCommandOutput,
    /// `[Request interrupted by user...`.
    InterruptMarker,
}

/// Token usage block, matches Anthropic API `usage` shape.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct TokenUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
}

/// User/assistant message content. Legacy sessions ship a plain string,
/// modern sessions ship an array of content blocks.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl Default for MessageContent {
    fn default() -> Self {
        MessageContent::Text(String::new())
    }
}

/// Image source metadata (base64 payload elided from downstream consumers
/// to keep memory bounded — we only remember the media type).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ImageSource {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub media_type: String,
    #[serde(default)]
    pub data: String,
}

/// Single content block inside a message.
///
/// `Unknown` is the forward-compat catch-all for block types the current
/// parser does not recognise (e.g. future Anthropic SDK additions); the
/// parser logs a `debug!` on each occurrence so we can spot drift.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        #[serde(default)]
        text: String,
    },
    Thinking {
        #[serde(default)]
        thinking: String,
        #[serde(default)]
        signature: String,
    },
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
    Image {
        source: ImageSource,
    },
    #[serde(other)]
    Unknown,
}

/// Extracted `tool_use` reference.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub is_task: bool,
    pub task_description: Option<String>,
    pub task_subagent_type: Option<String>,
}

/// Extracted `tool_result` reference.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: serde_json::Value,
    pub is_error: bool,
}

/// Parsed message — one JSONL line after classification.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMessage {
    pub uuid: String,
    pub parent_uuid: Option<String>,
    pub message_type: MessageType,
    pub category: MessageCategory,
    pub timestamp: DateTime<Utc>,
    pub role: Option<String>,
    pub content: MessageContent,
    pub usage: Option<TokenUsage>,
    pub model: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub agent_id: Option<String>,
    pub is_sidechain: bool,
    pub is_meta: bool,
    pub user_type: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolResult>,
    pub source_tool_use_id: Option<String>,
    pub source_tool_assistant_uuid: Option<String>,
    pub is_compact_summary: bool,
    pub request_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_block_text_roundtrip() {
        let json = r#"{"type":"text","text":"hello"}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        assert_eq!(
            block,
            ContentBlock::Text {
                text: "hello".into()
            }
        );
    }

    #[test]
    fn content_block_tool_use_roundtrip() {
        let json = r#"{"type":"tool_use","id":"abc","name":"Bash","input":{"cmd":"ls"}}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "abc");
                assert_eq!(name, "Bash");
                assert_eq!(input["cmd"], "ls");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn content_block_tool_result_roundtrip() {
        let json = r#"{"type":"tool_result","tool_use_id":"abc","content":"ok","is_error":false}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "abc");
                assert_eq!(content, serde_json::json!("ok"));
                assert!(!is_error);
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    #[test]
    fn content_block_unknown_type_falls_back() {
        let json = r#"{"type":"future_block_kind","foo":"bar"}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        assert_eq!(block, ContentBlock::Unknown);
    }

    #[test]
    fn message_content_legacy_string() {
        let json = r#""plain text""#;
        let content: MessageContent = serde_json::from_str(json).unwrap();
        assert_eq!(content, MessageContent::Text("plain text".into()));
    }

    #[test]
    fn message_content_modern_blocks() {
        let json =
            r#"[{"type":"text","text":"hi"},{"type":"tool_use","id":"x","name":"Y","input":{}}]"#;
        let content: MessageContent = serde_json::from_str(json).unwrap();
        match content {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 2);
                assert!(matches!(blocks[0], ContentBlock::Text { .. }));
                assert!(matches!(blocks[1], ContentBlock::ToolUse { .. }));
            }
            MessageContent::Text(_) => panic!("expected Blocks variant"),
        }
    }

    #[test]
    fn token_usage_missing_optional_fields() {
        let json = r#"{"input_tokens":10,"output_tokens":20}"#;
        let usage: TokenUsage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 20);
        assert_eq!(usage.cache_read_input_tokens, 0);
        assert_eq!(usage.cache_creation_input_tokens, 0);
    }

    #[test]
    fn message_type_kebab_case_deserialization() {
        assert_eq!(
            serde_json::from_str::<MessageType>("\"file-history-snapshot\"").unwrap(),
            MessageType::FileHistorySnapshot
        );
        assert_eq!(
            serde_json::from_str::<MessageType>("\"queue-operation\"").unwrap(),
            MessageType::QueueOperation
        );
    }
}
