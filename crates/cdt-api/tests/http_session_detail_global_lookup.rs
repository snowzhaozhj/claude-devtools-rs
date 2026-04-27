//! 集成测试：HTTP `GET /api/sessions/:id` 的全局反查路径。
//!
//! 覆盖 spec：
//! - `http-data-api` §"Serve projects and sessions over HTTP under /api prefix"
//!   Scenario `GET session detail resolves project id internally` /
//!   `GET session detail unknown session returns 404` /
//!   `POST sessions batch with mixed-existence ids`
//! - `ipc-data-api` §"Resolve project id from session id alone"
//!   Scenario `LocalDataApi 直扫 FS 命中主会话` / `LocalDataApi 命中 subagent jsonl` /
//!   `LocalDataApi 多 project 命中第一个` / `LocalDataApi 找不到时返 None` /
//!   `与 get_session_detail 口径一致`
//!
//! tmpdir 起 `LocalDataApi`，`scanner.projects_dir()` 指向 tmp，让
//! `LocalDataApi::find_session_project` / `get_session_detail` 共享同一份
//! projects 根（`get_session_detail` 已切到 `scanner.projects_dir()`，见
//! change `fix-http-session-detail-and-event-bridge` 的 task 2.2）。

use std::sync::Arc;

use cdt_api::{DataApi, LocalDataApi};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;

/// 写一个最小可解析的 fixture jsonl：1 条 user + 1 条 assistant text。
async fn write_fixture_session(dir: &std::path::Path, session_id: &str, title: &str) {
    tokio::fs::create_dir_all(dir).await.unwrap();
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
        "timestamp": "2026-04-18T10:00:00Z",
        "cwd": "/tmp/proj",
        "message": {"role": "user", "content": title}
    })
    .to_string();
    let assistant = serde_json::json!({
        "type": "assistant",
        "uuid": format!("a-{session_id}"),
        "timestamp": "2026-04-18T10:00:01Z",
        "cwd": "/tmp/proj",
        "message": {
            "role": "assistant",
            "model": "claude-sonnet",
            "content": [{"type": "text", "text": "ack"}]
        }
    })
    .to_string();

    f.write_all(user.as_bytes()).await.unwrap();
    f.write_all(b"\n").await.unwrap();
    f.write_all(assistant.as_bytes()).await.unwrap();
    f.write_all(b"\n").await.unwrap();
    f.flush().await.unwrap();
}

/// 构造指向 tmp 目录的 `LocalDataApi`。
async fn build_api(tmp: &TempDir) -> Arc<LocalDataApi> {
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base);
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.unwrap();
    let mut notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    notif_mgr.load().await.unwrap();
    let ssh_mgr = SshConnectionManager::new();
    Arc::new(LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr))
}

#[tokio::test]
async fn find_session_project_hits_main_session() {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(projects_base.join("-proj-A")).unwrap();
    std::fs::create_dir_all(projects_base.join("-proj-B")).unwrap();
    write_fixture_session(&projects_base.join("-proj-A"), "sid-A", "title A").await;
    write_fixture_session(&projects_base.join("-proj-B"), "sid-B", "title B").await;

    let api = build_api(&tmp).await;

    let pid_a = api.find_session_project("sid-A").await.unwrap();
    assert_eq!(pid_a.as_deref(), Some("-proj-A"));

    let pid_b = api.find_session_project("sid-B").await.unwrap();
    assert_eq!(pid_b.as_deref(), Some("-proj-B"));
}

#[tokio::test]
async fn find_session_project_returns_none_for_unknown_sid() {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(projects_base.join("-proj-A")).unwrap();
    write_fixture_session(&projects_base.join("-proj-A"), "sid-A", "title A").await;

    let api = build_api(&tmp).await;

    let result = api.find_session_project("sid-ghost").await.unwrap();
    assert!(
        result.is_none(),
        "找不到时 SHALL 返回 Ok(None)，实际：{result:?}"
    );
}

