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

/// 单个 session 元数据增量推送 payload。
///
/// 由 `LocalDataApi::list_sessions` 触发的后台扫描任务在每扫完一个 session
/// 文件后通过 `subscribe_session_metadata()` broadcast 发出。Tauri host
/// 桥接为 `session-metadata-update` 前端事件。
///
/// 详见 `openspec/specs/ipc-data-api/spec.md` §"Emit session metadata
/// updates" 与 `openspec/specs/sidebar-navigation/spec.md` §"会话元数据
/// 增量 patch"。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadataUpdate {
    pub project_id: String,
    pub session_id: String,
    pub title: Option<String>,
    pub message_count: usize,
    pub is_ongoing: bool,
}
