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
fn interrupt_marker_is_hard_noise() {
    let line = r#"{"type":"user","uuid":"u1","timestamp":"2026-04-11T10:00:00Z","message":{"role":"user","content":"[Request interrupted by user for tool use]"}}"#;
    let msg = parse_entry(line).unwrap().expect("message should parse");
    assert_eq!(
        msg.category,
        MessageCategory::HardNoise(HardNoiseReason::InterruptMarker)
    );
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
