//! Query orchestration layer for CLI / MCP consumers.
//!
//! Wraps `LocalDataApi` and provides:
//! - `SessionQueryOptions`: range/tail/filter for session detail
//! - `QueryFilter`: cross-session filtering (since, grep, `errors_only`, etc.)
//! - `QueryEngine`: high-level operations (show, detail, errors, search)

#![forbid(unsafe_code)]

pub mod cost;
mod engine;
mod error;
mod filter;
mod options;
pub mod stats;
pub mod summary;
pub mod token;

pub use engine::QueryEngine;
pub use error::QueryError;
pub use filter::QueryFilter;
pub use options::{ChunkKindFilter, SessionQueryOptions};
pub use token::{CharRatioEstimator, TokenEstimator};
