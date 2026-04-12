//! 文件系统监听器——owns **file-watching** capability。
//!
//! 使用 `notify-debouncer-mini` 实现 100ms 去抖，通过
//! `tokio::sync::broadcast` 向所有订阅者分发事件。

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, DebouncedEvent, new_debouncer};
use tokio::sync::broadcast;

use cdt_core::{FileChangeEvent, TodoChangeEvent};

use crate::error::WatchError;

const CHANNEL_CAPACITY: usize = 256;
const DEBOUNCE_MS: u64 = 100;

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
    pub fn with_paths(projects_dir: PathBuf, todos_dir: PathBuf) -> Self {
        let (file_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (todo_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self {
            file_tx,
            todo_tx,
            projects_dir,
            todos_dir,
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
    /// 使用 `tokio::sync::mpsc` 桥接 `notify` 的同步回调与异步运行时。
    pub async fn start(&self) -> Result<(), WatchError> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<EventBatch>();

        let mut debouncer = new_debouncer(
            Duration::from_millis(DEBOUNCE_MS),
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    let _ = tx.send(EventBatch::Events(events));
                }
                Err(err) => {
                    let _ = tx.send(EventBatch::Error(err));
                }
            },
        )?;

        if self.projects_dir.is_dir() {
            debouncer
                .watcher()
                .watch(&self.projects_dir, RecursiveMode::Recursive)?;
        }
        if self.todos_dir.is_dir() {
            debouncer
                .watcher()
                .watch(&self.todos_dir, RecursiveMode::NonRecursive)?;
        }

        // 持有 debouncer 防止被 drop
        let _debouncer = debouncer;

        while let Some(batch) = rx.recv().await {
            match batch {
                EventBatch::Events(events) => {
                    self.route_events(&events);
                }
                EventBatch::Error(err) => {
                    tracing::warn!(error = %err, "transient filesystem error");
                }
            }
        }

        Ok(())
    }

    /// 将去抖后的事件路由到对应的 broadcast channel。
    fn route_events(&self, events: &[DebouncedEvent]) {
        for event in events {
            let path = &event.path;

            if path.starts_with(&self.projects_dir) {
                if let Some(file_event) = self.parse_project_event(path) {
                    let _ = self.file_tx.send(file_event);
                }
            } else if path.starts_with(&self.todos_dir) {
                if let Some(todo_event) = Self::parse_todo_event(path) {
                    let _ = self.todo_tx.send(todo_event);
                }
            }
        }
    }

    /// 从 projects 目录下的路径解析 `FileChangeEvent`。
    ///
    /// 路径格式：`<projects_dir>/<project_id>/<session_id>.jsonl`
    fn parse_project_event(&self, path: &Path) -> Option<FileChangeEvent> {
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
        let deleted = !path.exists();

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

/// 从 notify 回调发往异步侧的批次。
enum EventBatch {
    Events(Vec<DebouncedEvent>),
    Error(notify_debouncer_mini::notify::Error),
}
