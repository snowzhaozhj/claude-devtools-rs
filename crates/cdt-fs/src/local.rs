//! 本地文件系统 `FileSystemProvider` 实现。
//!
//! 单态、内部无状态；本 crate 内部允许调 `tokio::fs::*`，但其它业务 crate
//! 通过 trait 调用——`.claude/rules/fs-abstraction.md` H1 allowlist 这里。

use std::path::Path;
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

use crate::dir_entry::{DirEntry, EntryKind};
use crate::error::FsError;
use crate::kind::FsKind;
use crate::metadata::{FsIdentity, FsMetadata};
use crate::provider::{FileSystemProvider, FsHandle};

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalFileSystemProvider;

impl LocalFileSystemProvider {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    async fn read_dir_entries(
        &self,
        path: &Path,
        include_metadata: bool,
    ) -> Result<Vec<DirEntry>, FsError> {
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
                    let metadata = if include_metadata && kind.is_file() {
                        match entry.metadata().await {
                            Ok(meta) => Some(fs_metadata_from_std(&meta)),
                            Err(err) => {
                                tracing::warn!(path = %entry.path().display(), error = %err, "dir entry metadata unavailable");
                                None
                            }
                        }
                    } else {
                        None
                    };
                    out.push(DirEntry {
                        name,
                        kind,
                        metadata,
                    });
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

fn fs_metadata_from_std(meta: &std::fs::Metadata) -> FsMetadata {
    let mtime = meta.modified().unwrap_or(UNIX_EPOCH);
    FsMetadata {
        size: meta.len(),
        mtime,
        identity: identity_from_std(meta),
    }
}

/// `cfg(unix)` 路径永远返 `Some(Unix { dev, ino })`；其他平台永远返 `None`。
///
/// clippy 在单一 cfg 编译下会认为 "返回类型总是 `Some`"——这是 cfg-gated 多态，
/// 跨平台保持统一签名 `-> Option<FsIdentity>`。
#[allow(clippy::unnecessary_wraps)]
#[cfg(unix)]
fn identity_from_std(meta: &std::fs::Metadata) -> Option<FsIdentity> {
    use std::os::unix::fs::MetadataExt;
    Some(FsIdentity::Unix {
        dev: meta.dev(),
        ino: meta.ino(),
    })
}

#[cfg(not(unix))]
fn identity_from_std(_meta: &std::fs::Metadata) -> Option<FsIdentity> {
    None
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
        self.read_dir_entries(path, false).await
    }

    async fn read_dir_with_metadata(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
        self.read_dir_entries(path, true).await
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
        Ok(fs_metadata_from_std(&meta))
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

    async fn open_read(&self, path: &Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, FsError> {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| wrap_io(path, e))?;
        Ok(Box::new(file))
    }
}

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
    async fn exists_and_stat_carry_identity_on_unix() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("a.txt");
        tokio::fs::write(&file, b"hello world").await.unwrap();

        let fs = LocalFileSystemProvider::new();
        assert!(fs.exists(&file).await);
        let stat = fs.stat(&file).await.unwrap();
        assert_eq!(stat.size, 11);
        assert!(stat.mtime_ms() > 0);
        #[cfg(unix)]
        {
            assert!(
                matches!(stat.identity, Some(FsIdentity::Unix { .. })),
                "Local Unix SHALL fill identity"
            );
        }
        #[cfg(not(unix))]
        {
            assert!(
                stat.identity.is_none(),
                "Local Windows SHALL fill identity=None"
            );
        }
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

    #[tokio::test]
    async fn open_read_returns_streaming_handle() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("multi.txt");
        let mut f = tokio::fs::File::create(&file).await.unwrap();
        f.write_all(b"line1\nline2\n").await.unwrap();
        f.flush().await.unwrap();

        let fs = LocalFileSystemProvider::new();
        let reader = fs.open_read(&file).await.unwrap();
        let mut lines = BufReader::new(reader).lines();
        let mut collected = Vec::new();
        while let Some(line) = lines.next_line().await.unwrap() {
            collected.push(line);
        }
        assert_eq!(collected, vec!["line1".to_string(), "line2".to_string()]);
    }

    #[tokio::test]
    async fn stat_many_returns_in_order() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        tokio::fs::write(&a, b"aaa").await.unwrap();
        tokio::fs::write(&b, b"bbbb").await.unwrap();

        let fs = LocalFileSystemProvider::new();
        let paths = [a.as_path(), b.as_path()];
        let results = fs.stat_many(&paths).await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].as_ref().unwrap().size, 3);
        assert_eq!(results[1].as_ref().unwrap().size, 4);
    }

    #[tokio::test]
    async fn stat_many_keeps_per_path_errors() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("missing");
        tokio::fs::write(&a, b"aaa").await.unwrap();

        let fs = LocalFileSystemProvider::new();
        let paths = [a.as_path(), b.as_path()];
        let results = fs.stat_many(&paths).await;
        assert!(results[0].is_ok());
        assert!(matches!(
            results[1].as_ref().unwrap_err(),
            FsError::NotFound(_)
        ));
    }
}
