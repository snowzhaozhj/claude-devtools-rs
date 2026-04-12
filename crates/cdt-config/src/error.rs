//! `cdt-config` 的错误类型。

use std::path::PathBuf;

/// configuration-management capability 的错误枚举。
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// 文件系统 I/O 错误。
    #[error("config I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    /// JSON 序列化/反序列化错误。
    #[error("config JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// 配置字段校验失败。
    #[error("config validation error: {0}")]
    Validation(String),

    /// `@mention` 路径逃逸（沙盒外）。
    #[error("path escape: {0}")]
    PathEscape(String),
}

impl ConfigError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn path_escape(msg: impl Into<String>) -> Self {
        Self::PathEscape(msg.into())
    }
}
