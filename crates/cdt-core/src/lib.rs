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
pub mod context;
pub mod message;
pub mod process;
pub mod project;
pub mod search;
pub mod tokens;
pub mod tool_execution;
pub mod watch_event;

pub use chunk::{
    AIChunk, AssistantResponse, Chunk, ChunkMetrics, CompactChunk, SemanticStep, SlashCommand,
    SystemChunk, TeammateMessage, UserChunk,
};
pub use context::{
    ClaudeMdContextInjection, ClaudeMdFileInfo, ClaudeMdScope, CompactionTokenDelta,
    ContextInjection, ContextPhase, ContextPhaseInfo, ContextStats, CountsByCategory,
    MentionedFileInfo, MentionedFileInjection, TaskCoordinationBreakdown,
    TaskCoordinationInjection, TaskCoordinationKind, ThinkingTextBreakdown, ThinkingTextInjection,
    ThinkingTextKind, TokensByCategory, ToolOutputInjection, ToolTokenBreakdown,
    UserMessageInjection,
};
pub use message::{
    ContentBlock, HardNoiseReason, ImageSource, MessageCategory, MessageContent, MessageType,
    ParsedMessage, TokenUsage, ToolCall, ToolResult,
};
pub use process::{MainSessionImpact, Process, SubagentCandidate, TeamMeta};
pub use project::{
    Project, RepositoryGroup, RepositoryIdentity, Session, SessionMetadata, Worktree,
};
pub use search::{SearchHit, SearchSessionsResult, SessionSearchResult};
pub use tokens::{estimate_content_tokens, estimate_tokens};
pub use tool_execution::{TeammateSpawnInfo, ToolExecution, ToolOutput};
pub use watch_event::{FileChangeEvent, TodoChangeEvent};

pub mod prelude {
    //! 给消费方用的再导出集合。
    pub use super::chunk::{
        AIChunk, AssistantResponse, Chunk, ChunkMetrics, CompactChunk, SemanticStep, SystemChunk,
        TeammateMessage, UserChunk,
    };
    pub use super::message::{
        ContentBlock, HardNoiseReason, ImageSource, MessageCategory, MessageContent, MessageType,
        ParsedMessage, TokenUsage, ToolCall, ToolResult,
    };
    pub use super::process::{MainSessionImpact, Process, SubagentCandidate, TeamMeta};
    pub use super::project::{
        Project, RepositoryGroup, RepositoryIdentity, Session, SessionMetadata, Worktree,
    };
    pub use super::tool_execution::{TeammateSpawnInfo, ToolExecution, ToolOutput};
}
