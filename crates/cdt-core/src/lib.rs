//! claude-devtools-rs 的共享类型与 trait。
//!
//! 本 crate 是整个 workspace 的地基：承载跨 capability 边界的类型
//! （`ParsedMessage`、`ContentBlock`、`ToolCall`、`TokenUsage` 等），
//! 也是其他 crate 唯一可以无代价依赖的基础 crate
//! （任何 crate 依赖它都不会被动引入运行时设施）。
//!
//! 不变式（见 `openspec/specs/rust-workspace-layout/spec.md`）：
//! - 禁止依赖 `tokio`、`axum`、`notify`、`ssh2`、`reqwest` 等任何运行时
//!   基础设施 crate。
//! - 必须能在同步单元测试里直接使用，不需要异步运行时。
//! - 任何被两个及以上 capability crate 使用的类型都应放在这里，避免重复定义。

pub mod chunk;
pub mod message;
pub mod process;
pub mod tool_execution;

pub use chunk::{
    AIChunk, AssistantResponse, Chunk, ChunkMetrics, CompactChunk, SemanticStep, SystemChunk,
    UserChunk,
};
pub use message::{
    ContentBlock, HardNoiseReason, ImageSource, MessageCategory, MessageContent, MessageType,
    ParsedMessage, TokenUsage, ToolCall, ToolResult,
};
pub use process::{Process, SubagentCandidate, TeamMeta};
pub use tool_execution::{ToolExecution, ToolOutput};

pub mod prelude {
    //! 给消费方用的再导出集合。
    pub use super::chunk::{
        AIChunk, AssistantResponse, Chunk, ChunkMetrics, CompactChunk, SemanticStep, SystemChunk,
        UserChunk,
    };
    pub use super::message::{
        ContentBlock, HardNoiseReason, ImageSource, MessageCategory, MessageContent, MessageType,
        ParsedMessage, TokenUsage, ToolCall, ToolResult,
    };
    pub use super::process::{Process, SubagentCandidate, TeamMeta};
    pub use super::tool_execution::{ToolExecution, ToolOutput};
}
