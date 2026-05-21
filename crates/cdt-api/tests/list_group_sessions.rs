//! Integration tests for `list_group_sessions` k-way merge pagination.
//!
//! 覆盖 spec：`openspec/specs/ipc-data-api/spec.md` §"Expose group session
//! listing via k-way merge pagination" 9 个 Scenario 的可在单 worktree group
//! 上验证子集：
//!
//! - 首页（`cursor=null`）返第一页 + `next_cursor`
//! - 续页（传入 `next_cursor`）拿剩余
//! - `next_cursor=null` 当 sessions 全部消费
//! - 同 mtime 下 sid 字典序稳序
//! - `pageSize=0` 拒绝（`ValidationError`）
//! - 损坏 base64 cursor fallback 首页（warn + 不报错）
//!
//! 多 worktree group + worktree filter Exhausted 的 Scenario 需要建真实
//! git common-dir + linked worktree fixture，留 followup。本测覆盖单
//! worktree（standalone project）group——`group.id == project.id`，已能覆盖
//! 上面 6 个核心契约行为。

use std::sync::Arc;

use cdt_api::{DataApi, LocalDataApi};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;

async fn write_fixture_session(
    dir: &std::path::Path,
    session_id: &str,
    title: &str,
    mtime_unix: i64,
) {
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
    f.write_all(user.as_bytes()).await.unwrap();
    f.write_all(b"\n").await.unwrap();
    f.flush().await.unwrap();
    drop(f);

    // 设置 mtime 让测试可控排序（filetime 必须显式写）。
    let ft = filetime::FileTime::from_unix_time(mtime_unix, 0);
    filetime::set_file_mtime(&path, ft).unwrap();
}

struct Harness {
    _tmp: TempDir,
    project_id: String,
    api: Arc<LocalDataApi>,
}

async fn build_harness(sessions: &[(&str, &str, i64)]) -> Harness {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    let project_dir_name = "-tmp-group-fixture";
    let project_dir = projects_base.join(project_dir_name);
    std::fs::create_dir_all(&project_dir).unwrap();

    for (sid, title, mtime) in sessions {
        write_fixture_session(&project_dir, sid, title, *mtime).await;
    }

    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base);
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.unwrap();
    let mut notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    notif_mgr.load().await.unwrap();
    let ssh_mgr = SshConnectionManager::new();
    let api = Arc::new(LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr));

    // 触发一次 list_repository_groups 让 worktree_meta_cache 落地。
    let groups = api.list_repository_groups().await.unwrap();
    let project_id = groups
        .iter()
        .find_map(|g| {
            g.worktrees
                .iter()
                .find(|w| w.id == project_dir_name)
                .map(|_| g.id.clone())
        })
        .expect("standalone group");

    Harness {
        _tmp: tmp,
        project_id,
        api,
    }
}

#[tokio::test]
async fn list_group_sessions_rejects_page_size_zero() {
    let h = build_harness(&[("s1", "first", 100)]).await;
    let err = h
        .api
        .list_group_sessions(&h.project_id, 0, None)
        .await
        .expect_err("pageSize=0 SHALL Err");
    assert!(
        format!("{err:?}").to_lowercase().contains("page"),
        "error message SHALL reference pageSize, got {err:?}"
    );
}

#[tokio::test]
async fn list_group_sessions_first_page_returns_sessions_and_next_cursor() {
    // 3 个 session，mtime 倒序：s3=300 / s2=200 / s1=100。
    let h = build_harness(&[
        ("s1", "first", 100),
        ("s2", "second", 200),
        ("s3", "third", 300),
    ])
    .await;

    let page = h
        .api
        .list_group_sessions(&h.project_id, 2, None)
        .await
        .unwrap();
    assert_eq!(
        page.sessions.len(),
        2,
        "first page returns pageSize sessions"
    );
    // mtime 倒序：s3 → s2
    assert_eq!(page.sessions[0].session_id, "s3");
    assert_eq!(page.sessions[1].session_id, "s2");
    assert!(
        page.next_cursor.is_some(),
        "next_cursor SHALL be Some when more sessions remain, got None"
    );
}

