//! 集成测试：CORS layer 行为。
//!
//! 覆盖 spec：`http-data-api` §"HTTP server SHALL layer CORS middleware
//! for localhost origins"——
//! - `localhost origin 通过 CORS`
//! - `127.0.0.1 origin 通过 CORS`
//! - `非 localhost origin 被 CORS 拒绝`
//! - `preflight OPTIONS 请求`
//!
//! 用 `tower::ServiceExt::oneshot` 直接打 router，不起真 listener。

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
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

#[tokio::test]
async fn localhost_origin_echoes_in_cors_response() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, None);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/projects")
        .header(header::ORIGIN, "http://localhost:3456")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let allow = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    assert_eq!(
        allow.as_deref(),
        Some("http://localhost:3456"),
        "SHALL echo localhost origin to Access-Control-Allow-Origin"
    );
}

#[tokio::test]
async fn ipv4_loopback_origin_echoes_in_cors_response() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, None);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/projects")
        .header(header::ORIGIN, "http://127.0.0.1:3456")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let allow = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    assert_eq!(allow.as_deref(), Some("http://127.0.0.1:3456"));
}

#[tokio::test]
async fn lookalike_subdomain_origin_rejected_by_cors() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, None);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/projects")
        .header(header::ORIGIN, "https://localhost.evil.com")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    // CORS 拒绝表现：响应 SHALL 不携带 Access-Control-Allow-Origin。
    // 浏览器据此阻止 JS 读响应；axum 仍会照常处理请求并返回 200，但
    // CORS 层不 echo origin（这正是 `tower_http::cors::AllowOrigin::predicate`
    // 的行为）。
    let allow = resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN);
    assert!(
        allow.is_none(),
        "non-localhost origin SHALL NOT be echoed; got {allow:?}"
    );
}

#[tokio::test]
async fn preflight_options_request_includes_method_and_headers() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, None);

    let req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/config")
        .header(header::ORIGIN, "http://localhost:3456")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "PATCH")
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    let status = resp.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::NO_CONTENT,
        "preflight SHALL succeed; got {status}"
    );
    let methods = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_METHODS)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(
        methods.to_uppercase().contains("PATCH"),
        "Access-Control-Allow-Methods SHALL include PATCH, got {methods}"
    );
    let headers = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        headers.contains("content-type"),
        "Access-Control-Allow-Headers SHALL include content-type, got {headers}"
    );
}
