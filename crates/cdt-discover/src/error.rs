//! cdt-discover 顶层错误。
//!
//! `FsError` 的真相源在 `cdt-fs` crate（capability `fs-abstraction`），这里
//! 通过 `pub use` re-export 兼容历史 import 路径（如 `use cdt_discover::FsError`）。
//! 新代码 SHOULD 直接 `use cdt_fs::FsError`。

use thiserror::Error;

pub use cdt_fs::FsError;

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
