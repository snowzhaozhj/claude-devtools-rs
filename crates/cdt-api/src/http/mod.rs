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

/// 静态资源 serve 策略——dev `cargo tauri dev` 默认重定向浏览器到 vite dev
/// server，让两端共享同一份热重载 UI；release 与 cdt-cli 用 `Dir`。
///
/// 历史回归：dev 模式下 Tauri 内置 HTTP server 直接 `ServeDir` 一份**预构建**
/// 的 `ui/dist`（`pnpm --dir ui build` 产物），UI 改动后只更新 vite dev server
/// 的内存 bundle，磁盘 dist 不动——浏览器访问 `localhost:3456` 看到的是
/// 上次 build 时刻的旧 UI，与桌面 Tauri 窗口（走 vite hmr）行为分叉。
/// `Redirect` 让浏览器跳到 vite 自己的 origin，HMR / source map / 最新源码
/// 全部在 vite 域内工作；`/api/*` 仍由 axum 处理保证 HTTP 后端行为真实。
#[derive(Clone, Debug)]
pub enum StaticServe {
    /// 不挂 fallback，未命中 `/api/*` 直接 404。cdt-cli 默认行为 + 测试常用。
    None,
    /// SPA `ServeDir`：未命中 `/api/*` 的 GET fallback 到该目录的文件 / `index.html`。
    /// release 走 `resource_dir`；dev 模式下 `CDT_DEV_USE_PREBUILT_DIST=1` 切回此分支
    /// 验证 release 形态。
    Dir(PathBuf),
    /// HTTP 302 redirect：未命中 `/api/*` 的请求重定向到指定 base URL（典型
    /// `http://127.0.0.1:5173`），自动追加 `?http=1` query 让前端 `main.ts`
    /// 走 `BrowserTransport`。dev `cargo tauri dev` 默认此分支。
    Redirect(String),
}

impl From<Option<PathBuf>> for StaticServe {
    fn from(opt: Option<PathBuf>) -> Self {
        match opt {
            Some(p) => Self::Dir(p),
            None => Self::None,
        }
    }
}

/// 启动 HTTP server。
///
/// 绑定到 `127.0.0.1:<port>`，阻塞直到 shutdown signal。
/// 端口冲突时返回明确错误（spec: `SHALL NOT switch ports silently`）。
///
/// `static_serve` 决定未命中 `/api/*` 的请求如何处理；详 `StaticServe` doc。
pub async fn start_server(
    state: AppState,
    port: u16,
    static_serve: StaticServe,
) -> Result<(), ApiError> {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to bind to port {port}: {e}")))?;
    tracing::info!("HTTP server listening on {addr}");
    serve_with_listener(state, listener, static_serve).await
}

/// 用调用方已 bind 的 `TcpListener` 启 server。
///
/// server-mode 在 Tauri 进程里需要先 bind 拿到具体 `io::Error`（端口冲突 vs 权限
/// 等）再决定是否报错给前端，因此把 bind 与 serve 拆成两个 entry。
/// 单实例 `start_server` 仍保持原签名，内部转发到本函数。
pub async fn serve_with_listener(
    state: AppState,
    listener: tokio::net::TcpListener,
    static_serve: StaticServe,
) -> Result<(), ApiError> {
    let router = build_router(state, static_serve);
    axum::serve(listener, router)
        .await
        .map_err(|e| ApiError::internal(format!("HTTP server error: {e}")))?;
    Ok(())
}
