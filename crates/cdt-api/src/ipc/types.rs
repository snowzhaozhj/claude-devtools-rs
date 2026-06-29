//! API 请求/响应类型。

use std::collections::{BTreeMap, HashMap};

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

/// k-way merge group-session 分页响应。**Server 无状态** —— cursor 自描述
/// 每个 worktree 的指针位置（base64 JSON），重启服务后仍可继续分页。
/// change `simplify-repository-as-project::D3`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSessionPage {
    pub sessions: Vec<SessionSummary>,
    pub next_cursor: Option<String>,
}

/// 单 worktree 在 group session 流中的指针状态。
///
/// tag 值用 `snake_case` 与其他面向 IPC 的 enum 一致（如
/// `cdt_core::message::MessageContent` / `cdt_api::ipc::events::*`）；字段
/// （如 `mtime_ms`）继续按 camelCase 暴露给前端。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum WorktreeOffset {
    /// 从该 worktree 的第一条 session 开始消费。
    NotStarted,
    /// 已消费到 `(mtime_ms, sid)` 这条；续页时找第一条**严格**在其之后的：
    /// `(s.mtime_ms < mtime_ms) || (s.mtime_ms == mtime_ms && s.sid > sid)`。
    AfterMtime { mtime_ms: i64, sid: String },
    /// 该 worktree 流已耗尽，k-way merge 跳过。前端也用此变体表达
    /// "worktree filter 排除该 worktree" 语义（D6 server-side filter）。
    Exhausted,
}

/// k-way merge cursor 内部表示——`perWorktree` key 是 `worktree.id`
/// （即 `project.id`），value 是该 worktree 的指针状态。
/// 序列化为 base64(JSON) 传 IPC。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GroupCursor {
    pub per_worktree: std::collections::BTreeMap<String, WorktreeOffset>,
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
    #[serde(default)]
    pub created: i64,
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
    /// / `list_group_sessions` 路径填，让前端按 worktree 过滤展示。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_id: Option<String>,
    /// 同 `worktree_id`，记录 worktree 的人类展示名（`Worktree.name`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_name: Option<String>,
    /// 该 session 所属 `RepositoryGroup.id`，让前端按 group 维度过滤 SSE
    /// event / cache key（design `simplify-repository-as-project::D7`）。
    /// `list_repository_groups` 未跑过时为 None，前端 fallback 到 `project_id`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    /// 该 session 所属 `Worktree.cwd_relative_to_repo_root`（如 `crates`、
    /// `.claude/worktrees/feat-x`）。repo 根本身或解析失败时为 None。
    /// 由 IPC handler 通过 `LocalDataApi::worktree_meta_cache` join 填入
    /// （change `simplify-repository-as-project::D2` scheme c），`cdt-core::Session`
    /// 不存此字段。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd_relative_to_repo_root: Option<String>,
    /// session jsonl 首条带 `cwd` 字段消息的 `cwd` 值；缺失时为 `None`。
    /// 由 `ProjectScanner` 在 head-read 阶段填充，让 sidebar 在同一 project 下
    /// 区分不同 cwd 的 session（典型场景：worktree / monorepo 子目录）。
    ///
    /// Spec：`ipc-data-api::Session 列表序列化暴露 cwd 字段`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
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

/// `SessionDetail.metrics` 字段 typed 形态。wire 内部字段保 `snake_case` 与
/// 历史 hand-built `json!({"message_count": N})` 逐字节一致（详 change
/// `typed-ipc-payload::design.md::D5` + `D7`：暂不修正 `camelCase` IPC 契约
/// 违规，留 followup issue 单独 PR）。
///
/// 命名加 `SessionDetail` 前缀避免与 `cdt_discover::SessionMetadata`（cache
/// 内部类型）撞名。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionDetailMetrics {
    pub message_count: usize,
}

/// `SessionDetail.metadata` 字段 typed 形态。三个字段全 nullable 反映
/// fs `metadata()` 失败 / jsonl `cwd` 字段缺失等真实 backend 行为。wire 内部
/// 字段保 `snake_case`（详 `SessionDetailMetrics` doc）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionDetailMetadata {
    pub last_modified: Option<i64>,
    pub size: Option<u64>,
    pub cwd: Option<String>,
}

