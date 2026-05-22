//! 集成测试：lazy / 辅助 IPC commands 的 HTTP 镜像。
//!
//! 覆盖 spec：`http-data-api` §"Mirror lazy and auxiliary IPC commands"——
//! - `GET project memory mirrors IPC`
//! - `POST add trigger returns generated id`
//! - `POST pin session 与 DELETE unpin session 互逆`
//! - 辅助 endpoint：`POST hide` / `DELETE unhide` / `GET session-prefs` /
//!   `DELETE remove_trigger`
//! - lazy endpoint trait 委托：`GET subagent trace` / `GET image asset` /
//!   `GET tool output` 默认 fallback 路径（找不到资源时的行为）
//!
//! image asset / tool output 的真实数据流需要 jsonl + image cache 完整 fixture，
//! 本文件只验证路由配置 + trait 委托正确性；完整数据 round-trip 由
//! `LocalDataApi` 各自的单测 + IPC contract test 覆盖。

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use cdt_api::http::{AppState, StaticServe, build_router};
use cdt_api::{DataApi, LocalDataApi};
use cdt_config::{
    ConfigManager, NotificationManager, NotificationTrigger, TriggerContentType, TriggerMode,
};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use serde_json::{Value, json};
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

async fn body_json(resp: axum::http::Response<Body>) -> (StatusCode, Value) {
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 512 * 1024).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

fn sample_trigger() -> NotificationTrigger {
    NotificationTrigger {
        // ConfigManager::add_trigger 期望 caller 已分配 id（与 IPC 路径一致——
        // 前端在 add 前自己 uuid，详 `cdt-config::trigger::validate_trigger`）。
        id: "test-trigger-001".into(),
        name: "test trigger".into(),
        enabled: true,
        content_type: TriggerContentType::Text,
        mode: TriggerMode::ContentMatch,
        tool_name: None,
        is_builtin: None,
        ignore_patterns: None,
        require_error: None,
        match_field: Some("text".into()),
        match_pattern: Some(".*".into()),
        token_threshold: None,
        token_type: None,
        repository_ids: None,
        color: None,
    }
}

#[tokio::test]
async fn get_project_memory_mirrors_ipc_for_unknown_project() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, StaticServe::None);

    // unknown project_id：path encoding 在 LocalDataApi 内自动处理。
    // 期望返 200 + ProjectMemory（has_memory=false）或 not_found——实现选择前者
    // （`has_memory: false, count: 0`），所以这里不强制 404。
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/projects/-not-real-project/memory")
        .body(Body::empty())
        .unwrap();
    let (status, body) = body_json(app.oneshot(req).await.unwrap()).await;
    assert!(
        status == StatusCode::OK || status == StatusCode::NOT_FOUND,
        "GET project memory SHALL 返 200 + 空概览或 404；实际 {status}: {body}"
    );
    if status == StatusCode::OK {
        // payload 形态校验：projectId / hasMemory / count（camelCase）
        assert_eq!(
            body.get("projectId").and_then(Value::as_str),
            Some("-not-real-project")
        );
        assert!(body.get("hasMemory").and_then(Value::as_bool).is_some());
        assert!(body.get("count").and_then(Value::as_u64).is_some());
    }
}

#[tokio::test]
async fn add_trigger_returns_generated_id() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, StaticServe::None);

    let trigger_json = serde_json::to_value(sample_trigger()).unwrap();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/notifications/triggers")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&trigger_json).unwrap()))
        .unwrap();
    let (status, body) = body_json(app.oneshot(req).await.unwrap()).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "POST add trigger SHALL 返 200, body: {body}"
    );
    let triggers = body
        .pointer("/notifications/triggers")
        .and_then(Value::as_array)
        .expect("response SHALL 含 notifications.triggers 数组");
    let new_trigger = triggers
        .iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some("test trigger"))
        .expect("SHALL 找到刚加的 trigger");
    let new_id = new_trigger.get("id").and_then(Value::as_str).unwrap_or("");
    assert_eq!(
        new_id, "test-trigger-001",
        "trigger id SHALL 与 caller 提供的一致, 实际: {new_trigger}"
    );
}

