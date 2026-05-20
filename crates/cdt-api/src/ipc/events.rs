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
        deleted: bool,
        project_list_changed: bool,
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
    /// Session 元数据增量。
    SessionMetadataUpdate {
        project_id: String,
        session_id: String,
        title: Option<String>,
        message_count: usize,
        is_ongoing: bool,
        git_branch: Option<String>,
        /// 携带 backend emit-time 的 active context scope（`"local"` 或 SSH
        /// `context_id`）。`None` 表示 backend 未带该字段（回退路径）。
        /// 前端二次过滤理由见上方 `SessionMetadataUpdate` 字段 doc。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context_id: Option<String>,
    },
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
    /// 会话最后一条消息所在的 git 分支（若 JSONL 行携带 `git_branch`）。
    /// 详见 `openspec/specs/ipc-data-api/spec.md`
    /// §"Expose git branch on session summary and metadata updates"。
    #[serde(default)]
    pub git_branch: Option<String>,
    /// 该 update 所属的 context（`"local"` 或 SSH `context_id`）。
    ///
    /// 后端 `scan_metadata_for_page` 在 emit 前先 check `active_context_id()
    /// == expected_scope` 拦第一道；但 check 与 send 之间 context 仍可被
    /// disconnect/switch（TOCTOU）。前端 listener SHALL 以 `contextStore.
    /// activeContextId` 为真相源做二次过滤——不匹配就 ignore，避免旧 context
    /// 的 metadata patch 当前 sidebar（codex 二审 PR #178 V2 必须修 2）。
    ///
    /// `Option` 为兼容性兜底：本字段仅在新 backend emit 时携带，旧 backend
    /// 反序列化为 `None` 时前端 fallback 不做二次过滤（行为退化为 PR #178
    /// 第一轮：依赖 emit-time check 单一防线）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
}
