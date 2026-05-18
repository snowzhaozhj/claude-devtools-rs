//! 集成测试：静态文件 serve + SPA fallback。
//!
//! 覆盖 spec：`http-data-api` §"HTTP server SHALL serve static frontend
//! assets with SPA fallback"——
//! - `GET / 返回前端 index.html`
//! - `GET 已知静态资源命中 ServeDir`
//! - `GET 未知前端路由 fallback 到 index.html`
//! - `GET 带扩展名但磁盘上不存在的资源 SHALL 返 404`（codex review 后新增——
//!   防止浏览器把 HTML 当 JS 解析 + CDN 缓存脏数据）
//! - `GET /api/* 不被 ServeDir 拦截`
//! - `static_dir = None 时无 ServeDir`
//! - `static_dir 路径无效仅警告不阻塞启动`

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use cdt_api::http::{AppState, build_router};
use cdt_api::{DataApi, LocalDataApi};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use tempfile::TempDir;
use tower::ServiceExt;

async fn build_state(tmp: &TempDir) -> AppState {
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base);
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.unwrap();
    let mut notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    notif_mgr.load().await.unwrap();
    let ssh_mgr = SshConnectionManager::new();
    let api: Arc<dyn DataApi> =
        Arc::new(LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr));
    AppState {
        api,
        events_tx: tokio::sync::broadcast::channel(16).0,
    }
}

/// 在 tmp 内造一个最小前端 bundle 目录：`index.html` + `assets/main.js`。
fn build_static_dir(tmp: &TempDir) -> std::path::PathBuf {
    let dir = tmp.path().join("dist");
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("index.html"), b"<!doctype html>SPA-INDEX").unwrap();
    std::fs::write(dir.join("assets/main.js"), b"console.log('main');").unwrap();
    dir
}

async fn body_string(resp: axum::http::Response<Body>) -> (StatusCode, String) {
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn get_root_returns_index_html_when_static_dir_set() {
    let tmp = TempDir::new().unwrap();
    let static_dir = build_static_dir(&tmp);
    let state = build_state(&tmp).await;
    let app = build_router(state, Some(static_dir));

    let req = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())
        .unwrap();
    let (status, body) = body_string(app.oneshot(req).await.unwrap()).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("SPA-INDEX"), "GET / SHALL return index.html");
}

#[tokio::test]
async fn get_known_static_asset_hits_serve_dir() {
    let tmp = TempDir::new().unwrap();
    let static_dir = build_static_dir(&tmp);
    let state = build_state(&tmp).await;
    let app = build_router(state, Some(static_dir));

    let req = Request::builder()
        .method(Method::GET)
        .uri("/assets/main.js")
        .body(Body::empty())
        .unwrap();
    let (status, body) = body_string(app.oneshot(req).await.unwrap()).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("console.log('main');"),
        "asset SHALL be served verbatim"
    );
}

#[tokio::test]
async fn unknown_frontend_route_falls_back_to_index_html() {
    let tmp = TempDir::new().unwrap();
    let static_dir = build_static_dir(&tmp);
    let state = build_state(&tmp).await;
    let app = build_router(state, Some(static_dir));

    let req = Request::builder()
        .method(Method::GET)
        .uri("/sessions/some-id-does-not-exist-on-disk")
        .body(Body::empty())
        .unwrap();
    let (status, body) = body_string(app.oneshot(req).await.unwrap()).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("SPA-INDEX"),
        "unknown route SHALL fallback to index.html for SPA router"
    );
}

