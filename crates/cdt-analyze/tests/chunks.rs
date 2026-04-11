//! chunk-building 的端到端快照测试。
//!
//! 从 `tests/fixtures/*.jsonl` 读取原始 JSONL，用 `cdt_parse::parse_entry`
//! 逐行解析成 `ParsedMessage`，然后交给 `cdt_analyze::build_chunks`。
//! 快照锁定 chunk 序列的形态与字段取值，用来保护 Task / subagent port 完成后
//! 不会意外回退已固化的结构约定。

use std::path::PathBuf;

use cdt_analyze::build_chunks;
use cdt_core::{Chunk, ParsedMessage};
use cdt_parse::{dedupe_by_request_id, parse_entry_at};

fn parse_fixture(name: &str) -> Vec<ParsedMessage> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path.push(name);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    let mut out = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match parse_entry_at(line, i + 1) {
            Ok(Some(msg)) => out.push(msg),
            Ok(None) => {}
            Err(e) => panic!("fixture {name} line {}: {e:?}", i + 1),
        }
    }
    dedupe_by_request_id(out)
}

fn summarize(chunks: &[Chunk]) -> Vec<String> {
    chunks
        .iter()
        .map(|c| match c {
            Chunk::User(u) => format!(
                "User(uuid={}, ts={}, content={:?}, tokens_in={}, tokens_out={})",
                u.uuid,
                u.timestamp.to_rfc3339(),
                u.content,
                u.metrics.input_tokens,
                u.metrics.output_tokens
            ),
            Chunk::Ai(a) => format!(
                "Ai(responses={}, ts={}, duration_ms={:?}, tokens_in={}, tokens_out={}, tool_count={}, steps={}, tool_executions={}, subagents={})",
                a.responses.len(),
                a.timestamp.to_rfc3339(),
                a.duration_ms,
                a.metrics.input_tokens,
                a.metrics.output_tokens,
                a.metrics.tool_count,
                a.semantic_steps.len(),
                a.tool_executions.len(),
                a.subagents.len()
            ),
            Chunk::System(s) => format!(
                "System(uuid={}, ts={}, text={:?})",
                s.uuid,
                s.timestamp.to_rfc3339(),
                s.content_text
            ),
            Chunk::Compact(c) => format!(
                "Compact(uuid={}, ts={}, summary={:?})",
                c.uuid,
                c.timestamp.to_rfc3339(),
                c.summary_text
            ),
        })
        .collect()
}

#[test]
fn simple_user_assistant_snapshot() {
    let msgs = parse_fixture("simple.jsonl");
    let chunks = build_chunks(&msgs);
    insta::assert_debug_snapshot!(summarize(&chunks));
}

#[test]
fn multi_assistant_coalescing_snapshot() {
    let msgs = parse_fixture("multi_ai.jsonl");
    let chunks = build_chunks(&msgs);
    insta::assert_debug_snapshot!(summarize(&chunks));
}

#[test]
fn compact_boundary_snapshot() {
    let msgs = parse_fixture("with_compact.jsonl");
    let chunks = build_chunks(&msgs);
    insta::assert_debug_snapshot!(summarize(&chunks));
}
