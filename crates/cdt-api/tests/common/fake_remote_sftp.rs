#![allow(clippy::doc_markdown, clippy::cast_possible_truncation)]

//! Counted `FakeRemoteSftp` helper —— 跨 integration test 共享。
//!
//! 含 `Arc<AtomicUsize>` op counter（read 路径 5 个 + write 路径 4 个）用于断言
//! SSH 路径的真实 fs op 形态。
//!
//! 现有 consumer：
//! - `perf_ssh_cache_hit.rs` —— SSH cache hit hot path 零 fs op 形态守护
//! - `ipc_contract.rs` —— `active_ssh_context_reads_remote_projects_and_sessions`
//!   每个 IPC method 调用后断言至少一次远端 fs op，防止退化为 local fs 的假阳性
//! - `ssh_memory_crud.rs` —— SSH 远端 memory CRUD 端到端（change `ssh-project-memory-remote-rw`）
//!
//! 历史背景：`ssh_reconnect_lifecycle.rs` 仍是独立 inline 副本，未接入 counter；
//! 那条路径核心是 reconnect lifecycle 不是 fs op 守护，迁移收益低。
//!
//! Integration test 跨文件共享 helper 用 `#[path]` 引入：
//! ```rust,ignore
//! #[path = "fake_remote_sftp.rs"]
//! mod fake_remote_sftp;
//! use fake_remote_sftp::{CountedFakeRemoteSftp, FakeCounters};
//! ```

#![allow(dead_code)] // 各 consumer 用到的 helper 子集不同（new / add_session 等仅 perf 用）。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use cdt_discover::{EntryKind, FsMetadata};
use cdt_ssh::{RemoteEntry, SftpClient, SftpClientError};

#[derive(Default)]
pub struct CountedFakeRemoteSftp {
    files: Mutex<HashMap<String, Vec<u8>>>,
    dirs: Mutex<HashMap<String, Vec<RemoteEntry>>>,
    pub metadata_count: Arc<AtomicUsize>,
    pub try_exists_count: Arc<AtomicUsize>,
    pub read_count: Arc<AtomicUsize>,
    pub read_dir_count: Arc<AtomicUsize>,
    pub read_lines_head_count: Arc<AtomicUsize>,
    pub write_count: Arc<AtomicUsize>,
    pub mkdir_count: Arc<AtomicUsize>,
    pub remove_count: Arc<AtomicUsize>,
    pub rename_count: Arc<AtomicUsize>,
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
        let fake = Self::default();
        fake.add_session(remote_home, project_id, session_id, content);
        fake
    }

    /// 在已有 fake 上追加一条 session（同 project_id 多次调用支持多 session）。
    pub fn add_session(
        &self,
        remote_home: &str,
        project_id: &str,
        session_id: &str,
        content: String,
    ) {
        let project_dir = format!("{remote_home}/{project_id}");
        let file_path = format!("{project_dir}/{session_id}.jsonl");
        let size = content.len() as u64;
        let mtime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000);

        let mut dirs = self
            .dirs
            .lock()
            .expect("CountedFakeRemoteSftp dirs poisoned");
        // remote_home → 含 project dir entry
        let home_entries = dirs.entry(remote_home.to_owned()).or_default();
        if !home_entries.iter().any(|e| e.name == project_id) {
            home_entries.push(RemoteEntry {
                name: project_id.to_owned(),
                kind: EntryKind::Dir,
                metadata: None,
                mtime_missing: false,
            });
        }

        // project_dir → 含 session file entry（含真 mtime）
        dirs.entry(project_dir.clone())
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
        drop(dirs);

        self.files
            .lock()
            .expect("CountedFakeRemoteSftp files poisoned")
            .insert(file_path, content.into_bytes());
    }

    /// 注册一个目录条目（不含文件）—— for memory CRUD test 准备 memory 目录 fixture。
    pub fn add_dir(&self, parent: &str, name: &str) {
        let mut dirs = self
            .dirs
            .lock()
            .expect("CountedFakeRemoteSftp dirs poisoned");
        let parent_entries = dirs.entry(parent.to_owned()).or_default();
        if !parent_entries.iter().any(|e| e.name == name) {
            parent_entries.push(RemoteEntry {
                name: name.to_owned(),
                kind: EntryKind::Dir,
                metadata: None,
                mtime_missing: false,
            });
        }
        let full = format!("{parent}/{name}");
        dirs.entry(full).or_default();
    }

    /// 注册一个文件条目 —— for memory CRUD test 准备 `MEMORY.md` 等 fixture。
    pub fn add_file(&self, parent_dir: &str, name: &str, content: &str) {
        let mut dirs = self
            .dirs
            .lock()
            .expect("CountedFakeRemoteSftp dirs poisoned");
        let parent_entries = dirs.entry(parent_dir.to_owned()).or_default();
        if !parent_entries.iter().any(|e| e.name == name) {
            parent_entries.push(RemoteEntry {
                name: name.to_owned(),
                kind: EntryKind::File,
                metadata: Some(FsMetadata {
                    size: content.len() as u64,
                    mtime: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000),
                    identity: None,
                }),
                mtime_missing: false,
            });
        }
        drop(dirs);
        self.files
            .lock()
            .expect("CountedFakeRemoteSftp files poisoned")
            .insert(format!("{parent_dir}/{name}"), content.as_bytes().to_vec());
    }

    /// Snapshot 所有 read 路径 counter 当前值，便于测试做 before/after diff。
    pub fn snapshot_counters(&self) -> FakeCounters {
        FakeCounters {
            metadata: self.metadata_count.load(Ordering::SeqCst),
            try_exists: self.try_exists_count.load(Ordering::SeqCst),
            read: self.read_count.load(Ordering::SeqCst),
            read_dir: self.read_dir_count.load(Ordering::SeqCst),
            read_lines_head: self.read_lines_head_count.load(Ordering::SeqCst),
        }
    }

    /// Snapshot 所有 write 路径 counter 当前值。
    pub fn snapshot_write_counters(&self) -> FakeWriteCounters {
        FakeWriteCounters {
            write: self.write_count.load(Ordering::SeqCst),
            mkdir: self.mkdir_count.load(Ordering::SeqCst),
            remove: self.remove_count.load(Ordering::SeqCst),
            rename: self.rename_count.load(Ordering::SeqCst),
        }
    }

    /// 取当前 written file 内容（测试断言用）。
    pub fn read_file(&self, path: &str) -> Option<Vec<u8>> {
        self.files
            .lock()
            .expect("CountedFakeRemoteSftp files poisoned")
            .get(path)
            .cloned()
    }
}

