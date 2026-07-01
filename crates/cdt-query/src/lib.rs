//! Query orchestration layer for CLI / MCP consumers.
//!
//! Wraps `LocalDataApi` and provides:
//! - `SessionQueryOptions`: range/tail/filter for session detail
//! - `QueryEngine`: high-level operations (show, detail, errors, search)

#![forbid(unsafe_code)]

pub mod cost;
mod engine;
mod error;
pub mod extract;
mod options;
pub mod stats;
pub mod step;
pub mod summary;
pub mod token;
pub mod turn_view;

pub use engine::QueryEngine;
pub use error::QueryError;
pub use extract::{ChunkOverviewEntry, ToolExecEntry};
pub use options::{ChunkKindFilter, SessionQueryOptions};
pub use token::{CharRatioEstimator, TokenEstimator};
