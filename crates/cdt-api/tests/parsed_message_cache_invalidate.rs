//! `LocalDataApi::new_with_watcher` 路径下 parsed-message cache 主动失效集成测试。
//!
//! 行为契约：`openspec/specs/ipc-data-api/spec.md` §"parsed-message 缓存按
//! file-change 广播主动失效"。
//!
//! 注：`get_tool_output` / `get_image_asset` hot path 当前用
//! `path_decoder::get_projects_base_path()` 推算 JSONL 路径——本测试用
//! `prime_parsed_msg_cache_for_test` helper 在 tempdir 路径下直接走 cache 写入，
//! 然后验证 watcher 广播触发的 invalidate task 是否剔除条目。

use std::sync::Arc;
use std::time::Duration;

use cdt_api::LocalDataApi;
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use cdt_watch::FileWatcher;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

async fn append_jsonl_line(path: &std::path::Path, line: &str) {
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

fn assistant_tool_use_line(uuid: &str, ts: &str, tool_id: &str, sid: &str) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"{uuid}","timestamp":"{ts}","sessionId":"{sid}","cwd":"/tmp","message":{{"role":"assistant","model":"claude-sonnet","content":[{{"type":"tool_use","id":"{tool_id}","name":"Bash","input":{{"command":"ls"}}}}]}}}}"#
    )
}

struct TestRig {
    api: Arc<LocalDataApi>,
    jsonl: std::path::PathBuf,
    session_id: String,
    _tmp: TempDir,
    _watcher: Arc<FileWatcher>,
}

async fn setup() -> TestRig {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    let todos_base = tmp.path().join("todos");
    std::fs::create_dir_all(&projects_base).unwrap();
    std::fs::create_dir_all(&todos_base).unwrap();

    let project_id = "-tmp-proj";
    let session_id = "sess-invalidate-1".to_owned();
    let session_dir = projects_base.join(project_id);
    std::fs::create_dir_all(&session_dir).unwrap();
    let jsonl = session_dir.join(format!("{session_id}.jsonl"));
    append_jsonl_line(
        &jsonl,
        &assistant_tool_use_line("u-a-1", "2026-05-16T10:00:00.000Z", "tu-1", &session_id),
    )
    .await;

    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base.clone());
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.unwrap();
    let mut notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    notif_mgr.load().await.unwrap();
    let ssh_mgr = SshConnectionManager::new();

    let watcher = Arc::new(FileWatcher::with_paths(
        projects_base.clone(),
        todos_base.clone(),
    ));
    let api = Arc::new(LocalDataApi::new_with_watcher(
        scanner,
        config_mgr,
        notif_mgr,
        ssh_mgr,
        watcher.as_ref(),
        projects_base.clone(),
    ));

    let watcher_for_task = watcher.clone();
    tokio::spawn(async move {
        let _ = watcher_for_task.start().await;
    });

    // 给 watcher 一点时间挂载 notify backend（debounce 100ms）
    tokio::time::sleep(Duration::from_millis(200)).await;

    TestRig {
        api,
        jsonl,
        session_id,
        _tmp: tmp,
        _watcher: watcher,
    }
}

async fn wait_for_cache_len(api: &LocalDataApi, expected: usize, deadline: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < deadline {
        if api.parsed_msg_cache_len() == expected {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

/// Scenario：file-change 广播 SHALL 剔除 cache 中对应 path 条目。
#[tokio::test]
async fn file_change_invalidates_parsed_message_cache_entry() {
    let rig = setup().await;

    // 1) 显式写入 cache（不走 hot path，避免 hot path 用 path_decoder 真实路径的限制）
    let _ = rig
        .api
        .prime_parsed_msg_cache_for_test(&rig.jsonl)
        .await
        .expect("prime 应成功");
    assert_eq!(
        rig.api.parsed_msg_cache_len(),
        1,
        "首次 prime 后 cache 应持有 1 条"
    );

    // 2) 追加新内容触发 watcher emit FileChangeEvent
    append_jsonl_line(
        &rig.jsonl,
        &assistant_tool_use_line("u-a-2", "2026-05-16T10:00:02.000Z", "tu-2", &rig.session_id),
    )
    .await;

    // 3) 等 invalidate task remove 条目
    let emptied = wait_for_cache_len(&rig.api, 0, Duration::from_secs(3)).await;
    assert!(
        emptied,
        "file-change 广播应在合理时间内剔除 cache 条目，当前 len={}",
        rig.api.parsed_msg_cache_len()
    );
}

/// Scenario：未发生 file-change 时 cache 条目不被剔除（持久命中）。
#[tokio::test]
async fn cache_persists_without_file_change() {
    let rig = setup().await;

    let _ = rig
        .api
        .prime_parsed_msg_cache_for_test(&rig.jsonl)
        .await
        .expect("prime 应成功");
    assert_eq!(rig.api.parsed_msg_cache_len(), 1);

    // 不触发任何文件变化；等 500ms 后 cache 条目应仍存在
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        rig.api.parsed_msg_cache_len(),
        1,
        "未发生 file-change 时 cache 条目 SHALL 保持"
    );
}

/// Scenario：file-change 后再次 prime 同 path → 重新写入 cache。
#[tokio::test]
async fn cache_can_be_re_primed_after_invalidate() {
    let rig = setup().await;

    let _ = rig
        .api
        .prime_parsed_msg_cache_for_test(&rig.jsonl)
        .await
        .expect("prime 1");
    assert_eq!(rig.api.parsed_msg_cache_len(), 1);

    append_jsonl_line(
        &rig.jsonl,
        &assistant_tool_use_line("u-a-2", "2026-05-16T10:00:02.000Z", "tu-2", &rig.session_id),
    )
    .await;

    let _ = timeout(
        Duration::from_secs(3),
        wait_for_cache_len(&rig.api, 0, Duration::from_secs(3)),
    )
    .await;
    assert_eq!(
        rig.api.parsed_msg_cache_len(),
        0,
        "invalidate 后 cache 应为空"
    );

    let _ = rig
        .api
        .prime_parsed_msg_cache_for_test(&rig.jsonl)
        .await
        .expect("prime 2");
    assert_eq!(
        rig.api.parsed_msg_cache_len(),
        1,
        "再次 prime 后 cache 应重新写入"
    );
}
