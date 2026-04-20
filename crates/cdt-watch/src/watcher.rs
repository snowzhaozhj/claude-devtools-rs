//! 文件系统监听器——owns **file-watching** capability。
//!
//! 使用 `notify` 裸接原始事件，自己用 `tokio::time` 实现 100ms 去抖，
//! 通过 `tokio::sync::broadcast` 向所有订阅者分发事件。
//!
//! 自实现 debounce 的理由：`notify-debouncer-mini` 用系统时钟做 timer，
//! 测试无法用 `tokio::time::pause()` 控制，导致 burst 测试不可确定。
//! 本文件的 [`run_debounce_loop`] 只依赖 tokio mpsc + `tokio::time`，
//! 端到端 `tests/file_watching.rs` 只做烟雾测，时序语义在下方 `mod tests`
//! 用 `#[tokio::test(start_paused = true)]` 确定性覆盖。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{Instant, sleep_until};

use cdt_core::{FileChangeEvent, TodoChangeEvent};

use crate::error::WatchError;

const CHANNEL_CAPACITY: usize = 256;
const DEBOUNCE: Duration = Duration::from_millis(100);

/// 文件系统监听器，监听 projects 和 todos 目录变更。
pub struct FileWatcher {
    file_tx: broadcast::Sender<FileChangeEvent>,
    todo_tx: broadcast::Sender<TodoChangeEvent>,
    projects_dir: PathBuf,
    todos_dir: PathBuf,
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWatcher {
    /// 创建监听默认路径（`~/.claude/projects/` 和 `~/.claude/todos/`）的 watcher。
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let projects_dir = home.join(".claude").join("projects");
        let todos_dir = home.join(".claude").join("todos");
        Self::with_paths(projects_dir, todos_dir)
    }

