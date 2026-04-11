//! context-tracking capability 的 Rust 实现。
//!
//! Spec：`openspec/specs/context-tracking/spec.md`。
//!
//! 入口：
//! - [`compute_context_stats`]：对单个 `AIChunk` 聚合 6 类 injection、产出
//!   per-turn `ContextStats`；纯函数。
//! - [`process_session_context_with_phases`]：对整条 session（`&[Chunk]`）
//!   做 phase 管理 + compaction delta，产出 `SessionContextResult`。
//!
//! 本 crate 保持同步，不依赖 tokio，外部需要的 token 字典通过参数注入；
//! 真实读 CLAUDE.md 文件由 `port-configuration-management` 负责。

mod aggregator;
mod session;
mod stats;
mod types;

pub use session::{
    ProcessSessionParams, SessionContextResult, process_session_context_with_phases,
};
pub use stats::{ComputeStatsParams, ComputeStatsResult, compute_context_stats};
pub use types::TokenDictionaries;
