//! API 请求/响应类型。

use serde::{Deserialize, Serialize};

// =============================================================================
// 分页
// =============================================================================

/// 分页请求参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedRequest {
    pub page_size: usize,
    #[serde(default)]
    pub cursor: Option<String>,
}

/// 分页响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub total: usize,
}

// =============================================================================
// 项目
// =============================================================================

/// 项目信息（列表返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub id: String,
    pub path: String,
    pub display_name: String,
    pub session_count: usize,
}

// =============================================================================
// 会话
// =============================================================================

/// 会话摘要（列表返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_id: String,
    pub project_id: String,
    pub timestamp: i64,
    pub message_count: usize,
    /// 第一条用户消息（清洗后），用作 sidebar 标题。
    #[serde(default)]
    pub title: Option<String>,
    /// 会话是否仍在进行（最后一个 ending event 之后仍有 AI 活动）。
    ///
    /// 计算规则见 `cdt_analyze::check_messages_ongoing` 与
    /// `openspec/specs/sidebar-navigation/spec.md` §"Ongoing indicator
    /// on session item"。
    #[serde(default)]
    pub is_ongoing: bool,
    /// 会话最后一条消息所在的 git 分支（若 JSONL 行携带 `git_branch`）。
    /// 骨架阶段为 `None`，由后台 metadata scan 通过 `session-metadata-update`
    /// 异步 patch。详见 `openspec/specs/ipc-data-api/spec.md`
    /// §"Expose git branch on session summary and metadata updates"。
    #[serde(default)]
    pub git_branch: Option<String>,
    /// 当 session 属于某 `RepositoryGroup` 内的 worktree 时，记录 worktree 的
    /// project id（与 `Worktree.id == Project.id` 一致）。`list_sessions` /
    /// `list_sessions_sync` 路径**不**填（默认 None），仅 `get_worktree_sessions`
    /// 路径填，让前端按 worktree 过滤展示。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_id: Option<String>,
    /// 同 `worktree_id`，记录 worktree 的人类展示名（`Worktree.name`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_name: Option<String>,
}

/// 会话详情。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session_id: String,
    pub project_id: String,
    pub chunks: serde_json::Value,
    pub metrics: serde_json::Value,
    pub metadata: serde_json::Value,
    /// session 级别的 context injections（6 类结构化数据），
    /// 由 `process_session_context_with_phases` 计算。
    /// 当前等同于 `injections_by_phase[最大 phaseNumber]`（latest phase），
    /// 保留独立字段是为了让 `ContextPanel` 不切 phase 时直接消费、与旧前端兼容。
    #[serde(default)]
    pub context_injections: serde_json::Value,
    /// 每 phase 完整 accumulated injections，key = `phaseNumber.to_string()`。
    /// compact 后 Phase 1 的 injections 已 reset 不在 latest accumulated 内，
    /// 这里独立保留以供 Phase Selector 切到旧 phase 时显示。
    /// 见 spec `context-tracking` "Expose per-phase injections and phase metadata
    /// via `SessionDetail` IPC"。
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub injections_by_phase: serde_json::Value,
    /// session 级 phase 元数据（`ContextPhaseInfo` 序列化），含 `phases` /
    /// `ai_group_phase_map` / `compaction_token_deltas` / `compaction_count`。
    /// 前端 Phase Selector 按 `phases.length > 1` 决定显隐。
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub phase_info: serde_json::Value,
    /// 会话是否仍在进行。由 `cdt_analyze::check_messages_ongoing`
    /// 计算，值应与同 session 的 `SessionSummary.is_ongoing` 一致。
    #[serde(default)]
    pub is_ongoing: bool,
}

// =============================================================================
// 搜索
// =============================================================================

/// 搜索请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

// =============================================================================
// 配置
// =============================================================================

/// 配置更新请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdateRequest {
    pub section: String,
    pub data: serde_json::Value,
}

// =============================================================================
// SSH
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SshAuthMethod {
    #[default]
    SshConfig,
    Password,
}

/// SSH 连接请求。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectRequest {
    #[serde(alias = "hostAlias")]
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub auth_method: SshAuthMethod,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub context_id: Option<String>,
}

impl std::fmt::Debug for SshConnectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshConnectRequest")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("auth_method", &self.auth_method)
            .field(
                "password",
                &if self.password.is_some() {
                    "<redacted>"
                } else {
                    "<none>"
                },
            )
            .field("context_id", &self.context_id)
            .finish()
    }
}

impl From<SshAuthMethod> for cdt_ssh::AuthMethodKind {
    fn from(value: SshAuthMethod) -> Self {
        match value {
            SshAuthMethod::SshConfig => Self::SshConfig,
            SshAuthMethod::Password => Self::Password,
        }
    }
}

impl From<SshConnectRequest> for cdt_ssh::SshConnectRequest {
    fn from(value: SshConnectRequest) -> Self {
        Self {
            host: value.host,
            port: value.port,
            username: value.username,
            auth_method: value.auth_method.into(),
            password: value.password,
            context_id: value.context_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectionResult {
    pub context_id: String,
    pub status: cdt_ssh::SshStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth_chain: Vec<cdt_ssh::AuthAttempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshState {
    pub active_context_id: Option<String>,
    pub contexts: Vec<cdt_ssh::SshContextState>,
}

// =============================================================================
// Context
// =============================================================================

/// Context 信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextInfo {
    pub id: String,
    pub kind: String,
    pub is_active: bool,
    #[serde(default)]
    pub host: Option<String>,
}

// =============================================================================
// Sidebar 偏好（Pin/Hide）
// =============================================================================

/// 某个 project 当前持久化的 session pin/hide 列表。
///
/// 供前端 `sidebarStore` 首次加载某 project 时 prime 本地 `$state`。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSessionPrefs {
    /// 当前 project 被 pin 的 session id 列表（按 `pinnedAt` 倒序）。
    pub pinned: Vec<String>,
    /// 当前 project 被 hide 的 session id 列表（按 `hiddenAt` 倒序）。
    pub hidden: Vec<String>,
}

// =============================================================================
// Memory
// =============================================================================

/// 项目 memory 概览。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMemory {
    pub project_id: String,
    pub has_memory: bool,
    pub count: usize,
    pub default_file: Option<String>,
    pub layers: Vec<MemoryLayer>,
}

/// 单个 memory layer。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLayer {
    pub file: String,
    pub title: String,
    #[serde(default)]
    pub hook: Option<String>,
    pub kind: MemoryLayerKind,
}

/// Memory layer 来源类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLayerKind {
    Index,
    Entry,
    Orphan,
}

/// 单个 memory 文件内容。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryFileContent {
    pub project_id: String,
    pub file: String,
    pub file_path: String,
    pub content: String,
}
