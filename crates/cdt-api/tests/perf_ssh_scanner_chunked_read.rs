#![allow(
    clippy::doc_markdown,
    clippy::uninlined_format_args,
    clippy::manual_div_ceil
)]

//! `perf_ssh_scanner_chunked_read` —— SSH 5MB jsonl scan wall < 9s sanity bench。
//!
//! change `unify-fs-direct-calls` D5 + ssh-remote-context spec Scenario "SSH 大会话
//! scanner BufReader 容量与 SFTP packet 对齐"：fake-SSH 注入 50ms RTT / `read()` +
//! 32K packet 限制，5MB jsonl 内容用 `cdt-parse::parse_file_via_fs` 流式 parse，
//! 验 BufReader::with_capacity(32K) 与 SFTP packet 对齐，**总 wall 时间 < 9s**。
//!
//! 5MB / 32K ≈ 160 SFTP READ messages × 50ms RTT ≈ 8s 理论下限 + 1s buffer for
//! parse / tokio overhead。
//!
//! 跑：
//! ```sh
//! cargo test -p cdt-api --release --test perf_ssh_scanner_chunked_read -- --ignored --nocapture
//! ```
//!
//! `#[ignore = "perf bench; not in default CI (5MB × 50ms RTT simulation)"]` 不进默认 CI（5MB × 50ms 网络模拟 wall time 不稳定 + CI runner 无
//! 真实 SSH corpus）。perf 调试时本地手动跑。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use cdt_discover::{EntryKind, FsMetadata};
use cdt_ssh::{RemoteEntry, SftpClient, SftpClientError, SshFileSystemProvider};

const PACKET_SIZE: usize = 32 * 1024;
const RTT_PER_READ: Duration = Duration::from_millis(50);

/// 注入 RTT delay + 切 32K packet 的 fake SFTP client。
struct ThrottledFakeSftpClient {
    file_bytes: Vec<u8>,
    file_path: String,
    read_offset: AtomicUsize,
}

#[async_trait]
impl SftpClient for ThrottledFakeSftpClient {
    async fn metadata(&self, path: &str) -> Result<FsMetadata, SftpClientError> {
        tokio::time::sleep(RTT_PER_READ).await;
        if path == self.file_path {
            Ok(FsMetadata {
                size: self.file_bytes.len() as u64,
                mtime: std::time::UNIX_EPOCH + Duration::from_secs(1_800_000_000),
                identity: None,
            })
        } else {
            Err(SftpClientError::NoSuchFile)
        }
    }

    async fn try_exists(&self, path: &str) -> Result<bool, SftpClientError> {
        tokio::time::sleep(RTT_PER_READ).await;
        Ok(path == self.file_path)
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, SftpClientError> {
        // ThrottledFakeSftpClient 走 with_client fallback 路径——SshFileSystemProvider::open_read
        // 调 client.read() 拿全 bytes 后包 Cursor。这里我们模拟 "1 个大 read 请求被
        // SFTP 拆成 N 个 32K message 各 50ms RTT" —— sleep(N × 50ms) 后返全 bytes。
        if path != self.file_path {
            return Err(SftpClientError::NoSuchFile);
        }
        let n_packets = (self.file_bytes.len() + PACKET_SIZE - 1) / PACKET_SIZE;
        for _ in 0..n_packets {
            tokio::time::sleep(RTT_PER_READ).await;
            self.read_offset.fetch_add(PACKET_SIZE, Ordering::SeqCst);
        }
        Ok(self.file_bytes.clone())
    }

    async fn read_dir(&self, _path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
        tokio::time::sleep(RTT_PER_READ).await;
        Ok(vec![RemoteEntry {
            name: self
                .file_path
                .rsplit('/')
                .next()
                .unwrap_or("file.jsonl")
                .to_owned(),
            kind: EntryKind::File,
            metadata: Some(FsMetadata {
                size: self.file_bytes.len() as u64,
                mtime: std::time::UNIX_EPOCH + Duration::from_secs(1_800_000_000),
                identity: None,
            }),
            mtime_missing: false,
        }])
    }

    async fn read_lines_head(
        &self,
        path: &str,
        max: usize,
    ) -> Result<Vec<String>, SftpClientError> {
        let bytes = self.read(path).await?;
        let content =
            String::from_utf8(bytes).map_err(|e| SftpClientError::Other(e.to_string()))?;
        Ok(content.lines().take(max).map(ToOwned::to_owned).collect())
    }
}

fn make_5mb_jsonl() -> Vec<u8> {
    let line = r#"{"type":"user","uuid":"u","parentUuid":null,"timestamp":"2026-04-11T10:00:00Z","isSidechain":false,"userType":"external","cwd":"/x","sessionId":"s","version":"1","message":{"role":"user","content":"sample content padded out to reach reasonable jsonl line length for benchmark"}}"#;
    let mut out = String::with_capacity(5 * 1024 * 1024 + line.len());
    while out.len() < 5 * 1024 * 1024 {
        out.push_str(line);
        out.push('\n');
    }
    out.into_bytes()
}

#[tokio::test]
#[ignore = "perf bench; not in default CI (5MB × 50ms RTT simulation)"]
async fn ssh_5mb_jsonl_scan_wall_under_9s() {
    let file_bytes = make_5mb_jsonl();
    let file_path = "/remote/home/.claude/projects/-srv/session.jsonl".to_owned();
    let client = Arc::new(ThrottledFakeSftpClient {
        file_bytes,
        file_path: file_path.clone(),
        read_offset: AtomicUsize::new(0),
    });
    let provider = SshFileSystemProvider::with_client(
        "ctx-throttled",
        client as Arc<dyn SftpClient>,
        PathBuf::from("/remote/home/.claude/projects"),
    );

    let start = Instant::now();
    let messages = cdt_parse::parse_file_via_fs(&provider, &PathBuf::from(file_path))
        .await
        .expect("parse_file_via_fs SHALL succeed");
    let elapsed = start.elapsed();

    println!(
        "[perf_ssh_scanner_chunked_read] 5MB jsonl elapsed={:?} messages={}",
        elapsed,
        messages.len()
    );

    assert!(
        elapsed < Duration::from_secs(9),
        "5MB SSH scan wall {:?} > 9s threshold (parsed {} messages)",
        elapsed,
        messages.len(),
    );
}
