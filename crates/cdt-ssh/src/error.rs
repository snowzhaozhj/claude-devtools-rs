//! SSH 错误类型。

use std::path::PathBuf;

/// `ssh-remote-context` capability 的错误枚举。
#[derive(Debug, thiserror::Error)]
pub enum SshError {
    /// SSH config 解析错误。
    #[error("SSH config error: {0}")]
    Config(String),

    /// 连接错误。
    #[error("SSH connection error: {0}")]
    Connection(String),

    /// 认证失败。
    #[error("SSH auth error: {0}")]
    Auth(String),

    /// SFTP 操作错误。
    #[error("SFTP error at {path}: {message}")]
    Sftp { path: PathBuf, message: String },

    /// I/O 错误。
    #[error("SSH I/O error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },
}
