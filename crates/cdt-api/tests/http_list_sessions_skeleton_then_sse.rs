//! Integration test：HTTP `GET /api/projects/{projectId}/sessions` 走骨架 +
//! SSE patch 路径。
//!
//! 覆盖 spec：
//! - `ipc-data-api` §"Expose project and session queries" 段落
//!   "HTTP `list_sessions` 复用 IPC 骨架 + push 实现"
//! - `http-data-api` §"Serve projects and sessions over HTTP under /api prefix"
//!   Scenario `GET paginated sessions returns skeleton with cache-hit inline real values`
//!   Scenario `HTTP list_sessions 后台扫描产物经 SSE 推送`（spec `ipc-data-api`）

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

#[tokio::test]
async fn http_get_sessions_returns_skeleton_with_placeholder_metadata() {
    let mut h = build_harness(&["改 auth", "修 sidebar bug", "perf 优化"]).await;
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
        // 骨架契约：title=null / messageCount=0 / isOngoing=false（cache miss
        // 路径，未走 try_lookup_cached_metadata 命中分支）
        assert!(
            item.get("title").is_some_and(serde_json::Value::is_null),
            "skeleton title SHALL be null, got {:?}",
            item.get("title")
        );
        assert_eq!(
            item.get("messageCount").and_then(serde_json::Value::as_u64),
            Some(0),
            "skeleton messageCount SHALL be 0"
        );
        assert_eq!(
            item.get("isOngoing").and_then(serde_json::Value::as_bool),
            Some(false),
            "skeleton isOngoing SHALL be false"
        );
        let sid = item
            .get("sessionId")
            .and_then(serde_json::Value::as_str)
            .unwrap();
        assert!(h.session_ids.contains(&sid.to_owned()));
        assert_eq!(
            item.get("projectId").and_then(serde_json::Value::as_str),
            Some(h.project_id.as_str())
        );
    }
    assert_eq!(
        body.get("total").and_then(serde_json::Value::as_u64),
        Some(3),
        "total SHALL match all sessions in project"
    );

    // 后台扫描应通过 SSE bridge emit 3 条 SessionMetadataUpdate（每条带真值）。
    let mut received = std::collections::HashMap::new();
    while received.len() < 3 {
        let event = timeout(Duration::from_secs(5), h.events_rx.recv())
            .await
            .expect("timed out waiting for PushEvent")
            .expect("recv ok");
        if let PushEvent::SessionMetadataUpdate {
            project_id,
            session_id,
            title,
            message_count,
            is_ongoing,
            git_branch: _,
            context_id: _,
        } = event
        {
            assert_eq!(project_id, h.project_id);
            received.insert(session_id, (title.clone(), message_count, is_ongoing));
        }
    }
    for sid in &h.session_ids {
        let (title, count, ongoing) = received.get(sid).expect("missing update");
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
