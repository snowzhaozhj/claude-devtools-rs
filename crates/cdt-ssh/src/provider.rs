//! `SshFileSystemProvider`：`FileSystemProvider` 的 SSH 后端实现。
//!
//! Spec：`openspec/specs/ssh-remote-context/spec.md::Requirement: Read sessions and
//! files over SSH`。
//!
//! 内部把 SFTP I/O 抽到 [`SftpClient`] trait——生产路径是 [`RusshSftpClient`]
//! 包装 `russh_sftp::client::session::SftpSession`，测试路径通过
//! [`SshFileSystemProvider::with_client`] 注入 fake client。
//!
//! 错误分类（[`SftpClientError`]）：
//! - `NoSuchFile` → [`FsError::NotFound`]
//! - `PermissionDenied` → [`FsError::Io`] kind `PermissionDenied`
//! - `Transient` → 走 [`with_retry`] 3 次指数退避（75ms × attempt）
//! - `Other` → 立即传播
//!
//! 瞬时错误码（design.md D1 + tasks 5.7）：SFTP `StatusCode::Failure`(=4)
//! 与 IO 层的 `EAGAIN` / `ECONNRESET` / `ETIMEDOUT` / `EPIPE` —— 远端短暂抖动
//! 时让 scanner 不立即崩。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use async_trait::async_trait;
use russh_sftp::client::SftpSession;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::fs::File;
use russh_sftp::protocol::{FileType, StatusCode};
use tokio::sync::Mutex;

use cdt_discover::{DirEntry, EntryKind, FileSystemProvider, FsError, FsKind, FsMetadata};

/// SFTP 操作重试的最大尝试次数。
const MAX_RETRY_ATTEMPTS: u32 = 3;
/// 指数退避基数（实际 wait = `RETRY_BACKOFF_BASE * attempt`）。
const RETRY_BACKOFF_BASE: Duration = Duration::from_millis(75);

/// SFTP 客户端错误分类——既驱动 retry 决策，也映射到 [`FsError`]。
#[derive(Debug, Clone, thiserror::Error)]
pub enum SftpClientError {
    #[error("no such file or directory")]
    NoSuchFile,
    #[error("permission denied")]
    PermissionDenied,
    /// 瞬时错误（`StatusCode::Failure` / IO `EAGAIN` / `ECONNRESET` / `ETIMEDOUT` / `EPIPE`）——
    /// 由 [`with_retry`] 重试 3 次。
    #[error("transient sftp error: {0}")]
    Transient(String),
    /// 其他永久性错误（unsupported / 协议异常 / 不可恢复的 IO 失败）。
    #[error("sftp error: {0}")]
    Other(String),
}

impl SftpClientError {
    /// 判定该错误是否值得重试。
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Transient(_))
    }
}

/// 远端目录项（与 `cdt-discover::DirEntry` 一一对应，避免 trait 跨 crate 泄漏 SFTP 类型）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEntry {
    pub name: String,
    pub kind: EntryKind,
    pub metadata: Option<FsMetadata>,
    pub mtime_missing: bool,
}

/// SFTP I/O seam —— `SshFileSystemProvider` 通过此 trait 访问远端，生产路径是
/// [`RusshSftpClient`]，测试可注入 fake。
///
/// `open_read` 不在 trait 中——它返回 `russh_sftp::client::fs::File` 具体类型
/// （`impl AsyncRead`），跨 trait 抽象会引入类型耦合且 fake 没必要 mock 一个
/// `AsyncRead`；inherent [`SshFileSystemProvider::open_read_stream`] 直接走
/// 真 sftp 句柄。
#[async_trait]
pub trait SftpClient: Send + Sync + 'static {
    async fn metadata(&self, path: &str) -> Result<FsMetadata, SftpClientError>;
    async fn try_exists(&self, path: &str) -> Result<bool, SftpClientError>;
    async fn read(&self, path: &str) -> Result<Vec<u8>, SftpClientError>;
    async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpClientError>;
    async fn read_lines_head(&self, path: &str, max: usize)
    -> Result<Vec<String>, SftpClientError>;
}

