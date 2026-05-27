//! `open_read_overhead` — 量化 `FileSystemProvider::open_read` 的
//! `Box<dyn AsyncRead + Send + Unpin>` 动态分发相对直读 `tokio::fs::File`
//! 的 overhead。
//!
//! 验收：dyn 路径 median ≤ 直读 × 1.3。

use cdt_fs::{FileSystemProvider, LocalFileSystemProvider};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::OnceLock;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, BufReader};

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

fn rt() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

async fn direct_read(path: &std::path::Path) -> usize {
    let file = tokio::fs::File::open(path).await.expect("open file");
    let mut reader = BufReader::new(file).lines();
    let mut count: usize = 0;
    while let Some(_line) = reader.next_line().await.expect("read line") {
        count += 1;
    }
    count
}

async fn dyn_read(fs: &LocalFileSystemProvider, path: &std::path::Path) -> usize {
    let reader = fs.open_read(path).await.expect("open_read");
    let mut lines = BufReader::new(reader).lines();
    let mut count: usize = 0;
    while let Some(_line) = lines.next_line().await.expect("read line") {
        count += 1;
    }
    count
}

fn main() {
    divan::main();
}

#[divan::bench]
fn direct_read_small(bencher: divan::Bencher<'_, '_>) {
    let fx = fixtures();
    bencher.bench(|| rt().block_on(direct_read(&fx.small)));
}

#[divan::bench]
fn dyn_read_small(bencher: divan::Bencher<'_, '_>) {
    let fx = fixtures();
    let fs = LocalFileSystemProvider::new();
    bencher.bench(|| rt().block_on(dyn_read(&fs, &fx.small)));
}

#[divan::bench]
fn direct_read_large(bencher: divan::Bencher<'_, '_>) {
    let fx = fixtures();
    bencher.bench(|| rt().block_on(direct_read(&fx.large)));
}

#[divan::bench]
fn dyn_read_large(bencher: divan::Bencher<'_, '_>) {
    let fx = fixtures();
    let fs = LocalFileSystemProvider::new();
    bencher.bench(|| rt().block_on(dyn_read(&fs, &fx.large)));
}
