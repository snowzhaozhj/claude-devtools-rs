//! chunk-building capability。
//!
//! Spec：`openspec/specs/chunk-building/spec.md`。
//!
//! 本模块把 `cdt_core::ParsedMessage` 流按语义切成独立 `Chunk`，
//! 不做 tool 链接、Task 过滤或 subagent 归集——那些留给后续 port。

mod builder;
mod metrics;
mod promote;
mod semantic;

pub use builder::{build_chunks, build_chunks_with_subagents};
pub use metrics::aggregate_metrics;
pub use promote::promote_result_agent_tasks;
pub use semantic::extract_semantic_steps;
