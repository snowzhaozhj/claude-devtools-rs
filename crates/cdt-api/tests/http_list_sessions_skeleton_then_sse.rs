//! Integration test：HTTP `GET /api/projects/{projectId}/sessions` 走骨架 +
//! SSE patch 路径。
//!
//! 覆盖 spec：
//! - `ipc-data-api` §"Expose project and session queries" 段落
//!   "HTTP `list_sessions` 复用 IPC 骨架 + push 实现"
//! - `http-data-api` §"Serve projects and sessions over HTTP under /api prefix"
//!   Scenario `GET paginated sessions returns skeleton with cache-hit inline real values`
//!   Scenario `HTTP list_sessions 后台扫描产物经 SSE 推送`（spec `ipc-data-api`）
//! - change `eager-first-page-metadata` D8：`cursor=None` 同步真值 / `cursor=Some` 走骨架

use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use cdt_api::http::{AppState, build_router};
use cdt_api::{DataApi, LocalDataApi, PushEvent, spawn_event_bridge};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_core::{FileChangeEvent, TodoChangeEvent};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use serde_json::Value;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio::time::timeout;
use tower::ServiceExt;

async fn write_fixture_session(dir: &std::path::Path, session_id: &str, title: &str) {
    let path = dir.join(format!("{session_id}.jsonl"));
    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .await
        .unwrap();
    let user = serde_json::json!({
        "type": "user",
        "uuid": format!("u-{session_id}"),
        "timestamp": "2026-05-20T10:00:00Z",
        "cwd": "/tmp/proj",
        "message": {"role": "user", "content": title}
    })
    .to_string();
    let assistant = serde_json::json!({
        "type": "assistant",
        "uuid": format!("a-{session_id}"),
        "timestamp": "2026-05-20T10:00:01Z",
        "cwd": "/tmp/proj",
        "message": {
            "role": "assistant",
            "model": "claude-sonnet",
            "content": [{
                "type": "tool_use",
                "id": format!("tu-{session_id}"),
                "name": "Bash",
                "input": {"command": "ls"}
            }]
        }
    })
    .to_string();
    f.write_all(user.as_bytes()).await.unwrap();
    f.write_all(b"\n").await.unwrap();
    f.write_all(assistant.as_bytes()).await.unwrap();
    f.write_all(b"\n").await.unwrap();
    f.flush().await.unwrap();
}

struct Harness {
    _tmp: TempDir,
    project_id: String,
    session_ids: Vec<String>,
    router: axum::Router,
    events_rx: broadcast::Receiver<PushEvent>,
}

async fn build_harness(titles: &[&str]) -> Harness {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    let project_dir_name = "-tmp-http-fixture";
    let project_dir = projects_base.join(project_dir_name);
    std::fs::create_dir_all(&project_dir).unwrap();

    let mut session_ids = Vec::new();
    for (i, title) in titles.iter().enumerate() {
        let sid = format!("sess-{i:04}");
        write_fixture_session(&project_dir, &sid, title).await;
        session_ids.push(sid);
    }

    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base);
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.unwrap();
    let mut notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    notif_mgr.load().await.unwrap();
    let ssh_mgr = SshConnectionManager::new();
    let api = Arc::new(LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr));

    let (events_tx, events_rx) = broadcast::channel::<PushEvent>(64);
    let (_file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(16);
    let (_todo_tx, todo_rx) = broadcast::channel::<TodoChangeEvent>(16);
    let (_error_tx, error_rx) = broadcast::channel(16);
    let metadata_rx = api.subscribe_session_metadata();
    spawn_event_bridge(events_tx.clone(), file_rx, todo_rx, error_rx, metadata_rx);

    let state = AppState {
        api: api as Arc<dyn DataApi>,
        events_tx,
    };
    let router = build_router(state, None);

    Harness {
        _tmp: tmp,
        project_id: project_dir_name.to_owned(),
        session_ids,
        router,
        events_rx,
    }
}

