#![allow(clippy::doc_markdown, clippy::uninlined_format_args, clippy::ptr_arg)]

//! `perf_scanner_open_read` —— scanner dyn `AsyncRead` vs direct `tokio::fs::File` 微基准。
//!
//! change `unify-fs-direct-calls` §12 D1 micro-bench：scanner 切 `Box<dyn AsyncRead>`
//! 后对 Local NVMe 端的开销量化保护——dyn dispatch + heap allocation + 32K BufReader
//! 需 ≤ direct × 1.3 否则视为退化打回 PR-D 既有 cache 命中收益。
//!
//! 跑：
//! ```sh
//! cargo test -p cdt-api --release --test perf_scanner_open_read -- --ignored --nocapture
//! ```
//!
//! 验收：candidate median ≤ baseline median × 1.3（vtable + heap alloc + buffer ≤ 30%
//! 退化）；超出 panic 打 baseline/candidate 数据点。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cdt_fs::{FileSystemProvider, LocalFileSystemProvider};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, BufReader};

const SCANNER_BUF_BYTES: usize = 32 * 1024;
const RUNS: usize = 5;
const ACCEPTANCE_RATIO: f64 = 1.3;

async fn write_jsonl_fixture(dir: &TempDir, name: &str, bytes_target: usize) -> PathBuf {
    let path = dir.path().join(name);
    let line = r#"{"type":"user","uuid":"u","parentUuid":null,"timestamp":"2026-04-11T10:00:00Z","isSidechain":false,"userType":"external","cwd":"/x","sessionId":"s","version":"1","message":{"role":"user","content":"sample text for benchmark purposes only padded with words to reach reasonable line length"}}"#;
    let mut content = String::with_capacity(bytes_target + line.len());
    while content.len() < bytes_target {
        content.push_str(line);
        content.push('\n');
    }
    tokio::fs::write(&path, &content).await.unwrap();
    path
}

async fn run_baseline(path: &PathBuf) -> Duration {
    let start = Instant::now();
    let file = tokio::fs::File::open(path).await.unwrap();
    let mut buf = BufReader::new(file);
    let mut line = String::new();
    let mut count = 0usize;
    loop {
        line.clear();
        let n = buf.read_line(&mut line).await.unwrap();
        if n == 0 {
            break;
        }
        count += 1;
    }
    std::hint::black_box(count);
    start.elapsed()
}

async fn run_candidate(provider: &dyn FileSystemProvider, path: &PathBuf) -> Duration {
    let start = Instant::now();
    let reader = provider.open_read(path).await.unwrap();
    let mut buf = BufReader::with_capacity(SCANNER_BUF_BYTES, reader);
    let mut line = String::new();
    let mut count = 0usize;
    loop {
        line.clear();
        let n = buf.read_line(&mut line).await.unwrap();
        if n == 0 {
            break;
        }
        count += 1;
    }
    std::hint::black_box(count);
    start.elapsed()
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

async fn bench_size(label: &str, target_bytes: usize) {
    let tmp = TempDir::new().unwrap();
    let path = write_jsonl_fixture(&tmp, &format!("{label}.jsonl"), target_bytes).await;
    let provider: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());

    // warm-up：跑 1 次让 page cache 暖
    let _ = run_baseline(&path).await;
    let _ = run_candidate(&*provider, &path).await;

    let mut baseline = Vec::with_capacity(RUNS);
    let mut candidate = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        baseline.push(run_baseline(&path).await);
        candidate.push(run_candidate(&*provider, &path).await);
    }

    let b_median = median(baseline.clone());
    let c_median = median(candidate.clone());
    let ratio = c_median.as_secs_f64() / b_median.as_secs_f64().max(1e-9);

    println!(
        "[perf_scanner_open_read] size={label} baseline_median={:?} candidate_median={:?} ratio={:.2} \n  baseline_samples={:?}\n  candidate_samples={:?}",
        b_median, c_median, ratio, baseline, candidate
    );

    assert!(
        ratio <= ACCEPTANCE_RATIO,
        "scanner dyn AsyncRead {label} median {:?} > {:.1}× baseline {:?} ({} runs each)",
        c_median,
        ACCEPTANCE_RATIO,
        b_median,
        RUNS,
    );
}

#[tokio::test]
#[ignore = "perf bench; not in default CI"]
async fn scanner_dyn_async_read_overhead_within_threshold_500kb() {
    bench_size("500KB", 500 * 1024).await;
}

#[tokio::test]
#[ignore = "perf bench; not in default CI"]
async fn scanner_dyn_async_read_overhead_within_threshold_5mb() {
    bench_size("5MB", 5 * 1024 * 1024).await;
}
