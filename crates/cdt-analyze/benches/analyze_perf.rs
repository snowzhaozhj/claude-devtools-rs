//! chunk-building + session state 性能基准。
//!
//! 跑法：`cargo bench -p cdt-analyze`
//!
//! cdt-analyze 是 sync crate（无 tokio 依赖），bench 全程同步。

use std::fmt::Write as _;
use std::sync::OnceLock;

use cdt_core::ParsedMessage;

struct Fixtures {
    small: Vec<ParsedMessage>,
    medium: Vec<ParsedMessage>,
    large: Vec<ParsedMessage>,
}

fn fixtures() -> &'static Fixtures {
    static FX: OnceLock<Fixtures> = OnceLock::new();
    FX.get_or_init(|| Fixtures {
        small: parse_synthetic_session(50),
        medium: parse_synthetic_session(500),
        large: parse_synthetic_session(2000),
    })
}

fn main() {
    divan::main();
}

// --- build_chunks ---

#[divan::bench(args = [50, 500, 2000])]
fn build_chunks(bencher: divan::Bencher<'_, '_>, n: usize) {
    let fx = fixtures();
    let messages = match n {
        50 => &fx.small,
        500 => &fx.medium,
        _ => &fx.large,
    };

    bencher.bench(|| divan::black_box(cdt_analyze::build_chunks(messages)));
}

// --- pair_tool_executions ---

#[divan::bench(args = [50, 500, 2000])]
fn pair_tool_executions(bencher: divan::Bencher<'_, '_>, n: usize) {
    let fx = fixtures();
    let messages = match n {
        50 => &fx.small,
        500 => &fx.medium,
        _ => &fx.large,
    };

    bencher.bench(|| divan::black_box(cdt_analyze::pair_tool_executions(messages)));
}

// --- check_messages_ongoing ---

#[divan::bench(args = [50, 500, 2000])]
fn check_messages_ongoing(bencher: divan::Bencher<'_, '_>, n: usize) {
    let fx = fixtures();
    let messages = match n {
        50 => &fx.small,
        500 => &fx.medium,
        _ => &fx.large,
    };

    bencher.bench(|| divan::black_box(cdt_analyze::check_messages_ongoing(messages)));
}

// --- Fixture generation via cdt_parse (sync entry-by-entry) ---

fn parse_synthetic_session(turns: usize) -> Vec<ParsedMessage> {
    let jsonl = build_session_jsonl(turns);
    let mut messages = Vec::with_capacity(turns * 3);
    for (i, line) in jsonl.lines().enumerate() {
        if let Ok(Some(msg)) = cdt_parse::parse_entry_at(line, i) {
            messages.push(msg);
        }
    }
    messages
}

fn build_session_jsonl(turns: usize) -> String {
    let mut out = String::with_capacity(turns * 600);
    for i in 0..turns {
        writeln!(
            out,
            r#"{{"type":"user","uuid":"u-{i}","timestamp":"2026-05-16T10:{min:02}:{sec:02}Z","cwd":"/workspace","message":{{"role":"user","content":"Question {i}"}}}}"#,
            min = (i * 3 / 60) % 60,
            sec = (i * 3) % 60,
        )
        .expect("write");

        let tool_use = format!(
            r#"{{"type":"tool_use","id":"tu-{i}","name":"Bash","input":{{"command":"echo {i}"}}}}"#,
        );
        writeln!(
            out,
            r#"{{"type":"assistant","uuid":"a-{i}","timestamp":"2026-05-16T10:{min:02}:{sec:02}Z","cwd":"/workspace","message":{{"role":"assistant","model":"claude-opus-4-7","content":[{{"type":"text","text":"Answer {i}"}},{tool_use}],"usage":{{"input_tokens":{input},"output_tokens":{output}}}}}}}"#,
            min = ((i * 3 + 1) / 60) % 60,
            sec = (i * 3 + 1) % 60,
            input = 100 + i,
            output = 20 + i % 10,
        )
        .expect("write");

        writeln!(
            out,
            r#"{{"type":"user","uuid":"tr-{i}","timestamp":"2026-05-16T10:{min:02}:{sec:02}Z","cwd":"/workspace","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"tu-{i}","content":"output {i}","is_error":false}}]}}}}"#,
            min = ((i * 3 + 2) / 60) % 60,
            sec = (i * 3 + 2) % 60,
        )
        .expect("write");
    }
    out
}