#[tokio::test]
async fn list_group_sessions_follow_up_cursor_returns_remaining_then_null() {
    let h = build_harness(&[
        ("s1", "first", 100),
        ("s2", "second", 200),
        ("s3", "third", 300),
    ])
    .await;

    let page1 = h
        .api
        .list_group_sessions(&h.project_id, 2, None)
        .await
        .unwrap();
    let cursor1 = page1.next_cursor.expect("first page next_cursor");

    let page2 = h
        .api
        .list_group_sessions(&h.project_id, 2, Some(&cursor1))
        .await
        .unwrap();
    assert_eq!(
        page2.sessions.len(),
        1,
        "second page contains the remaining session"
    );
    assert_eq!(page2.sessions[0].session_id, "s1");
    assert!(
        page2.next_cursor.is_none(),
        "next_cursor SHALL be None when all worktrees exhausted, got {:?}",
        page2.next_cursor
    );
}

#[tokio::test]
async fn list_group_sessions_same_mtime_breaks_tie_by_sid_ascending() {
    // 同 mtime 三个 session，sid 字典序：sa < sb < sc。
    // spec: "同 mtime sid 稳序"——max-heap 让 sid 小优先（sa 排前）。
    let h = build_harness(&[("sc", "c", 500), ("sa", "a", 500), ("sb", "b", 500)]).await;

    let page = h
        .api
        .list_group_sessions(&h.project_id, 5, None)
        .await
        .unwrap();
    assert_eq!(page.sessions.len(), 3);
    let sids: Vec<_> = page
        .sessions
        .iter()
        .map(|s| s.session_id.as_str())
        .collect();
    assert_eq!(
        sids,
        vec!["sa", "sb", "sc"],
        "同 mtime 时 SHALL 按 sid 字典序升序排列"
    );
}

#[tokio::test]
async fn list_group_sessions_corrupted_base64_cursor_falls_back_to_first_page() {
    let h = build_harness(&[("s1", "first", 100), ("s2", "second", 200)]).await;
    // 任意非 base64 字符（"!!!" 不是合法 base64 padding）→ decode fail
    let page = h
        .api
        .list_group_sessions(&h.project_id, 5, Some("!!!not-base64!!!"))
        .await
        .expect("corrupted cursor SHALL fallback to first page, not Err");
    assert_eq!(
        page.sessions.len(),
        2,
        "fallback returns first page entries"
    );
    assert_eq!(page.sessions[0].session_id, "s2"); // mtime desc
}

#[tokio::test]
async fn list_group_sessions_joins_worktree_meta_into_session_summary() {
    // spec ipc-data-api §"SessionSummary 增加 worktree 元信息字段"：
    // list_group_sessions 返回的 SessionSummary SHALL 含 worktreeId / groupId
    // 等 join 字段（apply_worktree_meta 写入）。
    let h = build_harness(&[("s1", "first", 100)]).await;
    let page = h
        .api
        .list_group_sessions(&h.project_id, 10, None)
        .await
        .unwrap();
    assert_eq!(page.sessions.len(), 1);
    let summary = &page.sessions[0];
    assert_eq!(
        summary.worktree_id.as_deref(),
        Some(summary.project_id.as_str()),
        "worktreeId SHALL 等于 worktree 的 project id"
    );
    assert!(
        summary.worktree_name.is_some(),
        "worktreeName SHALL 经 join 填入"
    );
    assert_eq!(
        summary.group_id.as_deref(),
        Some(h.project_id.as_str()),
        "standalone group 时 groupId SHALL 等于 worktree id（单 worktree group）"
    );
}

#[tokio::test]
async fn list_group_sessions_invalid_json_cursor_falls_back_to_first_page() {
    use base64::Engine;
    let h = build_harness(&[("s1", "first", 100)]).await;
    // base64-encoded garbage JSON (e.g. "not json") → json parse fail
    let bad_cursor = base64::engine::general_purpose::STANDARD.encode(b"not json");
    let page = h
        .api
        .list_group_sessions(&h.project_id, 5, Some(&bad_cursor))
        .await
        .expect("invalid JSON cursor SHALL fallback to first page, not Err");
    assert_eq!(page.sessions.len(), 1);
}
