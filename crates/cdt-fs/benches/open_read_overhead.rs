//! `open_read_overhead` —— 量化 `FileSystemProvider::open_read` 的
//! `Box<dyn AsyncRead + Send + Unpin>` 动态分发相对直读 `tokio::fs::File`
//! 的 overhead（design D4 / codex 第二轮 Medium #4 量化要求）。
//!
//! 对比同一 jsonl fixture 走两条读取路径：
//! - (A) `tokio::fs::File::open + BufReader::lines` 直读
//! - (B) `LocalFileSystemProvider::open_read` 拿到 `Box<dyn AsyncRead + Send + Unpin>`
//!   后包 `BufReader::lines`
//!
//! Fixture：
//! - `small.jsonl` ~500KB（5000 行）
//! - `large.jsonl` ~5MB（50000 行）
//!
//! 验收：dyn 路径 median ≤ 直读 × 1.3（见 `tasks.md` §11.10）。

use cdt_fs::{FileSystemProvider, LocalFileSystemProvider};
use criterion::{Criterion, criterion_group, criterion_main};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::OnceLock;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, BufReader};

/// 单一 fixture 目录，整个 bench 进程内复用，避免重复写 5MB 文件。
///
/// `TempDir` 在进程退出时 RAII 清理；`OnceLock` 防止 criterion 反复进 setup。
struct Fixtures {
    _dir: TempDir,
    small: PathBuf,
    large: PathBuf,
}

fn fixtures() -> &'static Fixtures {
    static FIXTURES: OnceLock<Fixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let dir = tempfile::tempdir().expect("create tempdir");
        let small = dir.path().join("small.jsonl");
        let large = dir.path().join("large.jsonl");

        // 同步写——bench 启动 setup 阶段，简单胜过 async。
        let mut s = String::with_capacity(550_000);
        for i in 0..5_000 {
            writeln!(s, "{{\"role\":\"user\",\"content\":\"line {i}\"}}")
                .expect("write to String never fails");
        }
        std::fs::write(&small, &s).expect("write small fixture");

        let mut l = String::with_capacity(5_500_000);
        for i in 0..50_000 {
            writeln!(l, "{{\"role\":\"user\",\"content\":\"line {i}\"}}")
                .expect("write to String never fails");
        }
        std::fs::write(&large, &l).expect("write large fixture");

        Fixtures {
            _dir: dir,
            small,
            large,
        }
    })
}

/// 直读路径：`tokio::fs::File` 单态，无 trait 动态分发。
async fn direct_read(path: &std::path::Path) -> usize {
    let file = tokio::fs::File::open(path).await.expect("open file");
    let mut reader = BufReader::new(file).lines();
    let mut count: usize = 0;
    while let Some(_line) = reader.next_line().await.expect("read line") {
        count += 1;
    }
    count
}

/// dyn 路径：`FileSystemProvider::open_read` 返 `Box<dyn AsyncRead + Send + Unpin>`。
async fn dyn_read(fs: &LocalFileSystemProvider, path: &std::path::Path) -> usize {
    let reader = fs.open_read(path).await.expect("open_read");
    let mut lines = BufReader::new(reader).lines();
    let mut count: usize = 0;
    while let Some(_line) = lines.next_line().await.expect("read line") {
        count += 1;
    }
    count
}

fn bench_open_read(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let fx = fixtures();
    let fs = LocalFileSystemProvider::new();

    // sample_size 10 与 tasks.md §11.10 "跑 10 次" 对齐——criterion 默认 100，
    // 这里压到 10 是因为大文件 5MB 单次 ~ms 级，10 次足够算 min / median / stddev。
    let mut group = c.benchmark_group("open_read_overhead");
    group.sample_size(10);

    group.bench_function("direct_read_small", |b| {
        b.to_async(&rt).iter(|| async {
            let n = direct_read(&fx.small).await;
            std::hint::black_box(n);
        });
    });

    group.bench_function("dyn_read_small", |b| {
        b.to_async(&rt).iter(|| async {
            let n = dyn_read(&fs, &fx.small).await;
            std::hint::black_box(n);
        });
    });

    group.bench_function("direct_read_large", |b| {
        b.to_async(&rt).iter(|| async {
            let n = direct_read(&fx.large).await;
            std::hint::black_box(n);
        });
    });

    group.bench_function("dyn_read_large", |b| {
        b.to_async(&rt).iter(|| async {
            let n = dyn_read(&fs, &fx.large).await;
            std::hint::black_box(n);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_open_read);
criterion_main!(benches);
