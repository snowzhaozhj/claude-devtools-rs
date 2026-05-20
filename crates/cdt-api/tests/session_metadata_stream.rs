//! Integration tests for `list_sessions` cursor-branched eager/skeleton +
//! async metadata broadcast。
//!
//! 覆盖 spec `ipc-data-api` §"Emit session metadata updates"、
//! `sidebar-navigation` §"骨架列表快速加载"、以及 change
//! `eager-first-page-metadata` D4b / D6b / D6c / D8 / D11 的可观察契约：
//! - cursor=None：前 `EAGER_FIRST_PAGE_LIMIT` 条同步等真值 inline 返、零 SSE emit
//! - cursor=Some：返回骨架 + 后台 scan + broadcast push（原行为不变）
//! - 首页 eager 单条解析失败 → 占位 + deferred retry emit + 不写正向 cache
//! - cursor=None pageSize > 20 → 前 20 eager + 剩余条 remainder scan + emit
//! - 同 project 切换：跨 project 的旧 scan 被 abort 让出 permits

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

/// 写一个完全损坏的 jsonl（非法 JSON），让 `extract_session_metadata`
/// 解析出全占位字段——触发 D6b（不写正向 cache）+ D6c（写 negative TTL）。
async fn write_corrupt_session(dir: &std::path::Path, session_id: &str) {
    let path = dir.join(format!("{session_id}.jsonl"));
    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .await
        .unwrap();
    f.write_all(b"{not valid json at all\n").await.unwrap();
    f.write_all(b"completely broken line\n").await.unwrap();
    f.flush().await.unwrap();
}

async fn build_api_in_dir(
    tmp: &TempDir,
    project_dir_name: &str,
    titles: &[&str],
) -> (LocalDataApi, String, Vec<String>) {
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();
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
    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);
    (api, project_dir_name.to_owned(), session_ids)
}

async fn build_api_with_fixtures(titles: &[&str]) -> (LocalDataApi, TempDir, String, Vec<String>) {
    let tmp = TempDir::new().unwrap();
    let (api, project_id, session_ids) = build_api_in_dir(&tmp, "-tmp-fixture", titles).await;
    (api, tmp, project_id, session_ids)
}

#[tokio::test]
async fn cursor_none_eager_inlines_real_values_with_zero_emit_within_window() {
    use cdt_api::DataApi;

    let titles = vec!["重构 auth 模块", "修复 sidebar bug", "性能优化探索"];
    let (api, _tmp, project_id, _session_ids) = build_api_with_fixtures(&titles).await;

    let mut rx = api.subscribe_session_metadata();
    let pagination = PaginatedRequest {
        page_size: 3,
        cursor: None,
    };
    let resp = api.list_sessions(&project_id, &pagination).await.unwrap();

    // cursor=None eager: 前 EAGER_FIRST_PAGE_LIMIT 条同步真值 inline
    assert_eq!(resp.items.len(), 3);
    for s in &resp.items {
        assert!(
            s.title.as_deref().is_some_and(|t| !t.is_empty()),
            "eager path title SHALL inline real value, got {:?}",
            s.title
        );
        assert_eq!(s.message_count, 2, "eager path SHALL inline message_count");
        assert!(s.is_ongoing, "eager path SHALL inline is_ongoing");
        assert_eq!(s.project_id, project_id);
    }

    // 验证 eager 路径 300ms 内零 broadcast emit
    let recv = timeout(Duration::from_millis(300), rx.recv()).await;
    assert!(
        recv.is_err(),
        "eager cursor=None SHALL NOT emit broadcast for items already inlined; got {recv:?}"
    );
}

