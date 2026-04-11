//! project-discovery 的 I/O 抽象层。
//!
//! 所有 scanner / resolver / grouper 都只通过 [`FileSystemProvider`] trait
//! 访问文件系统，使得后续 `ssh-remote-context` port 只需要提供一个
//! `SshFileSystemProvider` 实现就能复用同一套 scanner 逻辑，不改 scanner 代码。
//!
//! Spec：`openspec/specs/project-discovery/spec.md` 的
//! `Abstract filesystem access through a provider trait` Requirement。

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::FsError;

/// Provider 类型 —— 决定 scanner 是否可以对同一个项目下的多个 session 做
/// 全量 `cwd` 提取。SSH 模式下 scanner 必须退化为"只看第一个成功提取的
/// session"以避免把远端整个目录拉下来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsKind {
    Local,
    Ssh,
}

/// 目录项类型 —— 刻意保持最小集合，不暴露 `std::fs::FileType`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
    Other,
}

impl EntryKind {
    #[must_use]
    pub fn is_file(self) -> bool {
        matches!(self, EntryKind::File)
    }
    #[must_use]
    pub fn is_dir(self) -> bool {
        matches!(self, EntryKind::Dir)
    }
}

/// 目录遍历返回的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub kind: EntryKind,
}

/// `stat` 返回的最小 metadata。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsMetadata {
    pub size: u64,
    pub mtime: SystemTime,
}

impl FsMetadata {
    /// 把 mtime 折算成 epoch 毫秒，对齐 TS 里 `Project.mostRecentSession` 的类型。
    #[must_use]
    pub fn mtime_ms(&self) -> i64 {
        self.mtime
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_millis()).ok())
            .unwrap_or(0)
    }
}

/// 所有 discovery I/O 的唯一 seam。
#[async_trait]
pub trait FileSystemProvider: Send + Sync + 'static {
    fn kind(&self) -> FsKind;

    async fn exists(&self, path: &Path) -> bool;

    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError>;

    async fn read_to_string(&self, path: &Path) -> Result<String, FsError>;

    async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError>;

    /// 读取文件的前 `max` 行。用于 `ProjectPathResolver` 在 SSH 模式下
    /// 快速抽取 `cwd`，而不把整个文件拉下来。
    async fn read_lines_head(&self, path: &Path, max: usize) -> Result<Vec<String>, FsError>;
}

/// 本地文件系统后端。单态，内部无状态。
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalFileSystemProvider;

impl LocalFileSystemProvider {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

fn wrap_io(path: &Path, err: std::io::Error) -> FsError {
    if err.kind() == std::io::ErrorKind::NotFound {
        FsError::NotFound(path.to_path_buf())
    } else {
        FsError::Io {
            path: path.to_path_buf(),
            source: err,
        }
    }
}

#[async_trait]
impl FileSystemProvider for LocalFileSystemProvider {
    fn kind(&self) -> FsKind {
        FsKind::Local
    }

    async fn exists(&self, path: &Path) -> bool {
        tokio::fs::metadata(path).await.is_ok()
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
        let mut read = tokio::fs::read_dir(path)
            .await
            .map_err(|e| wrap_io(path, e))?;
        let mut out = Vec::new();
        loop {
            match read.next_entry().await {
                Ok(Some(entry)) => {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let file_type = match entry.file_type().await {
                        Ok(ft) => ft,
                        Err(err) => {
                            tracing::warn!(path = %entry.path().display(), error = %err, "skip unreadable dir entry");
                            continue;
                        }
                    };
                    let kind = if file_type.is_dir() {
                        EntryKind::Dir
                    } else if file_type.is_file() {
                        EntryKind::File
                    } else if file_type.is_symlink() {
                        EntryKind::Symlink
                    } else {
                        EntryKind::Other
                    };
                    out.push(DirEntry { name, kind });
                }
                Ok(None) => break,
                Err(err) => {
                    tracing::warn!(path = %path.display(), error = %err, "error walking dir");
                    break;
                }
            }
        }
        Ok(out)
    }

    async fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        tokio::fs::read_to_string(path)
            .await
            .map_err(|e| wrap_io(path, e))
    }

    async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError> {
        let meta = tokio::fs::metadata(path)
            .await
            .map_err(|e| wrap_io(path, e))?;
        let mtime = meta.modified().unwrap_or(UNIX_EPOCH);
        Ok(FsMetadata {
            size: meta.len(),
            mtime,
        })
    }

    async fn read_lines_head(&self, path: &Path, max: usize) -> Result<Vec<String>, FsError> {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| wrap_io(path, e))?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut out = Vec::with_capacity(max.min(64));
        while out.len() < max {
            match lines.next_line().await {
                Ok(Some(line)) => out.push(line),
                Ok(None) => break,
                Err(err) => return Err(wrap_io(path, err)),
            }
        }
        Ok(out)
    }
}

/// `Arc<dyn FileSystemProvider>` 的简写。
pub type FsHandle = std::sync::Arc<dyn FileSystemProvider>;

#[must_use]
pub fn local_handle() -> FsHandle {
    std::sync::Arc::new(LocalFileSystemProvider::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn exists_and_stat_roundtrip() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("a.txt");
        tokio::fs::write(&file, b"hello world").await.unwrap();

        let fs = LocalFileSystemProvider::new();
        assert!(fs.exists(&file).await);
        let stat = fs.stat(&file).await.unwrap();
        assert_eq!(stat.size, 11);
        assert!(stat.mtime_ms() > 0);
    }

    #[tokio::test]
    async fn read_dir_classifies_entries() {
        let dir = tempdir().unwrap();
        tokio::fs::write(dir.path().join("f.txt"), b"x")
            .await
            .unwrap();
        tokio::fs::create_dir(dir.path().join("sub")).await.unwrap();

        let fs = LocalFileSystemProvider::new();
        let mut entries = fs.read_dir(dir.path()).await.unwrap();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "f.txt");
        assert_eq!(entries[0].kind, EntryKind::File);
        assert_eq!(entries[1].name, "sub");
        assert_eq!(entries[1].kind, EntryKind::Dir);
    }

    #[tokio::test]
    async fn read_lines_head_bounds_to_max() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("multi.txt");
        let mut f = tokio::fs::File::create(&file).await.unwrap();
        f.write_all(b"line1\nline2\nline3\nline4\n").await.unwrap();
        f.flush().await.unwrap();

        let fs = LocalFileSystemProvider::new();
        let lines = fs.read_lines_head(&file, 2).await.unwrap();
        assert_eq!(lines, vec!["line1".to_string(), "line2".to_string()]);
    }

    #[tokio::test]
    async fn missing_path_is_not_found() {
        let dir = tempdir().unwrap();
        let fs = LocalFileSystemProvider::new();
        let err = fs.stat(&dir.path().join("nope")).await.unwrap_err();
        assert!(matches!(err, FsError::NotFound(_)));
    }
}
