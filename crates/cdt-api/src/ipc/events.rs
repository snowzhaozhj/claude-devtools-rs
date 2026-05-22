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
        /// 该 session 所属 `RepositoryGroup.id`（前端按 groupId 过滤 SSE，
        /// change `simplify-repository-as-project::D7`）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        group_id: Option<String>,
    },
    /// Active context 切换（SSH connect / disconnect / `switch_context` /
    /// polling watcher 检测到 SFTP 死亡触发的自愈 disconnect）。
    ///
    /// 桌面 Tauri runtime 走 `app.emit("context_changed", ...)` 桥（详
    /// `src-tauri/src/lib.rs`），浏览器 `?http=1` 调试 / 远端 HTTP 客户端
    /// 走本 variant + `/api/events` SSE 路径。两路 payload 形态 **必须一致**：
    /// `active_context_id: Option<String>` + `kind: "local"|"ssh"`，对齐
    /// `cdt_ssh::ContextChanged`。前端 `transport.ts::normalizePushPayload`
    /// 转成 `{ activeContextId, kind }` 喂给 `contextStore` listener。
    ///
    /// 加这个 variant 修历史 bug：HTTP server 缺 `ContextChanged` 桥让浏览器
    /// `?http=1` 下 `contextStore.activeContextId` 在 SSH 切换后永远 stale，
    /// 表现为 SSH 状态指示符与实际后端 active context 不一致。
    ContextChanged {
        active_context_id: Option<String>,
        kind: String,
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
    /// 该 session 所属 `RepositoryGroup.id`。前端按
    /// `payload.groupId === selectedGroupId` 过滤 SSE event，避免切 group
    /// 后被旧 group event 误 patch（change
    /// `simplify-repository-as-project::D7`）。`list_repository_groups`
    /// 未跑过时为 None；前端 fallback 用 `projectId` 当 groupId。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}