/// SSH 文件系统 provider —— 实现 [`FileSystemProvider`] 用于远端 SFTP。
pub struct SshFileSystemProvider {
    context_id: String,
    client: Arc<dyn SftpClient>,
    remote_home: PathBuf,
    /// 仅生产构造器填——`open_read_stream` 用真 [`SftpSession`] 句柄取
    /// `russh_sftp::client::fs::File`；测试路径为 `None`，调用时返
    /// [`FsError::Unsupported`]。
    sftp: Option<Arc<Mutex<SftpSession>>>,
}

impl SshFileSystemProvider {
    /// 生产路径构造器：与 [`crate::session::SshSessionManager`] 共享同一
    /// `Arc<Mutex<SftpSession>>`——session 持有它做远端命令 / disconnect，
    /// provider 用它做文件读 + 流式打开。
    #[must_use]
    pub fn new(
        context_id: impl Into<String>,
        sftp: Arc<Mutex<SftpSession>>,
        remote_home: PathBuf,
    ) -> Self {
        let client: Arc<dyn SftpClient> = Arc::new(RusshSftpClient {
            sftp: Arc::clone(&sftp),
        });
        Self {
            context_id: context_id.into(),
            client,
            remote_home,
            sftp: Some(sftp),
        }
    }

    /// 测试路径构造器：注入 fake [`SftpClient`]；`open_read_stream` 不可用。
    #[must_use]
    pub fn with_client(
        context_id: impl Into<String>,
        client: Arc<dyn SftpClient>,
        remote_home: PathBuf,
    ) -> Self {
        Self {
            context_id: context_id.into(),
            client,
            remote_home,
            sftp: None,
        }
    }

    #[must_use]
    pub fn context_id(&self) -> &str {
        &self.context_id
    }

    #[must_use]
    pub fn remote_home(&self) -> &Path {
        &self.remote_home
    }

    /// 流式打开远端文件（tasks 5.6 + spec `Requirement: Read sessions...`）。
    /// 返回的 `File` 实现 `tokio::io::AsyncRead` + `AsyncSeek`——caller 可用
    /// `AsyncBufReadExt::lines()` 流式读 JSONL，不全量拉到内存。
    ///
    /// 测试路径（[`with_client`]）下不可用，返 [`FsError::Unsupported`]。
    pub async fn open_read_stream(&self, path: &Path) -> Result<File, FsError> {
        let Some(sftp) = self.sftp.as_ref() else {
            return Err(FsError::Unsupported("open_read_stream"));
        };
        let path_str = path_to_string(path);
        let sftp_guard = sftp.lock().await;
        sftp_guard
            .open(path_str)
            .await
            .map_err(|e| map_sftp_io(path, &e))
    }
}

fn path_to_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[async_trait]
impl FileSystemProvider for SshFileSystemProvider {
    fn kind(&self) -> FsKind {
        FsKind::Ssh
    }

    async fn exists(&self, path: &Path) -> bool {
        let path_str = path_to_string(path);
        let client = Arc::clone(&self.client);
        // `try_exists` SFTP 端只走一次 metadata；放在 retry 内以容忍瞬时抖动。
        // 永久错误（PermissionDenied / Other）与 LocalFileSystemProvider 对齐降级为 false。
        with_retry(move || {
            let client = Arc::clone(&client);
            let path_str = path_str.clone();
            async move { client.try_exists(&path_str).await }
        })
        .await
        .unwrap_or_default()
    }

