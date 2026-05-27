//! JSONL 解析层性能基准。
//!
//! 跑法：`cargo bench -p cdt-parse`

use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::OnceLock;

use tempfile::TempDir;

struct Fixtures {
    _dir: TempDir,
    small: PathBuf,
    medium: PathBuf,
    large: PathBuf,
    large_with_dupes: PathBuf,
}

fn fixtures() -> &'static Fixtures {
    static FX: OnceLock<Fixtures> = OnceLock::new();
    FX.get_or_init(|| {
        let dir = tempfile::tempdir().expect("create tempdir");

        let small = dir.path().join("small.jsonl");
        std::fs::write(&small, build_session_jsonl(50, false)).expect("write small");

        let medium = dir.path().join("medium.jsonl");
        std::fs::write(&medium, build_session_jsonl(500, false)).expect("write medium");

        let large = dir.path().join("large.jsonl");
        std::fs::write(&large, build_session_jsonl(5000, false)).expect("write large");

        let large_with_dupes = dir.path().join("large_dupes.jsonl");
        std::fs::write(&large_with_dupes, build_session_jsonl(5000, true)).expect("write dupes");

        Fixtures {
            _dir: dir,
            small,
            medium,
            large,
            large_with_dupes,
        }
    })
}

fn rt() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

fn main() {
    divan::main();
}

// --- parse_entry (sync, per-line) ---

#[divan::bench(args = [50, 500, 5000])]
fn parse_entry_lines(bencher: divan::Bencher<'_, '_>, n: usize) {
    let fx = fixtures();
    let path = match n {
        50 => &fx.small,
        500 => &fx.medium,
        _ => &fx.large,
    };
    let content = std::fs::read_to_string(path).expect("read fixture");
    let lines: Vec<&str> = content.lines().collect();

    bencher.bench(|| {
        let mut count = 0usize;
        for (i, line) in lines.iter().enumerate() {
            if let Ok(Some(_msg)) = cdt_parse::parse_entry_at(line, i) {
                count += 1;
            }
        }
        divan::black_box(count)
    });
}

// --- parse_file (async, full pipeline) ---

#[divan::bench(args = [50, 500, 5000])]
fn parse_file_async(bencher: divan::Bencher<'_, '_>, n: usize) {
    let fx = fixtures();
    let path = match n {
        50 => fx.small.clone(),
        500 => fx.medium.clone(),
        _ => fx.large.clone(),
    };

    bencher.bench(|| {
        rt().block_on(async {
            let messages = cdt_parse::parse_file(&path).await.expect("parse_file");
            divan::black_box(messages)
        })
    });
}

// --- dedupe_by_request_id ---

#[divan::bench(args = [500, 5000])]
fn dedupe_by_request_id(bencher: divan::Bencher<'_, '_>, n: usize) {
    let fx = fixtures();
    let path = &fx.large_with_dupes;
    let all_messages = rt().block_on(async { cdt_parse::parse_file(path).await.expect("parse") });
    let messages: Vec<_> = all_messages.into_iter().take(n).collect();

    bencher
        .with_inputs(|| messages.clone())
        .bench_values(|msgs| {
            divan::black_box(cdt_parse::dedupe_by_request_id(msgs))
        });
}

// --- Fixture generation ---

fn build_session_jsonl(turns: usize, with_dupes: bool) -> String {
    let mut out = String::with_capacity(turns * 400);
    for i in 0..turns {
        let request_id = if with_dupes {
            format!("req-{}", i / 3)
        } else {
            format!("req-{i}")
        };

        writeln!(
            out,
            r#"{{"type":"user","uuid":"u-{i}","timestamp":"2026-05-16T10:{min:02}:{sec:02}Z","cwd":"/workspace","message":{{"role":"user","content":"Question {i}"}}}}"#,
            min = (i * 2 / 60) % 60,
            sec = (i * 2) % 60,
        )
        .expect("write");

        writeln!(
            out,
            r#"{{"type":"assistant","uuid":"a-{i}","timestamp":"2026-05-16T10:{min:02}:{sec:02}Z","cwd":"/workspace","requestId":"{request_id}","message":{{"role":"assistant","model":"claude-opus-4-7","content":[{{"type":"text","text":"Answer {i}"}}],"usage":{{"input_tokens":{input},"output_tokens":{output}}}}}}}"#,
            min = ((i * 2 + 1) / 60) % 60,
            sec = (i * 2 + 1) % 60,
            input = 100 + i,
            output = 20 + i % 10,
        )
        .expect("write");
    }
    out
}