#[tokio::test]
async fn find_session_project_hits_subagent_in_new_structure() {
    // 新结构：<project_dir>/<parent>/subagents/agent-<sub_sid>.jsonl
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    let proj_dir = projects_base.join("-proj-with-sub");
    let subagents_dir = proj_dir.join("parent-uuid").join("subagents");
    std::fs::create_dir_all(&subagents_dir).unwrap();
    // 写一个子 agent jsonl（路径定位测试不依赖内容是否解析得通）
    let sub_path = subagents_dir.join("agent-sub-1.jsonl");
    tokio::fs::write(&sub_path, b"").await.unwrap();

    let api = build_api(&tmp).await;

    let pid = api.find_session_project("sub-1").await.unwrap();
    assert_eq!(
        pid.as_deref(),
        Some("-proj-with-sub"),
        "SHALL 命中含 subagent jsonl 的 project"
    );
}

#[tokio::test]
async fn find_session_project_hits_first_matching_project() {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    // 三个 project；只有第三个含目标 sid
    std::fs::create_dir_all(projects_base.join("-proj-1")).unwrap();
    std::fs::create_dir_all(projects_base.join("-proj-2")).unwrap();
    std::fs::create_dir_all(projects_base.join("-proj-3")).unwrap();
    write_fixture_session(&projects_base.join("-proj-3"), "sid-3", "title 3").await;

    let api = build_api(&tmp).await;

    let pid = api.find_session_project("sid-3").await.unwrap();
    assert_eq!(pid.as_deref(), Some("-proj-3"));
}

#[tokio::test]
async fn get_session_detail_after_find_session_project_succeeds() {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    let proj_dir = projects_base.join("-proj-A");
    std::fs::create_dir_all(&proj_dir).unwrap();
    write_fixture_session(&proj_dir, "sid-A", "title A").await;

    let api = build_api(&tmp).await;

    let pid = api
        .find_session_project("sid-A")
        .await
        .unwrap()
        .expect("命中后 SHALL 返 Some");
    let detail = api
        .get_session_detail(&pid, "sid-A")
        .await
        .expect("反查 + detail 复合路径 SHALL 成功");
    assert_eq!(detail.session_id, "sid-A");
    assert_eq!(detail.project_id, "-proj-A");

    // 收紧断言：fixture 含 1 user + 1 assistant text，build_chunks_with_subagents
    // 后 SHALL 产 2 个 chunk（user + ai）；context_injections SHALL 是 JSON
    // array 形态（fixture 无 CLAUDE.md，应为空数组），证明 detail 真走过
    // build_chunks → process_session_context_with_phases 完整路径。
    let chunks = detail
        .chunks
        .as_array()
        .expect("chunks SHALL 是 JSON array");
    assert_eq!(
        chunks.len(),
        2,
        "fixture 1 user + 1 assistant SHALL 产 2 chunks，实际 {}",
        chunks.len()
    );
    assert!(
        detail.context_injections.is_array(),
        "context_injections SHALL 是 array（无 CLAUDE.md 时为空数组）"
    );
}

#[tokio::test]
async fn get_sessions_by_ids_handles_mixed_existence() {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    let proj_dir = projects_base.join("-proj-A");
    std::fs::create_dir_all(&proj_dir).unwrap();
    write_fixture_session(&proj_dir, "sid-existing", "title").await;

    let api = build_api(&tmp).await;

    let results = api
        .get_sessions_by_ids(&["sid-existing".to_owned(), "sid-ghost".to_owned()])
        .await
        .expect("batch SHALL 不报错，缺失条目走占位");

    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0].project_id, "-proj-A",
        "存在的 sid SHALL 反查到正确 project_id"
    );
    assert_eq!(results[0].session_id, "sid-existing");
    assert_eq!(
        results[1].project_id, "",
        "找不到的 sid SHALL projectId 为空字符串占位"
    );
    assert_eq!(
        results[1].metadata,
        serde_json::json!({"status": "not_found"}),
        "找不到的 sid SHALL metadata.status 为 not_found"
    );
}