    async fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        let path_str = path_to_string(path);
        let client = Arc::clone(&self.client);
        let bytes = with_retry(move || {
            let client = Arc::clone(&client);
            let path_str = path_str.clone();
            async move { client.read(&path_str).await }
        })
        .await
        .map_err(|e| map_client_error(path, e))?;
        String::from_utf8(bytes).map_err(|e| FsError::Utf8 {
            path: path.to_path_buf(),
            source: e,
        })
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
        let path_str = path_to_string(path);
        let client = Arc::clone(&self.client);
        let entries = with_retry(move || {
            let client = Arc::clone(&client);
            let path_str = path_str.clone();
            async move { client.read_dir(&path_str).await }
        })
        .await
        .map_err(|e| map_client_error(path, e))?;
        Ok(entries
            .into_iter()
            .map(|e| DirEntry {
                name: e.name,
                kind: e.kind,
                metadata: e.metadata,
            })
            .collect())
    }

    async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError> {
        let path_str = path_to_string(path);
        let client = Arc::clone(&self.client);
        with_retry(move || {
            let client = Arc::clone(&client);
            let path_str = path_str.clone();
            async move { client.metadata(&path_str).await }
        })
        .await
        .map_err(|e| map_client_error(path, e))
    }

    async fn read_lines_head(&self, path: &Path, max: usize) -> Result<Vec<String>, FsError> {
        let path_str = path_to_string(path);
        let client = Arc::clone(&self.client);
        with_retry(move || {
            let client = Arc::clone(&client);
            let path_str = path_str.clone();
            async move { client.read_lines_head(&path_str, max).await }
        })
        .await
        .map_err(|e| map_client_error(path, e))
    }
}

/// 重试 helper：瞬时错误最多 3 次，指数退避（75ms × attempt）。
///
/// 返回首次非瞬时错误或最后一次瞬时错误（attempt 3 仍 Transient 时）。
pub async fn with_retry<T, F, Fut>(mut op: F) -> Result<T, SftpClientError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, SftpClientError>>,
{
    let mut last_err: Option<SftpClientError> = None;
    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) if e.is_transient() && attempt < MAX_RETRY_ATTEMPTS => {
                last_err = Some(e);
                tokio::time::sleep(RETRY_BACKOFF_BASE * attempt).await;
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap_or(SftpClientError::Other("retry exhausted".into())))
}

/// 把 [`SftpClientError`] 投影到 [`FsError`]。
fn map_client_error(path: &Path, err: SftpClientError) -> FsError {
    match err {
        SftpClientError::NoSuchFile => FsError::NotFound(path.to_path_buf()),
        SftpClientError::PermissionDenied => FsError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied"),
        },
        SftpClientError::Transient(reason) => FsError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::other(format!("transient sftp error: {reason}")),
        },
        SftpClientError::Other(reason) => FsError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::other(reason),
        },
    }
}

/// 把 russh-sftp 的 `Error` 直接投到 [`FsError`]（用于 `open_read_stream`
/// inherent path，没经过 `SftpClient` trait）。
fn map_sftp_io(path: &Path, err: &SftpError) -> FsError {
    match classify_sftp_error(err) {
        SftpClientError::NoSuchFile => FsError::NotFound(path.to_path_buf()),
        other => map_client_error(path, other),
    }
}

/// 把 russh-sftp 错误投影到结构化分类。
fn classify_sftp_error(err: &SftpError) -> SftpClientError {
    match err {
        SftpError::Status(status) => match status.status_code {
            StatusCode::NoSuchFile => SftpClientError::NoSuchFile,
            StatusCode::PermissionDenied => SftpClientError::PermissionDenied,
            StatusCode::Failure => SftpClientError::Transient(status.error_message.clone()),
            other => SftpClientError::Other(format!("{other:?}: {}", status.error_message)),
        },
        SftpError::IO(reason) => {
            if is_transient_io_reason(reason) {
                SftpClientError::Transient(reason.clone())
            } else {
                SftpClientError::Other(reason.clone())
            }
        }
        SftpError::Timeout => SftpClientError::Transient("timeout".into()),
        SftpError::Limited(reason) => SftpClientError::Other(format!("limited: {reason}")),
        SftpError::UnexpectedPacket => SftpClientError::Other("unexpected packet".into()),
        SftpError::UnexpectedBehavior(reason) => SftpClientError::Other(reason.clone()),
    }
}

