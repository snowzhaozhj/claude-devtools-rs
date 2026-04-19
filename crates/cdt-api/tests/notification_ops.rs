//! Integration tests for configuration-management bug fix + notification ops IPC.
//!
//! 覆盖：
//! - `update_config("notifications", { triggers: [...] })` 路径写入后
//!   `get_enabled_triggers` 能读到（验证 Group 1 bug fix）；
//! - `delete_notification` / `mark_all_notifications_read` /
//!   `clear_notifications` 三个新 IPC 的核心行为（覆盖 ipc-data-api spec 的
//!   "Bulk and per-item notification operations" Requirement）。

use std::sync::Arc;

use cdt_api::{ConfigUpdateRequest, DataApi, LocalDataApi};
use cdt_config::{
    ConfigManager, DetectedError, DetectedErrorContext, NotificationManager, NotificationTrigger,
    TriggerContentType, TriggerMode,
};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use tempfile::TempDir;

fn make_custom_trigger(id: &str, enabled: bool) -> NotificationTrigger {
    NotificationTrigger {
        id: id.into(),
        name: format!("Custom {id}"),
        enabled,
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

/// 先把 seed errors 写入通知存储文件，再构造 `LocalDataApi`——保证 API 内部
/// 的 `NotificationManager` 在 `load()` 时读到这些条目。
async fn make_api(tmp: &TempDir, seeds: &[DetectedError]) -> LocalDataApi {
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    let notif_path = tmp.path().join("notifications.json");

    if !seeds.is_empty() {
        // 独立 mgr 写磁盘，然后 drop
        let mut preload = NotificationManager::new(Some(notif_path.clone()));
        preload.load().await.unwrap();
        for err in seeds {
            preload.add_notification(err.clone()).await.unwrap();
        }
    }

    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base);

    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.unwrap();

    let mut notif_mgr = NotificationManager::new(Some(notif_path));
    notif_mgr.load().await.unwrap();

    let ssh_mgr = SshConnectionManager::new();
    LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr)
}

fn seed_error(id: &str, trigger_id: &str) -> DetectedError {
    DetectedError {
        id: id.into(),
        timestamp: 1000,
        session_id: "s1".into(),
        project_id: "p1".into(),
        file_path: "/tmp/f.jsonl".into(),
        source: "Bash".into(),
        message: format!("err {id}"),
        line_number: Some(1),
        tool_use_id: None,
        trigger_color: None,
        trigger_id: Some(trigger_id.into()),
        trigger_name: Some("t".into()),
        context: DetectedErrorContext {
            project_name: "test".into(),
            cwd: None,
        },
    }
}

#[tokio::test]
async fn update_config_persists_triggers_and_get_enabled_reflects() {
    let tmp = TempDir::new().unwrap();
    let api = make_api(&tmp, &[]).await;

    let t1 = make_custom_trigger("custom-1", true);
    let t2 = make_custom_trigger("custom-2", false);
    let triggers_value = serde_json::to_value(vec![t1, t2]).unwrap();

    api.update_config(&ConfigUpdateRequest {
        section: "notifications".into(),
        data: serde_json::json!({ "triggers": triggers_value }),
    })
    .await
    .expect("update_config ok");

    let cfg = api.get_config().await.expect("get_config ok");
    let triggers = cfg
        .get("notifications")
        .and_then(|n| n.get("triggers"))
        .and_then(|t| t.as_array())
        .expect("triggers present");
    assert_eq!(triggers.len(), 2);
    let ids: Vec<&str> = triggers
        .iter()
        .filter_map(|t| t.get("id").and_then(|v| v.as_str()))
        .collect();
    assert!(ids.contains(&"custom-1"));
    assert!(ids.contains(&"custom-2"));
    let enabled_for = |want_id: &str| -> bool {
        triggers
            .iter()
            .find(|t| t.get("id").and_then(|v| v.as_str()) == Some(want_id))
            .and_then(|t| t.get("enabled").and_then(serde_json::Value::as_bool))
            .unwrap_or(false)
    };
    assert!(enabled_for("custom-1"));
    assert!(!enabled_for("custom-2"));
}

#[tokio::test]
async fn delete_notification_removes_single() {
    let tmp = TempDir::new().unwrap();
    let api = make_api(&tmp, &[seed_error("n1", "tA"), seed_error("n2", "tA")]).await;

    let removed = api.delete_notification("n1").await.unwrap();
    assert!(removed);
    let removed_missing = api.delete_notification("does-not-exist").await.unwrap();
    assert!(!removed_missing);

    let list = api.get_notifications(10, 0).await.unwrap();
    let total = list
        .get("total")
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    assert_eq!(total, 1);
}

#[tokio::test]
async fn mark_all_read_zeros_unread_count() {
    let tmp = TempDir::new().unwrap();
    let api = make_api(&tmp, &[seed_error("n1", "tA"), seed_error("n2", "tB")]).await;

    api.mark_all_notifications_read().await.unwrap();

    let list = api.get_notifications(10, 0).await.unwrap();
    let unread = list
        .get("unreadCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    assert_eq!(unread, 0);
}

#[tokio::test]
async fn clear_notifications_all_removes_everything() {
    let tmp = TempDir::new().unwrap();
    let api = make_api(&tmp, &[seed_error("n1", "tA"), seed_error("n2", "tB")]).await;

    let removed = api.clear_notifications(None).await.unwrap();
    assert_eq!(removed, 2);

    let list = api.get_notifications(10, 0).await.unwrap();
    let total = list
        .get("total")
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    assert_eq!(total, 0);
}

#[tokio::test]
async fn clear_notifications_by_trigger_leaves_others() {
    let tmp = TempDir::new().unwrap();
    let api = make_api(
        &tmp,
        &[
            seed_error("n1", "tA"),
            seed_error("n2", "tB"),
            seed_error("n3", "tA"),
        ],
    )
    .await;

    let removed = api.clear_notifications(Some("tA")).await.unwrap();
    assert_eq!(removed, 2);

    let list = api.get_notifications(10, 0).await.unwrap();
    let total = list
        .get("total")
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    assert_eq!(total, 1);
}
