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

    let titles = vec![
        "重构 auth 模块",
        "修复 sidebar bug",
        "性能优化探索",
        "旧会话",
    ];
    let (api, _tmp, project_id, session_ids) = build_api_with_fixtures(&titles).await;

    let mut rx = api.subscribe_session_metadata();

    let pagination = PaginatedRequest {
        page_size: 3,
        cursor: None,
    };
    let resp = api.list_sessions(&project_id, &pagination).await.unwrap();

    // 骨架契约：title=None / message_count=0 / is_ongoing=false
    assert_eq!(resp.items.len(), 3);
    assert_eq!(resp.next_cursor.as_deref(), Some("3"));
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

    let page_ids: std::collections::BTreeSet<_> =
        resp.items.iter().map(|s| s.session_id.clone()).collect();
    let all_ids: std::collections::BTreeSet<_> = session_ids.into_iter().collect();
    let non_page_ids: Vec<_> = all_ids.difference(&page_ids).cloned().collect();

    // 异步收齐当前页 update（最多等 5s）
    let mut received = std::collections::HashMap::new();
    while received.len() < resp.items.len() {
        let upd = timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("timed out waiting for SessionMetadataUpdate")
            .expect("recv");
        assert_eq!(upd.project_id, project_id);
        received.insert(upd.session_id.clone(), upd);
    }

    // 元数据真值断言：只扫描当前响应页
    for sid in &page_ids {
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
    for sid in non_page_ids {
        assert!(
            !received.contains_key(&sid),
            "non-page session {sid} should not be scanned"
        );
    }
}

#[tokio::test]
async fn repeated_list_sessions_returns_cached_metadata_inline() {
    // 回归 fix `session-title-race`：第一次 list_sessions 完成后 cache 已写入；
    // 第二次同 project 调用 SHALL 在骨架阶段直接 inline 填回 title/messageCount
    // /isOngoing/gitBranch，**不再**依赖后台 broadcast emit。
    //
    // 这是修复 sidebar 偶发 session 显示"短 hash"的兜底——即使前端 listener
    // 注册晚于 emit、tauri fire-and-forget 丢消息，重复打开列表仍能从 cache
    // 拿到完整元数据。
    use cdt_api::DataApi;

    let titles = vec!["重构 auth 模块", "修复 sidebar bug", "性能优化探索"];
    let (api, _tmp, project_id, _session_ids) = build_api_with_fixtures(&titles).await;

    let mut rx = api.subscribe_session_metadata();
    let pagination = PaginatedRequest {
        page_size: 3,
        cursor: None,
    };

    // 第一次：骨架 title=None；收齐 N 条 update 让 cache 写满
    let first = api.list_sessions(&project_id, &pagination).await.unwrap();
    assert_eq!(first.items.len(), 3);
    for s in &first.items {
        assert!(
            s.title.is_none(),
            "first call skeleton title should be None"
        );
    }
    for _ in 0..first.items.len() {
        let upd = timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("first-pass scan timed out")
            .expect("recv");
        assert_eq!(upd.project_id, project_id);
    }

    // 第二次：cache 全命中 → 骨架阶段就带元数据 → 不再 spawn 任何扫描任务
    // → 不再 emit 任何 update
    let second = api.list_sessions(&project_id, &pagination).await.unwrap();
    assert_eq!(second.items.len(), 3);
    for s in &second.items {
        assert!(
            s.title.as_deref().is_some_and(|t| !t.is_empty()),
            "second call should inline cached title, got {:?}",
            s.title
        );
        assert_eq!(
            s.message_count, 2,
            "second call should inline cached message_count"
        );
        assert!(s.is_ongoing, "second call should inline cached is_ongoing");
    }

    // 短时间内不应收到任何新 update（cache 全命中 → page_jobs 空 → 不 spawn）
    let result = timeout(Duration::from_millis(300), rx.recv()).await;
    assert!(
        result.is_err(),
        "cache fast-path 应跳过后台扫描，却收到了 update：{result:?}"
    );
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

#[tokio::test]
async fn concurrent_list_sessions_does_not_orphan_scan() {
    // 并发调用同 project 的 list_sessions 不能让任一 task 变成"孤儿"——
    // 即第二次 list_sessions 进入时 SHALL 能 abort 当前 in-flight scan，
    // 后续 list_sessions 也 SHALL 能 abort 第二次的 scan。
    //
    // 回归 codex 二审第二轮发现的 race：spawn 与 insert 之间 lock 释放，
    // A 的 spawn → B abort/spawn/insert → A 晚到 insert 覆盖 B 的 entry，
    // 后续 C 无法 abort B 的 task。修复后 abort/spawn/insert 在同一 sync
    // lock 下原子完成，event 总数依然受 2*N 上界约束。
    use cdt_api::DataApi;
    use std::sync::Arc as StdArc;

    let titles: Vec<String> = (0..16).map(|i| format!("title-{i}")).collect();
    let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let (api, _tmp, project_id, _session_ids) = build_api_with_fixtures(&title_refs).await;
    let api = StdArc::new(api);

    let mut rx = api.subscribe_session_metadata();
    let pagination = PaginatedRequest {
        page_size: 50,
        cursor: None,
    };

    // join_all 三个并发 list_sessions（同 project）
    let api_a = api.clone();
    let pid_a = project_id.clone();
    let pag_a = pagination.clone();
    let api_b = api.clone();
    let pid_b = project_id.clone();
    let pag_b = pagination.clone();
    let api_c = api.clone();
    let pid_c = project_id.clone();

    let _ = tokio::join!(
        async move { api_a.list_sessions(&pid_a, &pag_a).await.unwrap() },
        async move { api_b.list_sessions(&pid_b, &pag_b).await.unwrap() },
        async move { api_c.list_sessions(&pid_c, &pagination).await.unwrap() },
    );

    // 收集 update 事件；至多 3*N（最坏情况三次都全跑完）；race 修复后实际
    // 应远低于该上限（前 2 次基本被 abort）。
    let mut total = 0_usize;
    let max_expected = titles.len() * 3;
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
                    "received more than 3*N updates ({total}); orphan scan 未被 abort"
                );
            }
            Ok(Err(_)) | Err(_) => break,
        }
    }
    assert!(
        total >= titles.len(),
        "expected at least N={} updates from final scan, got {total}",
        titles.len()
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
