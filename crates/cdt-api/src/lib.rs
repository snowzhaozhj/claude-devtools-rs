//! Data API facade.
//!
//! Owns two baseline capabilities:
//! - **ipc-data-api** — trait surface exposing the full operation set
//!   (projects, sessions, search, config, notifications, ssh, context,
//!   validation, auxiliary reads). Transport-agnostic.
//! - **http-data-api** — axum HTTP/SSE server under `/api` prefix,
//!   mirrors the `DataApi` trait for web/remote clients.

pub mod http;
pub mod ipc;
pub mod notifier;

pub use http::{AppState, spawn_event_bridge, start_server};
pub use ipc::{
    ApiError, ApiErrorCode, ConfigUpdateRequest, ContextInfo, DataApi, LocalDataApi,
    METADATA_SCAN_CONCURRENCY, PaginatedRequest, PaginatedResponse, ProjectInfo,
    ProjectSessionPrefs, PushEvent, SearchRequest, SessionDetail, SessionMetadataUpdate,
    SessionSummary, SshConnectRequest,
};
pub use notifier::NotificationPipeline;