#[tokio::test]
async fn cursor_some_paged_returns_skeleton_and_emits_metadata_updates() {
    use cdt_api::DataApi;

    let titles = vec![
        "重构 auth 模块",
        "修复 sidebar bug",
        "性能优化探索",
        "旧会话",
    ];
    let (api, _tmp, project_id, session_ids) = build_api_with_fixtures(&titles).await;

    let mut rx = api.subscribe_session_metadata();

    // 先 cursor=None 抢占 eager（pageSize=2，eager 内联前 2 条，零 emit）
    let first = api
        .list_sessions(
            &project_id,
            &PaginatedRequest {
                page_size: 2,
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(first.items.len(), 2);
    let next_cursor = first
        .next_cursor
        .expect("first page SHALL have next_cursor");

    // cursor=Some 翻页：骨架 + 后台 scan + broadcast
    let resp = api
        .list_sessions(
            &project_id,
            &PaginatedRequest {
                page_size: 2,
                cursor: Some(next_cursor),
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.items.len(), 2);
    for s in &resp.items {
        assert!(
            s.title.is_none(),
            "cursor=Some skeleton title SHALL be None, got {:?}",
            s.title
        );
        assert_eq!(s.message_count, 0);
        assert!(!s.is_ongoing);
        assert!(session_ids.contains(&s.session_id));
    }

    // 异步收齐当前页 update
    let mut received = std::collections::HashMap::new();
    while received.len() < resp.items.len() {
        let upd = timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("timed out waiting for SessionMetadataUpdate")
            .expect("recv");
        if resp.items.iter().any(|s| s.session_id == upd.session_id) {
            received.insert(upd.session_id.clone(), upd);
        }
    }
    for upd in received.values() {
        assert!(upd.title.as_deref().is_some_and(|t| !t.is_empty()));
        assert_eq!(upd.message_count, 2);
        assert!(upd.is_ongoing);
    }
}

#[tokio::test]
async fn repeated_cursor_none_lists_inline_truth_both_times() {
    // 之前是 fix `session-title-race` 回归：第一次 list 完后 cache 写入；
    // 第二次同 project 调用骨架阶段 inline。eager 路径下两次都直接同步等到
    // 真值（cache 命中或 miss 都 inline），且都零 broadcast emit。
    use cdt_api::DataApi;

    let titles = vec!["重构 auth 模块", "修复 sidebar bug", "性能优化探索"];
    let (api, _tmp, project_id, _session_ids) = build_api_with_fixtures(&titles).await;

    let mut rx = api.subscribe_session_metadata();
    let pagination = PaginatedRequest {
        page_size: 3,
        cursor: None,
    };

    let first = api.list_sessions(&project_id, &pagination).await.unwrap();
    for s in &first.items {
        assert!(s.title.as_deref().is_some_and(|t| !t.is_empty()));
        assert_eq!(s.message_count, 2);
    }

    let second = api.list_sessions(&project_id, &pagination).await.unwrap();
    for s in &second.items {
        assert!(
            s.title.as_deref().is_some_and(|t| !t.is_empty()),
            "second call SHALL inline cached title, got {:?}",
            s.title
        );
        assert_eq!(s.message_count, 2);
        assert!(s.is_ongoing);
    }

    // 两次累计 300ms 内零 emit
    let recv = timeout(Duration::from_millis(300), rx.recv()).await;
    assert!(
        recv.is_err(),
        "eager cursor=None repeated calls SHALL stay silent on broadcast; got {recv:?}"
    );
}

#[tokio::test]
async fn cursor_none_page_size_over_20_eager_first_20_plus_remainder_scan() {
    // 1.4：pageSize > EAGER_FIRST_PAGE_LIMIT 时前 20 条 eager inline、剩余条走
    // 骨架 + spawn `scan_metadata_for_page` + broadcast。
    use cdt_api::DataApi;

    let titles: Vec<String> = (0..25).map(|i| format!("session-{i:02}")).collect();
    let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let (api, _tmp, project_id, _session_ids) = build_api_with_fixtures(&title_refs).await;

    let mut rx = api.subscribe_session_metadata();
    let pagination = PaginatedRequest {
        page_size: 25,
        cursor: None,
    };
    let resp = api.list_sessions(&project_id, &pagination).await.unwrap();
    assert_eq!(resp.items.len(), 25);

    let mut eager_inline_count = 0usize;
    let mut placeholder_count = 0usize;
    for s in &resp.items {
        if s.title.is_some() {
            eager_inline_count += 1;
        } else {
            placeholder_count += 1;
        }
    }
    assert_eq!(eager_inline_count, 20, "前 20 条 SHALL eager inline 真值");
    assert_eq!(
        placeholder_count, 5,
        "剩余 5 条 SHALL 保留骨架占位等 broadcast"
    );

    // remainder 5 条 SHALL 经 broadcast emit
    let mut received = std::collections::HashSet::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while received.len() < 5 {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }
        match timeout(deadline - now, rx.recv()).await {
            Ok(Ok(upd)) => {
                received.insert(upd.session_id);
            }
            Ok(Err(_)) | Err(_) => break,
        }
    }
    assert_eq!(
        received.len(),
        5,
        "remainder 5 条 SHALL 各 emit 一条 broadcast，实际 {}",
        received.len()
    );
}

#[tokio::test]
async fn corrupt_session_returns_placeholder_no_positive_cache_and_deferred_retry() {
    // D6b + 1.3：首页 eager 单条解析失败 → 响应该条占位（title=None）+ 正向
    // MetadataCache 不含该 sessionId entry（D6b 不写 cache）+ receiver 收到
    // deferred retry emit（spawn_deferred_metadata_retry 走 bypass_negative=true，
    // 仍解析失败也会经 broadcast 推送占位状态？实际：spawn_deferred 内部判
    // is_placeholder 不 emit，仅成功才 emit）。本测试聚焦 D6b 的 cache 不
    // 污染 + eager 占位回填。
    use cdt_api::DataApi;

    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();
    let project_dir = projects_base.join("-tmp-corrupt");
    std::fs::create_dir_all(&project_dir).unwrap();

    write_fixture_session(&project_dir, "good-01", "正常 session").await;
    write_corrupt_session(&project_dir, "bad-01").await;

    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base);
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.unwrap();
    let mut notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    notif_mgr.load().await.unwrap();
    let ssh_mgr = SshConnectionManager::new();
    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);

    let _rx = api.subscribe_session_metadata();
    let resp = api
        .list_sessions(
            "-tmp-corrupt",
            &PaginatedRequest {
                page_size: 20,
                cursor: None,
            },
        )
        .await
        .unwrap();

    let bad = resp
        .items
        .iter()
        .find(|s| s.session_id == "bad-01")
        .expect("bad-01 should appear");
    assert!(
        bad.title.is_none(),
        "corrupt session SHALL keep placeholder title"
    );
    assert_eq!(bad.message_count, 0);
    assert!(!bad.is_ongoing);

    let good = resp
        .items
        .iter()
        .find(|s| s.session_id == "good-01")
        .expect("good-01 should appear");
    assert!(good.title.as_deref().is_some_and(|t| !t.is_empty()));

    // D6c: 给 deferred retry 一些时间运行（500ms 延迟 + 解析时间）
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // D6b: 正向 cache 仅含 good，不含 bad
    let positive = api.metadata_cache_len_for_tests();
    assert_eq!(
        positive, 1,
        "正向 cache SHALL 仅含 good entry，实际 {positive}"
    );
    // D6c: negative cache 含 bad（写入 + retry 仍失败重写）
    let negative = api.metadata_cache_negative_len_for_tests();
    assert!(
        negative >= 1,
        "negative cache SHALL 含 bad entry（D6c），实际 {negative}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cross_project_active_scans_aborted_on_eager_project_switch() {
    // D4b: 多 project 翻页扫描进行中，调 list_sessions(projectB, cursor=None)
    // 进入 eager 时遍历 active_scans 按 projectId 解析 abort 所有非 projectB
    // 的 entry。验证 active_scans 中切到 projectB 之后只剩 projectB 自己的 key。
    //
    // 备注：小 fixture + 高并发下后台 scan task 几乎瞬时完成，**先有 projectA scan
    // 后被 abort** 与 **projectA scan 自然结束已 cleanup** 在 active_scans 黑盒视角
    // 上一致。所以测试主张的硬不变量是 post-condition（切换后 active_scans 中
    // 不含非 projectB key），precondition 用 best-effort 多次重发让窗口更宽。
    use cdt_api::DataApi;

    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    // projectA / projectC：每个 100 条让翻页扫描 spawn 真起来
    for pid in ["-tmp-projA", "-tmp-projC"] {
        let pdir = projects_base.join(pid);
        std::fs::create_dir_all(&pdir).unwrap();
        for i in 0..100 {
            write_fixture_session(&pdir, &format!("sess-{i:03}"), &format!("{pid}-{i}")).await;
        }
    }
    let pb_dir = projects_base.join("-tmp-projB");
    std::fs::create_dir_all(&pb_dir).unwrap();
    write_fixture_session(&pb_dir, "b-01", "projB only").await;

    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base);
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.unwrap();
    let mut notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    notif_mgr.load().await.unwrap();
    let ssh_mgr = SshConnectionManager::new();
    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);

    // 用 cursor=Some 多次连发让 projectA / projectC 的扫描在 active_scans 里堆叠
    for cursor in ["0", "50"] {
        for pid in ["-tmp-projA", "-tmp-projC"] {
            let _ = api
                .list_sessions(
                    pid,
                    &PaginatedRequest {
                        page_size: 50,
                        cursor: Some(cursor.to_owned()),
                    },
                )
                .await
                .unwrap();
        }
    }

    // 切到 projectB cursor=None → eager 路径开始时 abort 所有非 projectB 的 entry
    let _ = api
        .list_sessions(
            "-tmp-projB",
            &PaginatedRequest {
                page_size: 1,
                cursor: None,
            },
        )
        .await
        .unwrap();

    // 立即检查 active_scans——任何残存的非 projectB key 都意味着 D4b 没生效
    let after = api.active_scan_keys_for_tests();
    for key in &after {
        let pid = key.split('|').next().unwrap();
        assert_eq!(
            pid, "-tmp-projB",
            "after switch active_scans SHALL only contain projectB keys, got {after:?}"
        );
    }
}

