#![allow(clippy::doc_markdown, clippy::cast_possible_truncation)]

//! Counted `FakeRemoteSftp` helper —— 仅供 `perf_ssh_cache_hit.rs` 引入。
//!
//! 与 `ipc_contract.rs::FakeRemoteSftp` 与 `ssh_reconnect_lifecycle.rs::FakeRemoteSftp`
//! 是独立 inline 副本；本 helper 含 `Arc<AtomicUsize>` op counter（`metadata` /
//! `read` / `read_dir` / `read_lines_head` / `try_exists`）用于断言 SSH 路径
//! 的真实 fs op 形态。
//!
//! `ipc_contract.rs` 副本加 counter 的 PR 见 `openspec/followups.md`
//! `[coverage-gap] active context dispatch contract test 缺 read 计数器`，与
//! 本 PR scope 解耦——本 helper 仅供本 PR 的 perf bench 使用。
//!
//! 详 change `ssh-batch-readdir-with-metadata` design D4。
//!
//! Integration test 跨文件共享 helper 用 `#[path]` 引入：
//! ```rust,ignore
//! #[path = "fake_remote_sftp.rs"]
//! mod fake_remote_sftp;
//! use fake_remote_sftp::CountedFakeRemoteSftp;
//! ```

#![allow(dead_code)] // 仅供 perf_ssh_cache_hit.rs 引入，其它测试文件不用。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use cdt_discover::{EntryKind, FsMetadata};
use cdt_ssh::{RemoteEntry, SftpClient, SftpClientError};

#[derive(Default)]
pub struct CountedFakeRemoteSftp {
    files: HashMap<String, Vec<u8>>,
    dirs: HashMap<String, Vec<RemoteEntry>>,
    pub metadata_count: Arc<AtomicUsize>,
    pub try_exists_count: Arc<AtomicUsize>,
    pub read_count: Arc<AtomicUsize>,
    pub read_dir_count: Arc<AtomicUsize>,
    pub read_lines_head_count: Arc<AtomicUsize>,
}

impl CountedFakeRemoteSftp {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一条 session：写 `<remote_home>/<project_id>/<session_id>.jsonl`，
    /// 并把 project_dir 的 dir entry 与 remote_home 的 project entry 填充。
    pub fn with_session(
        remote_home: &str,
        project_id: &str,
        session_id: &str,
        content: String,
    ) -> Self {
        let mut fake = Self::default();
        fake.add_session(remote_home, project_id, session_id, content);
        fake
    }

    /// 在已有 fake 上追加一条 session（同 project_id 多次调用支持多 session）。
    pub fn add_session(
        &mut self,
        remote_home: &str,
        project_id: &str,
        session_id: &str,
        content: String,
    ) {
        let project_dir = format!("{remote_home}/{project_id}");
        let file_path = format!("{project_dir}/{session_id}.jsonl");
        let size = content.len() as u64;
        let mtime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000);

        // remote_home → 含 project dir entry
        let home_entries = self.dirs.entry(remote_home.to_owned()).or_default();
        if !home_entries.iter().any(|e| e.name == project_id) {
            home_entries.push(RemoteEntry {
                name: project_id.to_owned(),
                kind: EntryKind::Dir,
                metadata: None,
                mtime_missing: false,
            });
        }

        // project_dir → 含 session file entry（含真 mtime）
        self.dirs
            .entry(project_dir.clone())
            .or_default()
            .push(RemoteEntry {
                name: format!("{session_id}.jsonl"),
                kind: EntryKind::File,
                metadata: Some(FsMetadata {
                    size,
                    mtime,
                    identity: None,
                }),
                mtime_missing: false,
            });

        self.files.insert(file_path, content.into_bytes());
    }

    /// Snapshot 所有 counter 当前值，便于测试做 before/after diff。
    pub fn snapshot_counters(&self) -> FakeCounters {
        FakeCounters {
            metadata: self.metadata_count.load(Ordering::SeqCst),
            try_exists: self.try_exists_count.load(Ordering::SeqCst),
            read: self.read_count.load(Ordering::SeqCst),
            read_dir: self.read_dir_count.load(Ordering::SeqCst),
            read_lines_head: self.read_lines_head_count.load(Ordering::SeqCst),
        }
    }
}

/// 不变的 counter 快照。用于 `assert_eq!(before, after)` 检查"零新增 op"语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FakeCounters {
    pub metadata: usize,
    pub try_exists: usize,
    pub read: usize,
    pub read_dir: usize,
    pub read_lines_head: usize,
}

#[async_trait]
impl SftpClient for CountedFakeRemoteSftp {
    async fn metadata(&self, path: &str) -> Result<FsMetadata, SftpClientError> {
        self.metadata_count.fetch_add(1, Ordering::SeqCst);
        if let Some(bytes) = self.files.get(path) {
            Ok(FsMetadata {
                size: bytes.len() as u64,
                mtime: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000),
                identity: None,
            })
        } else if self.dirs.contains_key(path) {
            Ok(FsMetadata {
                size: 0,
                mtime: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000),
                identity: None,
            })
        } else {
            Err(SftpClientError::NoSuchFile)
        }
    }

    async fn try_exists(&self, path: &str) -> Result<bool, SftpClientError> {
        self.try_exists_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.files.contains_key(path) || self.dirs.contains_key(path))
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, SftpClientError> {
        self.read_count.fetch_add(1, Ordering::SeqCst);
        self.files
            .get(path)
            .cloned()
            .ok_or(SftpClientError::NoSuchFile)
    }

    async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
        self.read_dir_count.fetch_add(1, Ordering::SeqCst);
        self.dirs
            .get(path)
            .cloned()
            .ok_or(SftpClientError::NoSuchFile)
    }

    async fn read_lines_head(
        &self,
        path: &str,
        max: usize,
    ) -> Result<Vec<String>, SftpClientError> {
        self.read_lines_head_count.fetch_add(1, Ordering::SeqCst);
        let bytes = self
            .files
            .get(path)
            .cloned()
            .ok_or(SftpClientError::NoSuchFile)?;
        let content =
            String::from_utf8(bytes).map_err(|e| SftpClientError::Other(e.to_string()))?;
        Ok(content.lines().take(max).map(ToOwned::to_owned).collect())
    }
}