    /// 创建监听自定义路径的 watcher（用于测试）。
    ///
    /// macOS 上 `/var` → `/private/var` 的 symlink 会导致 `notify` 返回的路径
    /// 与传入路径前缀不匹配，因此 canonicalize 消除歧义。Windows 上 `std` 的
    /// `canonicalize` 会返回 `\\?\C:\...` UNC 前缀，与 `notify` 回调给的普通路径
    /// 不匹配，`starts_with` 永远 false —— 改用 `dunce::canonicalize` 自动去掉
    /// 非 UNC 路径的 `\\?\` 前缀。macOS / Linux 行为与 `std` 一致。
    pub fn with_paths(projects_dir: PathBuf, todos_dir: PathBuf) -> Self {
        let (file_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (todo_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self {
            file_tx,
            todo_tx,
            projects_dir: dunce::canonicalize(&projects_dir).unwrap_or(projects_dir),
            todos_dir: dunce::canonicalize(&todos_dir).unwrap_or(todos_dir),
        }
    }

    /// 订阅文件变更事件。
    pub fn subscribe_files(&self) -> broadcast::Receiver<FileChangeEvent> {
        self.file_tx.subscribe()
    }

    /// 订阅 todo 变更事件。
    pub fn subscribe_todos(&self) -> broadcast::Receiver<TodoChangeEvent> {
        self.todo_tx.subscribe()
    }

    /// 启动监听，阻塞直到出错或被取消。
    ///
    /// 使用 `tokio::sync::mpsc` 桥接 `notify` 的同步回调与异步运行时，
    /// debounce 时序由 `run_debounce_loop` 承担（纯 tokio 时钟、可单元测）。
    pub async fn start(&self) -> Result<(), WatchError> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<RawEvent>();

        let mut watcher: RecommendedWatcher = notify::recommended_watcher(
            move |result: Result<Event, notify::Error>| match result {
                Ok(event) => {
                    let _ = tx.send(RawEvent::Notify(event));
                }
                Err(err) => {
                    tracing::warn!(error = %err, "transient filesystem error");
                }
            },
        )?;

        if self.projects_dir.is_dir() {
            watcher.watch(&self.projects_dir, RecursiveMode::Recursive)?;
        }
        if self.todos_dir.is_dir() {
            watcher.watch(&self.todos_dir, RecursiveMode::NonRecursive)?;
        }

        // 持有 watcher 防止被 drop
        let _watcher = watcher;

        run_debounce_loop(rx, DEBOUNCE, |path, deleted| {
            self.route_event(&path, deleted);
        })
        .await;

        Ok(())
    }

    /// 将单个去抖后的事件路由到对应的 broadcast channel。
    fn route_event(&self, path: &Path, deleted: bool) {
        if path.starts_with(&self.projects_dir) {
            if let Some(file_event) = self.parse_project_event(path, deleted) {
                let _ = self.file_tx.send(file_event);
            }
        } else if path.starts_with(&self.todos_dir) {
            if let Some(todo_event) = Self::parse_todo_event(path) {
                let _ = self.todo_tx.send(todo_event);
            }
        }
    }

    /// 从 projects 目录下的路径解析 `FileChangeEvent`。
    ///
    /// 路径格式：`<projects_dir>/<project_id>/<session_id>.jsonl`
    fn parse_project_event(&self, path: &Path, deleted: bool) -> Option<FileChangeEvent> {
        let ext = path.extension()?;
        if !ext.eq_ignore_ascii_case("jsonl") {
            return None;
        }

        let rel = path.strip_prefix(&self.projects_dir).ok()?;
        let components: Vec<_> = rel.components().collect();
        if components.len() < 2 {
            return None;
        }

        // 保留 OS 原生分隔符 —— project_id 作为 IPC 不透明 key，消费端按字符串
        // 相等匹配 `ProjectScanner` 输出的 encoded 目录名，不做文件系统拼接。
        let project_id = components[..components.len() - 1]
            .iter()
            .collect::<PathBuf>()
            .to_string_lossy()
            .into_owned();
        let session_id = path.file_stem()?.to_string_lossy().into_owned();

        Some(FileChangeEvent {
            project_id,
            session_id,
            deleted,
        })
    }

    /// 从 todos 目录下的路径解析 `TodoChangeEvent`。
    ///
    /// 路径格式：`<todos_dir>/<session_id>.json`
    fn parse_todo_event(path: &Path) -> Option<TodoChangeEvent> {
        let ext = path.extension()?;
        if !ext.eq_ignore_ascii_case("json") {
            return None;
        }

        let session_id = path.file_stem()?.to_string_lossy().into_owned();
        Some(TodoChangeEvent { session_id })
    }
}

/// 从 `notify` 回调发往异步侧的原始事件。
enum RawEvent {
    Notify(Event),
}

/// 独立 debounce 循环 —— 纯 tokio mpsc + `tokio::time`，不依赖 `notify`。
///
/// 输入：`raw_rx` 原始事件流、`debounce` 窗口、`sink` 路由回调。
/// 每条 path 最后一次事件之后静默 `debounce` 毫秒就调用 `sink(path, deleted)`，
/// 其中 `deleted = !path.exists()`（macOS `FSEvents` 对 remove 不保证发
/// `EventKind::Remove`，debounce 窗口结束时检查最可靠）。
///
/// 循环直到 `raw_rx` 关闭（sender 全 drop）后退出。测试可注入 mock rx + sink
/// 捕获产出，配合 `#[tokio::test(start_paused = true)]` + `tokio::time::advance`
/// 精确控制时序。
async fn run_debounce_loop<F>(
    mut raw_rx: mpsc::UnboundedReceiver<RawEvent>,
    debounce: Duration,
    mut sink: F,
) where
    F: FnMut(PathBuf, bool),
{
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    loop {
        let next_flush = pending.values().map(|t| *t + debounce).min();

        tokio::select! {
            event = raw_rx.recv() => {
                match event {
                    Some(RawEvent::Notify(ev)) => {
                        let now = Instant::now();
                        for path in ev.paths {
                            pending.insert(path, now);
                        }
                    }
                    None => break,
                }
            }
            () = async {
                match next_flush {
                    Some(deadline) => sleep_until(deadline).await,
                    None => std::future::pending::<()>().await,
                }
            } => {
                let now = Instant::now();
                let ready: Vec<_> = pending
                    .keys()
                    .filter(|p| pending.get(*p).is_some_and(|t| now >= *t + debounce))
                    .cloned()
                    .collect();
                for path in ready {
                    pending.remove(&path);
                    let deleted = !path.exists();
                    sink(path, deleted);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Debounce 单元测试 —— 确定性时序覆盖。
    //!
    //! 不经 `notify`，直接构造 `notify::Event` 喂 mpsc；配合
    //! `tokio::time::pause()`（`start_paused = true`）让 `sleep_until` 按 advance
    //! 推进。所有被测试的路径用 tempdir 真实文件，`deleted` 判定基于
    //! `Path::exists()`。
    //!
    //! 端到端 `FSEvents` 测试在 `tests/file_watching.rs`，只做烟雾测。

    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;
    use tokio::time::Duration;

    fn notify_event_for(path: &Path) -> RawEvent {
        RawEvent::Notify(Event::new(notify::EventKind::Any).add_path(path.to_path_buf()))
    }

    #[derive(Default, Clone)]
    struct Sink(Arc<Mutex<Vec<(PathBuf, bool)>>>);

    impl Sink {
        fn callback(&self) -> impl FnMut(PathBuf, bool) + use<> {
            let inner = self.0.clone();
            move |p, d| inner.lock().unwrap().push((p, d))
        }

        fn drain(&self) -> Vec<(PathBuf, bool)> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }

    /// 同一 path 在 debounce 窗口内收 N 次事件 → 窗口到期后恰好 1 次 flush。
    #[tokio::test(start_paused = true)]
    async fn burst_collapses_to_exactly_one_flush() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("burst.jsonl");
        std::fs::write(&path, b"").unwrap();

        let (tx, rx) = mpsc::unbounded_channel();
        let sink = Sink::default();
        let debounce = Duration::from_millis(100);
        let handle = tokio::spawn(run_debounce_loop(rx, debounce, sink.callback()));

        // 30ms 内发 5 次事件（间隔 6ms）
        for _ in 0..5 {
            tx.send(notify_event_for(&path)).unwrap();
            tokio::time::advance(Duration::from_millis(6)).await;
        }

        // 让 loop 处理完当前 recv
        tokio::task::yield_now().await;

        // 此时 debounce 窗口还没到：最后一次事件之后再 advance 100ms
        tokio::time::advance(Duration::from_millis(110)).await;
        tokio::task::yield_now().await;

        drop(tx);
        handle.await.unwrap();

        let events = sink.drain();
        assert_eq!(
            events.len(),
            1,
            "burst should collapse to one flush: {events:?}"
        );
        assert_eq!(events[0].0, path);
        assert!(!events[0].1, "file exists, deleted flag should be false");
    }

    /// Debounce 窗口到期后 path 不存在 → `deleted=true`。
    #[tokio::test(start_paused = true)]
    async fn flush_after_window_reports_deleted_when_path_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("ghost.jsonl");
        // 故意不创建文件

        let (tx, rx) = mpsc::unbounded_channel();
        let sink = Sink::default();
        let handle = tokio::spawn(run_debounce_loop(
            rx,
            Duration::from_millis(100),
            sink.callback(),
        ));

        tx.send(notify_event_for(&path)).unwrap();
        // yield_now 让 loop 先 poll recv、把 event 写入 pending，再 advance 触发 flush
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(150)).await;
        tokio::task::yield_now().await;

        drop(tx);
        handle.await.unwrap();

        let events = sink.drain();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, path);
        assert!(events[0].1, "missing file should be flagged deleted");
    }

    /// 两条不同 path 并发 burst —— 各自 debounce，各 flush 1 次。
    #[tokio::test(start_paused = true)]
    async fn two_paths_debounce_independently() {
        let tmp = TempDir::new().unwrap();
        let p1 = tmp.path().join("a.jsonl");
        let p2 = tmp.path().join("b.jsonl");
        std::fs::write(&p1, b"").unwrap();
        std::fs::write(&p2, b"").unwrap();

        let (tx, rx) = mpsc::unbounded_channel();
        let sink = Sink::default();
        let handle = tokio::spawn(run_debounce_loop(
            rx,
            Duration::from_millis(100),
            sink.callback(),
        ));

        for _ in 0..3 {
            tx.send(notify_event_for(&p1)).unwrap();
            tx.send(notify_event_for(&p2)).unwrap();
            tokio::time::advance(Duration::from_millis(10)).await;
        }

        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(150)).await;
        tokio::task::yield_now().await;

        drop(tx);
        handle.await.unwrap();

        let mut events = sink.drain();
        events.sort_by_key(|(p, _)| p.clone());
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, p1);
        assert_eq!(events[1].0, p2);
    }

    /// 窗口期间新事件续窗 —— 不应在最后一次事件前 flush。
    #[tokio::test(start_paused = true)]
    async fn new_event_within_window_extends_debounce() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("slow.jsonl");
        std::fs::write(&path, b"").unwrap();

        let (tx, rx) = mpsc::unbounded_channel();
        let sink = Sink::default();
        let handle = tokio::spawn(run_debounce_loop(
            rx,
            Duration::from_millis(100),
            sink.callback(),
        ));

        tx.send(notify_event_for(&path)).unwrap();
        tokio::task::yield_now().await; // 让 loop 写入 pending（t = 0）
        // 80ms 后再来一次（t = 80），应续窗
        tokio::time::advance(Duration::from_millis(80)).await;
        tx.send(notify_event_for(&path)).unwrap();
        tokio::task::yield_now().await; // 让 loop 写入 pending（覆盖 t = 80）

        // 从第 2 次算 30ms（t = 110），不应 flush（debounce = 100ms，还差 70ms）
        tokio::time::advance(Duration::from_millis(30)).await;
        tokio::task::yield_now().await;
        assert!(sink.drain().is_empty(), "window should still be open");

        // 再 advance 到第 2 次之后 >100ms（t = 190）
        tokio::time::advance(Duration::from_millis(80)).await;
        tokio::task::yield_now().await;

        drop(tx);
        handle.await.unwrap();

        let events = sink.drain();
        assert_eq!(events.len(), 1);
    }

    /// 通道关闭后 loop 立即退出。
    #[tokio::test(start_paused = true)]
    async fn loop_exits_when_sender_drops() {
        let (tx, rx) = mpsc::unbounded_channel();
        let sink = Sink::default();
        let handle = tokio::spawn(run_debounce_loop(
            rx,
            Duration::from_millis(100),
            sink.callback(),
        ));
        drop(tx);
        handle.await.unwrap();
        assert!(sink.drain().is_empty());
    }

    // --- parse_project_event / parse_todo_event / route_event 路由单元测 ---
    //
    // 这些原本依赖端到端 file_watching.rs 测试间接覆盖；现在直接喂 `Path` 单测，
    // 不依赖 `notify` 真实事件，行为完全确定（跨平台稳定）。

    /// 建真实 tempdir + canonicalize 后的 projects/todos 路径 + 对应 watcher。
    ///
    /// 必须用 `dunce::canonicalize` 拿到 watcher 内部一致的路径 —— macOS 上
    /// tempdir 的 `/var/...` 会被 canonicalize 成 `/private/var/...`，测试里
    /// 拼接的 path 必须以 canonicalize 后的前缀开头才能通过 `starts_with` 检查。
    fn setup_watcher_dirs() -> (TempDir, PathBuf, PathBuf, FileWatcher) {
        let tmp = TempDir::new().unwrap();
        let projects_raw = tmp.path().join("projects");
        let todos_raw = tmp.path().join("todos");
        std::fs::create_dir_all(&projects_raw).unwrap();
        std::fs::create_dir_all(&todos_raw).unwrap();
        let projects = dunce::canonicalize(&projects_raw).unwrap();
        let todos = dunce::canonicalize(&todos_raw).unwrap();
        let watcher = FileWatcher::with_paths(projects.clone(), todos.clone());
        (tmp, projects, todos, watcher)
    }

    #[test]
    fn parse_project_event_extracts_project_and_session_id() {
        let (_tmp, projects, _todos, watcher) = setup_watcher_dirs();
        let jsonl_path = projects.join("proj1").join("sess-abc.jsonl");
        let event = watcher
            .parse_project_event(&jsonl_path, false)
            .expect("should parse");
        assert_eq!(event.project_id, "proj1");
        assert_eq!(event.session_id, "sess-abc");
        assert!(!event.deleted);
    }

    #[test]
    fn parse_project_event_rejects_non_jsonl_extension() {
        let (_tmp, projects, _todos, watcher) = setup_watcher_dirs();
        let non_jsonl = projects.join("proj1").join("notes.txt");
        assert!(watcher.parse_project_event(&non_jsonl, false).is_none());
    }

    #[test]
    fn parse_project_event_rejects_path_outside_projects_dir() {
        let (tmp, _projects, _todos, watcher) = setup_watcher_dirs();
        let outside = tmp.path().join("elsewhere").join("s.jsonl");
        assert!(watcher.parse_project_event(&outside, false).is_none());
    }

    #[test]
    fn parse_project_event_requires_at_least_project_and_session() {
        let (_tmp, projects, _todos, watcher) = setup_watcher_dirs();
        // projects/bare.jsonl —— 只有 1 层（没有 project_id 目录），应拒绝
        let bare = projects.join("bare.jsonl");
        assert!(watcher.parse_project_event(&bare, false).is_none());
    }

    #[test]
    fn parse_project_event_preserves_deleted_flag() {
        let (_tmp, projects, _todos, watcher) = setup_watcher_dirs();
        let path = projects.join("proj").join("sess.jsonl");
        let event = watcher.parse_project_event(&path, true).unwrap();
        assert!(event.deleted);
    }

    #[test]
    fn parse_todo_event_extracts_session_id() {
        let path = Path::new("/tmp/todos/sess-todo-1.json");
        let event = FileWatcher::parse_todo_event(path).expect("should parse");
        assert_eq!(event.session_id, "sess-todo-1");
    }

    #[test]
    fn parse_todo_event_rejects_non_json_extension() {
        let path = Path::new("/tmp/todos/sess.jsonl");
        assert!(FileWatcher::parse_todo_event(path).is_none());
    }

    #[test]
    fn route_event_dispatches_to_correct_channel() {
        let (_tmp, projects, todos, watcher) = setup_watcher_dirs();

        let mut file_rx = watcher.subscribe_files();
        let mut todo_rx = watcher.subscribe_todos();

        let session_path = projects.join("proj1").join("sess-x.jsonl");
        watcher.route_event(&session_path, false);

        let todo_path = todos.join("sess-todo.json");
        watcher.route_event(&todo_path, false);

        let file_event = file_rx.try_recv().expect("should have file event");
        assert_eq!(file_event.project_id, "proj1");
        assert_eq!(file_event.session_id, "sess-x");

        let todo_event = todo_rx.try_recv().expect("should have todo event");
        assert_eq!(todo_event.session_id, "sess-todo");
    }

    #[test]
    fn route_event_drops_path_outside_both_dirs() {
        let (tmp, _projects, _todos, watcher) = setup_watcher_dirs();
        let mut file_rx = watcher.subscribe_files();
        let mut todo_rx = watcher.subscribe_todos();

        let orphan = tmp.path().join("other").join("x.jsonl");
        watcher.route_event(&orphan, false);

        assert!(file_rx.try_recv().is_err());
        assert!(todo_rx.try_recv().is_err());
    }
}
