//! `LocalDataApi::list_sessions_filtered` 行为测试（CLI/MCP 流式扫描路径）。
//!
//! 覆盖 grep / branch / limit 过滤 + 无过滤全返 + 过滤先于 limit 截断。
//! since/until（mtime/created 维度）依赖文件系统时间戳，难以确定性构造，
//! 由 cdt-query `query_filter` + CLI 集成测试间接覆盖。

use std::sync::Arc;

use cdt_api::{LocalDataApi, SessionListFilter};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use tempfile::TempDir;

async fn setup_api() -> (Arc<LocalDataApi>, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    let scanner = ProjectScanner::new(
        Arc::new(LocalFileSystemProvider::new()),
        projects_base.clone(),
    );
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.expect("config load");
    let notif_mgr = NotificationManager::new(None);
    let ssh_mgr = SshConnectionManager::new();

    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);
    (Arc::new(api), tmp)
}

/// 写一个单消息 session jsonl，控制 title（首条 user 文本）与 git 分支。
async fn write_session(
    dir: &std::path::Path,
    session_id: &str,
    ts: &str,
    title: &str,
    branch: &str,
) {
    let line = format!(
        r#"{{"type":"user","uuid":"{session_id}","parentUuid":null,"timestamp":"{ts}","isSidechain":false,"userType":"external","cwd":"/tmp/proj","sessionId":"{session_id}","version":"1","gitBranch":"{branch}","message":{{"role":"user","content":"{title}"}}}}"#,
    );
    tokio::fs::write(dir.join(format!("{session_id}.jsonl")), format!("{line}\n"))
        .await
        .unwrap();
}

const PROJECT_ID: &str = "-tmp-proj";

async fn seed_three_sessions(tmp: &TempDir) {
    let dir = tmp.path().join("projects").join(PROJECT_ID);
    tokio::fs::create_dir_all(&dir).await.unwrap();
    write_session(
        &dir,
        "sess-a",
        "2026-05-01T10:00:00Z",
        "Fix auth bug",
        "feat/auth",
    )
    .await;
    write_session(
        &dir,
        "sess-b",
        "2026-05-02T10:00:00Z",
        "Add login page",
        "feat/login",
    )
    .await;
    write_session(
        &dir,
        "sess-c",
        "2026-05-03T10:00:00Z",
        "Refactor auth flow",
        "feat/auth",
    )
    .await;
}

#[tokio::test]
async fn no_filter_returns_all_sessions() {
    let (api, tmp) = setup_api().await;
    seed_three_sessions(&tmp).await;

    let out = api
        .list_sessions_filtered(PROJECT_ID, &SessionListFilter::default())
        .await
        .unwrap();
    assert_eq!(out.len(), 3, "无过滤 SHALL 返回全部 session");
}

#[tokio::test]
async fn grep_filters_by_title_substring_case_insensitive() {
    let (api, tmp) = setup_api().await;
    seed_three_sessions(&tmp).await;

    let filter = SessionListFilter {
        grep: Some("AUTH".to_owned()),
        ..Default::default()
    };
    let out = api
        .list_sessions_filtered(PROJECT_ID, &filter)
        .await
        .unwrap();
    let ids: Vec<&str> = out.iter().map(|s| s.session_id.as_str()).collect();
    assert_eq!(ids.len(), 2, "grep 'auth' SHALL 命中两个含 auth 的标题");
    assert!(ids.contains(&"sess-a"));
    assert!(ids.contains(&"sess-c"));
    assert!(!ids.contains(&"sess-b"));
}

#[tokio::test]
async fn branch_filters_by_git_branch_substring() {
    let (api, tmp) = setup_api().await;
    seed_three_sessions(&tmp).await;

    let filter = SessionListFilter {
        branch: Some("login".to_owned()),
        ..Default::default()
    };
    let out = api
        .list_sessions_filtered(PROJECT_ID, &filter)
        .await
        .unwrap();
    assert_eq!(out.len(), 1, "branch 'login' SHALL 仅命中一个 session");
    assert_eq!(out[0].session_id, "sess-b");
}

#[tokio::test]
async fn limit_caps_result_count() {
    let (api, tmp) = setup_api().await;
    seed_three_sessions(&tmp).await;

    let filter = SessionListFilter {
        limit: Some(2),
        ..Default::default()
    };
    let out = api
        .list_sessions_filtered(PROJECT_ID, &filter)
        .await
        .unwrap();
    assert_eq!(out.len(), 2, "limit=2 SHALL 最多返回两条");
}

#[tokio::test]
async fn filter_applies_before_limit() {
    // 流式扫描的关键不变量：过滤（grep/branch）在 limit 截断之前发生。
    // 三个 session 里两个匹配 grep="auth"，limit=2 → 两个都返回（不会因为
    // 中间有不匹配的 sess-b 被先截掉而漏掉 sess-c）。
    let (api, tmp) = setup_api().await;
    seed_three_sessions(&tmp).await;

    let filter = SessionListFilter {
        grep: Some("auth".to_owned()),
        limit: Some(2),
        ..Default::default()
    };
    let out = api
        .list_sessions_filtered(PROJECT_ID, &filter)
        .await
        .unwrap();
    let ids: Vec<&str> = out.iter().map(|s| s.session_id.as_str()).collect();
    assert_eq!(
        ids.len(),
        2,
        "过滤先于 limit：两个 auth session 都 SHALL 返回"
    );
    assert!(ids.contains(&"sess-a"));
    assert!(ids.contains(&"sess-c"));
}

#[tokio::test]
async fn results_carry_cwd_and_title_metadata() {
    let (api, tmp) = setup_api().await;
    seed_three_sessions(&tmp).await;

    let out = api
        .list_sessions_filtered(PROJECT_ID, &SessionListFilter::default())
        .await
        .unwrap();
    let a = out.iter().find(|s| s.session_id == "sess-a").unwrap();
    assert_eq!(a.title.as_deref(), Some("Fix auth bug"), "title SHALL 提取");
    assert_eq!(
        a.cwd.as_deref(),
        Some("/tmp/proj"),
        "cwd SHALL 在结果集补齐"
    );
    assert_eq!(
        a.git_branch.as_deref(),
        Some("feat/auth"),
        "git_branch SHALL 提取"
    );
}
