//! 通知持久化管理。
//!
//! 对应 TS `NotificationManager.ts`。
//! 存储到 `~/.claude/claude-devtools-notifications.json`，max 100 条。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::detected_error::DetectedError;
use crate::error::ConfigError;

const MAX_NOTIFICATIONS: usize = 100;

/// 存储的通知（`DetectedError` + read 状态）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredNotification {
    #[serde(flatten)]
    pub error: DetectedError,
    pub is_read: bool,
    pub created_at: i64,
}

/// 分页查询结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetNotificationsResult {
    pub notifications: Vec<StoredNotification>,
    pub total: usize,
    pub total_count: usize,
    pub unread_count: usize,
    pub has_more: bool,
}

/// 通知管理器。
pub struct NotificationManager {
    notifications: Vec<StoredNotification>,
    file_path: PathBuf,
}

impl NotificationManager {
    /// 创建管理器。
    pub fn new(file_path: Option<PathBuf>) -> Self {
        let path = file_path.unwrap_or_else(default_notifications_path);
        Self {
            notifications: Vec::new(),
            file_path: path,
        }
    }

    /// 从磁盘加载通知列表。
    pub async fn load(&mut self) -> Result<(), ConfigError> {
        match tokio::fs::read_to_string(&self.file_path).await {
            Ok(content) => {
                self.notifications = serde_json::from_str(&content).unwrap_or_default();
                self.prune();
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.notifications = Vec::new();
                Ok(())
            }
            Err(e) => Err(ConfigError::io(&self.file_path, e)),
        }
    }

    /// 保存到磁盘。
    async fn save(&self) -> Result<(), ConfigError> {
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ConfigError::io(parent, e))?;
        }
        let content = serde_json::to_string_pretty(&self.notifications)?;
        tokio::fs::write(&self.file_path, content)
            .await
            .map_err(|e| ConfigError::io(&self.file_path, e))
    }

    /// 添加通知，自动 prune + 保存。
    pub async fn add_notification(&mut self, error: DetectedError) -> Result<(), ConfigError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        self.notifications.push(StoredNotification {
            error,
            is_read: false,
            created_at: i64::try_from(now_ms).unwrap_or(i64::MAX),
        });

        self.prune();
        self.save().await
    }

    /// 分页获取通知。
    pub fn get_notifications(&self, limit: usize, offset: usize) -> GetNotificationsResult {
        let total = self.notifications.len();
        let unread_count = self.notifications.iter().filter(|n| !n.is_read).count();

        // 按 created_at 降序（最新在前）
        let mut sorted: Vec<&StoredNotification> = self.notifications.iter().collect();
        sorted.sort_by_key(|n| std::cmp::Reverse(n.created_at));

        let page: Vec<StoredNotification> = sorted
            .into_iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();

        let has_more = offset + page.len() < total;

        GetNotificationsResult {
            notifications: page,
            total,
            total_count: total,
            unread_count,
            has_more,
        }
    }

    /// 未读数。
    pub fn get_unread_count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.is_read).count()
    }

    /// 标记为已读。
    pub async fn mark_as_read(&mut self, notification_id: &str) -> Result<bool, ConfigError> {
        let found = self
            .notifications
            .iter_mut()
            .find(|n| n.error.id == notification_id);
        if let Some(n) = found {
            n.is_read = true;
            self.save().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 标记全部已读。
    pub async fn mark_all_as_read(&mut self) -> Result<(), ConfigError> {
        for n in &mut self.notifications {
            n.is_read = true;
        }
        self.save().await
    }

    /// 清除全部通知。
    pub async fn clear_all(&mut self) -> Result<(), ConfigError> {
        self.notifications.clear();
        self.save().await
    }

    /// Auto-prune 到 `MAX_NOTIFICATIONS`。
    fn prune(&mut self) {
        if self.notifications.len() > MAX_NOTIFICATIONS {
            // 按 created_at 升序排列，移除最老的
            self.notifications.sort_by_key(|n| n.created_at);
            let excess = self.notifications.len() - MAX_NOTIFICATIONS;
            self.notifications.drain(..excess);
        }
    }

    /// 获取文件路径。
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}

fn default_notifications_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("claude-devtools-notifications.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detected_error::DetectedErrorContext;
    use tempfile::tempdir;

    fn make_error(id: &str) -> DetectedError {
        DetectedError {
            id: id.into(),
            timestamp: 1000,
            session_id: "s1".into(),
            project_id: "p1".into(),
            file_path: "/tmp/f.jsonl".into(),
            source: "Bash".into(),
            message: "fail".into(),
            line_number: Some(1),
            tool_use_id: None,
            trigger_color: None,
            trigger_id: None,
            trigger_name: None,
            context: DetectedErrorContext {
                project_name: "test".into(),
                cwd: None,
            },
        }
    }

    #[tokio::test]
    async fn add_and_get_notifications() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notif.json");
        let mut mgr = NotificationManager::new(Some(path));

        mgr.add_notification(make_error("e1")).await.unwrap();
        mgr.add_notification(make_error("e2")).await.unwrap();

        let result = mgr.get_notifications(10, 0);
        assert_eq!(result.total, 2);
        assert_eq!(result.unread_count, 2);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn prune_limits_to_max() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notif.json");
        let mut mgr = NotificationManager::new(Some(path));

        for i in 0..110 {
            mgr.add_notification(make_error(&format!("e{i}")))
                .await
                .unwrap();
        }

        assert_eq!(mgr.get_notifications(200, 0).total, MAX_NOTIFICATIONS);
    }

    #[tokio::test]
    async fn mark_as_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notif.json");
        let mut mgr = NotificationManager::new(Some(path));

        mgr.add_notification(make_error("e1")).await.unwrap();
        assert_eq!(mgr.get_unread_count(), 1);

        mgr.mark_as_read("e1").await.unwrap();
        assert_eq!(mgr.get_unread_count(), 0);
    }

    #[tokio::test]
    async fn mark_all_as_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notif.json");
        let mut mgr = NotificationManager::new(Some(path));

        mgr.add_notification(make_error("e1")).await.unwrap();
        mgr.add_notification(make_error("e2")).await.unwrap();

        mgr.mark_all_as_read().await.unwrap();
        assert_eq!(mgr.get_unread_count(), 0);
    }

    #[tokio::test]
    async fn clear_all() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notif.json");
        let mut mgr = NotificationManager::new(Some(path));

        mgr.add_notification(make_error("e1")).await.unwrap();
        mgr.clear_all().await.unwrap();
        assert_eq!(mgr.get_notifications(10, 0).total, 0);
    }

    #[tokio::test]
    async fn persist_and_reload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notif.json");

        {
            let mut mgr = NotificationManager::new(Some(path.clone()));
            mgr.add_notification(make_error("e1")).await.unwrap();
        }

        let mut mgr2 = NotificationManager::new(Some(path));
        mgr2.load().await.unwrap();
        assert_eq!(mgr2.get_notifications(10, 0).total, 1);
    }

    #[tokio::test]
    async fn paging() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notif.json");
        let mut mgr = NotificationManager::new(Some(path));

        for i in 0..5 {
            mgr.add_notification(make_error(&format!("e{i}")))
                .await
                .unwrap();
        }

        let page1 = mgr.get_notifications(2, 0);
        assert_eq!(page1.notifications.len(), 2);
        assert!(page1.has_more);

        let page3 = mgr.get_notifications(2, 4);
        assert_eq!(page3.notifications.len(), 1);
        assert!(!page3.has_more);
    }
}
