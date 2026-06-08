//! 本地文件系统 `FileSystemProvider` 实现。
//!
//! 单态、内部无状态；本 crate 内部允许调 `tokio::fs::*`，但其它业务 crate
//! 通过 trait 调用——`crates/cdt-fs/ALLOWLIST.md` H1 allowlist 列在这里。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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

    /// **per-entry 错误降级语义**（codex 二审 M2 by-design）：
    /// - `entry.file_type().await` 失败：emit `tracing::warn!` + skip 该 entry
    ///   （上层 cache 不应将"entry 缺失"解释为"entry 不存在"——dir scan 只是
    ///   weak signal，cache key 用 mtime+size+identity 判定有效性，单 entry
    ///   读取失败属环境抖动而非 entry 删除）
    /// - `read.next_entry().await` 失败：emit `tracing::warn!` + 截断列表返
    ///   partial 结果（同上语义，业务路径 `ProjectScanner` 等 best-effort 消费方
    ///   依赖此降级行为）
    /// - 仅 per-file metadata 缺失（`entry.metadata()` 失败）单独降级该 entry 的
    ///   `metadata: None`，不影响其他 entry
    ///
    /// 后续 PR 若新增"cache invalidation 依赖完整 dir scan"的调用方，应该
    /// **包裹**本方法（自己跑一次完整 scan + 任一 per-entry 错就清 cache），
    /// 而**不**改变本方法默认行为。
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
                            // M2 by-design：per-entry file_type 失败降级 skip
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
        created: meta.created().ok(),
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

    async fn write_atomic(&self, path: &Path, content: &[u8]) -> Result<(), FsError> {
        let tmp = atomic_tmp_path(path);
        tokio::fs::write(&tmp, content)
            .await
            .map_err(|e| wrap_io(&tmp, e))?;
        if let Err(err) = tokio::fs::rename(&tmp, path).await {
            // best-effort 清理 tmp（清理失败不向上传播——rename 失败已是 primary error）
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(wrap_io(path, err));
        }
        Ok(())
    }

    async fn create_dir_all(&self, path: &Path) -> Result<(), FsError> {
        match tokio::fs::create_dir_all(path).await {
            Ok(()) => Ok(()),
            // tokio::fs::create_dir_all 对已存在目录返 Ok，不会进 AlreadyExists 分支；
            // 这里仍显式接住作防御性 forward-compat。
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(err) => Err(wrap_io(path, err)),
        }
    }

    async fn remove_file(&self, path: &Path) -> Result<(), FsError> {
        tokio::fs::remove_file(path)
            .await
            .map_err(|e| wrap_io(path, e))
    }
}

/// 进程内单调递增 + pid suffix，避免并发 `write_atomic` 同 path tmp 冲突。
///
/// 设计：fs-abstraction spec 写方法 atomic 契约段——**不**用 `SystemTime::now()` 纳秒
/// （Windows 100ns 时钟精度并发碰撞 race，详 design `ssh-project-memory-remote-rw` D2）。
static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

fn atomic_tmp_path(path: &Path) -> PathBuf {
    let seq = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp.{seq:016x}.{pid:08x}"));
    PathBuf::from(tmp)
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
    async fn write_atomic_overwrites_existing_content() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("note.md");
        tokio::fs::write(&file, b"old content").await.unwrap();

        let fs = LocalFileSystemProvider::new();
        fs.write_atomic(&file, b"new content body").await.unwrap();

        let actual = tokio::fs::read_to_string(&file).await.unwrap();
        assert_eq!(actual, "new content body");
    }

    #[tokio::test]
    async fn write_atomic_no_tmp_residue_on_success() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("note.md");

        let fs = LocalFileSystemProvider::new();
        fs.write_atomic(&file, b"hello").await.unwrap();

        let entries = fs.read_dir(dir.path()).await.unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["note.md"]);
        assert!(!names.iter().any(|n| n.contains(".tmp.")));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn write_atomic_concurrent_writes_yield_intact_content() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("contended.md");
        let fs = LocalFileSystemProvider::new();

        let mut joins = Vec::new();
        for i in 0..8u8 {
            let f = file.clone();
            joins.push(tokio::spawn(async move {
                let provider = LocalFileSystemProvider::new();
                let content = vec![b'A' + i; 16 * 1024];
                provider.write_atomic(&f, &content).await.unwrap();
            }));
        }
        for j in joins {
            j.await.unwrap();
        }

        let final_content = fs.read_to_string(&file).await.unwrap();
        // 必须是某一次写的完整内容（16 KiB 同字节），而不是混合 / 截断
        assert_eq!(final_content.len(), 16 * 1024);
        let first_byte = final_content.as_bytes()[0];
        assert!(final_content.bytes().all(|b| b == first_byte));

        // tmp 残留检查
        let entries = fs.read_dir(dir.path()).await.unwrap();
        for e in &entries {
            assert!(!e.name.contains(".tmp."), "tmp 残留: {}", e.name);
        }
    }

    #[tokio::test]
    async fn create_dir_all_idempotent() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b").join("c");

        let fs = LocalFileSystemProvider::new();
        fs.create_dir_all(&nested).await.unwrap();
        // 第二次调不报错
        fs.create_dir_all(&nested).await.unwrap();

        assert!(fs.exists(&nested).await);
    }

    #[tokio::test]
    async fn remove_file_returns_not_found_for_missing() {
        let dir = tempdir().unwrap();
        let fs = LocalFileSystemProvider::new();
        let err = fs
            .remove_file(&dir.path().join("ghost.md"))
            .await
            .unwrap_err();
        assert!(matches!(err, FsError::NotFound(_)));
    }

    #[tokio::test]
    async fn remove_file_deletes_existing_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("victim.md");
        tokio::fs::write(&file, b"x").await.unwrap();

        let fs = LocalFileSystemProvider::new();
        fs.remove_file(&file).await.unwrap();
        assert!(!fs.exists(&file).await);
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