/// 会话详情。
///
/// 本结构 6 个字段（`chunks` / `metrics` / `metadata` / `context_injections`
/// / `injections_by_phase` / `phase_info`）由 change `typed-ipc-payload` 从
/// `serde_json::Value` typed 化；wire JSON 形状保持与 typed 化前 byte-for-byte
/// 一致。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session_id: String,
    pub project_id: String,
    pub chunks: Vec<cdt_core::Chunk>,
    pub metrics: SessionDetailMetrics,
    pub metadata: SessionDetailMetadata,
    /// session 级别的 context injections（6 类结构化数据），
    /// 由 `process_session_context_with_phases` 计算。
    /// 当前等同于 `injections_by_phase[最大 phaseNumber]`（latest phase），
    /// 保留独立字段是为了让 `ContextPanel` 不切 phase 时直接消费、与旧前端兼容。
    #[serde(default)]
    pub context_injections: Vec<cdt_core::ContextInjection>,
    /// 每 phase 完整 accumulated injections，key = `phaseNumber.to_string()`。
    /// compact 后 Phase 1 的 injections 已 reset 不在 latest accumulated 内，
    /// 这里独立保留以供 Phase Selector 切到旧 phase 时显示。
    /// 见 spec `context-tracking` "Expose per-phase injections and phase metadata
    /// via `SessionDetail` IPC"。
    ///
    /// 用 `BTreeMap` 让 JSON object key 字典序稳定（详 change
    /// `typed-ipc-payload::design.md::D4`）；前端按 `Number(k)` 数值排序消费，
    /// 不依赖 wire 顺序。
    #[serde(default)]
    pub injections_by_phase: BTreeMap<String, Vec<cdt_core::ContextInjection>>,
    /// session 级 phase 元数据，含 `phases` / `ai_group_phase_map` /
    /// `compaction_token_deltas` / `compaction_count`。前端 Phase Selector
    /// 按 `phases.length > 1` 决定显隐。
    #[serde(default)]
    pub phase_info: cdt_core::ContextPhaseInfo,
    /// Per-turn context stats (sparse map: only turns with `new_count` > 0).
    /// Key = `AIChunk.chunk_id` (byte-for-byte).
    /// Used by frontend "Context +N" badge per AI turn header.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub turn_context_stats: HashMap<String, cdt_core::TurnContextStats>,
    /// 会话是否仍在进行。由 `cdt_analyze::check_messages_ongoing`
    /// 计算，值应与同 session 的 `SessionSummary.is_ongoing` 一致。
    #[serde(default)]
    pub is_ongoing: bool,
    /// 会话标题（清洗后），与同 sessionId 的 `SessionSummary.title` 共用单一派生
    /// 源 `extract_session_metadata_from_parsed`。前端 `SessionDetail.svelte` 顶
    /// `<h1>` SHALL 直接消费，`None` 时 fallback 到完整 `sessionId`。Spec：
    /// `ipc-data-api::SessionDetail 暴露与 SessionSummary 同源派生的 title`。
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workflow_items: Vec<cdt_core::WorkflowItem>,
}

/// `get_session_detail` IPC 返回值 —— tagged union。
///
/// - `Full`：文件有变化（或首次调用无 `known_fingerprint`），携带完整 payload + 新 fingerprint。
/// - `Unchanged`：文件签名与调用方持有的 `known_fingerprint` 一致，跳过 parse/build/serialize。
///   前端收到 `Unchanged` 应保留现有 `SessionDetail`，不动 store。
///
/// Wire 形态：`{ "status": "full"|"unchanged", "fingerprint": "...", "detail"?: {...} }`。
/// `detail` 仅 `full` 时存在。前端按 `status` 判分支。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum SessionDetailResponse {
    Full {
        fingerprint: String,
        detail: Box<SessionDetail>,
    },
    Unchanged {
        fingerprint: String,
    },
}

impl SessionDetailResponse {
    /// 对 `Full` variant 的 chunks 执行展示裁剪（omit image/response/tool/subagent）。
    /// `Unchanged` variant 无数据，调用为 no-op。
    pub fn apply_omissions(&mut self) {
        if let Self::Full { detail, .. } = self {
            super::local::apply_display_omissions(&mut detail.chunks);
        }
    }

    /// 导出专用裁剪：保留 tool output + response content，裁剪 image；subagent messages
    /// 不整体清空，改为三层封顶填充（递归渲染内部对话所需，见 `cap_subagent_messages`）。
    pub fn apply_export_omissions(&mut self) {
        if let Self::Full { detail, .. } = self {
            super::local::apply_export_omissions(&mut detail.chunks);
        }
    }
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
    #[serde(default)]
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
