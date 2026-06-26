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
        /// 结构性变化提示：true 时表示 group 内 session 集合可能变化
        /// （新增 / 删除 / 重命名），前端 SHALL revalidate
        /// `list_repository_groups` 让 `RepositoryGroup.totalSessions` 同步；
        /// false（典型 JSONL append 场景）放行，不触发整张列表重拉。
        /// 字段由后端 `spawn_unified_cache_invalidator` 在三档判定后 enrich
        /// （change `enrich-file-change-with-session-list-changed::D3` /
        /// `D4`）。`#[serde(default)]` 兼容旧 fixture / 旧客户端：缺字段时反序列化
        /// 拿 `false`，行为退化为不触发 loadProjects。
        #[serde(default)]
        session_list_changed: bool,
        /// 事件涉及文件的 mtime（毫秒 since UNIX epoch）。watcher 在能取到 mtime
        /// 时填入；取不到（典型：SFTP server 不返 mtime / 删除事件 / IO 失败）
        /// 缺省字段。change `dashboard-mtime-overlay` 引入：让前端 / 后端
        /// `ProjectScanCache` mtime overlay 路径在不触发结构性 invalidate 的
        /// 普通 append 场景下也能拿到新鲜 mtime；缺字段时消费方退化到既有
        /// 行为（spec `push-events::file-change payload 形态` /
        /// `ipc-data-api::ProjectScanCache 维护 per-project mtime overlay`）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mtime_ms: Option<i64>,
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
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        user_intents: Vec<String>,
        #[serde(default)]
        last_active: i64,
        #[serde(default)]
        duration_ms: i64,
        #[serde(default)]
        total_cost: f64,
        #[serde(default)]
        tool_error_count: usize,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files_modified: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        git_summary: Vec<String>,
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
    /// broadcast 链路 lag 信号——`LocalDataApi.file_tx → events_tx` 或
    /// `events_tx → SSE` client 任一跳 `broadcast::error::RecvError::Lagged`
    /// 时由对应 bridge 显式 emit，让前端 silent refresh 兜底（change
    /// `enrich-file-change-with-session-list-changed::D6`）。
    ///
    /// 与既有 `crate::http::sse::SSE_LAGGED_SENTINEL` （`{"type":"sse_lagged"}`）
    /// **向后兼容**：旧 sentinel 不含 `source` / `missed`，前端 transport
    /// 按 `payload.source ?? ""` / `payload.missed ?? 0` 读 undefined 不报错；
    /// 新形态 `{"type":"sse_lagged","source":"file-change","missed":7}` 给
    /// 前端 telemetry 用。
    ///
    /// `tag = "type"` + 既有 `rename_all = snake_case` 已让 variant 序列化
    /// 为 `"sse_lagged"`；字段 `source` / `missed` 单词无下划线，`snake_case`
    /// 即字面。**禁止**给 enum 加 `rename_all_fields = camelCase`——会破坏
    /// 既有 `project_id` / `session_id` 等字段的 wire 形态。
    SseLagged { source: String, missed: u64 },
    /// Background job state.json 变更。前端收到后 re-fetch `list_jobs`。
    JobsUpdate { job_id: String },
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_intents: Vec<String>,
    #[serde(default)]
    pub last_active: i64,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub total_cost: f64,
    #[serde(default)]
    pub tool_error_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_modified: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub git_summary: Vec<String>,
}