async fn body_json(resp: axum::http::Response<Body>) -> (StatusCode, Value) {
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

/// D8 cursor=None：eager 路径前 `EAGER_FIRST_PAGE_LIMIT` 条 SHALL 同步带真值 inline 返。
#[tokio::test]
async fn http_get_sessions_cursor_none_inlines_real_values() {
    let h = build_harness(&["改 auth", "修 sidebar bug", "perf 优化"]).await;
    let url = format!("/api/projects/{}/sessions?pageSize=20", h.project_id);
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_json(resp).await;
    assert_eq!(status, StatusCode::OK);

    let items = body
        .get("items")
        .and_then(serde_json::Value::as_array)
        .expect("body.items array");
    assert_eq!(items.len(), 3);
    for item in items {
        // eager 路径：cursor=None 同步等到 metadata 真值（D8）
        assert!(
            item.get("title")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|t| !t.is_empty()),
            "cursor=None eager path SHALL inline real title, got {:?}",
            item.get("title")
        );
        assert_eq!(
            item.get("messageCount").and_then(serde_json::Value::as_u64),
            Some(2),
            "cursor=None eager path SHALL inline real messageCount=2"
        );
        assert_eq!(
            item.get("isOngoing").and_then(serde_json::Value::as_bool),
            Some(true),
            "cursor=None eager path SHALL inline real isOngoing=true"
        );
        let sid = item
            .get("sessionId")
            .and_then(serde_json::Value::as_str)
            .unwrap();
        assert!(h.session_ids.contains(&sid.to_owned()));
    }
}

/// D8 cursor=Some(_)：翻页路径保留原"骨架 + 后台扫描 → SSE patch"行为。
#[tokio::test]
async fn http_get_sessions_cursor_some_returns_skeleton_then_sse() {
    // 4 个 session + pageSize=2 让 cursor=None 仅吃前 2 条 eager，留 cursor=Some 翻第二页。
    let titles = ["改 auth", "修 sidebar bug", "perf 优化", "添加 tracing"];
    let mut h = build_harness(&titles).await;

    // 第一次 GET：cursor=None pageSize=2 → eager 前 2 条 inline 真值 + 返 nextCursor
    let url_first = format!("/api/projects/{}/sessions?pageSize=2", h.project_id);
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&url_first)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_json(resp).await;
    assert_eq!(status, StatusCode::OK);
    let next_cursor = body
        .get("nextCursor")
        .and_then(serde_json::Value::as_str)
        .expect("first page SHALL return nextCursor")
        .to_owned();
    // 排空 eager 路径不该 emit 但保险一下：清掉任何可能的 deferred retry 残留
    while let Ok(Ok(_)) = timeout(Duration::from_millis(50), h.events_rx.recv()).await {}

    // 第二次 GET：cursor=Some(_) → 骨架 + 后台扫描 + SSE emit
    let url_second = format!(
        "/api/projects/{}/sessions?pageSize=2&cursor={next_cursor}",
        h.project_id
    );
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&url_second)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_json(resp).await;
    assert_eq!(status, StatusCode::OK);
    let items = body
        .get("items")
        .and_then(serde_json::Value::as_array)
        .expect("body.items array");
    assert!(!items.is_empty(), "second page SHALL have remaining items");
    for item in items {
        // 翻页路径：骨架契约（title=null / messageCount=0 / isOngoing=false）
        assert!(
            item.get("title").is_some_and(serde_json::Value::is_null),
            "cursor=Some skeleton title SHALL be null, got {:?}",
            item.get("title")
        );
        assert_eq!(
            item.get("messageCount").and_then(serde_json::Value::as_u64),
            Some(0),
            "cursor=Some skeleton messageCount SHALL be 0"
        );
        assert_eq!(
            item.get("isOngoing").and_then(serde_json::Value::as_bool),
            Some(false),
            "cursor=Some skeleton isOngoing SHALL be false"
        );
    }
    let skeleton_ids: Vec<String> = items
        .iter()
        .map(|it| {
            it.get("sessionId")
                .and_then(serde_json::Value::as_str)
                .unwrap()
                .to_owned()
        })
        .collect();

    // 后台扫描 SHALL 通过 SSE bridge emit 对应的 SessionMetadataUpdate（每条带真值）
    let mut received = std::collections::HashMap::new();
    while received.len() < skeleton_ids.len() {
        let event = timeout(Duration::from_secs(5), h.events_rx.recv())
            .await
            .expect("timed out waiting for PushEvent for skeleton items")
            .expect("recv ok");
        if let PushEvent::SessionMetadataUpdate {
            project_id,
            session_id,
            title,
            message_count,
            is_ongoing,
            git_branch: _,
        } = event
        {
            assert_eq!(project_id, h.project_id);
            if skeleton_ids.contains(&session_id) {
                received.insert(session_id, (title.clone(), message_count, is_ongoing));
            }
        }
    }
    for sid in &skeleton_ids {
        let (title, count, ongoing) = received.get(sid).expect("missing update for skeleton item");
        assert!(
            title.as_deref().is_some_and(|t| !t.is_empty()),
            "title SHALL be populated, got {title:?}",
        );
        assert_eq!(*count, 2, "fixture has 1 user + 1 assistant");
        assert!(
            *ongoing,
            "fixture assistant tool_use sans tool_result + recent mtime SHALL be ongoing",
        );
    }
}
