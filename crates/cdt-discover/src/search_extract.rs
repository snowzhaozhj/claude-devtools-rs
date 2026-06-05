//! 从 `ParsedMessage` 序列中提取可搜索文本。
//!
//! 复用 `cdt-parse` 的噪声分类，排除 hard-noise 和 sidechain 消息。
//! AI 消息采用 buffer flushing 模型：连续 assistant 消息累积，
//! 遇到 user/system/compact 时 flush，只取 buffer 最后一条的最后 text block。

use cdt_core::{ContentBlock, MessageCategory, MessageContent, MessageType, ParsedMessage};

use crate::search_text::json_value_to_search_text;

const MAX_TOOL_SEARCH_BYTES: usize = 8192;

/// 可搜索条目——从一条消息中提取的文本。
#[derive(Debug, Clone)]
pub struct SearchableEntry {
    pub uuid: String,
    pub text: String,
    pub message_type: String,
}

/// 从消息序列中提取可搜索文本和 session title。
///
/// 返回 `(entries, session_title)`。`session_title` 取第一条用户消息的前 100 字符。
pub fn extract_searchable_entries(messages: &[ParsedMessage]) -> (Vec<SearchableEntry>, String) {
    let mut entries = Vec::new();
    let mut session_title = String::new();
    let mut ai_buffer: Vec<&ParsedMessage> = Vec::new();

    for msg in messages {
        if msg.category.is_hard_noise() || msg.is_sidechain {
            continue;
        }

        match msg.category {
            MessageCategory::User => {
                flush_ai_buffer(&ai_buffer, &mut entries);
                ai_buffer.clear();

                let text = extract_text_from_content(&msg.content);
                if !text.is_empty() {
                    if session_title.is_empty() {
                        session_title = truncate_chars(&text, 100);
                    }
                    entries.push(SearchableEntry {
                        uuid: msg.uuid.clone(),
                        text,
                        message_type: format_message_type(msg.message_type),
                    });
                }
                extract_tool_blocks(&msg.uuid, &msg.content, &mut entries);
            }
            MessageCategory::Assistant => {
                ai_buffer.push(msg);
            }
            // System / Compact / Interruption：flush pending AI buffer
            // 保持顺序；Interruption 不进入搜索索引（它是控制信号，非可
            // 搜索内容）。
            MessageCategory::System | MessageCategory::Compact | MessageCategory::Interruption => {
                flush_ai_buffer(&ai_buffer, &mut entries);
                ai_buffer.clear();
            }
            MessageCategory::HardNoise(_) => {}
        }
    }

    // flush 末尾残留的 AI buffer
    flush_ai_buffer(&ai_buffer, &mut entries);

    (entries, session_title)
}

/// flush AI buffer：取最后一条 assistant 消息的最后 text block + 所有消息的 tool blocks。
fn flush_ai_buffer(buffer: &[&ParsedMessage], entries: &mut Vec<SearchableEntry>) {
    if let Some(last) = buffer.last() {
        let text = extract_last_text_block(&last.content);
        if !text.is_empty() {
            entries.push(SearchableEntry {
                uuid: last.uuid.clone(),
                text,
                message_type: format_message_type(last.message_type),
            });
        }
    }
    for msg in buffer {
        extract_tool_blocks(&msg.uuid, &msg.content, entries);
    }
}

/// 从 content blocks 中提取 `ToolUse` input 和 `ToolResult` content 为搜索条目。
fn extract_tool_blocks(uuid: &str, content: &MessageContent, entries: &mut Vec<SearchableEntry>) {
    let blocks = match content {
        MessageContent::Blocks(blocks) => blocks,
        MessageContent::Text(_) => return,
    };
    for block in blocks {
        match block {
            ContentBlock::ToolUse { name, input, .. } => {
                let text = json_value_to_search_text(input, MAX_TOOL_SEARCH_BYTES);
                if !text.trim().is_empty() {
                    entries.push(SearchableEntry {
                        uuid: uuid.to_owned(),
                        text: format!("tool:{name} {text}"),
                        message_type: "tool_use".to_owned(),
                    });
                }
            }
            ContentBlock::ToolResult { content, .. } => {
                let text = json_value_to_search_text(content, MAX_TOOL_SEARCH_BYTES);
                if !text.trim().is_empty() {
                    entries.push(SearchableEntry {
                        uuid: uuid.to_owned(),
                        text,
                        message_type: "tool_result".to_owned(),
                    });
                }
            }
            _ => {}
        }
    }
}

/// 提取消息内容中所有 text blocks 的拼接文本。
fn extract_text_from_content(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut parts = Vec::new();
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    if !text.is_empty() {
                        parts.push(text.as_str());
                    }
                }
            }
            parts.join("\n")
        }
    }
}

/// 只提取最后一个 text block 的文本。
fn extract_last_text_block(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .rev()
            .find_map(|b| {
                if let ContentBlock::Text { text } = b {
                    if !text.is_empty() {
                        return Some(text.clone());
                    }
                }
                None
            })
            .unwrap_or_default(),
    }
}

fn format_message_type(mt: MessageType) -> String {
    match mt {
        MessageType::User => "user".to_owned(),
        MessageType::Assistant => "assistant".to_owned(),
        MessageType::System => "system".to_owned(),
        MessageType::Summary => "summary".to_owned(),
        MessageType::FileHistorySnapshot => "file_history_snapshot".to_owned(),
        MessageType::QueueOperation => "queue_operation".to_owned(),
    }
}

