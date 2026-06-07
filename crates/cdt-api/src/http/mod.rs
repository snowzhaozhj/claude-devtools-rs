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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::ipc::ApiError;

/// 从 Tauri binary 嵌入资源预加载的静态资源表。
///
/// 启动时一次性从 `AssetResolver::iter()` 构建精确索引，避免运行时依赖
/// `AssetResolver::get()` 的 SPA fallback 语义（它会把未命中路径 fallback
/// 到 `index.html`，导致 JS/CSS 404 变成返回 HTML 白屏）。
#[derive(Clone)]
pub struct EmbeddedAssets {
    assets: Arc<HashMap<String, (Vec<u8>, String)>>,
}

impl std::fmt::Debug for EmbeddedAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddedAssets")
            .field("count", &self.assets.len())
            .finish()
    }
}

impl EmbeddedAssets {
    /// 从 `(path, bytes)` 迭代器构建。mime type 按扩展名推断。
    pub fn from_assets(iter: impl Iterator<Item = (String, Vec<u8>)>) -> Self {
        let mut map = HashMap::new();
        for (path, bytes) in iter {
            let mime = mime_from_extension(&path);
            map.insert(path, (bytes, mime));
        }
        tracing::info!(
            target: "cdt_api::http",
            count = map.len(),
            "embedded assets loaded"
        );
        Self {
            assets: Arc::new(map),
        }
    }

    /// 精确查找——不做 SPA fallback。
    pub fn get(&self, path: &str) -> Option<(&[u8], &str)> {
        self.assets
            .get(path)
            .map(|(b, m)| (b.as_slice(), m.as_str()))
    }
}

fn mime_from_extension(path: &str) -> String {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// 静态资源 serve 策略。
#[derive(Clone, Debug)]
pub enum StaticServe {
    /// 不挂 fallback，未命中 `/api/*` 直接 404。cdt-cli 默认行为 + 测试常用。
    None,
    /// SPA `ServeDir`：未命中 `/api/*` 的 GET fallback 到该目录的文件 / `index.html`。
    /// dev 模式下 `CDT_DEV_USE_PREBUILT_DIST=1` 切回此分支验证 release 形态。
    Dir(PathBuf),
    /// HTTP 302 redirect：未命中 `/api/*` 的请求重定向到指定 base URL（典型
    /// `http://127.0.0.1:5173`），自动追加 `?http=1` query 让前端 `main.ts`
    /// 走 `BrowserTransport`。dev `cargo tauri dev` 默认此分支。
    Redirect(String),
    /// 从 Tauri binary 嵌入资源 serve。release 桌面端默认此分支——零冗余，
    /// 不需要在 `bundle.resources` 里重复拷贝前端文件。
    Embedded(EmbeddedAssets),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_assets_exact_lookup() {
        let assets = EmbeddedAssets::from_assets(
            vec![
                ("index.html".to_string(), b"<html></html>".to_vec()),
                ("assets/app.js".to_string(), b"console.log(1)".to_vec()),
            ]
            .into_iter(),
        );
        let (bytes, mime) = assets.get("index.html").unwrap();
        assert_eq!(bytes, b"<html></html>");
        assert!(mime.contains("text/html"));

        let (bytes, mime) = assets.get("assets/app.js").unwrap();
        assert_eq!(bytes, b"console.log(1)");
        assert!(mime.contains("javascript"));

        assert!(assets.get("missing.js").is_none());
    }

    #[test]
    fn embedded_assets_no_spa_fallback() {
        let assets = EmbeddedAssets::from_assets(
            vec![("index.html".to_string(), b"<html></html>".to_vec())].into_iter(),
        );
        assert!(
            assets.get("nonexistent.js").is_none(),
            "exact lookup must not fallback to index.html"
        );
    }

    #[test]
    fn mime_from_extension_covers_common_types() {
        assert!(mime_from_extension("app.js").contains("javascript"));
        assert!(mime_from_extension("style.css").contains("text/css"));
        assert!(mime_from_extension("icon.svg").contains("svg"));
        assert!(mime_from_extension("font.woff2").contains("woff2"));
        assert!(mime_from_extension("data.json").contains("json"));
        assert!(mime_from_extension("unknown.xyz").contains("octet-stream"));
    }
}
