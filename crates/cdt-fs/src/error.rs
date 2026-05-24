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

/// Transport-dead 关键字单源——任一 lowercase 后命中的字符串视为底层
/// transport channel（典型 SSH SFTP）已死/半死。
///
/// caller SHALL 据此 fail-fast 而非凑半成品列表。caller 含：
/// - [`FsError::is_likely_channel_dead`]（`TransientExhausted` 分支）
/// - `cdt-ssh::polling_watcher::classify_failure`（`PollFailureKind::Permanent`
///   判定，crate-level transport-dead vs timeout-class 三态分流）
///
/// 不覆盖 `cdt-ssh::provider::is_transient_io_reason`——那里走 transport-dead ∪
/// timeout-class 并集（缺 `session closed` / `eof`，多 `timed out` /
/// `etimedout`），是 SFTP 字符串 `io::Error` → `Transient` 归类的子集，
/// 与本函数语义独立。
#[must_use]
pub fn is_transport_dead_reason(reason_lowercase: &str) -> bool {
    reason_lowercase.contains("session closed")
        || reason_lowercase.contains("eof")
        || reason_lowercase.contains("broken pipe")
        || reason_lowercase.contains("epipe")
        || reason_lowercase.contains("connection reset")
        || reason_lowercase.contains("econnreset")
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

    /// 该错误是否暗示底层 transport channel（典型 SSH SFTP）已死 / 半死。
    ///
    /// caller（典型 `cdt-discover::ProjectScanner` SSH 分支）SHALL 据此 fail-fast
    /// abort 整轮 scan 而非 silent continue 凑半成品列表，让上层（`list_repository_groups`
    /// → IPC caller）拿到 hard error 触发自愈路径而非误以为 scan 已完成。
    ///
    /// 语义独立于 [`Self::is_retryable`]：channel-dead 是更强的"该不该 abort 整轮 scan"
    /// 信号——`Disconnected.is_retryable() == true` 但 `is_likely_channel_dead() == true`
    /// 同时成立（表达"重连后能 retry，但当前 scan 已不该继续"）。
    ///
    /// 命中规则（spec `fs-abstraction::FsError 提供错误语义元方法`）：
    /// - `Disconnected`：恒 true
    /// - `TransientExhausted { last_reason }`：含 `session closed` / `eof` / `broken pipe`
    ///   / `epipe` / `connection reset` / `econnreset` 任一 transport-dead 关键字时 true；
    ///   纯 `timeout` / `eagain` / `would block` 返 false（保留容错语义，留给 polling
    ///   watcher 的独立 timeout counter 在持续 18s 后自行触发 `dead_signal`）
    /// - `Io { source }`：`source.kind()` 是 `BrokenPipe` / `ConnectionReset` /
    ///   `ConnectionAborted` 时 true
    /// - `NotFound` / `Utf8` / `Unsupported`：恒 false（与 channel 状态无关）
    #[must_use]
    pub fn is_likely_channel_dead(&self) -> bool {
        match self {
            FsError::Disconnected { .. } => true,
            FsError::TransientExhausted { last_reason, .. } => {
                is_transport_dead_reason(&last_reason.to_ascii_lowercase())
            }
            FsError::Io { source, .. } => matches!(
                source.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
            ),
            FsError::NotFound(_) | FsError::Utf8 { .. } | FsError::Unsupported(_) => false,
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

    /// spec `fs-abstraction::FsError 提供错误语义元方法` Scenario "Disconnected
    /// 触发 channel-dead"。
    #[test]
    fn is_likely_channel_dead_classifies_disconnected() {
        let err = FsError::Disconnected {
            path: PathBuf::from("/p"),
            reason: "anything".into(),
        };
        assert!(err.is_likely_channel_dead());
        // 与 is_retryable 语义独立：Disconnected 同时是 retryable
        assert!(err.is_retryable());
    }

    /// spec `fs-abstraction::FsError 提供错误语义元方法` Scenario `TransientExhausted`
    /// 含 transport-dead 关键字触发 channel-dead。
    #[test]
    fn is_likely_channel_dead_classifies_transient_exhausted_with_transport_dead_keyword() {
        for reason in [
            "session closed",
            "Eof",
            "broken pipe",
            "EPIPE while writing",
            "connection reset by peer",
            "ECONNRESET",
        ] {
            let err = FsError::TransientExhausted {
                path: PathBuf::from("/p"),
                attempts: 3,
                last_reason: reason.into(),
            };
            assert!(
                err.is_likely_channel_dead(),
                "transport-dead keyword {reason:?} SHALL trigger is_likely_channel_dead",
            );
        }
    }

    /// spec `fs-abstraction::FsError 提供错误语义元方法` Scenario `TransientExhausted`
    /// 仅含纯 timeout 不触发 channel-dead。
    #[test]
    fn is_likely_channel_dead_pure_timeout_returns_false() {
        for reason in ["timeout", "ETIMEDOUT", "timed out", "EAGAIN", "would block"] {
            let err = FsError::TransientExhausted {
                path: PathBuf::from("/p"),
                attempts: 3,
                last_reason: reason.into(),
            };
            assert!(
                !err.is_likely_channel_dead(),
                "pure timeout-class reason {reason:?} SHALL NOT trigger is_likely_channel_dead",
            );
        }
    }

    /// spec `fs-abstraction::FsError 提供错误语义元方法` Scenario `Io` `BrokenPipe` /
    /// `ConnectionReset` / `ConnectionAborted` 触发 channel-dead。
    #[test]
    fn is_likely_channel_dead_io_kinds() {
        let mk = |kind: io::ErrorKind| FsError::Io {
            path: PathBuf::from("/p"),
            source: io::Error::new(kind, "x"),
        };
        assert!(mk(io::ErrorKind::BrokenPipe).is_likely_channel_dead());
        assert!(mk(io::ErrorKind::ConnectionReset).is_likely_channel_dead());
        assert!(mk(io::ErrorKind::ConnectionAborted).is_likely_channel_dead());
        // 非 transport-dead IO kind 不触发
        assert!(!mk(io::ErrorKind::PermissionDenied).is_likely_channel_dead());
        assert!(!mk(io::ErrorKind::TimedOut).is_likely_channel_dead());
        assert!(!mk(io::ErrorKind::WouldBlock).is_likely_channel_dead());
    }

    /// spec `fs-abstraction::FsError 提供错误语义元方法` Scenario `NotFound` / `Utf8` /
    /// `Unsupported` 不触发 channel-dead。
    #[test]
    fn is_likely_channel_dead_notfound_utf8_unsupported_returns_false() {
        assert!(!FsError::NotFound(PathBuf::from("/p")).is_likely_channel_dead());
        assert!(
            !FsError::Utf8 {
                path: PathBuf::from("/p"),
                source: String::from_utf8(vec![0xFFu8]).unwrap_err(),
            }
            .is_likely_channel_dead()
        );
        assert!(!FsError::Unsupported("op").is_likely_channel_dead());
    }
}