/// 不变的 read 路径 counter 快照。用于 `assert_eq!(before, after)` 检查"零新增 op"语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FakeCounters {
    pub metadata: usize,
    pub try_exists: usize,
    pub read: usize,
    pub read_dir: usize,
    pub read_lines_head: usize,
}

/// 不变的 write 路径 counter 快照。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FakeWriteCounters {
    pub write: usize,
    pub mkdir: usize,
    pub remove: usize,
    pub rename: usize,
}

#[async_trait]
impl SftpClient for CountedFakeRemoteSftp {
    async fn metadata(&self, path: &str) -> Result<FsMetadata, SftpClientError> {
        self.metadata_count.fetch_add(1, Ordering::SeqCst);
        let files = self.files.lock().expect("files poisoned");
        if let Some(bytes) = files.get(path) {
            Ok(FsMetadata {
                size: bytes.len() as u64,
                mtime: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000),
                identity: None,
            })
        } else if self.dirs.lock().expect("dirs poisoned").contains_key(path) {
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
        let exists_in_files = self
            .files
            .lock()
            .expect("files poisoned")
            .contains_key(path);
        let exists_in_dirs = self.dirs.lock().expect("dirs poisoned").contains_key(path);
        Ok(exists_in_files || exists_in_dirs)
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, SftpClientError> {
        self.read_count.fetch_add(1, Ordering::SeqCst);
        self.files
            .lock()
            .expect("files poisoned")
            .get(path)
            .cloned()
            .ok_or(SftpClientError::NoSuchFile)
    }

    async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
        self.read_dir_count.fetch_add(1, Ordering::SeqCst);
        self.dirs
            .lock()
            .expect("dirs poisoned")
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
            .lock()
            .expect("files poisoned")
            .get(path)
            .cloned()
            .ok_or(SftpClientError::NoSuchFile)?;
        let content =
            String::from_utf8(bytes).map_err(|e| SftpClientError::Other(e.to_string()))?;
        Ok(content.lines().take(max).map(ToOwned::to_owned).collect())
    }

