use cdt_core::{ContentBlock, HardNoiseReason, MessageCategory, MessageContent, MessageType};
use cdt_parse::parse_entry;

#[test]
fn assistant_message_with_tool_use_blocks() {
    let line = r#"{"type":"assistant","uuid":"a1","timestamp":"2026-04-11T10:00:00Z","isSidechain":false,"requestId":"r1","message":{"role":"assistant","model":"claude-opus-4-6","content":[{"type":"text","text":"ok"},{"type":"tool_use","id":"t1","name":"Task","input":{"description":"scan","subagent_type":"explorer"}},{"type":"tool_use","id":"t2","name":"Bash","input":{"cmd":"ls"}}]}}"#;
    let msg = parse_entry(line).unwrap().expect("message should parse");
    assert_eq!(msg.message_type, MessageType::Assistant);
    assert_eq!(msg.category, MessageCategory::Assistant);
    assert_eq!(msg.tool_calls.len(), 2);

    let task = &msg.tool_calls[0];
    assert_eq!(task.id, "t1");
    assert!(task.is_task);
    assert_eq!(task.task_description.as_deref(), Some("scan"));
    assert_eq!(task.task_subagent_type.as_deref(), Some("explorer"));

    let bash = &msg.tool_calls[1];
    assert!(!bash.is_task);
    assert_eq!(bash.name, "Bash");
}

#[test]
fn user_message_with_tool_result_blocks() {
    let line = r#"{"type":"user","uuid":"u1","timestamp":"2026-04-11T10:00:00Z","isMeta":true,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"file.txt","is_error":false}]}}"#;
    let msg = parse_entry(line).unwrap().expect("message should parse");
    assert!(msg.is_meta);
    assert_eq!(msg.tool_results.len(), 1);
    assert_eq!(msg.tool_results[0].tool_use_id, "t1");
    assert!(!msg.tool_results[0].is_error);
}

#[test]
fn compact_summary_boundary() {
    let line = r#"{"type":"user","uuid":"u1","timestamp":"2026-04-11T10:00:00Z","isCompactSummary":true,"message":{"role":"user","content":"summary body"}}"#;
    let msg = parse_entry(line).unwrap().expect("message should parse");
    assert!(msg.is_compact_summary);
    assert_eq!(msg.category, MessageCategory::Compact);
}

#[test]
fn legacy_string_content_preserved() {
    let line = r#"{"type":"user","uuid":"u1","timestamp":"2026-04-11T10:00:00Z","message":{"role":"user","content":"help me"}}"#;
    let msg = parse_entry(line).unwrap().expect("message should parse");
    match msg.content {
        MessageContent::Text(s) => assert_eq!(s, "help me"),
        MessageContent::Blocks(_) => panic!("expected legacy string content"),
    }
}

#[test]
fn modern_block_array_content_preserved() {
    let line = r#"{"type":"user","uuid":"u1","timestamp":"2026-04-11T10:00:00Z","message":{"role":"user","content":[{"type":"text","text":"hi"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":""}}]}}"#;
    let msg = parse_entry(line).unwrap().expect("message should parse");
    match msg.content {
        MessageContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 2);
            assert!(matches!(blocks[0], ContentBlock::Text { .. }));
            assert!(matches!(blocks[1], ContentBlock::Image { .. }));
        }
        MessageContent::Text(_) => panic!("expected block array content"),
    }
}

#[test]
fn synthetic_assistant_is_hard_noise() {
    let line = r#"{"type":"assistant","uuid":"a1","timestamp":"2026-04-11T10:00:00Z","message":{"role":"assistant","model":"<synthetic>","content":[]}}"#;
    let msg = parse_entry(line).unwrap().expect("message should parse");
    assert_eq!(
        msg.category,
        MessageCategory::HardNoise(HardNoiseReason::SyntheticAssistant)
    );
}

#[test]
fn interrupt_marker_is_interruption_category() {
    let line = r#"{"type":"user","uuid":"u1","timestamp":"2026-04-11T10:00:00Z","message":{"role":"user","content":"[Request interrupted by user for tool use]"}}"#;
    let msg = parse_entry(line).unwrap().expect("message should parse");
    assert_eq!(msg.category, MessageCategory::Interruption);
}

