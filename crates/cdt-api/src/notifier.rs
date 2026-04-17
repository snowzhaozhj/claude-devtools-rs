//! 自动通知管线：file-change → `detect_errors` → `NotificationManager::add_notification` → broadcast。
//!
//! 订阅 `cdt_watch::FileWatcher::subscribe_files()`，对每个 `FileChangeEvent`：
//! 1. 若 `deleted=true`，跳过
//! 2. 按 `~/.claude/projects/<project_id>/<session_id>.jsonl` 找文件
//! 3. 全量 `parse_file` → `detect_errors` → 逐条 `add_notification`
//! 4. 新条目（`add_notification` 返回 `Ok(true)`）通过 `error_tx` 广播
//!
//! 配合 `DetectedError` 的确定性 id + `NotificationManager` 的按 id 去重，
//! 重复扫描同一文件不会产生重复通知。

use std::path::PathBuf;
use std::sync::Arc;

use cdt_config::{ConfigManager, DetectedError, NotificationManager, detect_errors};
use cdt_core::FileChangeEvent;
use cdt_discover::path_decoder;
use cdt_parse::parse_file;
use tokio::sync::{Mutex, broadcast};

/// 自动通知管线。
pub struct NotificationPipeline {
    file_rx: broadcast::Receiver<FileChangeEvent>,
    config_mgr: Arc<Mutex<ConfigManager>>,
    notif_mgr: Arc<Mutex<NotificationManager>>,
    error_tx: broadcast::Sender<DetectedError>,
    /// `~/.claude/projects/` 的实际路径。显式参数化是为了测试可用 tmp 目录。
    projects_dir: PathBuf,
}

impl NotificationPipeline {
    pub fn new(
        file_rx: broadcast::Receiver<FileChangeEvent>,
        config_mgr: Arc<Mutex<ConfigManager>>,
        notif_mgr: Arc<Mutex<NotificationManager>>,
        error_tx: broadcast::Sender<DetectedError>,
        projects_dir: PathBuf,
    ) -> Self {
        Self {
            file_rx,
            config_mgr,
            notif_mgr,
            error_tx,
            projects_dir,
        }
    }

    /// 主循环：阻塞直到 `file_rx` 关闭或进程退出。
    ///
    /// `RecvError::Lagged(n)` 时记 warning 继续——丢的事件会在下次 file change 时
    /// 被全量 re-parse 覆盖，不会永久漏检。
    pub async fn run(mut self) {
        loop {
            match self.file_rx.recv().await {
                Ok(event) => {
                    self.process_file_change(&event).await;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(
                        lagged = n,
                        "notification pipeline lagged; subsequent events will re-scan affected sessions"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::info!("notification pipeline channel closed, stopping");
                    break;
                }
            }
        }
    }

    async fn process_file_change(&self, event: &FileChangeEvent) {
        if event.deleted {
            return;
        }

        let triggers = {
            let mgr = self.config_mgr.lock().await;
            mgr.get_enabled_triggers()
        };
        if triggers.is_empty() {
            return;
        }

        let base_dir = path_decoder::extract_base_dir(&event.project_id);
        let jsonl_path = self
            .projects_dir
            .join(base_dir)
            .join(format!("{}.jsonl", event.session_id));

        let messages = match parse_file(&jsonl_path).await {
            Ok(m) => m,
            Err(err) => {
                tracing::debug!(
                    path = %jsonl_path.display(),
                    error = %err,
                    "notifier skip: parse failed"
                );
                return;
            }
        };

        let file_path_str = jsonl_path.to_string_lossy().into_owned();
        let errors = detect_errors(
            &messages,
            &triggers,
            &event.session_id,
            &event.project_id,
            &file_path_str,
        );
        if errors.is_empty() {
            return;
        }

        let mut mgr = self.notif_mgr.lock().await;
        for err in errors {
            match mgr.add_notification(err.clone()).await {
                Ok(true) => {
                    let _ = self.error_tx.send(err);
                }
                Ok(false) => {
                    // duplicate, expected on re-scan
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        session_id = %event.session_id,
                        "notifier: add_notification failed"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_config::{NotificationTrigger, TriggerContentType, TriggerMode};
    use tempfile::tempdir;

    fn make_error_trigger() -> NotificationTrigger {
        NotificationTrigger {
            id: "t1".into(),
            name: "Error".into(),
            enabled: true,
            content_type: TriggerContentType::ToolResult,
            mode: TriggerMode::ErrorStatus,
            require_error: Some(true),
            is_builtin: None,
            tool_name: None,
            ignore_patterns: None,
            match_field: None,
            match_pattern: None,
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        }
    }

    async fn make_pipeline() -> (
        NotificationPipeline,
        broadcast::Sender<FileChangeEvent>,
        broadcast::Receiver<DetectedError>,
        Arc<Mutex<NotificationManager>>,
        Arc<Mutex<ConfigManager>>,
        tempfile::TempDir,
    ) {
        let tmp = tempdir().unwrap();
        let notif_path = tmp.path().join("notif.json");
        let config_path = tmp.path().join("config.json");

        let mut notif_mgr = NotificationManager::new(Some(notif_path));
        notif_mgr.load().await.unwrap();
        let notif_mgr = Arc::new(Mutex::new(notif_mgr));

        let mut config_mgr = ConfigManager::new(Some(config_path));
        config_mgr.load().await.unwrap();
        config_mgr.add_trigger(make_error_trigger()).await.unwrap();
        let config_mgr = Arc::new(Mutex::new(config_mgr));

        let (file_tx, file_rx) = broadcast::channel(16);
        let (error_tx, error_rx) = broadcast::channel(16);

        let projects_dir = tmp.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();

        let pipeline = NotificationPipeline::new(
            file_rx,
            config_mgr.clone(),
            notif_mgr.clone(),
            error_tx,
            projects_dir,
        );

        (pipeline, file_tx, error_rx, notif_mgr, config_mgr, tmp)
    }

    #[tokio::test]
    async fn notifier_skips_deleted_events() {
        let (pipeline, _file_tx, _error_rx, _notif_mgr, _config_mgr, _tmp) = make_pipeline().await;
        pipeline
            .process_file_change(&FileChangeEvent {
                project_id: "p1".into(),
                session_id: "s1".into(),
                deleted: true,
            })
            .await;
        // 仅断言不 panic 且不 I/O
    }

    #[tokio::test]
    async fn notifier_missing_file_is_silent() {
        // parse_file 读不到真实文件时，notifier 应记日志跳过，不 panic、不 send
        let (pipeline, _file_tx, mut error_rx, notif_mgr, _config_mgr, _tmp) =
            make_pipeline().await;
        pipeline
            .process_file_change(&FileChangeEvent {
                project_id: "does-not-exist".into(),
                session_id: "s-nope".into(),
                deleted: false,
            })
            .await;

        assert!(error_rx.try_recv().is_err());
        assert_eq!(notif_mgr.lock().await.get_notifications(10, 0).total, 0);
    }
}
