//! 文件系统监听器——owns **file-watching** capability。
//!
//! 使用 `notify` 裸接原始事件，自己用 `tokio::time` 实现 100ms 去抖，
//! 通过 `tokio::sync::broadcast` 向所有订阅者分发事件。
//!
//! 自实现 debounce 的理由：`notify-debouncer-mini` 用系统时钟做 timer，
//! 测试无法用 `tokio::time::pause()` 控制，导致 burst 测试不可确定。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::broadcast;
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
    /// 与传入路径前缀不匹配，因此 canonicalize 消除歧义。
    pub fn with_paths(projects_dir: PathBuf, todos_dir: PathBuf) -> Self {
        let (file_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (todo_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self {
            file_tx,
            todo_tx,
            projects_dir: projects_dir.canonicalize().unwrap_or(projects_dir),
            todos_dir: todos_dir.canonicalize().unwrap_or(todos_dir),
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
    /// 自己用 `tokio::time` 实现 100ms debounce。
    pub async fn start(&self) -> Result<(), WatchError> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RawEvent>();

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

        // debounce 状态：path → 最后一次事件时间
        // deleted 在 flush 时通过 `!path.exists()` 判断，因为：
        // 1. macOS FSEvents 对 remove 不保证发 `EventKind::Remove`
        // 2. debounce 窗口结束时检查文件是否存在是最可靠的判断
        let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

        loop {
            let next_flush = pending.values().map(|t| *t + DEBOUNCE).min();

            tokio::select! {
                event = rx.recv() => {
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
                        .filter(|p| {
                            pending.get(*p).is_some_and(|t| now >= *t + DEBOUNCE)
                        })
                        .cloned()
                        .collect();
                    for path in ready {
                        pending.remove(&path);
                        let deleted = !path.exists();
                        self.route_event(&path, deleted);
                    }
                }
            }
        }

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

        let project_id = components[..components.len() - 1]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
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
