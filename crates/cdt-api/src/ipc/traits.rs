//! `DataApi` trait 定义。
//!
//! Spec：`openspec/specs/ipc-data-api/spec.md`。
//!
//! trait-based facade——不绑定具体 IPC 传输。

use async_trait::async_trait;

use super::error::ApiError;
use super::types::{
    ConfigUpdateRequest, ContextInfo, GroupSessionPage, MemoryFileContent, PaginatedRequest,
    PaginatedResponse, ProjectInfo, ProjectMemory, ProjectSessionPrefs, SearchRequest,
    SessionDetail, SessionDetailResponse, SessionSummary, SshConnectRequest,
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
    /// 返回**骨架** `SessionSummary`（`title` / `messageCount` /
    /// `isOngoing` 为占位值），元数据通过 `subscribe_session_metadata()`
    /// 异步推送。IPC 与 HTTP 路径共用本方法（spec ipc-data-api §"Expose
    /// project and session queries" 段落 "HTTP `list_sessions` 复用 IPC
    /// 骨架 + push 实现"）。
    async fn list_sessions(
        &self,
        project_id: &str,
        pagination: &PaginatedRequest,
    ) -> Result<PaginatedResponse<SessionSummary>, ApiError>;

    /// 同步完整返回 session 列表（含全部元数据）。**保留作为 trait fallback**
    /// 供未来非 SSE-aware 客户端使用；axum HTTP route 已切换到 `list_sessions`
    /// 骨架 + SSE patch 路径，本方法**不**再被 HTTP handler 调用。
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
    ///
    /// `known_fingerprint`：调用方持有的上次 fingerprint。后端 stat 文件后生成
    /// 新 fingerprint，若与 `known_fingerprint` 一致返 `Unchanged`（p95 < 5 ms）。
    async fn get_session_detail(
        &self,
        project_id: &str,
        session_id: &str,
        known_fingerprint: Option<&str>,
    ) -> Result<SessionDetailResponse, ApiError>;

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

    /// 写入（新增 / 覆盖）指定项目 memory 目录内的 Markdown 文件——atomic 语义；
    /// 返回写入后的最新 [`ProjectMemory`]，前端无需再调 [`Self::get_project_memory`]。
    /// 设计：change `ssh-project-memory-remote-rw` D9。
    async fn add_memory(
        &self,
        project_id: &str,
        file: &str,
        content: &str,
    ) -> Result<ProjectMemory, ApiError>;

    /// 删除指定项目 memory 目录内的 Markdown 文件；
    /// 返回删除后的最新 [`ProjectMemory`]。
    /// 文件不存在 SHALL 返 [`ApiError::not_found`]。
    async fn delete_memory(&self, project_id: &str, file: &str) -> Result<ProjectMemory, ApiError>;

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
    ///
    /// change `typed-ipc-payload`：返回类型从 `serde_json::Value` typed 化为
    /// `Vec<cdt_core::Chunk>`（wire 形状不变）。
    async fn get_subagent_trace(
        &self,
        _root_session_id: &str,
        _subagent_session_id: &str,
    ) -> Result<Vec<cdt_core::Chunk>, ApiError> {
        Ok(Vec::new())
    }

    async fn get_workflow_agent_trace(
        &self,
        _parent_session_id: &str,
        _run_id: &str,
        _agent_id: &str,
    ) -> Result<Vec<cdt_core::Chunk>, ApiError> {
        Ok(Vec::new())
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
    ///
    /// change `typed-ipc-payload`：返回类型从 `serde_json::Value` typed 化为
    /// `cdt_core::SearchSessionsResult`（wire 形状不变；empty query 路径详
    /// `design.md::D8` 修了缺字段 bug）。
    async fn search(
        &self,
        request: &SearchRequest,
    ) -> Result<cdt_core::SearchSessionsResult, ApiError>;

    /// 搜索 repository group 内所有 worktree 的 sessions。
    async fn search_group_sessions(
        &self,
        group_id: &str,
        query: &str,
    ) -> Result<cdt_core::SearchSessionsResult, ApiError>;

    // =========================================================================
    // 配置 + 通知
    // =========================================================================

    /// 获取当前配置。
    ///
    /// change `typed-ipc-payload`：返回类型从 `serde_json::Value` typed 化为
    /// `cdt_config::AppConfig`（wire 形状不变；前端已 `Promise<AppConfig>` typed）。
    async fn get_config(&self) -> Result<cdt_config::AppConfig, ApiError>;

    /// 获取当前配置的 optimistic concurrency version。
    async fn config_version(&self) -> Result<u64, ApiError> {
        Ok(0)
    }

    /// 原子读取 config + version（单次 lock）。
    async fn get_config_versioned(&self) -> Result<(cdt_config::AppConfig, u64), ApiError> {
        let config = self.get_config().await?;
        let version = self.config_version().await?;
        Ok((config, version))
    }

    /// 更新配置。
    ///
    /// change `typed-ipc-payload`：返回类型从 `serde_json::Value` typed 化为
    /// `cdt_config::AppConfig`（wire 形状不变）。
    async fn update_config(
        &self,
        request: &ConfigUpdateRequest,
    ) -> Result<cdt_config::AppConfig, ApiError>;

    /// 原子更新 config 并返回 `(config, version)` 元组（单次 lock）。
    async fn update_config_versioned(
        &self,
        request: &ConfigUpdateRequest,
    ) -> Result<(cdt_config::AppConfig, u64), ApiError> {
        let config = self.update_config(request).await?;
        let version = self.config_version().await?;
        Ok((config, version))
    }

    /// 获取通知列表（分页）。
    ///
    /// change `typed-ipc-payload`：返回类型从 `serde_json::Value` typed 化为
    /// `cdt_config::GetNotificationsResult`（wire 形状不变；前端已
    /// `Promise<GetNotificationsResult>` typed）。
    async fn get_notifications(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<cdt_config::GetNotificationsResult, ApiError>;

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
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2（SSH 子集 7 个低频 method 暂留 Value）
    async fn ssh_connect(&self, request: &SshConnectRequest)
    -> Result<serde_json::Value, ApiError>;

    /// SSH 断开。
    async fn ssh_disconnect(&self, context_id: &str) -> Result<(), ApiError>;

    /// 测试 SSH 连接，不注册 active context。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2
    async fn ssh_test_connection(
        &self,
        request: &SshConnectRequest,
    ) -> Result<serde_json::Value, ApiError>;

    /// 获取 SSH/context 状态。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2
    async fn ssh_get_state(&self) -> Result<serde_json::Value, ApiError>;

    /// 列出 ssh config hosts。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2
    async fn ssh_get_config_hosts(&self) -> Result<serde_json::Value, ApiError>;

    /// 解析 SSH host alias。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2
    async fn resolve_ssh_host(&self, alias: &str) -> Result<serde_json::Value, ApiError>;

    /// 保存最近一次 SSH 连接。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2
    async fn ssh_save_last_connection(
        &self,
        request: &SshConnectRequest,
    ) -> Result<serde_json::Value, ApiError>;

    /// 读取最近一次 SSH 连接。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2
    async fn ssh_get_last_connection(&self) -> Result<serde_json::Value, ApiError>;

    /// 获取当前活跃 context。
    async fn get_active_context(&self) -> Result<ContextInfo, ApiError>;

    // =========================================================================
    // 文件 + 路径验证
    // =========================================================================

    /// 校验文件路径。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2（文件 / 路径子集 4 个低频 method 暂留 Value）
    async fn validate_path(
        &self,
        path: &str,
        project_root: Option<&str>,
    ) -> Result<serde_json::Value, ApiError>;

    /// 读取 CLAUDE.md 文件（多 scope）。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2
    async fn read_claude_md_files(&self, project_root: &str)
    -> Result<serde_json::Value, ApiError>;

    /// 读取 `@mention` 文件。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2
    async fn read_mentioned_file(
        &self,
        path: &str,
        project_root: &str,
    ) -> Result<serde_json::Value, ApiError>;

    // =========================================================================
    // 辅助读取
    // =========================================================================

    /// 读取 agent 配置。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2
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
                    is_repo_root: true,
                    cwd_relative_to_repo_root: None,
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

    /// k-way merge 流式分页拉取 group 内所有 worktree 合并后的 sessions。
    ///
    /// **Server 无状态**：cursor 自描述每个 worktree 的指针位置（base64
    /// JSON）；MUST NOT 在产出当前页前把 group 全 sessions collect 到 Vec
    /// 全量排序（避免 RSS 击穿）；MUST NOT 对每个 worktree 调
    /// `list_sessions_sync(page_size = usize::MAX)`。
    ///
    /// 全序定义：`(mtime_ms desc, sid asc)`——mtime 大的排前，同 mtime 时
    /// sid 字典序小的排前。续页 `AfterMtime { mtime_ms, sid }` SHALL 找
    /// 第一条满足 `(s.mtime_ms < mtime_ms) || (s.mtime_ms == mtime_ms &&
    /// s.sid > sid)` 的条目（严格在 cursor 之后，无 off-by-one）。
    ///
    /// 默认实现 fallback 到 `get_worktree_sessions(group_id, { page_size })`
    /// 兜底——SSH / 远端 trait impl 暂未实现 k-way merge 时仍可用，但 perf
    /// 会回退到老路径。`LocalDataApi` override 真版本。
    ///
    /// 详 `openspec/specs/ipc-data-api/spec.md` §"Expose group session
    /// listing via k-way merge pagination"。
    /// change `simplify-repository-as-project::D3`。
    async fn list_group_sessions(
        &self,
        group_id: &str,
        page_size: usize,
        cursor: Option<&str>,
    ) -> Result<GroupSessionPage, ApiError> {
        if page_size == 0 {
            return Err(ApiError::validation("pageSize must be > 0"));
        }
        // fallback：跑 get_worktree_sessions 拿一页，重打包成 GroupSessionPage。
        // 注意：fallback 路径会走老的"全量扫描"路径，仅在 trait impl 没 override
        // 时降级使用。
        let pagination = PaginatedRequest {
            page_size,
            cursor: cursor.map(std::borrow::ToOwned::to_owned),
        };
        let resp = self.get_worktree_sessions(group_id, &pagination).await?;
        Ok(GroupSessionPage {
            sessions: resp.items,
            next_cursor: resp.next_cursor,
        })
    }

    // =========================================================================
    // WSL distro 枚举（Windows 平台）
    // =========================================================================

    /// 枚举本机 WSL distro 并返回每个 distro 的 `~/.claude` UNC 候选路径。
    ///
    /// 仅在 `target_os = "windows"` 上执行真实枚举；其他平台返回空报告。
    /// Spec：`openspec/specs/wsl-distro-discovery/spec.md`。
    async fn list_wsl_distros(&self) -> Result<cdt_discover::WslDistroScanReport, ApiError> {
        cdt_discover::wsl::list_distros()
            .await
            .map_err(|e| ApiError::internal(format!("wsl scan: {e}")))
    }

    // =========================================================================
    // 通知 trigger / pin / hide / session prefs
    //
    // 为让 HTTP 路径（浏览器 runtime）能镜像 IPC 同名 command，把这 7 个方法
    // 提升到 trait（spec：`http-data-api::Mirror lazy and auxiliary IPC commands`
    // / `server-mode`）。default fallback 返回 not_found / 空对象，让远端
    // mock 实现保持安全降级；`LocalDataApi` 在自己的 `impl DataApi` 块里
    // override 真实读写逻辑。
    // =========================================================================

    /// 添加 notification trigger，返回更新后的完整 `AppConfig` JSON。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2（Trigger CRUD 2 个低频 method 暂留 Value；返回完整 AppConfig 时可一起 typed 化）
    async fn add_trigger(
        &self,
        _trigger: cdt_config::NotificationTrigger,
    ) -> Result<serde_json::Value, ApiError> {
        Err(ApiError::internal(
            "add_trigger not implemented on this transport",
        ))
    }

    /// 删除 notification trigger，返回更新后的完整 `AppConfig` JSON。
    // TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2
    async fn remove_trigger(&self, _trigger_id: &str) -> Result<serde_json::Value, ApiError> {
        Err(ApiError::internal(
            "remove_trigger not implemented on this transport",
        ))
    }

    /// Pin 一个 session（project + session 维度），写入配置文件。
    async fn pin_session(&self, _project_id: &str, _session_id: &str) -> Result<(), ApiError> {
        Err(ApiError::internal(
            "pin_session not implemented on this transport",
        ))
    }

    /// 取消 pin。
    async fn unpin_session(&self, _project_id: &str, _session_id: &str) -> Result<(), ApiError> {
        Err(ApiError::internal(
            "unpin_session not implemented on this transport",
        ))
    }

    /// 隐藏一个 session。
    async fn hide_session(&self, _project_id: &str, _session_id: &str) -> Result<(), ApiError> {
        Err(ApiError::internal(
            "hide_session not implemented on this transport",
        ))
    }

    /// 取消隐藏。
    async fn unhide_session(&self, _project_id: &str, _session_id: &str) -> Result<(), ApiError> {
        Err(ApiError::internal(
            "unhide_session not implemented on this transport",
        ))
    }

    /// 返回当前 project 的 pin/hide session id 列表（按"最近在前"约定）。
    async fn get_project_session_prefs(
        &self,
        _project_id: &str,
    ) -> Result<ProjectSessionPrefs, ApiError> {
        Ok(ProjectSessionPrefs::default())
    }

    /// 拉一次当前 telemetry 快照（pull-based）。详 `OpenSpec` change
    /// `add-telemetry-signal-bus`。默认实现走 [`cdt_telemetry::take_snapshot`]——
    /// 所有实现共用全局 Registry，不需要在 trait 实现里持有状态。
    async fn get_telemetry_snapshot(&self) -> Result<cdt_telemetry::TelemetrySnapshot, ApiError> {
        Ok(cdt_telemetry::take_snapshot())
    }

    // =========================================================================
    // 外部应用交互（右键菜单：在终端打开 / 在编辑器打开 / 列出当前平台终端）
    // 详 openspec/specs/frontend-context-menu/spec.md 三个 Requirement +
    // openspec/changes/frontend-context-menu-phase-2/design.md::D1-D5
    // =========================================================================

    /// 在用户偏好终端 app 中打开目录（仅 cd 不执行命令）。默认实现返回
    /// `not implemented`——`LocalDataApi` override 真版本（HTTP 镜像同名）。
    async fn open_in_terminal(&self, _path: &str) -> Result<(), ApiError> {
        Err(ApiError::internal(
            "open_in_terminal not implemented on this transport",
        ))
    }

    /// 在用户偏好编辑器中打开文件（含可选行号 / 列号）。
    async fn open_in_editor(
        &self,
        _path: &str,
        _line: Option<u32>,
        _column: Option<u32>,
    ) -> Result<(), ApiError> {
        Err(ApiError::internal(
            "open_in_editor not implemented on this transport",
        ))
    }

    /// 当前平台支持的 `TerminalApp` 列表（`snake_case` 序列化值）。
    /// 默认实现走 `cdt_api::ipc::external_app::list_available_terminals`——
    /// 该函数无状态依赖，所有 transport 共用同一份逻辑。
    async fn list_available_terminals(&self) -> Result<Vec<String>, ApiError> {
        Ok(super::external_app::list_available_terminals())
    }

    /// 批量上报 correctness event 计数（前端 5s/50 累计窗口 flush）。
    /// 详 `OpenSpec` change `add-telemetry-signal-bus` D10。
    /// 白名单 kind: `stale_update.triggered` / `cache.signature_skew_observed_in_ui`。
    /// 未在白名单的 kind silently ignore + inc `telemetry.unregistered_correctness_event`。
    async fn record_correctness_events(
        &self,
        items: Vec<CorrectnessEventItem>,
    ) -> Result<(), ApiError> {
        let r = cdt_telemetry::registry();
        for item in items {
            if r.check_correctness_kind(&item.kind) {
                // SAFETY: kind 来自前端，但已通过白名单校验后只增 counter；
                // counter name 必须是 &'static str — 用 match 转 static literal。
                match item.kind.as_str() {
                    "stale_update.triggered" => r.counter("stale_update.triggered").add(item.count),
                    "cache.signature_skew_observed_in_ui" => r
                        .counter("cache.signature_skew_observed_in_ui")
                        .add(item.count),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// 单条 correctness event 计数请求项。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectnessEventItem {
    pub kind: String,
    pub count: u64,
}
