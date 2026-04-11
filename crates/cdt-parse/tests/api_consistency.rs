//! 验证：对同一份输入，逐行循环调用 `parse_entry` 得到的 `ParsedMessage`
//! 列表应与 `parse_file` 的产出完全一致（前提是没有 requestId 重复，
//! 即不会触发 dedup 分支）。

use std::io::Write;

use cdt_parse::{parse_entry, parse_file};
use tempfile::NamedTempFile;

const FIXTURE: &str = concat!(
    r#"{"type":"user","uuid":"u1","timestamp":"2026-04-11T10:00:00Z","message":{"role":"user","content":"a"}}"#,
    "\n",
    r#"{"type":"assistant","uuid":"a1","timestamp":"2026-04-11T10:00:01Z","requestId":"r-unique","message":{"role":"assistant","model":"m","content":[{"type":"text","text":"ok"}]}}"#,
    "\n",
    r#"{"type":"user","uuid":"u2","timestamp":"2026-04-11T10:00:02Z","isMeta":true,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tid","content":"done","is_error":false}]}}"#,
    "\n",
);

#[tokio::test]
async fn parse_entry_loop_matches_parse_file_when_no_dedup_needed() {
    let via_entry: Vec<_> = FIXTURE
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| parse_entry(l).unwrap().expect("message should parse"))
        .collect();

    let mut f = NamedTempFile::new().unwrap();
    f.write_all(FIXTURE.as_bytes()).unwrap();
    let via_file = parse_file(f.path()).await.unwrap();

    assert_eq!(via_entry.len(), via_file.len());
    for (a, b) in via_entry.iter().zip(via_file.iter()) {
        // timestamp is parsed the same way both times so equality is total.
        assert_eq!(a, b);
    }
}
