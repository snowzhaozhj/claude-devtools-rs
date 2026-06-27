//! ipc-data-api capability — trait surface + 类型定义。
//!
//! Spec：`openspec/specs/ipc-data-api/spec.md`。

pub(crate) mod backend_resolvers;
pub mod error;
pub mod events;
pub mod external_app;
pub(crate) mod image_disk_cache;
pub mod local;
pub(crate) mod parsed_message_cache;
mod path_resolve;
// project_scan_cache 在 test/test-utils feature 下暴露给集成测试访问
// `spawn_project_scan_cache_invalidator` + `ProjectScanCache::insert` 等
// 测试 helper（详 change `project-scan-cache-semantic-invalidation` §5）。
#[cfg(not(any(test, feature = "test-utils")))]
pub(crate) mod project_scan_cache;
#[cfg(any(test, feature = "test-utils"))]
pub mod project_scan_cache;
pub mod session_metadata;
pub mod traits;
pub mod types;
pub(crate) mod workflow_manifest;
pub(crate) mod workflow_script;

pub use cdt_discover::{WslDistroCandidate, WslDistroScanReport};
pub use error::{ApiError, ApiErrorCode};
pub use events::{PushEvent, SessionMetadataUpdate};
pub use local::{LocalDataApi, METADATA_SCAN_CONCURRENCY, SessionListFilter};
pub use traits::{CorrectnessEventItem, DataApi};
pub use types::{
    ConfigUpdateRequest, ContextInfo, MemoryFileContent, MemoryLayer, MemoryLayerKind,
    PaginatedRequest, PaginatedResponse, ProjectInfo, ProjectMemory, ProjectSessionPrefs,
    SearchRequest, SessionDetail, SessionDetailMetadata, SessionDetailMetrics,
    SessionDetailResponse, SessionSummary, SshAuthMethod, SshConnectRequest, SshConnectionResult,
    SshState,
};
