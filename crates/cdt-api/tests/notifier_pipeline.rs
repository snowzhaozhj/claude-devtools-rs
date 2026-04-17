//! End-to-end integration test for the auto notification pipeline.
//!
//! 覆盖 `notification-triggers` spec 的 "Automatic background notification pipeline"
//! Requirement：写 JSONL → `FileWatcher` 广播 → `detect_errors` → `NotificationManager`
//! → `subscribe_detected_errors()` 端接收 `DetectedError`。
//!
//! 同时验证 `subscribe_detected_errors()` 在无 watcher 时返回一条永不发消息的
//! receiver（spec 的 "Subscription without a watcher attached" 场景）。

use std::sync::Arc;
use std::time::Duration;

use cdt_api::LocalDataApi;
use cdt_config::{
    ConfigManager, NotificationManager, NotificationTrigger, TriggerContentType, TriggerMode,
};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use cdt_watch::FileWatcher;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

fn make_error_trigger() -> NotificationTrigger {
    NotificationTrigger {
        id: "test-error".into(),
        name: "Tool error".into(),
        enabled: true,
        content_type: TriggerContentType::ToolResult,
        mode: TriggerMode::ErrorStatus,
        require_error: Some(true),
        is_builtin: None,
        tool_name: None,
        ignore_patterns: None,
        match_field: None,
        match_pattern: None,
        token_threshold: None,
        token_type: None,
        repository_ids: None,
        color: None,
    }
}

async fn append_jsonl_line(path: &std::path::Path, line: &str) {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.unwrap();
    }
    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
        .unwrap();
    f.write_all(line.as_bytes()).await.unwrap();
    f.write_all(b"\n").await.unwrap();
    f.flush().await.unwrap();
    f.sync_all().await.unwrap();
}

#[tokio::test]
async fn pipeline_emits_detected_error_on_new_jsonl_line() {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    let todos_base = tmp.path().join("todos");
    std::fs::create_dir_all(&todos_base).unwrap();

    // 先建 session 目录和初始文件，再启动 watcher（避免 watcher 漏掉 create 事件）
    let project_id = "-tmp-proj";
    let session_id = "sess-err-1";
    let session_dir = projects_base.join(project_id);
    std::fs::create_dir_all(&session_dir).unwrap();
    let jsonl = session_dir.join(format!("{session_id}.jsonl"));
    tokio::fs::write(&jsonl, "").await.unwrap();

    // 构造 LocalDataApi + 启动 FileWatcher + 注册 error trigger
    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base.clone());
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.unwrap();
    config_mgr.add_trigger(make_error_trigger()).await.unwrap();
    let mut notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    notif_mgr.load().await.unwrap();
    let ssh_mgr = SshConnectionManager::new();

    let watcher = Arc::new(FileWatcher::with_paths(
        projects_base.clone(),
        todos_base.clone(),
    ));
    let api = LocalDataApi::new_with_watcher(
        scanner,
        config_mgr,
        notif_mgr,
        ssh_mgr,
        watcher.as_ref(),
        projects_base.clone(),
    );

    let mut error_rx = api.subscribe_detected_errors();

    // 启动 watcher
    let watcher_for_task = watcher.clone();
    let _watcher_handle = tokio::spawn(async move {
        let _ = watcher_for_task.start().await;
    });

    // 给 watcher 一点时间挂载 notify backend（debounce 100ms）
    tokio::time::sleep(Duration::from_millis(150)).await;

    // 追加一条含 is_error: true 的 assistant message
    let line = serde_json::json!({
        "type": "assistant",
        "uuid": "u-err-1",
        "timestamp": "2026-04-17T10:00:00Z",
        "cwd": "/tmp/x",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "tu-1",
                    "name": "Bash",
                    "input": {"command": "ls"}
                }
            ]
        }
    })
    .to_string();
    append_jsonl_line(&jsonl, &line).await;

    let line2 = serde_json::json!({
        "type": "user",
        "uuid": "u-res-1",
        "timestamp": "2026-04-17T10:00:01Z",
        "message": {
            "role": "user",
            "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": "tu-1",
                    "is_error": true,
                    "content": "command failed catastrophically"
                }
            ]
        }
    })
    .to_string();
    append_jsonl_line(&jsonl, &line2).await;

    // 允许最多 5 秒收到 error（FS 事件 + debounce + parse）
    let err = timeout(Duration::from_secs(5), error_rx.recv())
        .await
        .expect("timed out waiting for DetectedError")
        .expect("error_rx recv");

    assert_eq!(err.session_id, session_id);
    assert_eq!(err.project_id, project_id);
    assert!(err.message.contains("command failed"));
    assert_eq!(err.trigger_id.as_deref(), Some("test-error"));
    assert_eq!(err.id.len(), 32); // 确定性 SHA-256 前 16 字节 hex
}

#[tokio::test]
async fn subscribe_detected_errors_without_watcher_is_silent_receiver() {
    // `LocalDataApi::new()`（无 watcher 构造器）下，subscribe 返回的 receiver
    // 不会 panic，也不会收到消息。
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base);
    let config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    let notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    let ssh_mgr = SshConnectionManager::new();
    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);

    let mut rx = api.subscribe_detected_errors();
    // 无 watcher 下 receiver 可能立即返回 `Closed`（sender 已 drop）或永不收到消息；
    // 两者在语义上都满足 "silent receiver"——关键是不会 panic，也不会收到 `Ok(error)`。
    let result = timeout(Duration::from_millis(200), rx.recv()).await;
    match result {
        Err(_) | Ok(Err(_)) => {} // timeout 或 Closed/Lagged：都是 silent
        Ok(Ok(err)) => panic!("unexpected DetectedError received: {err:?}"),
    }
}
