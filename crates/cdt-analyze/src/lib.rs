//! Semantic analysis layer over parsed messages.
//!
//! Owns four baseline capabilities (see `openspec/specs/`):
//! - **chunk-building** — builds independent `UserChunk` / `AIChunk` /
//!   `SystemChunk` / `CompactChunk` from a stream of `ParsedMessage`s.
//!   See `openspec/specs/chunk-building/spec.md`.
//! - **tool-execution-linking** — pairs `tool_use` with `tool_result` by id;
//!   resolves Task calls to subagent sessions via a three-phase fallback
//!   (result-based → description-based → positional); builds tool execution
//!   records with error state.
//! - **context-tracking** — classifies context injections into six categories
//!   (claude-md / mentioned-file / tool-output / thinking-text /
//!   team-coordination / user-message) and accumulates per-turn stats with
//!   compaction phase resets.
//! - **team-coordination-metadata** — detects teammate messages, enriches
//!   `Process.team`, routes team coordination tools through a dedicated
//!   summary formatter.
//!
//! Port notes from `openspec/followups.md`:
//! - **Fix, don't replicate**: Task-tool filtering when a subagent is resolved
//!   must actually happen in `AIChunk` construction (the TS impl forgets).
//! - **Fix, don't replicate**: duplicate `tool_use_id` must log a warning.

pub mod chunk;
pub mod context;
pub mod session_state;
pub mod tool_linking;

pub use chunk::{
    aggregate_metrics, build_chunks, build_chunks_with_subagents, extract_semantic_steps,
};
pub use context::{
    ComputeStatsParams, ComputeStatsResult, ProcessSessionParams, SessionContextResult,
    compute_context_stats, process_session_context_with_phases,
};
pub use session_state::check_messages_ongoing;
pub use tool_linking::{
    Resolution, ResolvedTask, ToolLinkingResult, filter_resolved_tasks, pair_tool_executions,
    resolve_subagents,
};

pub mod team;
