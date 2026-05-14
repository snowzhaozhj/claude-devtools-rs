//! `DataApi` trait 定义。
//!
//! Spec：`openspec/specs/ipc-data-api/spec.md`。
//!
//! trait-based facade——不绑定具体 IPC 传输。

use async_trait::async_trait;

use super::error::ApiError;
use super::types::{
    ConfigUpdateRequest, ContextInfo, MemoryFileContent, PaginatedRequest, PaginatedResponse,
    ProjectInfo, ProjectMemory, SearchRequest, SessionDetail, SessionSummary, SshConnectRequest,
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

    /// 按 id 批量获取某项目下的轻量会话摘要。
    async fn get_session_summaries_by_ids(
        &self,
        project_id: &str,
        session_ids: &[String],
    ) -> Result<Vec<SessionSummary>, ApiError>;

    /// 获取指定项目的 memory layers 概览。
    async fn get_project_memory(&self, project_id: &str) -> Result<ProjectMemory, ApiError>;

    /// 读取指定项目 memory 目录内的单个 Markdown 文件。
    async fn read_memory_file(
        &self,
        project_id: &str,
        file: &str,
    ) -> Result<MemoryFileContent, ApiError>;

    /// 通过仅 `session_id` 反查所属 `project_id`。
    ///
    /// HTTP `GET /api/sessions/:id` 不携带 `project_id`，需要全局查找；同样
    /// 用于 `get_sessions_by_ids` 这种只接受 session id 的批量入口。
    ///
    /// 默认实现遍历 `list_projects` + `list_sessions_sync`，复杂度
    /// `O(项目数 × 会话数)`。`LocalDataApi` 提供基于 `scanner.projects_dir()`
    /// 的 FS 直扫覆盖（更快，但不强依赖：远端实现保留默认 fallback 即可）。
    /// `Ok(None)` 表示找不到。
    async fn find_session_project(&self, session_id: &str) -> Result<Option<String>, ApiError> {
        let projects = self.list_projects().await?;
        for project in projects {
            let pagination = PaginatedRequest {
                page_size: usize::MAX,
                cursor: None,
            };
            let resp = self.list_sessions_sync(&project.id, &pagination).await?;
            if resp.items.iter().any(|s| s.session_id == session_id) {
                return Ok(Some(project.id));
            }
        }
        Ok(None)
    }

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

    /// 按需把内联 image base64 落盘到 cache 目录并返回 `asset://` URL。
    ///
    /// `get_session_detail` 默认把 `ContentBlock::Image.source.data` 裁剪为空
    /// + 设 `data_omitted=true`（详见 `openspec/specs/ipc-data-api/spec.md`
    /// `Lazy load inline image asset` requirement）；前端 `ImageBlock` 在视口
    /// 内时调本方法拿可直接用作 `<img src>` 的 URL。`block_id` 编码：
    /// `"<chunkUuid>:<blockIndex>"`。
    ///
    /// 默认实现返回空字符串；`LocalDataApi` 提供真实落盘版本。
    async fn get_image_asset(
        &self,
        _root_session_id: &str,
        _session_id: &str,
        _block_id: &str,
    ) -> Result<String, ApiError> {
        Ok(String::new())
    }

    /// 按需拉取一条 tool execution 的完整 `output`。
    ///
    /// `get_session_detail` 默认把 `tool_executions[].output` 内 `text` /
    /// `value` 字段清空 + 设 `output_omitted=true`（详见
    /// `openspec/specs/ipc-data-api/spec.md` `Lazy load tool output`
    /// requirement）；前端 `ExecutionTrace` 在用户点击展开时调本方法按需拉。
    ///
    /// 默认实现返回 `ToolOutput::Missing`；`LocalDataApi` 提供真实读盘版本。
    async fn get_tool_output(
        &self,
        _root_session_id: &str,
        _session_id: &str,
        _tool_use_id: &str,
    ) -> Result<cdt_core::ToolOutput, ApiError> {
        Ok(cdt_core::ToolOutput::Missing)
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

    /// 按 id 删除单条通知。存在并成功删除返回 `true`，不存在返回 `false`。
    async fn delete_notification(&self, notification_id: &str) -> Result<bool, ApiError>;

    /// 批量标记所有通知为已读。
    async fn mark_all_notifications_read(&self) -> Result<(), ApiError>;

    /// 清空通知。`trigger_id=None` 清全部；`Some(id)` 仅清该 trigger 产生的通知。
    /// 返回被删条数。
    async fn clear_notifications(&self, trigger_id: Option<&str>) -> Result<usize, ApiError>;

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

    // =========================================================================
    // 仓库分组（worktree 聚合）
    // =========================================================================

    /// 列出按 git 仓库聚合的项目分组。
    ///
    /// 同一 git 仓库的多个 worktree SHALL 聚合到同一 `RepositoryGroup`；
    /// 无 git 元数据的项目 SHALL 单独成组（`identity == None`）。
    ///
    /// 默认实现 fallback 到 `list_projects` 单成员 group 包装——任何远端实现
    /// 没真接 `WorktreeGrouper` 时仍能给前端一致的形态。
    async fn list_repository_groups(&self) -> Result<Vec<cdt_core::RepositoryGroup>, ApiError> {
        let projects = self.list_projects().await?;
        Ok(projects
            .into_iter()
            .map(|p| cdt_core::RepositoryGroup {
                id: p.id.clone(),
                identity: None,
                name: p.display_name.clone(),
                worktrees: vec![cdt_core::Worktree {
                    id: p.id.clone(),
                    path: std::path::PathBuf::from(&p.path),
                    name: p.display_name.clone(),
                    git_branch: None,
                    is_main_worktree: true,
                    sessions: Vec::new(),
                    created_at: None,
                    most_recent_session: None,
                }],
                most_recent_session: None,
                total_sessions: p.session_count,
            })
            .collect())
    }

    /// 取得某个 `RepositoryGroup` 内所有 worktree 的合并 session 列表。
    ///
    /// 合并规则：先 fan-out 拉每个 worktree 的 sessions（用 `list_sessions_sync`），
    /// 给每条加 `worktreeId` / `worktreeName` 字段，再按 `timestamp` 倒序合并，
    /// 最后应用 `pagination`。
    ///
    /// 未命中 `group_id` 时 SHALL 返回 `not_found` 错误。
    async fn get_worktree_sessions(
        &self,
        group_id: &str,
        pagination: &PaginatedRequest,
    ) -> Result<PaginatedResponse<SessionSummary>, ApiError> {
        if pagination.page_size == 0 {
            return Err(ApiError::validation("pageSize must be > 0"));
        }

        let groups = self.list_repository_groups().await?;
        let group = groups
            .into_iter()
            .find(|g| g.id == group_id)
            .ok_or_else(|| ApiError::not_found(format!("repository group {group_id}")))?;

        let mut all: Vec<SessionSummary> = Vec::new();
        for wt in &group.worktrees {
            let inner = PaginatedRequest {
                page_size: usize::MAX,
                cursor: None,
            };
            let resp = self.list_sessions_sync(&wt.id, &inner).await?;
            for mut s in resp.items {
                s.worktree_id = Some(wt.id.clone());
                s.worktree_name = Some(wt.name.clone());
                all.push(s);
            }
        }
        all.sort_by_key(|s| std::cmp::Reverse(s.timestamp));

        let total = all.len();
        let offset = pagination
            .cursor
            .as_deref()
            .and_then(|c| c.parse::<usize>().ok())
            .unwrap_or(0);
        let end = offset.saturating_add(pagination.page_size).min(total);
        let items = if offset < total {
            all[offset..end].to_vec()
        } else {
            Vec::new()
        };
        let next_cursor = if end < total {
            Some(end.to_string())
        } else {
            None
        };

        Ok(PaginatedResponse {
            items,
            next_cursor,
            total,
        })
    }
}
