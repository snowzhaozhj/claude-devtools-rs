//! SSE 事件桥：把 backend `broadcast::Receiver` 转发为 `PushEvent` 发到
//! `AppState.events_tx`，再由 `sse_handler` 推给客户端。
//!
//! Spec：`openspec/specs/http-data-api/spec.md` §"Push events via Server-Sent
//! Events"。
//!
//! 三个 producer task 各自独立 loop：file-change / todo-change /
//! detected-error。`Lagged(_)` 跳过当条续 loop（事件 hint，下次同 session
//! 文件再变会重新触发；丢一两条不影响最终一致性，与 src-tauri host 桥
//! 模式一致），`Closed` 退出。`events_tx.send` 在无订阅者时返回 `Err`，
//! 静默忽略——SSE 客户端连接才订阅，无连接时事件本就 fire-and-forget。

use cdt_config::DetectedError;
use cdt_core::{FileChangeEvent, JobChangeEvent, TodoChangeEvent};
use cdt_ssh::{ContextChanged, ContextKind};
use tokio::sync::broadcast;

use crate::ipc::{PushEvent, SessionMetadataUpdate};

/// 启动 producer task：file / todo / detected-error / metadata / context-changed
/// → `PushEvent`。
///
/// 调用方持有 `events_tx`（与 `AppState.events_tx` 同 sender）；只要 sender
/// 存活，task 就持续运行。`*_rx` 关闭时对应 task 退出。task 句柄不返回——
/// 调用方用 sender drop 触发关闭即可（broadcast 语义）。
pub fn spawn_event_bridge(
    events_tx: broadcast::Sender<PushEvent>,
    file_rx: broadcast::Receiver<FileChangeEvent>,
    todo_rx: broadcast::Receiver<TodoChangeEvent>,
    error_rx: broadcast::Receiver<DetectedError>,
    metadata_rx: broadcast::Receiver<SessionMetadataUpdate>,
    context_rx: broadcast::Receiver<ContextChanged>,
    jobs_rx: broadcast::Receiver<JobChangeEvent>,
) {
    spawn_file_bridge(events_tx.clone(), file_rx);
    spawn_todo_bridge(events_tx.clone(), todo_rx);
    spawn_detected_error_bridge(events_tx.clone(), error_rx);
    spawn_metadata_bridge(events_tx.clone(), metadata_rx);
    spawn_context_changed_bridge(events_tx.clone(), context_rx);
    spawn_jobs_bridge(events_tx, jobs_rx);
}

fn spawn_jobs_bridge(
    events_tx: broadcast::Sender<PushEvent>,
    mut jobs_rx: broadcast::Receiver<JobChangeEvent>,
) {
    tokio::spawn(async move {
        loop {
            match jobs_rx.recv().await {
                Ok(event) => {
                    let _ = events_tx.send(PushEvent::JobsUpdate {
                        job_id: event.job_id,
                    });
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn spawn_file_bridge(
    events_tx: broadcast::Sender<PushEvent>,
    mut file_rx: broadcast::Receiver<FileChangeEvent>,
) {
    tokio::spawn(async move {
        loop {
            match file_rx.recv().await {
                Ok(event) => {
                    let _ = events_tx.send(PushEvent::FileChange {
                        project_id: event.project_id,
                        session_id: event.session_id,
                        deleted: event.deleted,
                        project_list_changed: event.project_list_changed,
                        session_list_changed: event.session_list_changed,
                        mtime_ms: event.mtime_ms,
                    });
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // `file_rx → events_tx` 一跳的 lag：原实现直接吞，下游 SSE
                    // 客户端永远拿不到信号，sidebar `totalSessions` 滞后到
                    // LOCAL_CACHE_TTL（5min）才被动恢复。改为显式 emit
                    // `PushEvent::SseLagged { source: "file-change", missed: n }`
                    // 让前端 silent refresh 兜底（change
                    // `enrich-file-change-with-session-list-changed::D6` 阻塞 3）。
                    let _ = events_tx.send(PushEvent::SseLagged {
                        source: "file-change".into(),
                        missed: n,
                    });
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn spawn_todo_bridge(
    events_tx: broadcast::Sender<PushEvent>,
    mut todo_rx: broadcast::Receiver<TodoChangeEvent>,
) {
    tokio::spawn(async move {
        loop {
            match todo_rx.recv().await {
                Ok(event) => {
                    // `TodoChangeEvent` 仅含 `session_id`；spec delta
                    // §"SSE client receives todo change" 约定 project_id 占位空字符串。
                    let _ = events_tx.send(PushEvent::TodoChange {
                        project_id: String::new(),
                        session_id: event.session_id,
                    });
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn spawn_detected_error_bridge(
    events_tx: broadcast::Sender<PushEvent>,
    mut error_rx: broadcast::Receiver<DetectedError>,
) {
    tokio::spawn(async move {
        loop {
            match error_rx.recv().await {
                Ok(err) => match serde_json::to_value(&err) {
                    Ok(notification) => {
                        let _ = events_tx.send(PushEvent::NewNotification { notification });
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "DetectedError serialize failed; skip SSE forward"
                        );
                    }
                },
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn spawn_metadata_bridge(
    events_tx: broadcast::Sender<PushEvent>,
    mut metadata_rx: broadcast::Receiver<SessionMetadataUpdate>,
) {
    tokio::spawn(async move {
        loop {
            match metadata_rx.recv().await {
                Ok(event) => {
                    let _ = events_tx.send(PushEvent::SessionMetadataUpdate {
                        project_id: event.project_id,
                        session_id: event.session_id,
                        title: event.title,
                        message_count: event.message_count,
                        is_ongoing: event.is_ongoing,
                        git_branch: event.git_branch,
                        group_id: event.group_id,
                    });
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// 桥 `cdt_ssh::ContextChanged` → `PushEvent::ContextChanged`。`kind` 序列化为
/// `"local"` / `"ssh"` 与桌面 Tauri 桥 `app.emit("context_changed", ...)`
/// payload 形态保持一致——浏览器 `?http=1` 调试与桌面端 listener 共用同一份
/// `contextStore` 状态机。
fn spawn_context_changed_bridge(
    events_tx: broadcast::Sender<PushEvent>,
    mut context_rx: broadcast::Receiver<ContextChanged>,
) {
    tokio::spawn(async move {
        loop {
            match context_rx.recv().await {
                Ok(event) => {
                    let kind = match event.kind {
                        ContextKind::Local => "local",
                        ContextKind::Ssh => "ssh",
                    };
                    let _ = events_tx.send(PushEvent::ContextChanged {
                        active_context_id: event.active_context_id,
                        kind: kind.to_owned(),
                    });
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
