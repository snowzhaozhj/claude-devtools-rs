//! Integration tests for skeleton `list_sessions` + async metadata broadcast.
//!
//! 覆盖 spec `ipc-data-api` §"Emit session metadata updates" 与
//! `sidebar-navigation` §"骨架列表快速加载" 的可观察契约：
//! - `list_sessions` 同步返回的 `SessionSummary` 元数据字段为占位
//! - `subscribe_session_metadata()` 收到 N 条 update（N = page 内 session 数）
//! - 同 project 重复调用不会引发事件无界爆炸（取消语义近似断言）

use std::sync::Arc;
use std::time::Duration;

use cdt_api::{LocalDataApi, PaginatedRequest};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

/// 在 tempdir 下构造一个 `projects/<encoded>/{sessId}.jsonl` 结构，并
/// 写入一条带 user 文本的最小 fixture，确保 `extract_session_metadata`
/// 能解析出 title 与 `message_count`。
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
        "timestamp": "2026-04-18T10:00:00Z",
        "cwd": "/tmp/proj",
        "message": {"role": "user", "content": title}
    })
    .to_string();
    // tool_use（无配对 tool_result）让 check_messages_ongoing 判 true，
    // 模拟"仍在进行的会话"——见 cdt-analyze session_state 测试
    // `ongoing_when_only_ai_activity`
    let assistant = serde_json::json!({
        "type": "assistant",
        "uuid": format!("a-{session_id}"),
        "timestamp": "2026-04-18T10:00:01Z",
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

async fn build_api_with_fixtures(titles: &[&str]) -> (LocalDataApi, TempDir, String, Vec<String>) {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    let project_dir_name = "-tmp-fixture";
    let project_dir = projects_base.join(project_dir_name);
    std::fs::create_dir_all(&project_dir).unwrap();

    let mut session_ids = Vec::new();
    for (i, title) in titles.iter().enumerate() {
        let sid = format!("sess-{i:04}");
        write_fixture_session(&project_dir, &sid, title).await;
        session_ids.push(sid);
    }

    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base.clone());
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.unwrap();
    let mut notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    notif_mgr.load().await.unwrap();
    let ssh_mgr = SshConnectionManager::new();
    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);
    (api, tmp, project_dir_name.to_owned(), session_ids)
}

#[tokio::test]
async fn list_sessions_returns_skeleton_and_emits_metadata_updates() {
    use cdt_api::DataApi;

    let titles = vec!["重构 auth 模块", "修复 sidebar bug", "性能优化探索"];
    let (api, _tmp, project_id, session_ids) = build_api_with_fixtures(&titles).await;

    let mut rx = api.subscribe_session_metadata();

    let pagination = PaginatedRequest {
        page_size: 50,
        cursor: None,
    };
    let resp = api.list_sessions(&project_id, &pagination).await.unwrap();

    // 骨架契约：title=None / message_count=0 / is_ongoing=false
    assert_eq!(resp.items.len(), titles.len());
    for s in &resp.items {
        assert!(
            s.title.is_none(),
            "skeleton title should be None, got {:?}",
            s.title
        );
        assert_eq!(s.message_count, 0, "skeleton message_count should be 0");
        assert!(!s.is_ongoing, "skeleton is_ongoing should be false");
        assert_eq!(s.project_id, project_id);
        assert!(session_ids.contains(&s.session_id));
    }

    // 异步收齐 N 条 update（最多等 5s）
    let mut received = std::collections::HashMap::new();
    while received.len() < titles.len() {
        let upd = timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("timed out waiting for SessionMetadataUpdate")
            .expect("recv");
        assert_eq!(upd.project_id, project_id);
        received.insert(upd.session_id.clone(), upd);
    }

    // 元数据真值断言
    for sid in &session_ids {
        let upd = received.get(sid).expect("missing update for session");
        // title fixture 是中文文本，extract_session_metadata 会清洗后取前 200 字符
        assert!(
            upd.title.as_deref().is_some_and(|t| !t.is_empty()),
            "title should be populated, got {:?}",
            upd.title
        );
        // 1 user + 1 assistant 应配对成 message_count=2
        assert_eq!(upd.message_count, 2, "expected message_count=2 for fixture");
        // assistant 是最后消息，无 ending event → ongoing
        assert!(
            upd.is_ongoing,
            "fixture ends on assistant tool/text → is_ongoing should be true"
        );
    }
}

#[tokio::test]
async fn repeated_list_sessions_aborts_previous_scan() {
    use cdt_api::DataApi;

    // 制造稍多 session，让两次扫描的事件总数与 N 的关系可观察
    let titles: Vec<String> = (0..16).map(|i| format!("title-{i}")).collect();
    let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let (api, _tmp, project_id, _session_ids) = build_api_with_fixtures(&title_refs).await;

    let mut rx = api.subscribe_session_metadata();
    let pagination = PaginatedRequest {
        page_size: 50,
        cursor: None,
    };

    // 连续两次调用——前一次未完成的扫描会被 abort
    let _ = api.list_sessions(&project_id, &pagination).await.unwrap();
    let _ = api.list_sessions(&project_id, &pagination).await.unwrap();

    // 收集一段时间内全部 update：上限是 2 * N（最坏情况两次都全跑完）
    let mut total = 0_usize;
    let max_expected = titles.len() * 2;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);

    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }
        match timeout(deadline - now, rx.recv()).await {
            Ok(Ok(upd)) => {
                assert_eq!(upd.project_id, project_id);
                total += 1;
                assert!(
                    total <= max_expected,
                    "received more than 2*N updates ({total}); abort 未生效"
                );
            }
            Ok(Err(_)) | Err(_) => break,
        }
    }

    // 至少有一次扫描完整跑完 → 收到至少 N 条
    assert!(
        total >= titles.len(),
        "expected at least N={} updates, got {total}",
        titles.len()
    );
    assert!(
        total <= max_expected,
        "abort 失效：updates={total} 超过 2*N={max_expected}"
    );
}

#[test]
fn metadata_scan_concurrency_is_eight() {
    // spec ipc-data-api §"Emit session metadata updates" 要求并发度受
    // `Semaphore` 限流；本测试把"上限值=8"作为契约直接断言，避免依赖
    // 文件 I/O timing 做运行时观测（macOS FSEvents flake 风险）。
    //
    // 实际限流逻辑见 `ipc::local::scan_metadata_for_page` 内的
    // `Semaphore::new(METADATA_SCAN_CONCURRENCY)` 调用。
    assert_eq!(cdt_api::METADATA_SCAN_CONCURRENCY, 8);
}
