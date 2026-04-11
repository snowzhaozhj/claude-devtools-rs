//! 本 crate 的错误类型。
//!
//! 设计决策见 `openspec/changes/port-project-discovery/design.md` §决策 6。

use std::path::PathBuf;

use thiserror::Error;

/// 文件系统层错误 —— 所有 `FileSystemProvider` 实现必须把下游 I/O 错误
/// 投影到这里。
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
}

/// discovery 流水线的顶层错误。
///
/// "根目录不存在"不走 `Err` —— 它在 scanner 内部被吸收为"空列表 + warn"，
/// 对齐 spec 的 `Root directory missing` scenario。
#[derive(Debug, Error)]
pub enum DiscoverError {
    #[error(transparent)]
    Fs(#[from] FsError),
    #[error("git command failed: {0}")]
    Git(String),
    #[error(transparent)]
    Parse(#[from] cdt_parse::ParseError),
}
