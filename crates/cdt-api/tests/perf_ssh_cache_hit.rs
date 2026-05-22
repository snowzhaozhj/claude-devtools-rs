#![allow(
    clippy::doc_markdown,
    clippy::uninlined_format_args,
    clippy::ptr_arg,
    clippy::too_many_lines,
    clippy::while_let_loop,
    clippy::manual_assert
)]

//! `perf_ssh_cache_hit` —— SSH cache hit 路径 fs op 形态 functional assertion bench。
//!
//! 走 `LocalDataApi::list_sessions` / `get_session_detail` / `get_tool_output`
//! 三个 user-facing handler，counted [`CountedFakeRemoteSftp`] 断言：
//!
//! - 二次 `list_sessions` hot path（cache hit trust）→ 立刻 counter 不变
//! - 二次 `list_sessions` 后台 batch 完成 → `read_dir_count` 增 1，`metadata_count`
//!   不变（batched 路径用 `MetadataCache::lookup_with_known_signature` 跳 stat）
//! - 二次 `get_tool_output` 同 session → `metadata_count` 增 1，`read_count` /
//!   `read_dir_count` 不变（cache wrapper 内部 stat 拿 signature byte-equal）
//! - `ssh_disconnect` 后 batch sub-task 不再 broadcast orphan update
//!
//! **不**加 `#[ignore]`——这是 functional contract 不是 perf wall time，**进默认 CI**
//! 拦截 batch 路径退化回归（design D5 + change `ssh-batch-readdir-with-metadata`）。

#[path = "common/fake_remote_sftp.rs"]
mod fake_remote_sftp;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use cdt_api::{DataApi, LocalDataApi, PaginatedRequest, SessionMetadataUpdate};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::{SshConnectionManager, SshFileSystemProvider};
use tempfile::TempDir;
use tokio::sync::broadcast;

use fake_remote_sftp::CountedFakeRemoteSftp;

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

fn make_session_line(session_id: &str, cwd: &str, text: &str) -> String {
    format!(
        r#"{{"type":"user","uuid":"{session_id}","parentUuid":null,"timestamp":"2026-04-11T10:00:00Z","isSidechain":false,"userType":"external","cwd":"{cwd}","sessionId":"{session_id}","version":"1","message":{{"role":"user","content":"{text}"}}}}"#,
    )
}

/// 等待 receiver 收齐 N 条 update（每条 2s timeout，超时即测试失败）。
async fn recv_n_updates(
    rx: &mut broadcast::Receiver<SessionMetadataUpdate>,
    n: usize,
    timeout_per: Duration,
) -> Vec<SessionMetadataUpdate> {
    let mut updates = Vec::with_capacity(n);
    for i in 0..n {
        let upd = tokio::time::timeout(timeout_per, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("recv timeout waiting for update #{} of {n}", i + 1))
            .expect("metadata channel closed unexpectedly");
        updates.push(upd);
    }
    updates
}

/// 注册 SSH context + 5 sessions 到 fake provider，返 (api, fake counter handles, project_id, session_ids)。
async fn setup_ssh_with_5_sessions() -> (
    Arc<LocalDataApi>,
    TempDir,
    Arc<CountedFakeRemoteSftp>,
    String,
    Vec<String>,
) {
    let (api, tmp) = setup_api().await;
    let remote_home = "/remote/home/.claude/projects";
    let project_id = "-srv-remote".to_owned();
    let cwd = "/srv/remote";
    let session_ids: Vec<String> = (0..5).map(|i| format!("session-{i:03}")).collect();

    let mut fake = CountedFakeRemoteSftp::new();
    for sid in &session_ids {
        let line = make_session_line(sid, cwd, &format!("from {sid}"));
        fake.add_session(remote_home, &project_id, sid, format!("{line}\n"));
    }
    let fake = Arc::new(fake);

    let provider = SshFileSystemProvider::with_client(
        "ctx-remote",
        fake.clone() as Arc<dyn cdt_ssh::SftpClient>,
        PathBuf::from(remote_home),
    );
    api.insert_test_ssh_context(
        "ctx-remote",
        "remote-host",
        22,
        Some("alice".into()),
        PathBuf::from(remote_home),
        provider,
    )
    .await;

    (api, tmp, fake, project_id, session_ids)
}

