//! 推送事件类型。

use serde::{Deserialize, Serialize};

/// 从后端推送到 UI 的事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PushEvent {
    /// Session 文件变更。
    FileChange {
        project_id: String,
        session_id: String,
    },
    /// Todo 文件变更。
    TodoChange {
        project_id: String,
        session_id: String,
    },
    /// 新通知。
    NewNotification { notification: serde_json::Value },
    /// SSH 连接状态变更。
    SshStatusChange { context_id: String, state: String },
}
