//! 跨 capability crate 共享的已解析消息类型。
//!
//! 这些类型是 TS 版 `ParsedMessage`
//! （见 `../claude-devtools/src/main/types/messages.ts`）的 Rust 对应物，
//! 是 `cdt-parse` 与所有下游消费方之间的契约。
//!
//! Spec：`openspec/specs/session-parsing/spec.md`。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// JSONL 原始条目的 `type` 字段。
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

/// 下游过滤使用的分类结果（在解析阶段完成）。
///
/// `HardNoise` 的消息在任何面向用户的渲染中都必须排除，但解析器仍会把它们
/// 产出来，以便统计 / 调试视图可以观察到 noise 比例。
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
    /// `system` / `summary` / `file-history-snapshot` / `queue-operation` 条目。
    NonConversationalEntry,
    /// `model == "<synthetic>"` 的 assistant 占位消息。
    SyntheticAssistant,
    /// 仅被 `<local-command-caveat>` 包裹的用户消息。
    LocalCommandCaveatOnly,
    /// 仅被 `<system-reminder>` 包裹的用户消息。
    SystemReminderOnly,
    /// 空的 `<local-command-stdout></local-command-stdout>` / stderr 输出。
    EmptyCommandOutput,
    /// 以 `[Request interrupted by user` 起首的中断标记。
    InterruptMarker,
}

/// Token 用量，字段与 Anthropic API 的 `usage` 一致。
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

/// 用户 / assistant 消息正文。老会话直接是字符串，
/// 新会话是一组 content block 数组。
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

/// 图片来源元数据。注意 base64 数据会完整保留在 `data` 字段里，下游若关心
/// 内存占用，应尽早丢弃或替换为引用。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ImageSource {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub media_type: String,
    #[serde(default)]
    pub data: String,
}

/// 消息正文中的单个 content block。
///
/// `Unknown` 是向前兼容的兜底分支，用于 parser 暂未识别的新 block 类型
/// （例如 Anthropic SDK 未来新增的类型）；遇到时保留为 `Unknown`，
/// 下游需要时可以通过 `tracing::debug!` 发现漂移。
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

/// 从消息正文里抽取出来的 `tool_use` 引用。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub is_task: bool,
    pub task_description: Option<String>,
    pub task_subagent_type: Option<String>,
}

/// 从消息正文里抽取出来的 `tool_result` 引用。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: serde_json::Value,
    pub is_error: bool,
}

/// 一条 JSONL 行经解析 + 分类后的结果。
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