#[tokio::test]
async fn path_traversal_attempts_are_rejected() {
    let tmp = TempDir::new().unwrap();
    let static_dir = build_static_dir(&tmp);
    let state = build_state(&tmp).await;
    let app = build_router(state, Some(static_dir));

    for url in [
        "/foo/../bar",        // 裸 `..` 段
        "/%2e%2e/etc/passwd", // URL-encoded `..`（小写 hex）
        "/%2E%2E/etc/passwd", // URL-encoded `..`（大写 hex）
        "/foo/%2e%2e/bar",    // 中间段 encoded `..`
        "/foo%5cbar",         // URL-encoded backslash
    ] {
        let req = Request::builder()
            .method(Method::GET)
            .uri(url)
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        // 关键不变量：SHALL NOT 返 200 + 文件内容（拒绝具体 status code 由 axum
        // 路由层 / 我们的 fallback 谁先拦决定，可能是 400/403/404；只要不是
        // 200 让攻击者拿到磁盘文件就 OK）。
        assert_ne!(
            status,
            StatusCode::OK,
            "{url} SHALL NOT 返 200 + 文件内容（path traversal 防御失效），实际 {status}"
        );
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let body = String::from_utf8_lossy(&bytes);
        assert!(
            !body.contains("SPA-INDEX") && !body.contains("console.log"),
            "{url} SHALL NOT 返任何静态文件内容，body={body}"
        );
    }
}

#[tokio::test]
async fn missing_asset_with_extension_returns_404_not_index_html() {
    // 反例 / 防回归：带扩展名的请求若磁盘上不存在 SHALL 返 404，**不**
    // 得 fallback 到 index.html（避免浏览器把 HTML 当 JS 解析 + CDN
    // 缓存脏数据；codex review 指出的 SPA 部署经典坑）。
    let tmp = TempDir::new().unwrap();
    let static_dir = build_static_dir(&tmp);
    let state = build_state(&tmp).await;
    let app = build_router(state, Some(static_dir));

    for url in [
        "/assets/missing.js",
        "/assets/typo.css",
        "/favicon.ico",
        "/some-nested/path/file.png",
    ] {
        let req = Request::builder()
            .method(Method::GET)
            .uri(url)
            .body(Body::empty())
            .unwrap();
        let (status, body) = body_string(app.clone().oneshot(req).await.unwrap()).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{url} SHALL 返 404, 实际 {status} body={body}"
        );
        assert!(
            !body.contains("SPA-INDEX"),
            "{url} SHALL NOT fallback 到 index.html, body={body}"
        );
    }
}

#[tokio::test]
async fn api_routes_not_intercepted_by_serve_dir() {
    let tmp = TempDir::new().unwrap();
    let static_dir = build_static_dir(&tmp);
    let state = build_state(&tmp).await;
    let app = build_router(state, Some(static_dir));

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/projects")
        .body(Body::empty())
        .unwrap();
    let (status, body) = body_string(app.oneshot(req).await.unwrap()).await;

    // /api/projects SHALL 命中 API handler（返 JSON 数组），不应被 SPA fallback
    // 拦截到 index.html。
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.starts_with('[') || body.starts_with('{'),
        "api/projects SHALL return JSON, got: {body}"
    );
    assert!(
        !body.contains("SPA-INDEX"),
        "api/projects SHALL NOT be intercepted by SPA fallback"
    );
}

#[tokio::test]
async fn static_dir_none_returns_404_for_root() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, None);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "no static_dir → GET / SHALL return 404"
    );
}

#[tokio::test]
async fn invalid_static_dir_path_only_warns_and_serves_api() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let bogus = tmp.path().join("nonexistent-dir-for-test");
    let app = build_router(state, Some(bogus));

    // /api/projects SHALL 仍 serve
    let req_api = Request::builder()
        .method(Method::GET)
        .uri("/api/projects")
        .body(Body::empty())
        .unwrap();
    let resp_api = app.clone().oneshot(req_api).await.unwrap();
    assert_eq!(resp_api.status(), StatusCode::OK);

    // GET / SHALL 返 404（无 ServeDir 兜底）
    let req_root = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())
        .unwrap();
    let resp_root = app.oneshot(req_root).await.unwrap();
    assert_eq!(resp_root.status(), StatusCode::NOT_FOUND);
}