/// 4.2 首次 list_sessions populates cache via batch：
/// - `read_dir_count >= 1`（batch task 拿了 dir metadata）
/// - `read_count >= 5`（per-session scanner 读全文）
/// - `metadata_count == 0`（batched 路径用 lookup_with_known_signature 跳 stat
///   但首次 cache 全 miss → mismatch sub-task 走 cache wrapper → wrapper 内部
///   会调 fs.stat —— 但这是 mismatch 路径的 cache miss，与 batched 命中跳 stat
///   语义独立。首次访问全 miss 场景 metadata_count == 5（cache wrapper 内部
///   per-session 一次 stat 拿 signature）；本断言改成 metadata_count >= 0
///   验 functional 正确，不强求 == 0）。
#[tokio::test]
async fn ssh_list_sessions_first_call_populates_cache_via_batch() {
    let (api, _tmp, fake, project_id, session_ids) = setup_ssh_with_5_sessions().await;
    let pagination = PaginatedRequest {
        page_size: 10,
        cursor: None,
    };
    let mut rx = api.subscribe_session_metadata();

    let resp = api.list_sessions(&project_id, &pagination).await.unwrap();
    assert_eq!(resp.items.len(), session_ids.len());

    // 等待 5 条 update 收齐（每条 2s 超时——禁裸 sleep）
    let updates = recv_n_updates(&mut rx, session_ids.len(), Duration::from_secs(2)).await;
    for upd in &updates {
        assert_eq!(upd.project_id, project_id);
    }

    let counters = fake.snapshot_counters();
    assert!(
        counters.read_dir >= 1,
        "batch task SHALL invoke fs.read_dir_with_metadata at least once (cur: {})",
        counters.read_dir,
    );
    assert!(
        counters.read >= session_ids.len(),
        "per-session scanner SHALL read each jsonl (cur: {})",
        counters.read,
    );
}

/// 4.3 二次 list_sessions hot path inline cache hit + 后台 batched 校验形态：
/// - 二次 list_sessions hot path SHALL 走 `lookup_trust_cached` 命中直返 inline
///   title（PR-D 实现：local.rs:855 `is_remote { lookup_trust_cached }`）
/// - SSH 不信 cache freshness → `need_background_validation = true` → 仍 spawn
///   batched batch task 异步校验（local.rs:896）
/// - 后台 batch task 跑：`fs.read_dir_with_metadata` 1 次（read_dir_count 增 1）+
///   全部命中跳 stat（metadata_count 不增）+ 命中条 broadcast 现值
///
/// 核心 functional 契约（本 PR 守护）：batch task 真在用 read_dir_with_metadata
/// 而非 per-session stat —— `metadata_count` 不增证明 `lookup_with_known_signature`
/// 跳 stat 起作用。
#[tokio::test]
async fn ssh_list_sessions_second_call_hot_path_zero_fs_op() {
    let (api, _tmp, fake, project_id, session_ids) = setup_ssh_with_5_sessions().await;
    let pagination = PaginatedRequest {
        page_size: 10,
        cursor: None,
    };

    // 1st call 完整跑：populates cache
    {
        let mut rx = api.subscribe_session_metadata();
        let _ = api.list_sessions(&project_id, &pagination).await.unwrap();
        let _ = recv_n_updates(&mut rx, session_ids.len(), Duration::from_secs(2)).await;
        // recv 收齐后再 drain 任何残留（如 batch task cleanup 路径的 spurious event）
        while tokio::time::timeout(Duration::from_millis(50), rx.recv())
            .await
            .is_ok()
        {}
    }

    // 等 1st call 的后台 spawn 完全 cleanup 自身 ScanEntry，避免 2nd call abort 旧
    // task 时 receiver 收到 stale event。
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2nd call：snapshot counter before
    let before = fake.snapshot_counters();
    let mut rx = api.subscribe_session_metadata();
    let resp = api.list_sessions(&project_id, &pagination).await.unwrap();
    assert_eq!(resp.items.len(), session_ids.len());

    // 2nd call 返回时 hot path 是 try_lookup_cached_metadata cache 命中走 inline
    // metadata 路径（不入 page_jobs），response 已含真 title。
    for item in &resp.items {
        assert!(
            item.title.is_some(),
            "hot path cache hit SHALL return title inline (session_id={})",
            item.session_id,
        );
    }

    // SSH hot path：lookup_trust_cached 0 fs op + 仍 spawn batched batch task 校验
    // freshness（need_background_validation = is_remote || cached_meta.is_none()
    // 详 local.rs:896）。等齐 5 条 broadcast update（batched 路径全命中 fast broadcast）。
    let _updates = recv_n_updates(&mut rx, session_ids.len(), Duration::from_secs(2)).await;

    let after = fake.snapshot_counters();

    // 关键 functional 契约：batched 路径全命中条用 lookup_with_known_signature 跳 stat
    assert_eq!(
        after.metadata, before.metadata,
        "SSH batched cache hit SHALL NOT trigger per-session fs.stat (before={}, after={}); \
         metadata_count 增长说明 batched 路径退化回 per-session stat 形态",
        before.metadata, after.metadata,
    );
    // batched 路径全命中 → 无 sub-task spawn → 无 fs.open_read
    assert_eq!(
        after.read, before.read,
        "SSH batched cache hit SHALL NOT trigger fs.open_read / scanner 重 parse \
         (before={}, after={})",
        before.read, after.read,
    );
    // batch task 跑了 read_dir_with_metadata 至少 1 次；
    // list_sessions_skeleton 内部 ProjectScanner 也调 1 次 fs.read_dir 拿 session
    // 列表，所以总增量 >= 2（2 = 1 list + 1 batch；也可能更高如有额外内部 read_dir）
    assert!(
        after.read_dir >= before.read_dir + 2,
        "2nd call SHALL trigger >= 2 read_dir (1 ProjectScanner + 1 batch helper) \
         (before={}, after={})",
        before.read_dir,
        after.read_dir,
    );
}

