//! 路径编解码 + 项目名提取吞吐基准。
//!
//! 跑法：`cargo bench -p cdt-discover`

use std::path::Path;
use std::sync::OnceLock;

use cdt_discover::{decode_path, encode_path, extract_project_name, is_valid_encoded_path};

struct Fixtures {
    unix: Vec<&'static str>,
    windows: Vec<&'static str>,
    encoded: Vec<String>,
}

fn fixtures() -> &'static Fixtures {
    static FX: OnceLock<Fixtures> = OnceLock::new();
    FX.get_or_init(|| {
        let unix = vec![
            "/Users/alice/Projects/my-app",
            "/home/bob/workspace/rust-project/src",
            "/var/lib/claude/sessions/project-abc",
            "/Users/charlie/Documents/Work/Company/frontend-monorepo",
            "/tmp/build-artifacts/release-v2",
        ];
        let windows = vec![
            r"C:\Users\alice\Projects\my-app",
            r"D:\workspace\rust-project\src",
            r"C:\Program Files\Claude\sessions\project-abc",
            r"C:\Users\charlie\Documents\Work\Company\frontend-monorepo",
            r"\\server\share\builds\release-v2",
        ];
        let encoded: Vec<String> = unix.iter().map(|p| encode_path(p)).collect();
        Fixtures {
            unix,
            windows,
            encoded,
        }
    })
}

fn main() {
    divan::main();
}

// --- encode_path ---

#[divan::bench(args = [100, 1000, 10000])]
fn encode_path_throughput(bencher: divan::Bencher<'_, '_>, n: usize) {
    let fx = fixtures();
    let paths = &fx.unix;

    bencher.bench(|| {
        let mut count = 0usize;
        for _ in 0..n {
            for path in paths {
                divan::black_box(encode_path(path));
                count += 1;
            }
        }
        count
    });
}

// --- decode_path ---

#[divan::bench(args = [100, 1000, 10000])]
fn decode_path_throughput(bencher: divan::Bencher<'_, '_>, n: usize) {
    let fx = fixtures();
    let encoded = &fx.encoded;

    bencher.bench(|| {
        let mut count = 0usize;
        for _ in 0..n {
            for enc in encoded {
                divan::black_box(decode_path(enc));
                count += 1;
            }
        }
        count
    });
}

// --- encode + decode roundtrip ---

#[divan::bench(args = [100, 1000])]
fn encode_decode_roundtrip(bencher: divan::Bencher<'_, '_>, n: usize) {
    let fx = fixtures();
    let all_paths: Vec<&str> = fx.unix.iter().chain(fx.windows.iter()).copied().collect();

    bencher.bench(|| {
        let mut count = 0usize;
        for _ in 0..n {
            for path in &all_paths {
                let encoded = encode_path(path);
                divan::black_box(decode_path(&encoded));
                count += 1;
            }
        }
        count
    });
}

// --- is_valid_encoded_path ---

#[divan::bench(args = [1000, 10000])]
fn validate_encoded_path(bencher: divan::Bencher<'_, '_>, n: usize) {
    let fx = fixtures();
    let encoded = &fx.encoded;

    bencher.bench(|| {
        let mut valid = 0usize;
        for _ in 0..n {
            for enc in encoded {
                if is_valid_encoded_path(enc) {
                    valid += 1;
                }
            }
        }
        divan::black_box(valid)
    });
}

// --- extract_project_name ---

#[divan::bench(args = [1000, 10000])]
fn extract_project_name_throughput(bencher: divan::Bencher<'_, '_>, n: usize) {
    let paths: Vec<&Path> = vec![
        Path::new("/Users/alice/Projects/my-app"),
        Path::new("/home/bob/workspace/rust-project"),
        Path::new("/var/lib/claude/sessions/project-abc"),
    ];

    bencher.bench(|| {
        let mut count = 0usize;
        for _ in 0..n {
            for path in &paths {
                divan::black_box(extract_project_name(path));
                count += 1;
            }
        }
        count
    });
}
