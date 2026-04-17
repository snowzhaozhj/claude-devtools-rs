use std::io::Write;

use cdt_parse::{dedupe_by_request_id, parse_entry, parse_file};
use tempfile::NamedTempFile;

fn entry(line: &str) -> cdt_core::ParsedMessage {
    parse_entry(line).unwrap().expect("message should parse")
}

fn assistant(uuid: &str, request_id: &str, ts_sec: u32) -> cdt_core::ParsedMessage {
    let line = format!(
        r#"{{"type":"assistant","uuid":"{uuid}","timestamp":"2026-04-11T10:00:{ts_sec:02}Z","requestId":"{request_id}","message":{{"role":"assistant","model":"m","content":[{{"type":"text","text":"{uuid}"}}]}}}}"#
    );
    entry(&line)
}

fn user(uuid: &str, ts_sec: u32) -> cdt_core::ParsedMessage {
    let line = format!(
        r#"{{"type":"user","uuid":"{uuid}","timestamp":"2026-04-11T10:00:{ts_sec:02}Z","message":{{"role":"user","content":"{uuid}"}}}}"#
    );
    entry(&line)
}

#[test]
fn two_entries_with_same_request_id_keeps_last() {
    let msgs = vec![assistant("a1", "r1", 0), assistant("a2", "r1", 1)];
    let out = dedupe_by_request_id(msgs);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].uuid, "a2");
}

#[test]
fn three_interleaved_with_same_request_id() {
    let msgs = vec![
        assistant("a1", "r1", 0),
        user("u1", 1),
        assistant("a2", "r1", 2),
        user("u2", 3),
        assistant("a3", "r1", 4),
    ];
    let out = dedupe_by_request_id(msgs);
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].uuid, "u1");
    assert_eq!(out[1].uuid, "u2");
    assert_eq!(out[2].uuid, "a3");
}

#[test]
fn non_assistant_with_request_id_is_not_deduped() {
    let line = r#"{"type":"user","uuid":"u-with-req","timestamp":"2026-04-11T10:00:00Z","requestId":"shared","message":{"role":"user","content":"x"}}"#;
    let u1 = entry(line);
    let a1 = assistant("a1", "shared", 1);
    let msgs = vec![u1, a1];
    let out = dedupe_by_request_id(msgs);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].uuid, "u-with-req");
    assert_eq!(out[1].uuid, "a1");
}

#[tokio::test]
async fn parse_file_does_not_dedupe_by_request_id() {
    // Claude Code 实际 JSONL 里同 requestId 的多条 assistant 记录承载不同
    // content block（thinking / text / 各 tool_use）——并非 streaming rewrite。
    // 早期 Rust port 错误地在 parse_file 中调用 dedup 导致丢失 tool_use，
    // 现已移除。本测试防护回归。详见 openspec/followups.md。
    let contents = concat!(
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-04-11T10:00:00Z","requestId":"r1","message":{"role":"assistant","model":"m","content":[{"type":"text","text":"first"}]}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a2","timestamp":"2026-04-11T10:00:01Z","requestId":"r1","message":{"role":"assistant","model":"m","content":[{"type":"text","text":"second"}]}}"#,
        "\n",
    );
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(contents.as_bytes()).unwrap();

    let out = parse_file(f.path()).await.unwrap();
    assert_eq!(out.len(), 2, "parse_file must preserve all records");
    assert_eq!(out[0].uuid, "a1");
    assert_eq!(out[1].uuid, "a2");
}
