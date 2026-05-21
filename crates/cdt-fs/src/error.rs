//! 文件系统层错误 —— 所有 `FileSystemProvider` 实现必须把下游 I/O 错误投影到这里。
//!
//! Spec：`openspec/specs/fs-abstraction/spec.md` §`FsError` 提供错误语义元方法。

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsError {
    #[error("path not found: {0}")]
    NotFound(PathBuf),
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("utf-8 decode error at {path}: {source}")]
    Utf8 {
        path: PathBuf,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("unsupported operation: {0}")]
    Unsupported(&'static str),
    /// 连接级故障（典型 SSH session 断开）—— `is_retryable() == true`，
    /// `should_invalidate_cache() == false`（数据仍可能有效，只是当前连不上）。
    #[error("connection disconnected at {path}: {reason}")]
    Disconnected { path: PathBuf, reason: String },
    /// 瞬时错误重试耗尽（典型 SSH `with_retry` 跑完 3 次仍 Transient）——
    /// `is_retryable() == false`（已经重试过了），`should_invalidate_cache() == false`
    /// （远端可能恢复）。
    #[error("transient errors exhausted at {path} after {attempts} attempts: {last_reason}")]
    TransientExhausted {
        path: PathBuf,
        attempts: u32,
        last_reason: String,
    },
}

impl FsError {
    /// 该错误是否值得调用方主动重试一次。
    ///
    /// - `NotFound` / `TransientExhausted` / `Utf8` / `Unsupported`：false（永久错误）
    /// - `Io`：根据 `source.kind()` 判定（`Interrupted` / `WouldBlock` 等可重试）
    /// - `Disconnected`：true（重连后可能恢复）
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            FsError::NotFound(_)
            | FsError::Utf8 { .. }
            | FsError::Unsupported(_)
            | FsError::TransientExhausted { .. } => false,
            FsError::Io { source, .. } => matches!(
                source.kind(),
                std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe
            ),
            FsError::Disconnected { .. } => true,
        }
    }

    /// 该错误是否意味着对应 path 的 cache entry 应被清掉。
    ///
    /// - `NotFound`：true（文件不存在）
    /// - `Utf8`：true（文件损坏）
    /// - 其它：false（数据可能仍有效，仅是临时连接 / IO 问题）
    #[must_use]
    pub fn should_invalidate_cache(&self) -> bool {
        match self {
            FsError::NotFound(_) | FsError::Utf8 { .. } => true,
            FsError::Io { .. }
            | FsError::Unsupported(_)
            | FsError::Disconnected { .. }
            | FsError::TransientExhausted { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn not_found_not_retryable_and_invalidates() {
        let err = FsError::NotFound(PathBuf::from("/missing"));
        assert!(!err.is_retryable());
        assert!(err.should_invalidate_cache());
    }

    #[test]
    fn disconnected_retryable_but_keeps_cache() {
        let err = FsError::Disconnected {
            path: PathBuf::from("/p"),
            reason: "channel closed".into(),
        };
        assert!(err.is_retryable());
        assert!(!err.should_invalidate_cache());
    }

    #[test]
    fn transient_exhausted_neither_retryable_nor_invalidates() {
        let err = FsError::TransientExhausted {
            path: PathBuf::from("/p"),
            attempts: 3,
            last_reason: "connection reset".into(),
        };
        assert!(!err.is_retryable());
        assert!(!err.should_invalidate_cache());
    }

    #[test]
    fn utf8_not_retryable_and_invalidates() {
        let err = FsError::Utf8 {
            path: PathBuf::from("/p"),
            source: String::from_utf8(vec![0xFFu8]).unwrap_err(),
        };
        assert!(!err.is_retryable());
        assert!(err.should_invalidate_cache());
    }

    #[test]
    fn unsupported_neither_retryable_nor_invalidates() {
        let err = FsError::Unsupported("op");
        assert!(!err.is_retryable());
        assert!(!err.should_invalidate_cache());
    }

    #[test]
    fn io_retryable_kinds_route_to_true() {
        let mk = |kind| FsError::Io {
            path: PathBuf::from("/p"),
            source: io::Error::new(kind, "x"),
        };
        assert!(mk(io::ErrorKind::Interrupted).is_retryable());
        assert!(mk(io::ErrorKind::WouldBlock).is_retryable());
        assert!(mk(io::ErrorKind::TimedOut).is_retryable());
        assert!(mk(io::ErrorKind::ConnectionReset).is_retryable());
        assert!(mk(io::ErrorKind::BrokenPipe).is_retryable());
    }

    #[test]
    fn io_permanent_kinds_route_to_false() {
        let mk = |kind| FsError::Io {
            path: PathBuf::from("/p"),
            source: io::Error::new(kind, "x"),
        };
        assert!(!mk(io::ErrorKind::PermissionDenied).is_retryable());
        assert!(!mk(io::ErrorKind::InvalidData).is_retryable());
        assert!(!mk(io::ErrorKind::Other).is_retryable());
    }

    #[test]
    fn io_does_not_invalidate_cache() {
        let err = FsError::Io {
            path: PathBuf::from("/p"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        };
        assert!(!err.should_invalidate_cache());
    }
}
