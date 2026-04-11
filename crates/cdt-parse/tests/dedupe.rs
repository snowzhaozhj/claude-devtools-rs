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
async fn parse_file_invokes_dedup_automatically() {
    // This is the wire-in test: the TS impl-bug was that dedup existed but was never called.
    let contents = concat!(
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-04-11T10:00:00Z","requestId":"r1","message":{"role":"assistant","model":"m","content":[{"type":"text","text":"partial"}]}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a2","timestamp":"2026-04-11T10:00:01Z","requestId":"r1","message":{"role":"assistant","model":"m","content":[{"type":"text","text":"final"}]}}"#,
        "\n",
    );
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(contents.as_bytes()).unwrap();

    let out = parse_file(f.path()).await.unwrap();
    assert_eq!(out.len(), 1, "dedup must be wired into parse_file");
    assert_eq!(out[0].uuid, "a2");
}
