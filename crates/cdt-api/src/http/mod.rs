//! http-data-api capability — axum HTTP/SSE server。
//!
//! Spec：`openspec/specs/http-data-api/spec.md`。
//!
//! 在 `/api` 前缀下镜像 `DataApi` trait 的全部操作；可选挂载静态文件 serve
//! 让浏览器加载完整前端 bundle（详 `Requirement: HTTP server SHALL serve
//! static frontend assets with SPA fallback`）。

pub mod bridge;
pub mod cors;
pub mod routes;
pub mod sse;
pub mod state;

pub use bridge::spawn_event_bridge;
pub use cors::localhost_cors_layer;
pub use routes::build_router;
pub use state::AppState;

use std::path::PathBuf;

use crate::ipc::ApiError;

/// 启动 HTTP server。
///
/// 绑定到 `127.0.0.1:<port>`，阻塞直到 shutdown signal。
/// 端口冲突时返回明确错误（spec: `SHALL NOT switch ports silently`）。
///
/// `static_dir` 为 `Some(<existing dir>)` 时挂 `tower_http::services::ServeDir` +
/// SPA fallback：`/api/*` 优先；未命中且非已知静态文件的 GET 请求 fallback 到
/// 该目录下的 `index.html` 让前端 client-side router 接管。`None` 或路径无效时
/// 仅 serve `/api/*`（行为与本 change 之前一致）。
pub async fn start_server(
    state: AppState,
    port: u16,
    static_dir: Option<PathBuf>,
) -> Result<(), ApiError> {
    let router = build_router(state, static_dir);
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