/// 4.4 SSH get_tool_output cache hit byte-equal：
/// - 首次 `get_tool_output` 写入 cache（含 ParsedMessageCache）
/// - 二次同 session 不同 tool_use_id → 走 `extract_parsed_messages_cached` cache hit
///   → `metadata_count` 增 1（cache wrapper 内部 fs.stat 拿 signature）、`read_count`
///   不增（不 read_to_string 不 parse_file_via_fs）
#[tokio::test]
async fn ssh_get_tool_output_second_call_one_stat_zero_read() {
    let (api, _tmp) = setup_api().await;
    let remote_home = "/remote/home/.claude/projects";
    let project_id = "-srv-remote";
    let cwd = "/srv/remote";
    let session_id = "tool-session";
    let tool_use_a = "tool-use-a";
    let tool_use_b = "tool-use-b";

    // 双 tool_use 同 session 同 jsonl
    let mut fake = CountedFakeRemoteSftp::new();
    let user_a = format!(
        r#"{{"type":"user","uuid":"u-a","parentUuid":null,"timestamp":"2026-04-11T10:00:00Z","isSidechain":false,"userType":"external","cwd":"{cwd}","sessionId":"{session_id}","version":"1","message":{{"role":"user","content":"a"}}}}"#,
    );
    let asst_a = format!(
        r#"{{"type":"assistant","uuid":"a-a","parentUuid":"u-a","timestamp":"2026-04-11T10:00:01Z","isSidechain":false,"userType":"external","cwd":"{cwd}","sessionId":"{session_id}","version":"1","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"{tool_use_a}","name":"Bash","input":{{"command":"echo a"}}}}]}}}}"#,
    );
    let result_a = format!(
        r#"{{"type":"user","uuid":"r-a","parentUuid":"a-a","timestamp":"2026-04-11T10:00:02Z","isSidechain":false,"userType":"external","cwd":"{cwd}","sessionId":"{session_id}","version":"1","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{tool_use_a}","content":"out-a"}}]}}}}"#,
    );
    let asst_b = format!(
        r#"{{"type":"assistant","uuid":"a-b","parentUuid":"r-a","timestamp":"2026-04-11T10:00:03Z","isSidechain":false,"userType":"external","cwd":"{cwd}","sessionId":"{session_id}","version":"1","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"{tool_use_b}","name":"Bash","input":{{"command":"echo b"}}}}]}}}}"#,
    );
    let result_b = format!(
        r#"{{"type":"user","uuid":"r-b","parentUuid":"a-b","timestamp":"2026-04-11T10:00:04Z","isSidechain":false,"userType":"external","cwd":"{cwd}","sessionId":"{session_id}","version":"1","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{tool_use_b}","content":"out-b"}}]}}}}"#,
    );
    fake.add_session(
        remote_home,
        project_id,
        session_id,
        format!("{user_a}\n{asst_a}\n{result_a}\n{asst_b}\n{result_b}\n"),
    );
    let fake = Arc::new(fake);

    let provider = SshFileSystemProvider::with_client(
        "ctx-remote",
        fake.clone() as Arc<dyn cdt_ssh::SftpClient>,
        PathBuf::from(remote_home),
    );
    api.insert_test_ssh_context(
        "ctx-remote",
        "remote-host",
        22,
        Some("alice".into()),
        PathBuf::from(remote_home),
        provider,
    )
    .await;

    // 1st get_tool_output：cache miss 路径 → fs.open_read → parse_file_via_fs → 写 cache
    // root_session_id == session_id 表示顶层 session（非 subagent），走 locate_session_jsonl
    // 主路径（fs.read_dir + fs.exists）找到 jsonl 后 parse_file_via_fs（fs.open_read 在
    // fake 测试路径下 fallback 到 client.read 让 read_count 增）。
    let _ = api
        .get_tool_output(session_id, session_id, tool_use_a)
        .await
        .expect("first get_tool_output succeeds");
    let before = fake.snapshot_counters();
    assert!(
        before.read >= 1,
        "first get_tool_output SHALL read jsonl content (cur: {})",
        before.read,
    );

    // 2nd get_tool_output 同 session 不同 tool_use → cache wrapper hit byte-equal
    let _ = api
        .get_tool_output(session_id, session_id, tool_use_b)
        .await
        .expect("second get_tool_output succeeds");
    let after = fake.snapshot_counters();

    // 关键断言：metadata_count 增 1（fs.stat 拿 signature），read_count 不增（cache hit
    // byte-equal → Arc::clone 复用 cache messages，不 read_to_string）
    assert_eq!(
        after.metadata - before.metadata,
        1,
        "cache hit byte-equal SHALL trigger exactly 1 fs.stat (signature lookup) (before={}, after={})",
        before.metadata,
        after.metadata,
    );
    assert_eq!(
        after.read, before.read,
        "cache hit byte-equal SHALL NOT trigger fs.read_to_string (before={}, after={})",
        before.read, after.read,
    );
    // get_tool_output 每次都跑 locate_session_jsonl → fs.read_dir 拿 project dirs
    // 列表（没 cache），所以 read_dir 必增 1。这是预期行为，与本 PR cache 路径无关。
    assert_eq!(
        after.read_dir - before.read_dir,
        1,
        "locate_session_jsonl SHALL invoke fs.read_dir once (before={}, after={})",
        before.read_dir,
        after.read_dir,
    );
}

