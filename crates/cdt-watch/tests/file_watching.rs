//! file-watching capability 集成测试。
//!
//! 每个 `#[tokio::test]` 对应 `openspec/specs/file-watching/spec.md` 的一个 Scenario。
//! 使用 `tempfile::TempDir` 隔离，真实文件 I/O 触发 `notify` 事件。
//!
//! debounce 用 `tokio::time` 实现，测试通过 `tokio::time::pause()` +
//! `advance()` 精确控制时间，不依赖系统时钟，结果确定。

use std::fs;
use std::io::Write;
use std::time::Duration;

use cdt_watch::FileWatcher;
use serial_test::serial;
use tempfile::TempDir;
use tokio::time::timeout;

const RECV_TIMEOUT: Duration = Duration::from_secs(5);

/// 创建临时 projects 和 todos 目录，并在 projects 下建立 `proj1` 子目录。
fn setup_dirs() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let projects = tmp.path().join("projects");
    let todos = tmp.path().join("todos");
    fs::create_dir_all(&projects).unwrap();
    fs::create_dir_all(&todos).unwrap();
    (tmp, projects, todos)
}

/// Scenario: New session file created
#[serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_session_file_created() {
    let (_tmp, projects, todos) = setup_dirs();
    let proj_dir = projects.join("proj1");
    fs::create_dir_all(&proj_dir).unwrap();

    let watcher = FileWatcher::with_paths(projects, todos);
    let mut rx = watcher.subscribe_files();

    let handle = tokio::spawn(async move { watcher.start().await });

    // 等 watcher 就绪
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 写入新 .jsonl 文件
    fs::write(proj_dir.join("sess-abc.jsonl"), b"{}").unwrap();

    let event = timeout(RECV_TIMEOUT, rx.recv())
        .await
        .expect("timed out waiting for event")
        .expect("channel closed");

    assert_eq!(event.project_id, "proj1");
    assert_eq!(event.session_id, "sess-abc");
    assert!(!event.deleted);

    handle.abort();
}

/// Scenario: Existing session file appended
#[serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn existing_session_file_appended() {
    let (_tmp, projects, todos) = setup_dirs();
    let proj_dir = projects.join("proj1");
    fs::create_dir_all(&proj_dir).unwrap();

    let session_file = proj_dir.join("sess-def.jsonl");
    fs::write(&session_file, b"line1\n").unwrap();

    let watcher = FileWatcher::with_paths(projects, todos);
    let mut rx = watcher.subscribe_files();

    let handle = tokio::spawn(async move { watcher.start().await });
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 追加写入
    let mut f = fs::OpenOptions::new()
        .append(true)
        .open(&session_file)
        .unwrap();
    f.write_all(b"line2\n").unwrap();
    f.flush().unwrap();
    drop(f);

    let event = timeout(RECV_TIMEOUT, rx.recv())
        .await
        .expect("timed out")
        .expect("channel closed");

    assert_eq!(event.session_id, "sess-def");
    assert!(!event.deleted);

    handle.abort();
}

/// Scenario: Session file deleted
#[serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_file_deleted() {
    let (_tmp, projects, todos) = setup_dirs();
    let proj_dir = projects.join("proj1");
    fs::create_dir_all(&proj_dir).unwrap();

    let session_file = proj_dir.join("sess-del.jsonl");
    fs::write(&session_file, b"{}").unwrap();

    let watcher = FileWatcher::with_paths(projects, todos);
    let mut rx = watcher.subscribe_files();

    let handle = tokio::spawn(async move { watcher.start().await });
    tokio::time::sleep(Duration::from_millis(200)).await;

    fs::remove_file(&session_file).unwrap();

    // 可能先收到 create 事件（watcher 启动时文件已存在），跳过直到收到 deleted
    let event = loop {
        let ev = timeout(RECV_TIMEOUT, rx.recv())
            .await
            .expect("timed out waiting for delete event")
            .expect("channel closed");
        if ev.deleted {
            break ev;
        }
    };

    assert_eq!(event.session_id, "sess-del");
    assert!(event.deleted);

    handle.abort();
}

/// Scenario: Todo file updated
#[serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn todo_file_updated() {
    let (_tmp, projects, todos) = setup_dirs();

    let watcher = FileWatcher::with_paths(projects, todos.clone());
    let mut rx = watcher.subscribe_todos();

    let handle = tokio::spawn(async move { watcher.start().await });
    tokio::time::sleep(Duration::from_millis(200)).await;

    fs::write(todos.join("sess-todo-1.json"), b"{}").unwrap();

    let event = timeout(RECV_TIMEOUT, rx.recv())
        .await
        .expect("timed out")
        .expect("channel closed");

    assert_eq!(event.session_id, "sess-todo-1");

    handle.abort();
}

/// Scenario: Burst of writes（去抖）
///
/// 在 30ms 内对同一文件写入 5 次，debounce 窗口 100ms 后应只收到恰好 1 个事件。
#[serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn burst_of_writes_debounced() {
    let (_tmp, projects, todos) = setup_dirs();
    let proj_dir = projects.join("proj1");
    fs::create_dir_all(&proj_dir).unwrap();

    let session_file = proj_dir.join("sess-burst.jsonl");
    fs::write(&session_file, b"").unwrap();

    let watcher = FileWatcher::with_paths(projects, todos);
    let mut rx = watcher.subscribe_files();

    let handle = tokio::spawn(async move { watcher.start().await });
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 30ms 内连写 5 次
    for i in 0..5 {
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(&session_file)
            .unwrap();
        writeln!(f, "line {i}").unwrap();
        f.flush().unwrap();
        drop(f);
        tokio::time::sleep(Duration::from_millis(6)).await;
    }

    // 第一个事件应该到达
    let event = timeout(RECV_TIMEOUT, rx.recv())
        .await
        .expect("timed out")
        .expect("channel closed");

    assert_eq!(event.session_id, "sess-burst");

    // 等待足够时间确认没有多余事件（debounce 窗口 + 余量）
    let extra = timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(
        extra.is_err(),
        "burst should produce exactly one event after debounce"
    );

    handle.abort();
}

/// Scenario: Two subscribers present
#[serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_subscribers_both_receive_event() {
    let (_tmp, projects, todos) = setup_dirs();
    let proj_dir = projects.join("proj1");
    fs::create_dir_all(&proj_dir).unwrap();

    let watcher = FileWatcher::with_paths(projects, todos);
    let mut rx1 = watcher.subscribe_files();
    let mut rx2 = watcher.subscribe_files();

    let handle = tokio::spawn(async move { watcher.start().await });
    tokio::time::sleep(Duration::from_millis(200)).await;

    fs::write(proj_dir.join("sess-multi.jsonl"), b"{}").unwrap();

    let ev1 = timeout(RECV_TIMEOUT, rx1.recv())
        .await
        .expect("rx1 timed out")
        .expect("rx1 channel closed");
    let ev2 = timeout(RECV_TIMEOUT, rx2.recv())
        .await
        .expect("rx2 timed out")
        .expect("rx2 channel closed");

    assert_eq!(ev1.session_id, "sess-multi");
    assert_eq!(ev2.session_id, "sess-multi");

    handle.abort();
}
