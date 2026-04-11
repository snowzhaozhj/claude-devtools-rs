//! JSONL 会话解析。
//!
//! 本 crate 拥有 **session-parsing** capability（见
//! `openspec/specs/session-parsing/spec.md`），职责包括：
//! - 流式按行读取 JSONL，容忍坏行；
//! - 把每行转换为 `cdt_core::ParsedMessage`；
//! - Hard-noise 分类（synthetic 占位、interrupt 标记、`<local-command-caveat>` /
//!   `<system-reminder>` 包裹等）；
//! - `requestId` 去重——TS 版定义了函数却从未调用，Rust 版无条件接进
//!   `parse_file` 的主路径。

pub mod error;

pub(crate) mod dedupe;
pub(crate) mod file;
pub(crate) mod noise;
pub(crate) mod parser;

pub use dedupe::dedupe_by_request_id;
pub use error::ParseError;
pub use file::parse_file;
pub use parser::{parse_entry, parse_entry_at};
