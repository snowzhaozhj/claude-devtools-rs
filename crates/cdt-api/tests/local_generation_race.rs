//! Integration tests for change `generation-race-audit`：覆盖
//! `openspec/specs/ipc-data-api/spec.md` §"映射缓存刷新约束" 与
//! "Expose group session listing via k-way merge pagination → `(groups, fs, ctx,
//! captured_generation)` 同源快照 / spawn 前 `(ctx + generation)` 二次校验" 行为契约。
//!
//! Race 1 / Race 2 的真并发触发依赖 fake SSH switch 的可控 delay 注入；本测试覆盖
//! **结构性 invariant**（非 timing-dependent）：
//!
//! - `list_repository_groups_refreshes_meta_cache_on_match_path`：常规路径 ctx +
//!   generation 双重校验 match → `refresh_worktree_meta_cache` 被调用一次（counter +1）
//! - `build_group_session_page_calls_active_fs_only_once`：D2 单一 snapshot 修法落地
//!   验证——整段调用 `active_fs_and_policy` 抽样次数 == 1（来自 inner，不再有第二次
//!   独立 `active_fs_and_context_strict`）
//! - `build_group_session_page_spawns_metadata_scan_on_match_path`：spawn 前锁内
//!   `(ctx + generation)` match 路径正常 spawn metadata scan task（确认 spawn 守卫
//!   没误把 match 路径也禁用）
//! - `concurrent_list_groups_does_not_panic`：N 轮并发 `list_repository_groups` 无
//!   panic，验证 `ssh_watcher_ops` 锁路径没死锁
//!
//! 真并发 race 触发测试（Test 1/2/3 in design.md D3）依赖 `cdt-ssh::FakeSshManager`
//! 的 delay injection 钩子，作为本 change follow-up 跟 cdt-ssh 一起改。

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
        "timestamp": "2026-05-22T10:00:00Z",
        "cwd": "/tmp/proj",
        "message": {"role": "user", "content": title}
    })
    .to_string();
    f.write_all(user.as_bytes()).await.unwrap();
    f.write_all(b"\n").await.unwrap();
    f.flush().await.unwrap();
    drop(f);
    let ft = filetime::FileTime::from_unix_time(mtime_unix, 0);
    filetime::set_file_mtime(&path, ft).unwrap();
}

struct Harness {
    _tmp: TempDir,
    api: Arc<LocalDataApi>,
}

async fn build_harness(sessions: &[(&str, &str, i64)]) -> Harness {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    let project_dir_name = "-tmp-gen-race-fixture";
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
    Harness { _tmp: tmp, api }
}

#[tokio::test]
async fn list_repository_groups_refreshes_meta_cache_on_match_path() {
    let h = build_harness(&[("s1", "first", 100), ("s2", "second", 200)]).await;
    assert_eq!(
        h.api.refresh_worktree_meta_cache_call_count(),
        0,
        "setup 后 refresh counter SHALL 为 0"
    );
    let groups = h.api.list_repository_groups().await.unwrap();
    assert!(
        !groups.is_empty(),
        "Local fixture SHALL 产出至少一个 RepositoryGroup"
    );
    assert_eq!(
        h.api.refresh_worktree_meta_cache_call_count(),
        1,
        "ctx + generation match 路径 SHALL 触发一次 refresh_worktree_meta_cache"
    );

    let _groups2 = h.api.list_repository_groups().await.unwrap();
    assert_eq!(
        h.api.refresh_worktree_meta_cache_call_count(),
        2,
        "再次调用 SHALL 再 +1（同 ctx + 同 generation 仍走 refresh 分支）"
    );
}

#[tokio::test]
async fn build_group_session_page_calls_active_fs_only_once() {
    let h = build_harness(&[("s1", "first", 100), ("s2", "second", 200)]).await;
    let groups = h.api.list_repository_groups().await.unwrap();
    let group_id = groups[0].id.clone();
    let baseline_count = h.api.active_fs_and_policy_call_count();
    let _page = h
        .api
        .list_group_sessions(&group_id, 50, None)
        .await
        .unwrap();
    let delta = h.api.active_fs_and_policy_call_count() - baseline_count;
    assert_eq!(
        delta, 1,
        "build_group_session_page 整段调用 SHALL 只触发 1 次 active_fs_and_policy（来自 inner，不再有第二次独立 active_fs_and_context_strict 拆分；实际 delta = {delta}）"
    );
}

#[tokio::test]
async fn build_group_session_page_spawns_metadata_scan_on_match_path() {
    let h = build_harness(&[("s1", "first", 100), ("s2", "second", 200)]).await;
    let groups = h.api.list_repository_groups().await.unwrap();
    let group_id = groups[0].id.clone();
    let baseline_spawn = h.api.metadata_scan_spawn_count();
    let page = h
        .api
        .list_group_sessions(&group_id, 50, None)
        .await
        .unwrap();
    assert_eq!(page.sessions.len(), 2, "page SHALL 含两条 session");
    let spawned = h.api.metadata_scan_spawn_count() - baseline_spawn;
    assert!(
        spawned >= 1,
        "ctx + generation match 路径 SHALL spawn 至少 1 个 metadata scan task（spawned: {spawned}）"
    );
}

#[tokio::test]
async fn concurrent_list_groups_does_not_panic() {
    let h = build_harness(&[("s1", "first", 100), ("s2", "second", 200)]).await;
    let api = h.api.clone();
    let mut handles = Vec::new();
    for _ in 0..16 {
        let api = api.clone();
        handles.push(tokio::spawn(async move {
            let _ = api.list_repository_groups().await;
        }));
    }
    for h in handles {
        h.await.expect("task SHALL NOT panic");
    }
    // 至少一次 refresh 落地（cache 一致性兜底，验证锁路径无死锁）
    assert!(
        h.api.refresh_worktree_meta_cache_call_count() >= 1,
        "并发 list_repository_groups SHALL 至少触发一次 refresh"
    );
}
