use std::fmt::Write as _;
use std::io::Write;

use cdt_core::{MessageCategory, MessageType};
use cdt_parse::parse_file;
use tempfile::NamedTempFile;

fn write_tmp(contents: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("tempfile");
    f.write_all(contents.as_bytes()).expect("write");
    f
}

#[tokio::test]
async fn empty_file_returns_empty_vec() {
    let f = write_tmp("");
    let out = parse_file(f.path()).await.unwrap();
    assert!(out.is_empty());
}

#[tokio::test]
async fn malformed_line_in_middle_is_skipped() {
    let contents = concat!(
        r#"{"type":"user","uuid":"u1","timestamp":"2026-04-11T10:00:00Z","message":{"role":"user","content":"a"}}"#,
        "\n",
        "{not valid json\n",
        r#"{"type":"user","uuid":"u2","timestamp":"2026-04-11T10:00:01Z","message":{"role":"user","content":"b"}}"#,
        "\n",
    );
    let f = write_tmp(contents);
    let out = parse_file(f.path()).await.unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].uuid, "u1");
    assert_eq!(out[1].uuid, "u2");
}

#[tokio::test]
async fn two_adjacent_malformed_lines_both_skipped() {
    let contents = concat!(
        r#"{"type":"user","uuid":"u1","timestamp":"2026-04-11T10:00:00Z","message":{"content":"a"}}"#,
        "\n",
        "garbage1\n",
        "garbage2\n",
        r#"{"type":"user","uuid":"u2","timestamp":"2026-04-11T10:00:01Z","message":{"content":"b"}}"#,
        "\n",
    );
    let f = write_tmp(contents);
    let out = parse_file(f.path()).await.unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].uuid, "u1");
    assert_eq!(out[1].uuid, "u2");
}

#[tokio::test]
async fn large_session_file_preserves_order() {
    // 10k entries, alternating user/assistant, no duplicate requestIds.
    let mut contents = String::with_capacity(10_000 * 160);
    for i in 0..10_000 {
        if i % 2 == 0 {
            writeln!(
                contents,
                r#"{{"type":"user","uuid":"u{i}","timestamp":"2026-04-11T10:00:00Z","message":{{"role":"user","content":"m{i}"}}}}"#
            )
            .unwrap();
        } else {
            writeln!(
                contents,
                r#"{{"type":"assistant","uuid":"a{i}","timestamp":"2026-04-11T10:00:00Z","requestId":"r{i}","message":{{"role":"assistant","model":"m","content":[{{"type":"text","text":"ok"}}]}}}}"#
            )
            .unwrap();
        }
    }
    let f = write_tmp(&contents);
    let out = parse_file(f.path()).await.unwrap();
    assert_eq!(out.len(), 10_000);
    // Spot-check file order is preserved.
    assert_eq!(out[0].uuid, "u0");
    assert_eq!(out[0].message_type, MessageType::User);
    assert_eq!(out[1].uuid, "a1");
    assert_eq!(out[1].message_type, MessageType::Assistant);
    assert_eq!(out.last().unwrap().uuid, "a9999");
}

#[tokio::test]
async fn fixture_mixed_session_classifies_correctly() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mixed.jsonl");
    let out = parse_file(path).await.unwrap();
    assert_eq!(out.len(), 4);
    assert_eq!(out[0].category, MessageCategory::User);
    assert_eq!(out[1].category, MessageCategory::Assistant);
    assert_eq!(out[1].tool_calls.len(), 1);
    assert_eq!(out[2].category, MessageCategory::User);
    assert!(out[2].is_meta);
    assert_eq!(out[2].tool_results.len(), 1);
    assert_eq!(out[3].category, MessageCategory::Assistant);
}
