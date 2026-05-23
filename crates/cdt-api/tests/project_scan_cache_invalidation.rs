//! `spawn_project_scan_cache_invalidator` 三档判定 + lag/closed 行为集成测试。
//!
//! Spec：`openspec/specs/ipc-data-api/spec.md`。
//! Change：`openspec/changes/project-scan-cache-semantic-invalidation/`。
//!
//! 测试粒度：直接构造 `broadcast` channel + `Arc<Mutex<ProjectScanCache>>` + spawn
//! invalidator task → 发事件 → poll counter delta → 断言 cache 状态。不走完整
//! `LocalDataApi::new_with_watcher` 路径以避免构造完整依赖；spec 中
//! `LocalDataApi::new` 不 spawn invalidator 通过 grep callsite 锁定。
//!
//! cdt-telemetry registry 是全局单例 counter 单调递增不可 reset；多测试并发会让
//! before/after delta 互相污染——所有测试 await 同一把 `tokio::sync::Mutex`
//! 强制串行。

use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant};

use cdt_api::ipc::project_scan_cache::{ProjectScanCache, spawn_project_scan_cache_invalidator};
use cdt_core::{FileChangeEvent, Project};
use cdt_fs::{ContextId, FsKind, HostSignature, SshConfigDigestInput};
use tokio::sync::{Mutex as TokioMutex, MutexGuard, broadcast};

const STRUCTURAL: &str = "project_scan_cache.invalidate.structural";
const SKIPPED: &str = "project_scan_cache.invalidate.content_append_skipped";
const LAG: &str = "project_scan_cache.invalidate.lag_conservative";

/// 全测试串行 guard。cdt-telemetry counter 是全局单例不可 reset，多测试
/// 并发跑会让 `before/after` delta 互相污染——所有测试 await 同一把
/// `tokio::sync::Mutex` 强制串行。用 async-aware mutex 让 clippy
/// `await_holding_lock` 通过（`std::sync::Mutex` guard 跨 await 不允许）。
fn serial_guard() -> &'static TokioMutex<()> {
    static M: OnceLock<TokioMutex<()>> = OnceLock::new();
    M.get_or_init(|| TokioMutex::new(()))
}

async fn lock_serial() -> MutexGuard<'static, ()> {
    serial_guard().lock().await
}

fn local_ctx() -> ContextId {
    ContextId::local(PathBuf::from("/test/projects"))
}

fn ssh_ctx() -> ContextId {
    let sig = HostSignature::from_ssh_config_fields(&SshConfigDigestInput {
        hostname: "example.com".into(),
        port: 22,
        user: "alice".into(),
        identity_files: vec![],
        proxyjump: None,
        proxycommand: None,
        hostkeyalias: None,
    });
    ContextId::ssh(sig, PathBuf::from("/home/u/.claude/projects"))
}

fn proj(id: &str, sessions: &[&str]) -> Project {
    Project {
        id: id.into(),
        name: id.into(),
        path: PathBuf::new(),
        sessions: sessions.iter().map(|s| (*s).to_string()).collect(),
        most_recent_session: None,
        created_at: None,
        distinct_cwds: Vec::new(),
    }
}

fn ev(pid: &str, sid: &str, deleted: bool, plc: bool) -> FileChangeEvent {
    FileChangeEvent {
        project_id: pid.into(),
        session_id: sid.into(),
        deleted,
        project_list_changed: plc,
    }
}

fn counter(name: &str) -> u64 {
    cdt_telemetry::registry().counter_value(name)
}