    async fn write(&self, path: &str, data: &[u8]) -> Result<(), SftpClientError> {
        self.write_count.fetch_add(1, Ordering::SeqCst);
        self.files
            .lock()
            .expect("files poisoned")
            .insert(path.to_owned(), data.to_vec());
        // 同步把 entry 加到父目录列表（让后续 read_dir 能看到）
        if let Some(slash) = path.rfind('/') {
            let (parent, name) = path.split_at(slash);
            let name = &name[1..]; // 去掉 leading '/'
            let mut dirs = self.dirs.lock().expect("dirs poisoned");
            let entries = dirs.entry(parent.to_owned()).or_default();
            if let Some(e) = entries.iter_mut().find(|e| e.name == name) {
                // 同名 entry 更新 size
                e.metadata = Some(FsMetadata {
                    size: data.len() as u64,
                    mtime: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000),
                    identity: None,
                });
            } else {
                entries.push(RemoteEntry {
                    name: name.to_owned(),
                    kind: EntryKind::File,
                    metadata: Some(FsMetadata {
                        size: data.len() as u64,
                        mtime: std::time::UNIX_EPOCH
                            + std::time::Duration::from_secs(1_800_000_000),
                        identity: None,
                    }),
                    mtime_missing: false,
                });
            }
        }
        Ok(())
    }

    async fn mkdir(&self, path: &str) -> Result<(), SftpClientError> {
        self.mkdir_count.fetch_add(1, Ordering::SeqCst);
        let mut dirs = self.dirs.lock().expect("dirs poisoned");
        if dirs.contains_key(path) {
            return Err(SftpClientError::Other("dir already exists".into()));
        }
        dirs.insert(path.to_owned(), Vec::new());
        // 加 entry 到父目录
        if let Some(slash) = path.rfind('/') {
            let (parent, name) = path.split_at(slash);
            let name = &name[1..];
            let parent_entries = dirs.entry(parent.to_owned()).or_default();
            if !parent_entries.iter().any(|e| e.name == name) {
                parent_entries.push(RemoteEntry {
                    name: name.to_owned(),
                    kind: EntryKind::Dir,
                    metadata: None,
                    mtime_missing: false,
                });
            }
        }
        Ok(())
    }

    async fn remove(&self, path: &str) -> Result<(), SftpClientError> {
        self.remove_count.fetch_add(1, Ordering::SeqCst);
        let removed = self
            .files
            .lock()
            .expect("files poisoned")
            .remove(path)
            .is_some();
        if !removed {
            return Err(SftpClientError::NoSuchFile);
        }
        // 清父目录 entry
        if let Some(slash) = path.rfind('/') {
            let (parent, name) = path.split_at(slash);
            let name = &name[1..];
            if let Some(entries) = self.dirs.lock().expect("dirs poisoned").get_mut(parent) {
                entries.retain(|e| e.name != name);
            }
        }
        Ok(())
    }

    async fn rename(&self, src: &str, dst: &str) -> Result<(), SftpClientError> {
        self.rename_count.fetch_add(1, Ordering::SeqCst);
        let mut files = self.files.lock().expect("files poisoned");
        // 标准 SFTP RENAME 拒覆盖 target（与 OpenSSH server 对齐）
        if files.contains_key(dst) {
            return Err(SftpClientError::Other(
                "rename target already exists".into(),
            ));
        }
        let bytes = files.remove(src).ok_or(SftpClientError::NoSuchFile)?;
        let bytes_len = bytes.len();
        files.insert(dst.to_owned(), bytes);
        drop(files);

        // 更新父目录 entry
        let mut dirs = self.dirs.lock().expect("dirs poisoned");
        if let Some(slash) = src.rfind('/') {
            let (parent, name) = src.split_at(slash);
            let name = &name[1..];
            if let Some(entries) = dirs.get_mut(parent) {
                entries.retain(|e| e.name != name);
            }
        }
        if let Some(slash) = dst.rfind('/') {
            let (parent, name) = dst.split_at(slash);
            let name = &name[1..];
            let parent_entries = dirs.entry(parent.to_owned()).or_default();
            parent_entries.push(RemoteEntry {
                name: name.to_owned(),
                kind: EntryKind::File,
                metadata: Some(FsMetadata {
                    size: bytes_len as u64,
                    mtime: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000),
                    identity: None,
                }),
                mtime_missing: false,
            });
        }
        Ok(())
    }
}
