//! 文件级异步解析：流式读行 + 容忍坏行 + 尾端 requestId 去重。

use std::path::{Path, PathBuf};

use cdt_core::ParsedMessage;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::dedupe::dedupe_by_request_id;
use crate::error::ParseError;
use crate::parser::parse_entry_at;

/// 解析一个完整的 JSONL 会话文件。
///
/// - 用 `tokio::fs` 按行流式读取，避免一次性把原始字节加载进内存。
/// - 收集每一条成功解析出的 `ParsedMessage`：坏行会 `tracing::warn!` 后
///   跳过，空文件直接返回空 Vec，不抛错。
/// - 在返回前对收集到的列表跑一遍 `dedupe_by_request_id`，顺便修掉
///   TS 版"函数存在但从未被调用"的 impl-bug。
///
/// 返回值保持文件顺序（去重后）。
pub async fn parse_file(path: impl AsRef<Path>) -> Result<Vec<ParsedMessage>, ParseError> {
    let path: PathBuf = path.as_ref().to_path_buf();
    let file = File::open(&path).await.map_err(|e| ParseError::Io {
        path: path.clone(),
        source: e,
    })?;

    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut out = Vec::new();
    let mut line_no: usize = 0;

    loop {
        let next = lines.next_line().await.map_err(|e| ParseError::Io {
            path: path.clone(),
            source: e,
        })?;
        let Some(line) = next else { break };
        line_no += 1;
        if line.trim().is_empty() {
            continue;
        }
        match parse_entry_at(&line, line_no) {
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

    Ok(dedupe_by_request_id(out))
}