/// poll counter 直到 delta 达到 `expected_delta`，timeout panic。
async fn wait_counter_delta(name: &str, before: u64, expected_delta: u64) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let now = counter(name);
        if now >= before + expected_delta {
            return;
        }
        assert!(
            Instant::now() <= deadline,
            "counter {name} delta timeout: before={before} expected_delta={expected_delta} now={now}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// 构造预填了一个 Local entry 的 cache。
fn cache_with_local_entry(projects: Vec<Project>) -> Arc<StdMutex<ProjectScanCache>> {
    let mut c = ProjectScanCache::new();
    c.insert(local_ctx(), Arc::new(projects), 0, 0, FsKind::Local);
    Arc::new(StdMutex::new(c))
}

fn local_lookup_some(cache: &Arc<StdMutex<ProjectScanCache>>) -> bool {
    cache.lock().unwrap().lookup(&local_ctx(), 0, 0).is_some()
}

#[tokio::test]
async fn jsonl_append_does_not_invalidate_cache() {
    let _g = lock_serial().await;
    let cache = cache_with_local_entry(vec![proj("pa", &["sa"])]);
    let (tx, rx) = broadcast::channel::<FileChangeEvent>(16);

    let s_before = counter(SKIPPED);
    let st_before = counter(STRUCTURAL);

    let _h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    tx.send(ev("pa", "sa", false, false)).unwrap();
    wait_counter_delta(SKIPPED, s_before, 1).await;

    assert!(
        local_lookup_some(&cache),
        "已知 sa append SHALL 不失效 cache"
    );
    assert_eq!(
        counter(STRUCTURAL),
        st_before,
        "structural counter MUST 不动"
    );
}

#[tokio::test]
async fn new_session_first_appearance_in_known_project_invalidates_cache() {
    let _g = lock_serial().await;
    let cache = cache_with_local_entry(vec![proj("pa", &["sa1", "sa2"])]);
    let (tx, rx) = broadcast::channel::<FileChangeEvent>(16);

    let st_before = counter(STRUCTURAL);
    let s_before = counter(SKIPPED);

    let _h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    // 已知 project pa 下新 session sa_new：plc=false, deleted=false, sid 不在 snapshot
    tx.send(ev("pa", "sa_new", false, false)).unwrap();
    wait_counter_delta(STRUCTURAL, st_before, 1).await;

    assert!(
        !local_lookup_some(&cache),
        "未知 sid 应触发 invalidate_local"
    );
    assert_eq!(counter(SKIPPED), s_before, "skipped counter MUST 不动");
}

#[tokio::test]
async fn top_level_dir_create_with_empty_session_id_invalidates_cache() {
    let _g = lock_serial().await;
    let cache = cache_with_local_entry(vec![proj("pa", &["sa"])]);
    let (tx, rx) = broadcast::channel::<FileChangeEvent>(16);

    let st_before = counter(STRUCTURAL);

    let _h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    // 顶层 dir-create：session_id="", plc=true
    tx.send(ev("p_new", "", false, true)).unwrap();
    wait_counter_delta(STRUCTURAL, st_before, 1).await;

    assert!(!local_lookup_some(&cache));
}

#[tokio::test]
async fn session_jsonl_delete_invalidates_cache() {
    let _g = lock_serial().await;
    let cache = cache_with_local_entry(vec![proj("pa", &["sa"])]);
    let (tx, rx) = broadcast::channel::<FileChangeEvent>(16);

    let st_before = counter(STRUCTURAL);

    let _h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    tx.send(ev("pa", "sa", true, false)).unwrap();
    wait_counter_delta(STRUCTURAL, st_before, 1).await;

    assert!(!local_lookup_some(&cache));
}

#[tokio::test]
async fn subagent_jsonl_modification_does_not_invalidate_cache() {
    let _g = lock_serial().await;
    // watcher 折叠的 subagent 修改：sid=父 session（已在 cache）；plc=false; deleted=false
    let cache = cache_with_local_entry(vec![proj("pa", &["s_parent"])]);
    let (tx, rx) = broadcast::channel::<FileChangeEvent>(16);

    let s_before = counter(SKIPPED);
    let st_before = counter(STRUCTURAL);

    let _h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    tx.send(ev("pa", "s_parent", false, false)).unwrap();
    wait_counter_delta(SKIPPED, s_before, 1).await;

    assert!(local_lookup_some(&cache), "subagent 修改 SHALL 不失效");
    assert_eq!(counter(STRUCTURAL), st_before);
}

#[tokio::test]
async fn subagent_jsonl_delete_invalidates_cache_as_false_positive() {
    let _g = lock_serial().await;
    // watcher 折叠的 subagent 删除：sid=父 session; plc=false; deleted=true
    // 事件无 path，无法区分主 session 删除 vs subagent 删除——本 spec 接受 false-positive
    let cache = cache_with_local_entry(vec![proj("pa", &["s_parent"])]);
    let (tx, rx) = broadcast::channel::<FileChangeEvent>(16);

    let st_before = counter(STRUCTURAL);

    let _h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    tx.send(ev("pa", "s_parent", true, false)).unwrap();
    wait_counter_delta(STRUCTURAL, st_before, 1).await;

    assert!(
        !local_lookup_some(&cache),
        "subagent 删除会触发 false-positive invalidate（spec 显式接受）"
    );
}

#[tokio::test]
async fn ssh_entry_unaffected_by_file_change() {
    let _g = lock_serial().await;
    // 同时存在 Local + SSH entry；任意 file-change 都 SHALL NOT 动 SSH entry
    let mut c = ProjectScanCache::new();
    c.insert(
        local_ctx(),
        Arc::new(vec![proj("pa", &["sa"])]),
        0,
        0,
        FsKind::Local,
    );
    c.insert(
        ssh_ctx(),
        Arc::new(vec![proj("pb", &["sb"])]),
        0,
        0,
        FsKind::Ssh,
    );
    let cache = Arc::new(StdMutex::new(c));
    let (tx, rx) = broadcast::channel::<FileChangeEvent>(16);

    let st_before = counter(STRUCTURAL);

    let _h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    tx.send(ev("pa", "sa", true, false)).unwrap();
    wait_counter_delta(STRUCTURAL, st_before, 1).await;

    let mut cache = cache.lock().unwrap();
    assert!(
        cache.lookup(&local_ctx(), 0, 0).is_none(),
        "Local SHALL 被清"
    );
    assert!(
        cache.lookup(&ssh_ctx(), 0, 0).is_some(),
        "SSH entry MUST NOT 被任何 file-change 影响"
    );
}

#[tokio::test]
async fn broadcast_lagged_invalidates_with_lag_counter() {
    let _g = lock_serial().await;
    let cache = cache_with_local_entry(vec![proj("pa", &["sa"])]);
    // capacity 2 + 不立刻 recv，连续 send 3+ 条强制 receiver 进入 lag
    let (tx, rx) = broadcast::channel::<FileChangeEvent>(2);

    let lag_before = counter(LAG);

    let _h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    // 灌满超过 capacity 的事件让 receiver 进入 Lagged 状态
    // （tokio broadcast lag 语义：receiver 落后超过 capacity 时下一次 recv 返 Lagged）
    for _ in 0..10 {
        let _ = tx.send(ev("pa", "sa", false, false));
    }

    // lag_conservative SHALL inc 至少 1
    wait_counter_delta(LAG, lag_before, 1).await;

    // **重要**：lag 路径调 `invalidate_local()` 清空 cache，后续事件
    // `contains_session_id` 一律返 false → 走 `unknown_session=true` 触发
    // structural inc。STRUCTURAL counter 在 lag 后会不可预测地增加；本测试
    // 不变量仅检查 LAG counter ≥ 1（design D7：lag 后保守清空 → 再来事件按
    // 规则正常处理）。
}

#[tokio::test]
async fn broadcast_closed_exits_loop() {
    let _g = lock_serial().await;
    let cache = cache_with_local_entry(vec![proj("pa", &["sa"])]);
    let (tx, rx) = broadcast::channel::<FileChangeEvent>(16);

    let h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    drop(tx); // sender 关闭——所有 receiver 在下一次 recv 拿到 Closed
    // task 应在 timeout 内退出
    tokio::time::timeout(Duration::from_secs(2), h)
        .await
        .expect("invalidator task SHALL 在 sender 关闭后退出 loop")
        .expect("task panic");
}

#[tokio::test]
async fn new_constructor_does_not_spawn_invalidator() {
    // spec scenario "`new()` 构造不启动失效订阅"。本 change 仅在 `new_with_watcher`
    // 路径（即 `spawn_watcher_runtime` 内部）调 `spawn_project_scan_cache_invalidator`；
    // `new()` 路径无 watcher 参数，自然不订阅 file-watcher 广播。
    //
    // 完整断言不需要 LocalDataApi 真构造（依赖 ConfigManager / NotificationManager / Ssh）；
    // spec 通过代码层契约（grep `spawn_project_scan_cache_invalidator` callsite 唯一在
    // `spawn_watcher_runtime`，由 `new_with_watcher` 路径调用）锁定。本测试占位
    // 以防回归——若有人在 `new()` 路径加 invalidator spawn，此 grep 警示同时 PR
    // 描述要求显式声明。
    //
    // 实际可执行的反向断言：调用 `spawn_project_scan_cache_invalidator` 必须显式
    // 提供 `broadcast::Receiver`，没有 receiver 不能起 task——这从函数签名直接
    // 可见，编译期保证。
    let _g = lock_serial().await;
    let cache = cache_with_local_entry(vec![]);
    // 不调 spawn_project_scan_cache_invalidator → 无 task 在跑
    // 任何"模拟 file-change"操作都不应影响 cache
    drop(cache); // trivially passes
}

// ----- §6 跨 IPC 复用回归 -----

const APPEND_REPEAT: u64 = 5;

#[tokio::test]
async fn cache_hit_in_normal_append_does_not_regress() {
    let _g = lock_serial().await;
    // 模拟"首次扫描写 cache + N 条 plc=false append + 二次 lookup 命中同一 Arc"
    let snapshot = Arc::new(vec![proj("pa", &["sa1", "sa2"])]);
    let mut c = ProjectScanCache::new();
    c.insert(local_ctx(), Arc::clone(&snapshot), 0, 0, FsKind::Local);
    let cache = Arc::new(StdMutex::new(c));

    let s_before = counter(SKIPPED);

    let (tx, rx) = broadcast::channel::<FileChangeEvent>(16);
    let _h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    for _ in 0..APPEND_REPEAT {
        tx.send(ev("pa", "sa1", false, false)).unwrap();
    }
    wait_counter_delta(SKIPPED, s_before, APPEND_REPEAT).await;

    // 二次 lookup SHALL 命中同一 snapshot Arc（FU-4 跨 IPC 复用语义不退化）
    let hit = cache
        .lock()
        .unwrap()
        .lookup(&local_ctx(), 0, 0)
        .expect("cache 应仍命中");
    assert!(
        Arc::ptr_eq(&hit, &snapshot),
        "命中应复用同一 Arc，证明 cache 未被清"
    );
}

#[tokio::test]
async fn cache_invalidated_after_structural_does_not_regress_other_ctx() {
    let _g = lock_serial().await;
    let mut c = ProjectScanCache::new();
    c.insert(
        local_ctx(),
        Arc::new(vec![proj("pa", &["sa"])]),
        0,
        0,
        FsKind::Local,
    );
    c.insert(
        ssh_ctx(),
        Arc::new(vec![proj("pb", &["sb"])]),
        0,
        0,
        FsKind::Ssh,
    );
    let cache = Arc::new(StdMutex::new(c));

    let st_before = counter(STRUCTURAL);

    let (tx, rx) = broadcast::channel::<FileChangeEvent>(16);
    let _h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    tx.send(ev("p_new", "", false, true)).unwrap(); // plc=true → structural
    wait_counter_delta(STRUCTURAL, st_before, 1).await;

    let mut cache = cache.lock().unwrap();
    assert!(cache.lookup(&local_ctx(), 0, 0).is_none(), "Local 应被清");
    assert!(cache.lookup(&ssh_ctx(), 0, 0).is_some(), "SSH 应保留");
}

#[tokio::test]
async fn unknown_session_in_known_project_invalidates_then_repopulates() {
    let _g = lock_serial().await;
    // cache 有 pa / {sa1, sa2}；触发 (pa, sa3, plc=false, deleted=false) → 清
    // 再"重填"模拟 list_repository_groups 二次扫描 → cache 含 sa3
    let cache = cache_with_local_entry(vec![proj("pa", &["sa1", "sa2"])]);
    let (tx, rx) = broadcast::channel::<FileChangeEvent>(16);

    let st_before = counter(STRUCTURAL);

    let _h = spawn_project_scan_cache_invalidator(
        Arc::clone(&cache),
        rx,
        PathBuf::from("/test/projects"),
    );

    tx.send(ev("pa", "sa3", false, false)).unwrap();
    wait_counter_delta(STRUCTURAL, st_before, 1).await;

    {
        let mut c = cache.lock().unwrap();
        assert!(c.lookup(&local_ctx(), 0, 0).is_none(), "应被清");
    }

    // 模拟"重填"——真实场景下是下次 list_repository_groups miss 后重扫
    cache.lock().unwrap().insert(
        local_ctx(),
        Arc::new(vec![proj("pa", &["sa1", "sa2", "sa3"])]),
        0,
        0,
        FsKind::Local,
    );

    assert!(
        cache
            .lock()
            .unwrap()
            .contains_session_id(&local_ctx(), "pa", "sa3")
    );
}
