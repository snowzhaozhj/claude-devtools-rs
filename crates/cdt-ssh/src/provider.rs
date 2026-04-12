//! `SshFileSystemProvider`：`FileSystemProvider` 的 SSH 后端。
//!
//! 当前为 placeholder 实现——所有文件操作返回 "not connected" 错误。
//! 完整 SFTP 集成需要真实 SSH 连接，留给后续 integration 阶段。

use std::path::Path;

use async_trait::async_trait;

use cdt_discover::{DirEntry, FileSystemProvider, FsError, FsKind, FsMetadata};

/// SSH 文件系统 provider（placeholder）。
pub struct SshFileSystemProvider {
    context_id: String,
}

impl SshFileSystemProvider {
    pub fn new(context_id: &str) -> Self {
        Self {
            context_id: context_id.to_owned(),
        }
    }

    fn not_connected_error(&self, path: &Path) -> FsError {
        FsError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                format!("SSH context '{}' not connected", self.context_id),
            ),
        }
    }
}

#[async_trait]
impl FileSystemProvider for SshFileSystemProvider {
    fn kind(&self) -> FsKind {
        FsKind::Ssh
    }

    async fn exists(&self, _path: &Path) -> bool {
        false
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
        Err(self.not_connected_error(path))
    }

    async fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        Err(self.not_connected_error(path))
    }

    async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError> {
        Err(self.not_connected_error(path))
    }

    async fn read_lines_head(&self, path: &Path, _max: usize) -> Result<Vec<String>, FsError> {
        Err(self.not_connected_error(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_returns_ssh() {
        let provider = SshFileSystemProvider::new("test-ctx");
        assert_eq!(provider.kind(), FsKind::Ssh);
    }

    #[tokio::test]
    async fn exists_returns_false_when_not_connected() {
        let provider = SshFileSystemProvider::new("test-ctx");
        assert!(!provider.exists(Path::new("/remote/path")).await);
    }

    #[tokio::test]
    async fn read_to_string_returns_error() {
        let provider = SshFileSystemProvider::new("test-ctx");
        let result = provider.read_to_string(Path::new("/remote/file")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn stat_returns_error() {
        let provider = SshFileSystemProvider::new("test-ctx");
        let result = provider.stat(Path::new("/remote/file")).await;
        assert!(result.is_err());
    }
}