/// 把 `io::Error.to_string()` 形态（russh-sftp 把 `io::Error` 转字符串）的
/// 已知瞬时错误归类为 [`SftpClientError::Transient`]。
///
/// 字符串匹配是无奈之举——russh-sftp 0.2.x 把 `io::Error` 调 `to_string()` 后
/// 塞进 `Error::IO(String)`，原 `ErrorKind` 丢失。后续 russh-sftp 升级到结构化
/// IO 错误时本函数可改为 `match ErrorKind`。
fn is_transient_io_reason(reason: &str) -> bool {
    let lower = reason.to_ascii_lowercase();
    for needle in [
        "eagain",
        "would block",
        "connection reset",
        "econnreset",
        "etimedout",
        "timed out",
        "epipe",
        "broken pipe",
    ] {
        if lower.contains(needle) {
            return true;
        }
    }
    false
}

/// 生产实现：包装 `russh-sftp` 的 `SftpSession`。
struct RusshSftpClient {
    sftp: Arc<Mutex<SftpSession>>,
}

#[async_trait]
impl SftpClient for RusshSftpClient {
    async fn metadata(&self, path: &str) -> Result<FsMetadata, SftpClientError> {
        let sftp = self.sftp.lock().await;
        let meta = sftp
            .metadata(path.to_owned())
            .await
            .map_err(|e| classify_sftp_error(&e))?;
        Ok(FsMetadata {
            size: meta.len(),
            mtime: meta.modified().unwrap_or(UNIX_EPOCH),
        })
    }

    async fn try_exists(&self, path: &str) -> Result<bool, SftpClientError> {
        let sftp = self.sftp.lock().await;
        sftp.try_exists(path.to_owned())
            .await
            .map_err(|e| classify_sftp_error(&e))
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, SftpClientError> {
        let sftp = self.sftp.lock().await;
        sftp.read(path.to_owned())
            .await
            .map_err(|e| classify_sftp_error(&e))
    }

    async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
        let sftp = self.sftp.lock().await;
        let read_dir = sftp
            .read_dir(path.to_owned())
            .await
            .map_err(|e| classify_sftp_error(&e))?;
        Ok(read_dir
            .map(|entry| {
                let meta = entry.metadata();
                let kind = file_type_to_entry_kind(entry.file_type());
                let modified = meta.modified();
                let mtime_missing = matches!(kind, EntryKind::File) && modified.is_err();
                let metadata = if matches!(kind, EntryKind::File) {
                    Some(FsMetadata {
                        size: meta.len(),
                        mtime: modified.unwrap_or(UNIX_EPOCH),
                    })
                } else {
                    None
                };
                RemoteEntry {
                    name: entry.file_name(),
                    kind,
                    metadata,
                    mtime_missing,
                }
            })
            .collect())
    }

    async fn read_lines_head(
        &self,
        path: &str,
        max: usize,
    ) -> Result<Vec<String>, SftpClientError> {
        // russh-sftp 没有原生 line API；SFTP 协议本身也是按 offset 读字节。
        // session metadata 探测场景 max 通常很小（≤ 10 行 / cwd 提取），全量
        // 读 + split 内存可控。大文件流式读走 `open_read_stream`。
        let bytes = self.read(path).await?;
        let text = String::from_utf8_lossy(&bytes);
        Ok(text.lines().take(max).map(ToOwned::to_owned).collect())
    }
}

