//! Integration test：HTTP `list_sessions` 在 `MetadataCache` 命中时骨架阶段
//! 直接 inline 填回真值（zero 后续 SSE emit）。
//!
//! 覆盖 spec `http-data-api` §"Serve projects and sessions over HTTP under
//! /api prefix" Scenario `GET paginated sessions returns skeleton with
//! cache-hit inline real values` 以及 `ipc-data-api` §"Expose project and
//! session queries" 段落 "HTTP `list_sessions` 复用 IPC 骨架 + push 实现"。

use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use cdt_api::http::{AppState, StaticServe, build_router};
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
async fn http_get_sessions_cache_hit_inlines_real_values_with_zero_emit() {
    // Setup：3 个 session fixture
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();
    let project_id = "-tmp-cache-fixture".to_owned();
    let project_dir = projects_base.join(&project_id);
    std::fs::create_dir_all(&project_dir).unwrap();
    let titles = ["改 auth", "修 sidebar", "perf 优化"];
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

    let (events_tx, mut events_rx) = broadcast::channel::<PushEvent>(64);
    let (_file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(16);
    let (_todo_tx, todo_rx) = broadcast::channel::<TodoChangeEvent>(16);
    let (_error_tx, error_rx) = broadcast::channel(16);
    let metadata_rx = api.subscribe_session_metadata();
    let context_rx = api.subscribe_context_changed();
    let jobs_rx = api.subscribe_jobs();
    spawn_event_bridge(
        events_tx.clone(),
        file_rx,
        todo_rx,
        error_rx,
        metadata_rx,
        context_rx,
        jobs_rx,
    );

    let state = AppState {
        api: api as Arc<dyn DataApi>,
        events_tx,
    };
    let router = build_router(state, StaticServe::None);

    // 第一次 GET：cache miss → 骨架返 + 后台扫描 → events_tx 收 3 条 emit
    let url = format!("/api/projects/{project_id}/sessions?pageSize=20");
    let resp = router
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
    assert_eq!(resp.status(), StatusCode::OK);
    let _ = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();

    // 等收齐 3 条 metadata update（确保后台扫描完成 + cache 写入）
    let mut warm_count = 0;
    while warm_count < 3 {
        let event = timeout(Duration::from_secs(5), events_rx.recv())
            .await
            .expect("warmup recv timed out")
            .expect("recv ok");
        if matches!(event, PushEvent::SessionMetadataUpdate { .. }) {
            warm_count += 1;
        }
    }

    // 第二次 GET：cache 已预热，骨架阶段 try_lookup_cached_metadata SHALL 全命中
    let resp = router
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
        // cache hit 路径：骨架阶段直接 inline 填回真值
        assert!(
            item.get("title")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|t| !t.is_empty()),
            "cache hit title SHALL be populated, got {:?}",
            item.get("title")
        );
        assert_eq!(
            item.get("messageCount").and_then(serde_json::Value::as_u64),
            Some(2),
            "cache hit messageCount SHALL be 2 (1 user + 1 assistant fixture)"
        );
        assert_eq!(
            item.get("isOngoing").and_then(serde_json::Value::as_bool),
            Some(true),
            "cache hit isOngoing SHALL be true (fixture tail tool_use sans tool_result + mtime fresh)"
        );
    }

    // cache 命中路径 SHALL 零 SSE emit。给后台 200ms 窗口看会否有意外 emit。
    let sleep_until = tokio::time::Instant::now() + Duration::from_millis(200);
    loop {
        let now = tokio::time::Instant::now();
        if now >= sleep_until {
            break;
        }
        match timeout(sleep_until - now, events_rx.recv()).await {
            Ok(Ok(PushEvent::SessionMetadataUpdate { session_id, .. })) => {
                panic!(
                    "cache-hit path SHALL NOT emit metadata update; got unexpected emit for {session_id}"
                );
            }
            Ok(Ok(_) | Err(broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(broadcast::error::RecvError::Closed)) | Err(_) => break,
        }
    }
}
