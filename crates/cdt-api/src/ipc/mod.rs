//! ipc-data-api capability — trait surface + 类型定义。
//!
//! Spec：`openspec/specs/ipc-data-api/spec.md`。

pub mod error;
pub mod events;
pub mod local;
pub mod traits;
pub mod types;

pub use error::{ApiError, ApiErrorCode};
pub use events::PushEvent;
pub use local::LocalDataApi;
pub use traits::DataApi;
pub use types::{
    ConfigUpdateRequest, ContextInfo, PaginatedRequest, PaginatedResponse, ProjectInfo,
    SearchRequest, SessionDetail, SessionSummary, SshConnectRequest,
};