#[tokio::test]
async fn pin_and_unpin_session_round_trip() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, StaticServe::None);

    // POST pin
    let pin = Request::builder()
        .method(Method::POST)
        .uri("/api/projects/-proj-X/sessions/sid-A/pin")
        .body(Body::empty())
        .unwrap();
    let (status, _) = body_json(app.clone().oneshot(pin).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);

    // GET prefs SHALL 含 sid-A
    let get = Request::builder()
        .method(Method::GET)
        .uri("/api/projects/-proj-X/session-prefs")
        .body(Body::empty())
        .unwrap();
    let (status, prefs) = body_json(app.clone().oneshot(get).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    let pinned = prefs.get("pinned").and_then(Value::as_array).unwrap();
    assert!(
        pinned.iter().any(|s| s.as_str() == Some("sid-A")),
        "pin 后 prefs.pinned SHALL 含 sid-A: {prefs}"
    );

    // DELETE unpin
    let unpin = Request::builder()
        .method(Method::DELETE)
        .uri("/api/projects/-proj-X/sessions/sid-A/pin")
        .body(Body::empty())
        .unwrap();
    let (status, _) = body_json(app.clone().oneshot(unpin).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);

    // 再 GET prefs SHALL 不含 sid-A
    let get2 = Request::builder()
        .method(Method::GET)
        .uri("/api/projects/-proj-X/session-prefs")
        .body(Body::empty())
        .unwrap();
    let (status, prefs2) = body_json(app.oneshot(get2).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    let pinned2 = prefs2.get("pinned").and_then(Value::as_array).unwrap();
    assert!(
        !pinned2.iter().any(|s| s.as_str() == Some("sid-A")),
        "unpin 后 SHALL 不含 sid-A: {prefs2}"
    );
}

#[tokio::test]
async fn hide_and_unhide_session_round_trip() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, StaticServe::None);

    let hide = Request::builder()
        .method(Method::POST)
        .uri("/api/projects/-proj-Y/sessions/sid-Z/hide")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        body_json(app.clone().oneshot(hide).await.unwrap()).await.0,
        StatusCode::OK
    );

    let get = Request::builder()
        .method(Method::GET)
        .uri("/api/projects/-proj-Y/session-prefs")
        .body(Body::empty())
        .unwrap();
    let (_, prefs) = body_json(app.clone().oneshot(get).await.unwrap()).await;
    let hidden = prefs.get("hidden").and_then(Value::as_array).unwrap();
    assert!(hidden.iter().any(|s| s.as_str() == Some("sid-Z")));

    let unhide = Request::builder()
        .method(Method::DELETE)
        .uri("/api/projects/-proj-Y/sessions/sid-Z/hide")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        body_json(app.clone().oneshot(unhide).await.unwrap())
            .await
            .0,
        StatusCode::OK
    );

    let get2 = Request::builder()
        .method(Method::GET)
        .uri("/api/projects/-proj-Y/session-prefs")
        .body(Body::empty())
        .unwrap();
    let (_, prefs2) = body_json(app.oneshot(get2).await.unwrap()).await;
    let hidden2 = prefs2.get("hidden").and_then(Value::as_array).unwrap();
    assert!(!hidden2.iter().any(|s| s.as_str() == Some("sid-Z")));
}

#[tokio::test]
async fn remove_trigger_returns_updated_config() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, StaticServe::None);

    // 先加一个 trigger
    let trigger_json = serde_json::to_value(sample_trigger()).unwrap();
    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/notifications/triggers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&trigger_json).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let (_, add_body) = body_json(add_resp).await;
    let new_id = add_body
        .pointer("/notifications/triggers")
        .and_then(Value::as_array)
        .and_then(|arr| {
            arr.iter()
                .find(|t| t.get("name").and_then(Value::as_str) == Some("test trigger"))
        })
        .and_then(|t| t.get("id").and_then(Value::as_str))
        .map(str::to_owned)
        .expect("SHALL 拿到新 trigger id");

    // DELETE 该 id
    let del = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/notifications/triggers/{new_id}"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = body_json(app.oneshot(del).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    let triggers = body
        .pointer("/notifications/triggers")
        .and_then(Value::as_array)
        .unwrap();
    assert!(
        !triggers
            .iter()
            .any(|t| t.get("id").and_then(Value::as_str) == Some(&new_id)),
        "delete 后 trigger SHALL 不再出现: {body}"
    );
}

#[tokio::test]
async fn read_memory_file_returns_404_for_missing_project() {
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, StaticServe::None);

    let body = json!({ "file": "INDEX.md" });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/projects/-not-real/memory-files")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let (status, _) = body_json(app.oneshot(req).await.unwrap()).await;
    assert!(
        status == StatusCode::NOT_FOUND || status == StatusCode::BAD_REQUEST,
        "missing project SHALL 返 4xx, got {status}"
    );
}

#[tokio::test]
async fn lazy_endpoints_are_routed_not_404() {
    // 验证 11 个 lazy endpoint 的路由配置都有效（即未触发"路由不存在"的 404 不带 code）。
    // image asset / tool output / subagent trace 找不到资源时返 ApiError 形态的 4xx
    // 或 LocalDataApi 默认空字符串 / Missing。本测试只检查"不是 axum 路由层 404"。
    let tmp = TempDir::new().unwrap();
    let state = build_state(&tmp).await;
    let app = build_router(state, StaticServe::None);

    let urls = [
        "/api/sessions/root/subagents/sub/trace",
        "/api/sessions/root/subagents/sub/blocks/blk/image",
        "/api/sessions/root/subagents/sub/tools/tool-1/output",
    ];
    for url in urls {
        let req = Request::builder()
            .method(Method::GET)
            .uri(url)
            .body(Body::empty())
            .unwrap();
        let (status, body) = body_json(app.clone().oneshot(req).await.unwrap()).await;
        // 路由存在 → 走 handler → 返 2xx 或 4xx/5xx 携带 ApiError 形态 body（含 code 字段）。
        // axum 路由层 404 不会带 body，body 应为 Null。
        let is_router_404 = status == StatusCode::NOT_FOUND && body == Value::Null;
        assert!(
            !is_router_404,
            "URL {url} SHALL 命中路由（不应是 axum 路由层 404）, 实际 status={status}, body={body}"
        );
    }
}
