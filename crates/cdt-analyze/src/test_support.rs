#![cfg(test)]

use cdt_core::{
    ContentBlock, MessageCategory, MessageContent, MessageType, ParsedMessage, ToolCall,
};
use chrono::{DateTime, Duration, Utc};

pub fn ts(n: i64) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
        + Duration::seconds(n)
}

pub fn blank_message(uuid: &str, n: i64) -> ParsedMessage {
    ParsedMessage {
        uuid: uuid.into(),
        parent_uuid: None,
        message_type: MessageType::User,
        category: MessageCategory::User,
        timestamp: ts(n),
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
        tool_calls: Vec::new(),
        tool_results: Vec::new(),
        source_tool_use_id: None,
        source_tool_assistant_uuid: None,
        is_compact_summary: false,
        request_id: None,
        tool_use_result: None,
    }
}

pub fn blank_typed(uuid: &str, n: i64, ty: MessageType, cat: MessageCategory) -> ParsedMessage {
    ParsedMessage {
        message_type: ty,
        category: cat,
        content: MessageContent::Blocks(Vec::new()),
        ..blank_message(uuid, n)
    }
}

pub fn user(uuid: &str, n: i64, text: &str) -> ParsedMessage {
    ParsedMessage {
        content: MessageContent::Text(text.into()),
        ..blank_message(uuid, n)
    }
}

pub fn user_blocks(uuid: &str, n: i64, blocks: Vec<ContentBlock>) -> ParsedMessage {
    ParsedMessage {
        content: MessageContent::Blocks(blocks),
        ..blank_message(uuid, n)
    }
}

pub fn assistant(uuid: &str, n: i64, blocks: &[ContentBlock]) -> ParsedMessage {
    ParsedMessage {
        message_type: MessageType::Assistant,
        category: MessageCategory::Assistant,
        content: MessageContent::Blocks(blocks.to_vec()),
        tool_calls: blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolUse { id, name, input } => Some(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                    is_task: name == "Task",
                    task_description: None,
                    task_subagent_type: None,
                }),
                _ => None,
            })
            .collect(),
        tool_results: Vec::new(),
        ..blank_message(uuid, n)
    }
}

pub fn assistant_with_tool(uuid: &str, n: i64, id: &str, name: &str) -> ParsedMessage {
    let input = serde_json::json!({});
    assistant(
        uuid,
        n,
        &[ContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }],
    )
}

pub fn user_with_result(
    uuid: &str,
    n: i64,
    id: &str,
    content: serde_json::Value,
    is_error: bool,
) -> ParsedMessage {
    ParsedMessage {
        content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
            tool_use_id: id.into(),
            content,
            is_error,
        }]),
        ..blank_message(uuid, n)
    }
}

pub fn make_user_msg(content: MessageContent) -> ParsedMessage {
    ParsedMessage {
        uuid: "m1".into(),
        parent_uuid: None,
        message_type: MessageType::User,
        category: MessageCategory::User,
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
    }
}
