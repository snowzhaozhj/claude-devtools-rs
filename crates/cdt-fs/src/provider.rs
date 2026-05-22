//! `FileSystemProvider` trait —— 所有 fs 操作的唯一 seam。
//!
//! Spec：`openspec/specs/fs-abstraction/spec.md` §`FileSystemProvider` trait 暴露 7 个核心方法。
//! 设计决策见 `openspec/changes/unify-fs-abstraction/design.md` D2 / D3 / D4。
//!
//! trait dyn-safe（不引入关联类型），以让 `Arc<dyn FileSystemProvider>` 注入。
//! 业务路径**禁止**直调 `tokio::fs::*`（见 H1 + xtask check-fs-direct-calls）。

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::dir_entry::DirEntry;
use crate::error::FsError;
use crate::kind::FsKind;
use crate::metadata::FsMetadata;

#[async_trait]
pub trait FileSystemProvider: Send + Sync + 'static {
    fn kind(&self) -> FsKind;

    async fn exists(&self, path: &Path) -> bool;

    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError>;

    async fn read_dir_with_metadata(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
        let mut entries = self.read_dir(path).await?;
        for entry in &mut entries {
            if entry.kind.is_file() {
                entry.metadata = Some(self.stat(&path.join(&entry.name)).await?);
            }
        }
        Ok(entries)
    }

    async fn read_to_string(&self, path: &Path) -> Result<String, FsError>;

    async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError>;

    /// 读文件前 `max` 行——用于 SSH provider 抽 `cwd` 时避免拉全文。
    async fn read_lines_head(&self, path: &Path, max: usize) -> Result<Vec<String>, FsError>;

    /// 流式打开 path 读全文——替代旧的 `SshFileSystemProvider::open_read_stream`
    /// inherent 方法（破抽象）。返 `Box<dyn AsyncRead + Send + Unpin>` 让调用方
    /// 不需 downcast 到具体 provider 即能 `BufReader::new(reader).lines()`。
    ///
    /// 设计：D4 钉死动态分发——vtable lookup 几 ns，相对 SFTP 50ms RTT 完全可
    /// 忽略；Local 上 jsonl streaming 也不在 hot path。本 trait 守 dyn-safe，
    /// 不引入关联类型。
    async fn open_read(&self, path: &Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, FsError>;

    /// 批量 stat 多个 path —— H2 hot path 禁 N 次串行 stat 的执行基础。
    ///
    /// default 实现走 `futures::future::join_all(paths.iter().map(|p| self.stat(p)))`，
    /// 返回 `Vec<Result<FsMetadata, FsError>>` 顺序与 input 严格对应。
    ///
    /// SSH override 暂用 default —— 由于底层 `Arc<Mutex<SftpSession>>` 全锁串行，
    /// 实际执行仍是 N 次串行 RTT（**已知限制**，留 PR-F 解决）。trait API 先就位
    /// 让 caller 一律调 `stat_many` 而非循环 `stat`。
    async fn stat_many(&self, paths: &[&Path]) -> Vec<Result<FsMetadata, FsError>> {
        let futures = paths.iter().map(|p| self.stat(p));
        futures::future::join_all(futures).await
    }

    /// Atomic 写文件——SHALL 通过 tmp file + rename 实现，写失败 best-effort 清理 tmp。
    /// reader 永远观察到旧内容或新内容整版，不观察到截断 / 半写状态。
    /// 设计：fs-abstraction spec `Requirement: FileSystemProvider trait 暴露 7 个核心方法`
    /// 写方法 atomic 契约段（change `ssh-project-memory-remote-rw` 引入）。
    async fn write_atomic(&self, path: &Path, content: &[u8]) -> Result<(), FsError>;

    /// 递归创建目录——已存在 SHALL NOT 报错。等价 `tokio::fs::create_dir_all`。
    async fn create_dir_all(&self, path: &Path) -> Result<(), FsError>;

    /// 删文件——不存在 SHALL 返 `FsError::NotFound(path)`，路径是目录 SHALL 返
    /// `FsError::Io`，**不**递归删。
    async fn remove_file(&self, path: &Path) -> Result<(), FsError>;
}

/// `Arc<dyn FileSystemProvider>` 的简写。
pub type FsHandle = Arc<dyn FileSystemProvider>;