#[test]
fn interrupt_marker_in_blocks_is_interruption_category() {
    let line = r#"{"type":"user","uuid":"u2","timestamp":"2026-04-11T10:00:00Z","message":{"role":"user","content":[{"type":"text","text":"[Request interrupted by user]"}]}}"#;
    let msg = parse_entry(line).unwrap().expect("message should parse");
    assert_eq!(msg.category, MessageCategory::Interruption);
}

#[test]
fn empty_line_returns_none() {
    assert!(parse_entry("").unwrap().is_none());
    assert!(parse_entry("   \n").unwrap().is_none());
}

#[test]
fn entry_without_uuid_returns_none() {
    let line = r#"{"type":"user","message":{"content":"x"}}"#;
    assert!(parse_entry(line).unwrap().is_none());
}

#[test]
fn unknown_type_returns_none() {
    let line = r#"{"type":"some-future-kind","uuid":"x"}"#;
    assert!(parse_entry(line).unwrap().is_none());
}

#[test]
fn attachment_queued_command_parsed_as_user_with_queued_flag() {
    let line = r#"{"type":"attachment","uuid":"att1","parentUuid":"p1","timestamp":"2026-05-30T15:18:46Z","attachment":{"type":"queued_command","prompt":"hello from user","commandMode":"prompt"}}"#;
    let msg = parse_entry(line).unwrap().expect("should parse");
    assert_eq!(msg.uuid, "att1");
    assert_eq!(msg.parent_uuid.as_deref(), Some("p1"));
    assert_eq!(msg.message_type, MessageType::User);
    assert_eq!(msg.category, MessageCategory::User);
    assert!(msg.is_queued_input);
    assert!(!msg.is_meta);
    let MessageContent::Text(text) = &msg.content else {
        panic!("expected Text content");
    };
    assert_eq!(text, "hello from user");
}

#[test]
fn attachment_queued_command_with_multimodal_prompt_parsed_as_blocks() {
    // 回归：带图片的排队命令 prompt 是 content-block 数组，曾因 prompt 定成
    // Option<String> 导致整行 serde 失败、entry 被丢 + 刷 warn。现在用 MessageContent 吃下两态。
    let line = r#"{"type":"attachment","uuid":"att5","parentUuid":"p5","timestamp":"2026-06-26T15:00:00Z","attachment":{"type":"queued_command","prompt":[{"type":"text","text":"look at this"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"AAAA"}}]}}"#;
    let msg = parse_entry(line)
        .unwrap()
        .expect("multimodal queued command should parse");
    assert_eq!(msg.uuid, "att5");
    assert_eq!(msg.message_type, MessageType::User);
    assert_eq!(msg.category, MessageCategory::User);
    assert!(msg.is_queued_input);
    let MessageContent::Blocks(blocks) = &msg.content else {
        panic!("expected Blocks content for multimodal prompt");
    };
    assert_eq!(blocks.len(), 2);
    assert!(matches!(blocks[0], ContentBlock::Text { .. }));
    assert!(matches!(blocks[1], ContentBlock::Image { .. }));
}

#[test]
fn attachment_queued_command_empty_blocks_prompt_skipped() {
    let line = r#"{"type":"attachment","uuid":"att6","timestamp":"2026-06-26T15:00:00Z","attachment":{"type":"queued_command","prompt":[]}}"#;
    assert!(parse_entry(line).unwrap().is_none());
}

#[test]
fn attachment_non_queued_command_skipped() {
    let line = r#"{"type":"attachment","uuid":"att2","timestamp":"2026-05-30T15:00:00Z","attachment":{"type":"skill_listing","content":"..."}}"#;
    assert!(parse_entry(line).unwrap().is_none());
}

#[test]
fn attachment_queued_command_empty_prompt_skipped() {
    let line = r#"{"type":"attachment","uuid":"att3","timestamp":"2026-05-30T15:00:00Z","attachment":{"type":"queued_command","prompt":""}}"#;
    assert!(parse_entry(line).unwrap().is_none());
}

#[test]
fn attachment_queued_command_missing_prompt_skipped() {
    let line = r#"{"type":"attachment","uuid":"att4","timestamp":"2026-05-30T15:00:00Z","attachment":{"type":"queued_command"}}"#;
    assert!(parse_entry(line).unwrap().is_none());
}