/// 按 char 截断，不按字节。
fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{HardNoiseReason, MessageCategory, MessageContent, MessageType, ParsedMessage};
    use chrono::Utc;

    fn make_msg(
        uuid: &str,
        category: MessageCategory,
        msg_type: MessageType,
        content: MessageContent,
    ) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.to_owned(),
            parent_uuid: None,
            message_type: msg_type,
            category,
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
            tool_calls: vec![],
            tool_results: vec![],
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            is_compact_summary: false,
            request_id: None,
            tool_use_result: None,
            is_queued_input: false,
        }
    }

    #[test]
    fn user_message_extracts_full_text() {
        let msgs = vec![make_msg(
            "u1",
            MessageCategory::User,
            MessageType::User,
            MessageContent::Text("hello world".into()),
        )];
        let (entries, title) = extract_searchable_entries(&msgs);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, "hello world");
        assert_eq!(entries[0].message_type, "user");
        assert_eq!(title, "hello world");
    }

    #[test]
    fn ai_buffer_flushes_only_last_text_block() {
        let msgs = vec![
            make_msg(
                "a1",
                MessageCategory::Assistant,
                MessageType::Assistant,
                MessageContent::Blocks(vec![
                    ContentBlock::Text {
                        text: "first response".into(),
                    },
                    ContentBlock::Text {
                        text: "second response".into(),
                    },
                ]),
            ),
            make_msg(
                "a2",
                MessageCategory::Assistant,
                MessageType::Assistant,
                MessageContent::Text("third msg".into()),
            ),
            // flush on user
            make_msg(
                "u1",
                MessageCategory::User,
                MessageType::User,
                MessageContent::Text("question".into()),
            ),
        ];
        let (entries, _) = extract_searchable_entries(&msgs);
        // buffer 只取 a2（最后一条），其 text 是 "third msg"
        assert_eq!(entries.len(), 2); // a2 + u1
        assert_eq!(entries[0].text, "third msg");
        assert_eq!(entries[0].uuid, "a2");
    }

    #[test]
    fn hard_noise_excluded() {
        let msgs = vec![make_msg(
            "n1",
            MessageCategory::HardNoise(HardNoiseReason::NonConversationalEntry),
            MessageType::System,
            MessageContent::Text("system noise".into()),
        )];
        let (entries, _) = extract_searchable_entries(&msgs);
        assert!(entries.is_empty());
    }

    #[test]
    fn sidechain_excluded() {
        let mut msg = make_msg(
            "s1",
            MessageCategory::User,
            MessageType::User,
            MessageContent::Text("sidechain content".into()),
        );
        msg.is_sidechain = true;
        let (entries, _) = extract_searchable_entries(&[msg]);
        assert!(entries.is_empty());
    }

    #[test]
    fn tool_use_input_command_indexed() {
        let msgs = vec![make_msg(
            "a1",
            MessageCategory::Assistant,
            MessageType::Assistant,
            MessageContent::Blocks(vec![ContentBlock::ToolUse {
                id: "tu1".into(),
                name: "Bash".into(),
                input: serde_json::json!({"command": "mw switch get carts2", "description": "Get switch"}),
            }]),
        ),
        make_msg(
            "u1",
            MessageCategory::User,
            MessageType::User,
            MessageContent::Text("ok".into()),
        )];
        let (entries, _) = extract_searchable_entries(&msgs);
        let tool_entries: Vec<_> = entries.iter().filter(|e| e.message_type == "tool_use").collect();
        assert_eq!(tool_entries.len(), 1);
        assert!(tool_entries[0].text.contains("mw switch get carts2"));
        assert!(tool_entries[0].text.starts_with("tool:Bash"));
    }

    #[test]
    fn tool_result_content_indexed_from_user_message() {
        let msgs = vec![make_msg(
            "u1",
            MessageCategory::User,
            MessageType::User,
            MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: "tu1".into(),
                content: serde_json::json!("enabled: true, value: 42"),
                is_error: false,
            }]),
        )];
        let (entries, _) = extract_searchable_entries(&msgs);
        let result_entries: Vec<_> = entries.iter().filter(|e| e.message_type == "tool_result").collect();
        assert_eq!(result_entries.len(), 1);
        assert!(result_entries[0].text.contains("enabled: true"));
    }

    #[test]
    fn json_key_not_indexed_in_tool_use() {
        let msgs = vec![make_msg(
            "a1",
            MessageCategory::Assistant,
            MessageType::Assistant,
            MessageContent::Blocks(vec![ContentBlock::ToolUse {
                id: "tu1".into(),
                name: "Bash".into(),
                input: serde_json::json!({"command": "ls -la"}),
            }]),
        ),
        make_msg(
            "u1",
            MessageCategory::User,
            MessageType::User,
            MessageContent::Text("done".into()),
        )];
        let (entries, _) = extract_searchable_entries(&msgs);
        let tool_entries: Vec<_> = entries.iter().filter(|e| e.message_type == "tool_use").collect();
        assert!(!tool_entries[0].text.contains("\"command\""));
    }

    #[test]
    fn session_title_truncated_to_100_chars() {
        let long_text: String = "a".repeat(200);
        let msgs = vec![make_msg(
            "u1",
            MessageCategory::User,
            MessageType::User,
            MessageContent::Text(long_text),
        )];
        let (_, title) = extract_searchable_entries(&msgs);
        assert_eq!(title.len(), 100);
    }
}
