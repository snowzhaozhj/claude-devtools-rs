//! Data API facade.
//!
//! Owns two baseline capabilities:
//! - **ipc-data-api** — trait surface exposing the full operation set
//!   (projects, sessions, search, config, notifications, ssh, context,
//!   validation, auxiliary reads). Transport-agnostic.
//! - **http-data-api** — axum HTTP/SSE server under `/api` prefix,
//!   mirrors the `DataApi` trait for web/remote clients.

pub(crate) mod cache_signature;
pub mod http;
pub mod ipc;
pub mod notifier;

pub use http::{
    AppState, StaticServe, build_router, serve_with_listener, spawn_event_bridge, start_server,
};
pub use ipc::session_metadata::TITLE_MAX_CHARS;
pub use ipc::{
    ApiError, ApiErrorCode, ConfigUpdateRequest, ContextInfo, CorrectnessEventItem, DataApi,
    LocalDataApi, METADATA_SCAN_CONCURRENCY, MemoryFileContent, MemoryLayer, MemoryLayerKind,
    PaginatedRequest, PaginatedResponse, ProjectInfo, ProjectMemory, ProjectSessionPrefs,
    PushEvent, SearchRequest, SessionDetail, SessionMetadataUpdate, SessionSummary, SshAuthMethod,
    SshConnectRequest, SshConnectionResult, SshState, WslDistroCandidate, WslDistroScanReport,
};
pub use notifier::NotificationPipeline;