#[tokio::test]
async fn d11_remainder_scan_dedupe_keeps_at_most_one_active_entry() {
    // D11: pageSize > 20 高频 silent refresh 时 remainder scan 同
    // `format!("{project_id}|None|remainder")` key 自然 dedupe——active_scans
    // 中始终至多 1 个 remainder entry。
    use cdt_api::DataApi;

    let titles: Vec<String> = (0..30).map(|i| format!("dedupe-{i:02}")).collect();
    let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let (api, _tmp, project_id, _) = build_api_with_fixtures(&title_refs).await;

    let pagination = PaginatedRequest {
        page_size: 30,
        cursor: None,
    };

    // 高频 silent refresh：连续 3 次 list_sessions
    for _ in 0..3 {
        let _ = api.list_sessions(&project_id, &pagination).await.unwrap();
    }

    // 检查 active_scans 中 remainder key 至多 1 个
    let keys = api.active_scan_keys_for_tests();
    let remainder_count = keys
        .iter()
        .filter(|k| k.ends_with("|None|remainder"))
        .count();
    assert!(
        remainder_count <= 1,
        "D11: remainder key SHALL dedupe to ≤ 1 entry, got {remainder_count} ({keys:?})"
    );
}

#[tokio::test]
async fn paged_cursor_different_pages_run_concurrently_without_abort() {
    // 回归 fix `session-list-per-cursor-abort`：cursor=Some(...) 翻页扫描间
    // 用 `(project_id, cursor)` 当 key，不同 cursor 互不 abort。
    use cdt_api::DataApi;

    let titles: Vec<String> = (0..24).map(|i| format!("page-{i:02}")).collect();
    let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let (api, _tmp, project_id, _session_ids) = build_api_with_fixtures(&title_refs).await;

    let mut rx = api.subscribe_session_metadata();

    // 抢一次 eager 把前 20 条 cache 起来
    let _ = api
        .list_sessions(
            &project_id,
            &PaginatedRequest {
                page_size: 20,
                cursor: None,
            },
        )
        .await
        .unwrap();
    while let Ok(Ok(_)) = timeout(Duration::from_millis(50), rx.recv()).await {}

    // 然后翻页 cursor=Some("20") page_size=2 拿剩 2 条 + cursor=Some("22") 拿剩 2 条
    let _ = api
        .list_sessions(
            &project_id,
            &PaginatedRequest {
                page_size: 2,
                cursor: Some("20".to_owned()),
            },
        )
        .await
        .unwrap();
    let _ = api
        .list_sessions(
            &project_id,
            &PaginatedRequest {
                page_size: 2,
                cursor: Some("22".to_owned()),
            },
        )
        .await
        .unwrap();

    // 两次翻页各 2 条独立扫描；emit 总数 SHALL ≥ 4（两页都跑完）
    let mut received = std::collections::HashSet::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while received.len() < 4 {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }
        match timeout(deadline - now, rx.recv()).await {
            Ok(Ok(upd)) => {
                received.insert(upd.session_id);
            }
            Ok(Err(_)) | Err(_) => break,
        }
    }
    assert_eq!(
        received.len(),
        4,
        "两页翻页各 2 条 cache miss SHALL 都 emit，实际 {}",
        received.len()
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
