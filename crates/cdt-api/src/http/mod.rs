//! http-data-api capability — axum HTTP/SSE server。
//!
//! Spec：`openspec/specs/http-data-api/spec.md`。
//!
//! 在 `/api` 前缀下镜像 `DataApi` trait 的全部操作。

pub mod routes;
pub mod sse;
pub mod state;

pub use routes::build_router;
pub use state::AppState;

use crate::ipc::ApiError;

/// 启动 HTTP server。
///
/// 绑定到指定端口，阻塞直到 shutdown signal。
/// 端口冲突时返回明确错误（spec: "SHALL NOT switch ports silently"）。
pub async fn start_server(state: AppState, port: u16) -> Result<(), ApiError> {
    let router = build_router(state);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to bind to port {port}: {e}")))?;

    tracing::info!("HTTP server listening on {addr}");

    axum::serve(listener, router)
        .await
        .map_err(|e| ApiError::internal(format!("HTTP server error: {e}")))?;

    Ok(())
}