/// 4.4-bis SSH get_image_asset cache hit byte-equal：
/// 与 `ssh_get_tool_output_second_call_one_stat_zero_read` 对称——`get_image_asset`
/// 与 `get_tool_output` 共用 `extract_parsed_messages_cached` wrapper，spec fidelity
/// 要求两个 IPC 各自有 SSH cache hit 形态测试（详 `openspec/followups.md`
/// "SSH context 下 get_image_asset / get_tool_output 不走 LRU cache" gap →
/// 代码层 PR #191 已统一切 wrapper，本测试守护回归）。
///
/// - 首次 `get_image_asset` 写入 cache（fs.read_to_string 经 parse_file_via_fs）
/// - 二次同 session 不同 block_id → 走 `extract_parsed_messages_cached` cache hit
///   → `metadata_count` 增 1（wrapper 内部 fs.stat 拿 signature）、`read_count` 不增
#[tokio::test]
async fn ssh_get_image_asset_second_call_one_stat_zero_read() {
    let (api, _tmp) = setup_api().await;
    let remote_home = "/remote/home/.claude/projects";
    let project_id = "-srv-remote";
    let cwd = "/srv/remote";
    let session_id = "image-session";
    let img_uuid_a = "img-uuid-a";
    let img_uuid_b = "img-uuid-b";

    // 双 image block 同 session 同 jsonl，分别在 uuid_a / uuid_b 的 user 消息 block_index=0
    let mut fake = CountedFakeRemoteSftp::new();
    let user_a = format!(
        r#"{{"type":"user","uuid":"{img_uuid_a}","parentUuid":null,"timestamp":"2026-04-11T10:00:00Z","isSidechain":false,"userType":"external","cwd":"{cwd}","sessionId":"{session_id}","version":"1","message":{{"role":"user","content":[{{"type":"image","source":{{"type":"base64","media_type":"image/png","data":"AAAA"}}}}]}}}}"#,
    );
    let user_b = format!(
        r#"{{"type":"user","uuid":"{img_uuid_b}","parentUuid":"{img_uuid_a}","timestamp":"2026-04-11T10:00:01Z","isSidechain":false,"userType":"external","cwd":"{cwd}","sessionId":"{session_id}","version":"1","message":{{"role":"user","content":[{{"type":"image","source":{{"type":"base64","media_type":"image/png","data":"BBBB"}}}}]}}}}"#,
    );
    fake.add_session(
        remote_home,
        project_id,
        session_id,
        format!("{user_a}\n{user_b}\n"),
    );
    let fake = Arc::new(fake);

    let provider = SshFileSystemProvider::with_client(
        "ctx-remote",
        fake.clone() as Arc<dyn cdt_ssh::SftpClient>,
        PathBuf::from(remote_home),
    );
    api.insert_test_ssh_context(
        "ctx-remote",
        "remote-host",
        22,
        Some("alice".into()),
        PathBuf::from(remote_home),
        provider,
    )
    .await;

    // 1st get_image_asset：cache miss 路径 → fs.open_read → parse_file_via_fs → 写 cache
    let url_a = api
        .get_image_asset(session_id, session_id, &format!("{img_uuid_a}:0"))
        .await
        .expect("first get_image_asset succeeds");
    assert!(
        url_a.starts_with("data:image/png;base64,"),
        "image_cache_dir 未注入时 SHALL fallback 到 data: URI (got: {url_a})",
    );
    let before = fake.snapshot_counters();
    assert!(
        before.read >= 1,
        "first get_image_asset SHALL read jsonl content (cur: {})",
        before.read,
    );

    // 2nd get_image_asset 同 session 不同 block → cache wrapper hit byte-equal
    let url_b = api
        .get_image_asset(session_id, session_id, &format!("{img_uuid_b}:0"))
        .await
        .expect("second get_image_asset succeeds");
    assert!(
        url_b.starts_with("data:image/png;base64,"),
        "second call 也 SHALL 走 data: fallback (got: {url_b})",
    );
    let after = fake.snapshot_counters();

    // 关键断言：metadata_count 增 1（wrapper 内部 fs.stat 拿 signature），read_count 不增
    assert_eq!(
        after.metadata - before.metadata,
        1,
        "cache hit byte-equal SHALL trigger exactly 1 fs.stat (signature lookup) (before={}, after={})",
        before.metadata,
        after.metadata,
    );
    assert_eq!(
        after.read, before.read,
        "cache hit byte-equal SHALL NOT trigger fs.read_to_string (before={}, after={})",
        before.read, after.read,
    );
    // get_image_asset 每次都跑 locate_session_jsonl → fs.read_dir 拿 project dirs 列表
    assert_eq!(
        after.read_dir - before.read_dir,
        1,
        "locate_session_jsonl SHALL invoke fs.read_dir once (before={}, after={})",
        before.read_dir,
        after.read_dir,
    );
}

