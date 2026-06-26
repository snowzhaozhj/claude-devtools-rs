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

/// 写 session 并显式设定 mtime（unix 秒），用于确定性验证全局 mtime 排序。
async fn write_session_at(dir: &std::path::Path, session_id: &str, mtime_unix: i64, title: &str) {
    write_session(dir, session_id, "2026-05-01T10:00:00Z", title, "main").await;
    let path = dir.join(format!("{session_id}.jsonl"));
    let ft = filetime::FileTime::from_unix_time(mtime_unix, 0);
    filetime::set_file_mtime(&path, ft).unwrap();
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
async fn limit_zero_returns_empty() {
    let (api, tmp) = setup_api().await;
    seed_three_sessions(&tmp).await;

    let filter = SessionListFilter {
        limit: Some(0),
        ..Default::default()
    };
    let out = api
        .list_sessions_filtered(PROJECT_ID, &filter)
        .await
        .unwrap();
    assert!(
        out.is_empty(),
        "limit=0 SHALL 返回空（不能 off-by-one 返回 1 条）"
    );
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

#[tokio::test]
async fn cross_project_global_merge_picks_newest_across_projects() {
    // 两个 project，session mtime 交错。全局 limit=2 SHALL 取**跨 project**
    // 全局最新的两条，而非每个 project 各取一条。
    let (api, tmp) = setup_api().await;
    let dir_x = tmp.path().join("projects").join("-tmp-projx");
    let dir_y = tmp.path().join("projects").join("-tmp-projy");
    tokio::fs::create_dir_all(&dir_x).await.unwrap();
    tokio::fs::create_dir_all(&dir_y).await.unwrap();

    // mtime: x-old=100, y-mid=200, x-new=300, y-newest=400
    write_session_at(&dir_x, "x-old", 100, "x old").await;
    write_session_at(&dir_y, "y-mid", 200, "y mid").await;
    write_session_at(&dir_x, "x-new", 300, "x new").await;
    write_session_at(&dir_y, "y-newest", 400, "y newest").await;

    let projects = vec![
        ("-tmp-projx".to_owned(), Some("ProjX".to_owned())),
        ("-tmp-projy".to_owned(), Some("ProjY".to_owned())),
    ];
    let filter = SessionListFilter {
        limit: Some(2),
        ..Default::default()
    };
    let out = api
        .list_sessions_filtered_cross_project(&projects, &filter)
        .await
        .unwrap();

    let ids: Vec<&str> = out.iter().map(|s| s.session_id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["y-newest", "x-new"],
        "全局 limit=2 SHALL 按 mtime 取跨 project 最新两条"
    );
    assert_eq!(
        out[0].project_name.as_deref(),
        Some("ProjY"),
        "project_name SHALL 按所属 project 回填"
    );
    assert_eq!(out[1].project_name.as_deref(), Some("ProjX"));
}

#[tokio::test]
async fn cross_project_grep_filters_across_projects_before_limit() {
    // 跨 project grep：内容过滤在全局 limit 之前生效。
    let (api, tmp) = setup_api().await;
    let dir_x = tmp.path().join("projects").join("-tmp-projx");
    let dir_y = tmp.path().join("projects").join("-tmp-projy");
    tokio::fs::create_dir_all(&dir_x).await.unwrap();
    tokio::fs::create_dir_all(&dir_y).await.unwrap();

    write_session_at(&dir_x, "x-deploy", 100, "deploy script").await;
    write_session_at(&dir_y, "y-noise", 200, "unrelated").await;
    write_session_at(&dir_x, "x-noise", 300, "other work").await;
    write_session_at(&dir_y, "y-deploy", 400, "deploy pipeline").await;

    let projects = vec![
        ("-tmp-projx".to_owned(), Some("ProjX".to_owned())),
        ("-tmp-projy".to_owned(), Some("ProjY".to_owned())),
    ];
    let filter = SessionListFilter {
        grep: Some("deploy".to_owned()),
        limit: Some(10),
        ..Default::default()
    };
    let out = api
        .list_sessions_filtered_cross_project(&projects, &filter)
        .await
        .unwrap();

    let ids: Vec<&str> = out.iter().map(|s| s.session_id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["y-deploy", "x-deploy"],
        "grep 'deploy' SHALL 跨 project 命中两条，按 mtime 排序"
    );
}

#[tokio::test]
async fn cross_project_skips_unreadable_project() {
    // 一个"坏 project"是文件而非目录（exists=true 但 read_dir 失败）。
    // 跨 project SHALL warn + skip，返回好 project 的 session，不整体报错。
    let (api, tmp) = setup_api().await;
    let dir_ok = tmp.path().join("projects").join("-tmp-ok");
    tokio::fs::create_dir_all(&dir_ok).await.unwrap();
    write_session_at(&dir_ok, "ok-sess", 100, "good session").await;
    // 坏 project：projects/-tmp-bad 是文件
    tokio::fs::write(tmp.path().join("projects").join("-tmp-bad"), b"not a dir")
        .await
        .unwrap();

    let projects = vec![
        ("-tmp-bad".to_owned(), Some("Bad".to_owned())),
        ("-tmp-ok".to_owned(), Some("Ok".to_owned())),
    ];
    let out = api
        .list_sessions_filtered_cross_project(&projects, &SessionListFilter::default())
        .await
        .expect("跨 project SHALL 不因单个坏 project 整体失败");
    let ids: Vec<&str> = out.iter().map(|s| s.session_id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["ok-sess"],
        "坏 project skip 后仍返回好 project 的 session"
    );
}

#[tokio::test]
async fn single_project_unreadable_propagates_error() {
    // 单 project：用户显式指定的 project 不可读 SHALL 传播错误（fail_fast）。
    let (api, tmp) = setup_api().await;
    tokio::fs::write(tmp.path().join("projects").join("-tmp-bad"), b"not a dir")
        .await
        .unwrap();

    let result = api
        .list_sessions_filtered("-tmp-bad", &SessionListFilter::default())
        .await;
    assert!(result.is_err(), "单 project 不可读 SHALL 返回 Err 而非空集");
}

#[tokio::test]
async fn cross_project_same_mtime_breaks_tie_by_sid() {
    // 同 mtime 时按 session id 升序（对齐 sidebar / spec k-way merge 稳序）。
    let (api, tmp) = setup_api().await;
    let dir_x = tmp.path().join("projects").join("-tmp-projx");
    let dir_y = tmp.path().join("projects").join("-tmp-projy");
    tokio::fs::create_dir_all(&dir_x).await.unwrap();
    tokio::fs::create_dir_all(&dir_y).await.unwrap();

    // 两个 session 同 mtime=500，sid 分别为 "aaa"（projX）/ "zzz"（projY）
    write_session_at(&dir_x, "aaa", 500, "from x").await;
    write_session_at(&dir_y, "zzz", 500, "from y").await;

    let projects = vec![
        ("-tmp-projx".to_owned(), Some("ProjX".to_owned())),
        ("-tmp-projy".to_owned(), Some("ProjY".to_owned())),
    ];
    let out = api
        .list_sessions_filtered_cross_project(&projects, &SessionListFilter::default())
        .await
        .unwrap();
    let ids: Vec<&str> = out.iter().map(|s| s.session_id.as_str()).collect();
    assert_eq!(ids, vec!["aaa", "zzz"], "同 mtime SHALL 按 sid 升序");
}
