//! session-parsing 的错误类型。

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("读取 {path} 时发生 IO 错误：{source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// 某一行无法作为 JSON 解析。`line` 为 1-based 行号。
    #[error("第 {line} 行 JSON 格式错误：{source}")]
    MalformedLine {
        line: usize,
        #[source]
        source: serde_json::Error,
    },

    /// JSON 解析成功，但字段形状与期望的 `ChatHistoryEntry` 不符。
    /// 有文件上下文时 `line` 为 1-based 行号；`parse_entry` 无文件
    /// 上下文时传 `0`。
    #[error("第 {line} 行结构不匹配：{reason}")]
    SchemaMismatch { line: usize, reason: String },
}
