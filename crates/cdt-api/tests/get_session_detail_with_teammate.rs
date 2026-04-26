//! 集成测试：teammate-message 嵌入路径下，`build_chunks` → `serde_json` 的
//! IPC payload 形态验证。
//!
//! 因 `LocalDataApi::get_session_detail` 路径解析硬编码 `~/.claude/projects/`
//! （4 处共享，重构属于另一个 change），本测试通过"直接构造 `ParsedMessage` 流
//! → `cdt_analyze::build_chunks` → `serde_json::to_value`"组合，断言 IPC 边界
//! 序列化结果含 `teammateMessages` 字段集与正确 reply-to 配对，与
//! `LocalDataApi::get_session_detail` 实际经过的 chunk-building → serialize
//! 路径等价。
//!
//! 覆盖：
//! - `chunk-building` §`Embed teammate messages into AIChunk`
//! - `team-coordination-metadata` §`Link teammate messages to triggering SendMessage`
//! - `ipc-data-api` §`Expose teammate messages on AIChunk`

use cdt_analyze::build_chunks;
use cdt_core::{Chunk, ContentBlock, MessageCategory, MessageContent, MessageType, ParsedMessage};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;

fn ts(secs: i64) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-04-26T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
        + Duration::seconds(secs)
}

fn blank(uuid: &str, n: i64) -> ParsedMessage {
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

fn user(uuid: &str, n: i64, text: &str) -> ParsedMessage {
    ParsedMessage {
        content: MessageContent::Text(text.into()),
        ..blank(uuid, n)
    }
}

fn teammate(uuid: &str, n: i64, recipient: &str, summary: &str, body: &str) -> ParsedMessage {
    let xml = format!(
        r#"<teammate-message teammate_id="{recipient}" color="blue" summary="{summary}">{body}</teammate-message>"#
    );
    ParsedMessage {
        content: MessageContent::Text(xml),
        ..blank(uuid, n)
    }
}

fn assistant_with_blocks(uuid: &str, n: i64, blocks: Vec<ContentBlock>) -> ParsedMessage {
    use cdt_core::ToolCall;
    let tool_calls: Vec<ToolCall> = blocks
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
        .collect();
    ParsedMessage {
        message_type: MessageType::Assistant,
        category: MessageCategory::Assistant,
        content: MessageContent::Blocks(blocks),
        tool_calls,
        ..blank(uuid, n)
    }
}

fn send_message_assistant(uuid: &str, n: i64, recipient: &str) -> ParsedMessage {
    assistant_with_blocks(
        uuid,
        n,
        vec![ContentBlock::ToolUse {
            id: format!("tu-{uuid}"),
            name: "SendMessage".into(),
            input: serde_json::json!({ "recipient": recipient, "content": "do work" }),
        }],
    )
}

fn assistant_text(uuid: &str, n: i64, text: &str) -> ParsedMessage {
    assistant_with_blocks(uuid, n, vec![ContentBlock::Text { text: text.into() }])
}

fn ai_chunks_json(messages: &[ParsedMessage]) -> Vec<Value> {
    let chunks: Vec<Chunk> = build_chunks(messages);
    chunks
        .iter()
        .filter_map(|c| match c {
            Chunk::Ai(_) => Some(serde_json::to_value(c).expect("chunk serializes")),
            _ => None,
        })
        .collect()
}

#[test]
fn teammate_messages_field_serializes_with_camel_case_and_reply_to() {
    let msgs = vec![
        user("u1", 0, "create a team"),
        send_message_assistant("a1", 1, "alice"),
        teammate("tm1", 2, "alice", "Hi alice ready", "alice reply body"),
        assistant_text("a2", 3, "got it"),
    ];
    let ai_jsons = ai_chunks_json(&msgs);
    assert_eq!(
        ai_jsons.len(),
        1,
        "teammate 不打断 a1+a2 合并，应只 1 个 AIChunk: {ai_jsons:#?}"
    );
    let ai = &ai_jsons[0];

    let teammate_messages = ai
        .get("teammateMessages")
        .and_then(|v| v.as_array())
        .expect("AIChunk JSON 应含 teammateMessages 数组");
    assert_eq!(teammate_messages.len(), 1);

    let tm = &teammate_messages[0];
    assert_eq!(tm.get("teammateId").and_then(|v| v.as_str()), Some("alice"));
    assert_eq!(tm.get("color").and_then(|v| v.as_str()), Some("blue"));
    assert_eq!(
        tm.get("summary").and_then(|v| v.as_str()),
        Some("Hi alice ready")
    );
    assert_eq!(
        tm.get("body").and_then(|v| v.as_str()),
        Some("alice reply body")
    );
    assert_eq!(
        tm.get("replyToToolUseId").and_then(|v| v.as_str()),
        Some("tu-a1"),
        "reply_to_tool_use_id 应配对到 alice 的 SendMessage tool_use_id"
    );
    assert_eq!(
        tm.get("isNoise").and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        tm.get("isResend").and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert!(tm.get("uuid").is_some(), "uuid 应序列化");
    assert!(tm.get("timestamp").is_some(), "timestamp 应序列化");
}

#[test]
fn orphan_teammate_has_null_or_missing_reply_to() {
    let msgs = vec![
        user("u1", 0, "hi"),
        assistant_text("a1", 1, "ack"),
        teammate("tm-orphan", 2, "alice", "unsolicited", "spontaneous reply"),
    ];
    let ai_jsons = ai_chunks_json(&msgs);
    assert_eq!(ai_jsons.len(), 1);
    let ai = &ai_jsons[0];

    let teammate_messages = ai
        .get("teammateMessages")
        .and_then(|v| v.as_array())
        .expect("orphan teammate 仍嵌入 last AIChunk（drain_trailing_teammates 兜底）");
    assert_eq!(teammate_messages.len(), 1);

    let tm = &teammate_messages[0];
    let reply = tm.get("replyToToolUseId");
    assert!(
        reply.is_none_or(serde_json::Value::is_null),
        "orphan teammate 的 replyToToolUseId 应缺失或为 null（serde Option::None + skip_serializing_if 控制），实际：{reply:?}"
    );
}

#[test]
fn ai_chunk_without_teammate_omits_field_in_json() {
    let msgs = vec![user("u1", 0, "hi"), assistant_text("a1", 1, "ack")];
    let ai_jsons = ai_chunks_json(&msgs);
    assert_eq!(ai_jsons.len(), 1);
    assert!(
        ai_jsons[0].get("teammateMessages").is_none(),
        "无 teammate 嵌入时 IPC payload 不应含 teammateMessages 键（skip_serializing_if = Vec::is_empty 控制），实际 JSON: {:#?}",
        ai_jsons[0]
    );
}

#[test]
fn multiple_teammates_each_pair_with_their_send_message() {
    let msgs = vec![
        user("u1", 0, "create team"),
        send_message_assistant("a1", 1, "alice"),
        send_message_assistant("a2", 2, "bob"),
        teammate("tm-alice", 3, "alice", "alice ack", "from alice"),
        teammate("tm-bob", 4, "bob", "bob ack", "from bob"),
        assistant_text("a3", 5, "all set"),
    ];
    let ai_jsons = ai_chunks_json(&msgs);
    assert_eq!(ai_jsons.len(), 1);
    let teammate_messages = ai_jsons[0]
        .get("teammateMessages")
        .and_then(|v| v.as_array())
        .expect("含 teammateMessages");
    assert_eq!(teammate_messages.len(), 2);
    assert_eq!(
        teammate_messages[0]
            .get("replyToToolUseId")
            .and_then(|v| v.as_str()),
        Some("tu-a1")
    );
    assert_eq!(
        teammate_messages[1]
            .get("replyToToolUseId")
            .and_then(|v| v.as_str()),
        Some("tu-a2")
    );
}