fn file_type_to_entry_kind(ft: FileType) -> EntryKind {
    if ft.is_dir() {
        EntryKind::Dir
    } else if ft.is_file() {
        EntryKind::File
    } else if ft.is_symlink() {
        EntryKind::Symlink
    } else {
        EntryKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::SystemTime;

    /// 通用 fake `SftpClient` —— 每方法可配置返回值或 transient 失败次数。
    #[derive(Default)]
    struct FakeSftpClient {
        metadata_response: tokio::sync::Mutex<Option<Result<FsMetadata, SftpClientError>>>,
        try_exists_response: tokio::sync::Mutex<Option<Result<bool, SftpClientError>>>,
        read_response: tokio::sync::Mutex<Option<Result<Vec<u8>, SftpClientError>>>,
        read_dir_response: tokio::sync::Mutex<Option<Result<Vec<RemoteEntry>, SftpClientError>>>,
        read_lines_response: tokio::sync::Mutex<Option<Result<Vec<String>, SftpClientError>>>,
        /// 给定 < `transient_failures_before_success` 次返回 Transient 错误，之后
        /// 返回 `transient_eventual_ok`；用于测试 retry 路径。
        transient_failures_before_success: AtomicU32,
        transient_call_count: AtomicU32,
        transient_eventual_ok: tokio::sync::Mutex<Option<Vec<u8>>>,
    }

    impl FakeSftpClient {
        fn arc() -> Arc<Self> {
            Arc::new(Self::default())
        }

        async fn set_metadata(&self, r: Result<FsMetadata, SftpClientError>) {
            *self.metadata_response.lock().await = Some(r);
        }
        async fn set_try_exists(&self, r: Result<bool, SftpClientError>) {
            *self.try_exists_response.lock().await = Some(r);
        }
        async fn set_read(&self, r: Result<Vec<u8>, SftpClientError>) {
            *self.read_response.lock().await = Some(r);
        }
        async fn set_read_dir(&self, r: Result<Vec<RemoteEntry>, SftpClientError>) {
            *self.read_dir_response.lock().await = Some(r);
        }
        async fn set_read_lines(&self, r: Result<Vec<String>, SftpClientError>) {
            *self.read_lines_response.lock().await = Some(r);
        }
        async fn set_transient_then_ok(&self, failures: u32, ok_bytes: Vec<u8>) {
            self.transient_failures_before_success
                .store(failures, Ordering::SeqCst);
            *self.transient_eventual_ok.lock().await = Some(ok_bytes);
        }
    }

    #[async_trait]
    impl SftpClient for FakeSftpClient {
        async fn metadata(&self, _path: &str) -> Result<FsMetadata, SftpClientError> {
            self.metadata_response
                .lock()
                .await
                .clone()
                .unwrap_or(Err(SftpClientError::Other("not configured".into())))
        }
        async fn try_exists(&self, _path: &str) -> Result<bool, SftpClientError> {
            self.try_exists_response
                .lock()
                .await
                .clone()
                .unwrap_or(Err(SftpClientError::Other("not configured".into())))
        }
        async fn read(&self, _path: &str) -> Result<Vec<u8>, SftpClientError> {
            let need = self
                .transient_failures_before_success
                .load(Ordering::SeqCst);
            if need > 0 {
                let already = self.transient_call_count.fetch_add(1, Ordering::SeqCst);
                if already < need {
                    return Err(SftpClientError::Transient(format!("attempt {already}")));
                }
                let bytes = self
                    .transient_eventual_ok
                    .lock()
                    .await
                    .clone()
                    .unwrap_or_default();
                return Ok(bytes);
            }
            self.read_response
                .lock()
                .await
                .clone()
                .unwrap_or(Err(SftpClientError::Other("not configured".into())))
        }
        async fn read_dir(&self, _path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
            self.read_dir_response
                .lock()
                .await
                .clone()
                .unwrap_or(Err(SftpClientError::Other("not configured".into())))
        }
        async fn read_lines_head(
            &self,
            _path: &str,
            _max: usize,
        ) -> Result<Vec<String>, SftpClientError> {
            self.read_lines_response
                .lock()
                .await
                .clone()
                .unwrap_or(Err(SftpClientError::Other("not configured".into())))
        }
    }

    fn make_provider(client: Arc<dyn SftpClient>) -> SshFileSystemProvider {
        SshFileSystemProvider::with_client("test-ctx", client, PathBuf::from("/remote/home"))
    }

    #[test]
    fn kind_is_ssh() {
        let client = FakeSftpClient::arc();
        let provider = make_provider(client);
        assert_eq!(provider.kind(), FsKind::Ssh);
        assert_eq!(provider.context_id(), "test-ctx");
        assert_eq!(provider.remote_home(), Path::new("/remote/home"));
    }

    #[tokio::test]
    async fn exists_returns_true_on_metadata_ok() {
        let fake = FakeSftpClient::arc();
        fake.set_try_exists(Ok(true)).await;
        let provider = make_provider(fake.clone());
        assert!(provider.exists(Path::new("/remote/file")).await);
    }

    #[tokio::test]
    async fn exists_returns_false_on_not_found() {
        let fake = FakeSftpClient::arc();
        fake.set_try_exists(Ok(false)).await;
        let provider = make_provider(fake.clone());
        assert!(!provider.exists(Path::new("/remote/missing")).await);
    }

    #[tokio::test]
    async fn read_to_string_decodes_utf8() {
        let fake = FakeSftpClient::arc();
        fake.set_read(Ok(b"hello world".to_vec())).await;
        let provider = make_provider(fake.clone());
        let text = provider
            .read_to_string(Path::new("/remote/a.txt"))
            .await
            .expect("ok");
        assert_eq!(text, "hello world");
    }

    #[tokio::test]
    async fn read_to_string_maps_permission_denied() {
        let fake = FakeSftpClient::arc();
        fake.set_read(Err(SftpClientError::PermissionDenied)).await;
        let provider = make_provider(fake.clone());
        let err = provider
            .read_to_string(Path::new("/remote/locked"))
            .await
            .expect_err("err");
        match err {
            FsError::Io { source, .. } => {
                assert_eq!(source.kind(), std::io::ErrorKind::PermissionDenied);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn stat_returns_not_found_on_missing() {
        let fake = FakeSftpClient::arc();
        fake.set_metadata(Err(SftpClientError::NoSuchFile)).await;
        let provider = make_provider(fake.clone());
        let err = provider
            .stat(Path::new("/remote/nope"))
            .await
            .expect_err("err");
        assert!(matches!(err, FsError::NotFound(_)));
    }

    #[tokio::test]
    async fn stat_returns_metadata_on_success() {
        let fake = FakeSftpClient::arc();
        let now = SystemTime::now();
        fake.set_metadata(Ok(FsMetadata {
            size: 42,
            mtime: now,
        }))
        .await;
        let provider = make_provider(fake.clone());
        let meta = provider.stat(Path::new("/remote/a")).await.expect("ok");
        assert_eq!(meta.size, 42);
        assert_eq!(meta.mtime, now);
    }

    #[tokio::test]
    async fn read_dir_maps_remote_entries() {
        let fake = FakeSftpClient::arc();
        fake.set_read_dir(Ok(vec![
            RemoteEntry {
                name: "a.jsonl".into(),
                kind: EntryKind::File,
                metadata: Some(FsMetadata {
                    size: 100,
                    mtime: SystemTime::UNIX_EPOCH,
                }),
                mtime_missing: true,
            },
            RemoteEntry {
                name: "sub".into(),
                kind: EntryKind::Dir,
                metadata: None,
                mtime_missing: false,
            },
        ]))
        .await;
        let provider = make_provider(fake.clone());
        let mut entries = provider
            .read_dir(Path::new("/remote/projects"))
            .await
            .expect("ok");
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "a.jsonl");
        assert_eq!(entries[0].kind, EntryKind::File);
        assert_eq!(entries[0].metadata.unwrap().size, 100);
        assert_eq!(entries[1].kind, EntryKind::Dir);
    }

    #[tokio::test]
    async fn read_lines_head_returns_first_n() {
        let fake = FakeSftpClient::arc();
        fake.set_read_lines(Ok(vec!["a".into(), "b".into()])).await;
        let provider = make_provider(fake.clone());
        let lines = provider
            .read_lines_head(Path::new("/remote/multi"), 2)
            .await
            .expect("ok");
        assert_eq!(lines, vec!["a".to_string(), "b".to_string()]);
    }

    #[tokio::test]
    async fn read_to_string_retries_transient_then_succeeds() {
        // Transient 失败 2 次后第 3 次成功——验证 with_retry 重试路径。
        let fake = FakeSftpClient::arc();
        fake.set_transient_then_ok(2, b"recovered".to_vec()).await;
        let provider = make_provider(fake.clone());
        let text = provider
            .read_to_string(Path::new("/remote/flaky"))
            .await
            .expect("ok after retries");
        assert_eq!(text, "recovered");
        // counter == 3：fetch_add 把 counter 推到 3（call 1/2 失败 + call 3 成功）。
        assert_eq!(fake.transient_call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn read_to_string_gives_up_after_max_transient() {
        // 4 次失败超出 MAX_RETRY_ATTEMPTS=3；最后返 transient 错误。
        let fake = FakeSftpClient::arc();
        fake.set_transient_then_ok(10, b"never".to_vec()).await;
        let provider = make_provider(fake.clone());
        let err = provider
            .read_to_string(Path::new("/remote/perma-flaky"))
            .await
            .expect_err("transient exhausted");
        match err {
            FsError::Io { source, .. } => {
                assert!(source.to_string().contains("transient"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn open_read_stream_unsupported_in_fake_path() {
        // `russh_sftp::client::fs::File` 不实现 Debug，所以不能用 `expect_err`——
        // 手工 match 取错误。
        let fake = FakeSftpClient::arc();
        let provider = make_provider(fake);
        match provider.open_read_stream(Path::new("/remote/a")).await {
            Ok(_) => panic!("fake path should not support open_read_stream"),
            Err(FsError::Unsupported(reason)) => assert_eq!(reason, "open_read_stream"),
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn sftp_client_error_transience() {
        assert!(SftpClientError::Transient("retry".into()).is_transient());
        assert!(!SftpClientError::NoSuchFile.is_transient());
        assert!(!SftpClientError::PermissionDenied.is_transient());
        assert!(!SftpClientError::Other("permanent".into()).is_transient());
    }

    #[test]
    fn classify_sftp_status_codes() {
        use russh_sftp::protocol::Status;
        let mk = |code, msg: &str| {
            classify_sftp_error(&SftpError::Status(Status {
                id: 0,
                status_code: code,
                error_message: msg.into(),
                language_tag: String::new(),
            }))
        };
        assert!(matches!(
            mk(StatusCode::NoSuchFile, ""),
            SftpClientError::NoSuchFile
        ));
        assert!(matches!(
            mk(StatusCode::PermissionDenied, ""),
            SftpClientError::PermissionDenied
        ));
        assert!(matches!(
            mk(StatusCode::Failure, "boom"),
            SftpClientError::Transient(_)
        ));
        assert!(matches!(
            mk(StatusCode::OpUnsupported, ""),
            SftpClientError::Other(_)
        ));
    }

    #[test]
    fn classify_sftp_io_transient_strings() {
        assert!(matches!(
            classify_sftp_error(&SftpError::IO("connection reset by peer".into())),
            SftpClientError::Transient(_)
        ));
        assert!(matches!(
            classify_sftp_error(&SftpError::IO("broken pipe".into())),
            SftpClientError::Transient(_)
        ));
        assert!(matches!(
            classify_sftp_error(&SftpError::IO("operation timed out".into())),
            SftpClientError::Transient(_)
        ));
        assert!(matches!(
            classify_sftp_error(&SftpError::IO("no such device".into())),
            SftpClientError::Other(_)
        ));
        assert!(matches!(
            classify_sftp_error(&SftpError::Timeout),
            SftpClientError::Transient(_)
        ));
    }
}
