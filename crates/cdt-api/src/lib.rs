//! Data API facade.
//!
//! Owns two baseline capabilities:
//! - **ipc-data-api** — trait surface exposing the full operation set
//!   (projects, sessions, search, config, notifications, ssh, context,
//!   validation, auxiliary reads). Transport-agnostic: consumers implement
//!   the `DataApi` trait for their UI framework.
//! - **http-data-api** — HTTP/SSE server under the `/api` prefix that
//!   mirrors the IPC operation set for web/remote clients.

pub mod ipc;

pub mod http {
    //! http-data-api capability — axum-based HTTP/SSE server (not yet wired).
}

pub use ipc::{
    ApiError, ApiErrorCode, ConfigUpdateRequest, ContextInfo, DataApi, PaginatedRequest,
    PaginatedResponse, ProjectInfo, PushEvent, SearchRequest, SessionDetail, SessionSummary,
    SshConnectRequest,
};
