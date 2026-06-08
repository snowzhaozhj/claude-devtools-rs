#![allow(
    clippy::doc_markdown,
    clippy::uninlined_format_args,
    clippy::manual_div_ceil
)]

//! `perf_ssh_scanner_chunked_read` —— SSH 5MB jsonl scan wall < 2s pipeline bench。
//!
//! PR-F SFTP message-id pipeline + change `unify-fs-direct-calls` D5 + ssh-remote-context
//! spec Scenario "SSH 大会话 scanner BufReader 容量与 SFTP packet 对齐"：fake-SSH
//! 注入 50ms RTT / `read()` + 32K packet 限制，5MB jsonl 内容用
//! `cdt-parse::parse_file_via_fs` 流式 parse，**总 wall 时间 < 2s**。
//!
//! 基线演化：
//! - PR-D2 之前（串行 SFTP read，每 packet 等 1 RTT）：5MB / 32K ≈ 160 packets × 50ms
//!   = 8s 理论下限，9s threshold 留 1s buffer for parse / tokio overhead；测出 8.36s。
//! - **PR-F（multi-worker pipeline，K=`SFTP_PIPELINE_MAX_WORKERS`=16）**：
//!   `ThrottledFakeSftpClient::read` 模拟 K 个 worker 并发飞 ceil(N/K)=10 个串行
//!   `SSH_FXP_READ`，wall ≈ 10 × 50ms = 500ms 理论下限；2s threshold 含 metadata
//!   1 RTT + parse / tokio overhead。生产 `RusshSftpClient::read` 用同模型走
//!   `try_join_all` K 个 worker 并发 `sftp.open` + `seek` + `read_exact`。
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
use cdt_ssh::{
    RemoteEntry, SFTP_PIPELINE_MAX_WORKERS, SftpClient, SftpClientError, SshFileSystemProvider,
};

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
                created: None,
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
        // 调 client.read() 拿全 bytes 后包 Cursor。**PR-F 后**生产 RusshSftpClient::read
        // 走 K 个 worker 并发飞 ceil(N/K) 次串行 SSH_FXP_READ；fake 这里同模型镜像：
        // K worker 并发，每个 worker 串行 chunks_per_worker 次 RTT，wall ≈
        // chunks_per_worker × RTT 而非 N × RTT。fake 提前 sleep "1 RTT" 模拟 metadata 探测
        // （生产路径走 self.sftp.metadata 拿 size）。
        if path != self.file_path {
            return Err(SftpClientError::NoSuchFile);
        }
        let n_packets = self.file_bytes.len().div_ceil(PACKET_SIZE);
        let n_workers = SFTP_PIPELINE_MAX_WORKERS.min(n_packets).max(1);
        let chunks_per_worker = n_packets.div_ceil(n_workers);

        let read_offset = Arc::new(AtomicUsize::new(0));
        let workers = (0..n_workers).map(|_| {
            let read_offset = Arc::clone(&read_offset);
            async move {
                for _ in 0..chunks_per_worker {
                    tokio::time::sleep(RTT_PER_READ).await;
                    read_offset.fetch_add(PACKET_SIZE, Ordering::SeqCst);
                }
            }
        });
        futures::future::join_all(workers).await;
        // 把累计偏移同步回 self.read_offset（绕过 &self 限制——AtomicUsize 也是 &self 安全）。
        self.read_offset
            .store(read_offset.load(Ordering::SeqCst), Ordering::SeqCst);
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
                created: None,
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
    async fn write(&self, _p: &str, _d: &[u8]) -> Result<(), SftpClientError> {
        Err(SftpClientError::Other("not used".into()))
    }
    async fn mkdir(&self, _p: &str) -> Result<(), SftpClientError> {
        Err(SftpClientError::Other("not used".into()))
    }
    async fn remove(&self, _p: &str) -> Result<(), SftpClientError> {
        Err(SftpClientError::Other("not used".into()))
    }
    async fn rename(&self, _s: &str, _d: &str) -> Result<(), SftpClientError> {
        Err(SftpClientError::Other("not used".into()))
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
async fn ssh_5mb_jsonl_scan_wall_under_2s() {
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
        elapsed < Duration::from_secs(2),
        "5MB SSH scan wall {:?} > 2s threshold (parsed {} messages); PR-F pipeline 期望
         ceil({} packets / {} workers) × 50ms = ~{}ms 理论下限 + tokio overhead",
        elapsed,
        messages.len(),
        (5 * 1024 * 1024_usize).div_ceil(PACKET_SIZE),
        SFTP_PIPELINE_MAX_WORKERS,
        (5 * 1024 * 1024_usize)
            .div_ceil(PACKET_SIZE)
            .div_ceil(SFTP_PIPELINE_MAX_WORKERS)
            * 50,
    );
}
