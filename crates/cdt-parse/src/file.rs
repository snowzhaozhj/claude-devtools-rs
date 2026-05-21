//! 文件级异步解析：流式读行 + 容忍坏行。
//!
//! - `parse_file_via_fs(fs, path)` 是 SSH-aware 入口：通过 `FileSystemProvider::open_read`
//!   拿 `Box<dyn AsyncRead + Send + Unpin>`，BufReader 容量 32 KiB 与 SFTP packet 上限对齐
//!   （详 change `unify-fs-direct-calls` design D5）。
//! - `parse_file(path)` 是兼容 Local-only 入口，内部包装到 `parse_file_via_fs(local_handle(), path)`。

use std::path::{Path, PathBuf};

use cdt_core::ParsedMessage;
use cdt_fs::FileSystemProvider;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::ParseError;
use crate::parser::parse_entry_at;

/// BufReader 容量 —— 与 SFTP `SSH_FXP_READ` reply 单消息上限对齐。
/// 详 change `unify-fs-direct-calls` design D5。
const SCANNER_BUF_BYTES: usize = 32 * 1024;

/// 通过 fs trait 解析 JSONL 会话文件 —— SSH-aware 入口。
///
/// - 用 `fs.open_read(path)` 拿 `Box<dyn AsyncRead + Send + Unpin>`，
///   BufReader 容量 32 KiB 与 SFTP packet 上限对齐
/// - 收集每一条成功解析的 `ParsedMessage`：坏行 `tracing::warn!` 后跳过
/// - **不**对同 `requestId` 去重（详 `parse_file` doc 内同段说明）
///
/// 返回值保持文件顺序。
pub async fn parse_file_via_fs(
    fs: &dyn FileSystemProvider,
    path: impl AsRef<Path>,
) -> Result<Vec<ParsedMessage>, ParseError> {
    let path: PathBuf = path.as_ref().to_path_buf();
    let reader = fs.open_read(&path).await.map_err(|e| ParseError::Io {
        path: path.clone(),
        source: std::io::Error::other(format!("fs.open_read failed: {e}")),
    })?;

    let mut buf = BufReader::with_capacity(SCANNER_BUF_BYTES, reader);
    let mut out = Vec::new();
    let mut line_no: usize = 0;
    let mut line = String::new();

    loop {
        line.clear();
        let n = buf.read_line(&mut line).await.map_err(|e| ParseError::Io {
            path: path.clone(),
            source: e,
        })?;
        if n == 0 {
            break;
        }
        line_no += 1;
        let trimmed_line = line.trim_end_matches(['\n', '\r']);
        if trimmed_line.trim().is_empty() {
            continue;
        }
        match parse_entry_at(trimmed_line, line_no) {
            Ok(Some(msg)) => out.push(msg),
            Ok(None) => {}
            Err(ParseError::MalformedLine { line, source }) => {
                tracing::warn!(
                    file = %path.display(),
                    line,
                    error = %source,
                    "skipping malformed JSONL line"
                );
            }
            Err(ParseError::SchemaMismatch { line, reason }) => {
                tracing::warn!(
                    file = %path.display(),
                    line,
                    reason = %reason,
                    "skipping entry with schema mismatch"
                );
            }
            Err(e @ ParseError::Io { .. }) => return Err(e),
        }
    }

    Ok(out)
}

/// 解析一个完整的 JSONL 会话文件（Local-only 兼容入口）。
///
/// 内部包装到 `parse_file_via_fs(cdt_fs::local_handle(), path)`，与原 `tokio::fs::File`
/// 路径行为完全等价。新代码 SHALL 优先调 `parse_file_via_fs` 接受 fs trait 注入。
pub async fn parse_file(path: impl AsRef<Path>) -> Result<Vec<ParsedMessage>, ParseError> {
    let fs = cdt_fs::local_handle();
    parse_file_via_fs(&*fs, path).await
}
