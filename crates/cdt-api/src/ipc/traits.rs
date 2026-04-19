//! `DataApi` trait 定义。
//!
//! Spec：`openspec/specs/ipc-data-api/spec.md`。
//!
//! trait-based facade——不绑定具体 IPC 传输。

use async_trait::async_trait;

use super::error::ApiError;
use super::types::{
    ConfigUpdateRequest, ContextInfo, PaginatedRequest, PaginatedResponse, ProjectInfo,
    SearchRequest, SessionDetail, SessionSummary, SshConnectRequest,
};

/// 数据 API 操作集。
///
/// 覆盖 spec 的 8 个 Requirement，按功能分组。
#[async_trait]
pub trait DataApi: Send + Sync {
    // =========================================================================
    // 项目 + 会话查询
    // =========================================================================

    /// 列出所有项目。
    async fn list_projects(&self) -> Result<Vec<ProjectInfo>, ApiError>;

    /// 分页列出项目的会话。
    ///
    /// IPC 路径下返回**骨架** `SessionSummary`（`title` / `messageCount` /
    /// `isOngoing` 为占位值），元数据通过 `subscribe_session_metadata()`
    /// 异步推送。HTTP 路径请改用 `list_sessions_sync`。
    async fn list_sessions(
        &self,
        project_id: &str,
        pagination: &PaginatedRequest,
    ) -> Result<PaginatedResponse<SessionSummary>, ApiError>;

    /// 同步完整返回 session 列表（含全部元数据）。HTTP API 专用——HTTP
    /// 无 push 通道，无法走骨架化路径。
    ///
    /// 默认实现 fallback 到 `list_sessions`（即返回骨架）；具体实现可
    /// override 为同步扫描（见 `LocalDataApi::list_sessions_sync`）。
    async fn list_sessions_sync(
        &self,
        project_id: &str,
        pagination: &PaginatedRequest,
    ) -> Result<PaginatedResponse<SessionSummary>, ApiError> {
        self.list_sessions(project_id, pagination).await
    }

    /// 获取会话详情（chunks + metrics + metadata）。
    async fn get_session_detail(
        &self,
        project_id: &str,
        session_id: &str,
    ) -> Result<SessionDetail, ApiError>;

    /// 批量获取会话。
    async fn get_sessions_by_ids(
        &self,
        session_ids: &[String],
    ) -> Result<Vec<SessionDetail>, ApiError>;

    /// 按需拉取一个 subagent 的完整 chunks 流。
    ///
    /// `get_session_detail` 返回的 `Process.messages` 默认裁剪为空（详见
    /// `openspec/specs/ipc-data-api/spec.md` `Lazy load subagent trace`
    /// requirement）；前端 `SubagentCard` 展开时调本方法按需获取。
    ///
    /// 默认实现返回空数组；`LocalDataApi` 提供真实读盘版本。
    async fn get_subagent_trace(
        &self,
        _root_session_id: &str,
        _subagent_session_id: &str,
    ) -> Result<serde_json::Value, ApiError> {
        Ok(serde_json::Value::Array(Vec::new()))
    }

    // =========================================================================
    // 搜索
    // =========================================================================

    /// 搜索（单会话/单项目/全局，由 request 字段控制范围）。
    async fn search(&self, request: &SearchRequest) -> Result<serde_json::Value, ApiError>;

    // =========================================================================
    // 配置 + 通知
    // =========================================================================

    /// 获取当前配置。
    async fn get_config(&self) -> Result<serde_json::Value, ApiError>;

    /// 更新配置。
    async fn update_config(
        &self,
        request: &ConfigUpdateRequest,
    ) -> Result<serde_json::Value, ApiError>;

    /// 获取通知列表（分页）。
    async fn get_notifications(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<serde_json::Value, ApiError>;

    /// 标记通知已读。
    async fn mark_notification_read(&self, notification_id: &str) -> Result<bool, ApiError>;

    // =========================================================================
    // SSH + Context
    // =========================================================================

    /// 列出所有 context。
    async fn list_contexts(&self) -> Result<Vec<ContextInfo>, ApiError>;

    /// 切换活跃 context。
    async fn switch_context(&self, context_id: &str) -> Result<(), ApiError>;

    /// SSH 连接。
    async fn ssh_connect(&self, request: &SshConnectRequest)
    -> Result<serde_json::Value, ApiError>;

    /// SSH 断开。
    async fn ssh_disconnect(&self, context_id: &str) -> Result<(), ApiError>;

    /// 解析 SSH host alias。
    async fn resolve_ssh_host(&self, alias: &str) -> Result<serde_json::Value, ApiError>;

    // =========================================================================
    // 文件 + 路径验证
    // =========================================================================

    /// 校验文件路径。
    async fn validate_path(
        &self,
        path: &str,
        project_root: Option<&str>,
    ) -> Result<serde_json::Value, ApiError>;

    /// 读取 CLAUDE.md 文件（多 scope）。
    async fn read_claude_md_files(&self, project_root: &str)
    -> Result<serde_json::Value, ApiError>;

    /// 读取 `@mention` 文件。
    async fn read_mentioned_file(
        &self,
        path: &str,
        project_root: &str,
    ) -> Result<serde_json::Value, ApiError>;

    // =========================================================================
    // 辅助读取
    // =========================================================================

    /// 读取 agent 配置。
    async fn read_agent_configs(&self, project_root: &str) -> Result<serde_json::Value, ApiError>;

    /// 获取 worktree 会话。
    async fn get_worktree_sessions(&self, group_id: &str) -> Result<serde_json::Value, ApiError>;
}