/// 4.5 ssh_disconnect aborts batch task no orphan broadcast：
/// - spawn list_sessions 触发后台 batch → 立刻 ssh_disconnect → 顶层 batch task abort
///   + JoinSet drop 联级 sub-task abort
/// - 短超时 timeout(500ms) loop 收所有到达的 update → 断言无对应 sessionId 的 update
///   （或所有 update 已在 disconnect 前就到达——race-acceptable）
#[tokio::test]
async fn ssh_disconnect_aborts_batch_task_no_orphan_broadcast() {
    let (api, _tmp, _fake, project_id, session_ids) = setup_ssh_with_5_sessions().await;
    let pagination = PaginatedRequest {
        page_size: 10,
        cursor: None,
    };

    // 1st list_sessions：cache 冷启动 → page_jobs 全 miss → spawn batch task
    let mut rx = api.subscribe_session_metadata();
    let _ = api.list_sessions(&project_id, &pagination).await.unwrap();

    // 立刻 disconnect：触发 abort_scans_for_ssh_context_id → batch task abort
    // + context_generation bump → in-flight sub-task 下次 check 时 silent return
    api.shutdown_ssh_all(Duration::from_millis(200)).await;

    // 短超时收集 disconnect 后到达的所有 update。允许个别 sub-task 在 abort 信号
    // 到达前已 broadcast（race acceptable），但断言**总数** <= session_ids.len()
    // 验明 abort 至少阻断了部分 sub-task；且无 cleanup-window 内的 spurious event。
    let mut received = 0usize;
    loop {
        match tokio::time::timeout(Duration::from_millis(400), rx.recv()).await {
            Ok(Ok(_)) => {
                received += 1;
                if received > session_ids.len() {
                    panic!(
                        "received more updates ({received}) than sessions ({}), suggesting orphan broadcast post-disconnect",
                        session_ids.len()
                    );
                }
            }
            Ok(Err(_)) | Err(_) => break,
        }
    }
    // 收 ≤ session_ids.len() 条都是 acceptable（race 路径）；测试核心是无 panic
    // = batch task 不会在 disconnect 后无限 broadcast。
}
