//! `LocalDataApi`：`DataApi` trait 的本地文件系统实现。
//!
//! 组装底层 crate 调用，作为默认的数据 API 实现。

use std::path::Path;

use async_trait::async_trait;
use tokio::sync::Mutex;

use cdt_analyze::build_chunks;
use cdt_config::{
    ConfigManager, NotificationManager, read_all_claude_md_files,
    read_mentioned_file as config_read_mentioned_file, validate_file_path,
};
use cdt_discover::{ProjectScanner, path_decoder};
use cdt_parse::parse_file;
use cdt_ssh::{ActiveContext, SshConnectionManager, parse_ssh_config_file, resolve_host};

use super::error::ApiError;
use super::session_metadata::extract_session_metadata;
use super::traits::DataApi;
use super::types::{
    ConfigUpdateRequest, ContextInfo, PaginatedRequest, PaginatedResponse, ProjectInfo,
    SearchRequest, SessionDetail, SessionSummary, SshConnectRequest,
};

/// 本地文件系统 `DataApi` 实现。
pub struct LocalDataApi {
    scanner: Mutex<ProjectScanner>,
    config_mgr: Mutex<ConfigManager>,
    notif_mgr: Mutex<NotificationManager>,
    ssh_mgr: Mutex<SshConnectionManager>,
}

impl LocalDataApi {
    pub fn new(
        scanner: ProjectScanner,
        config_mgr: ConfigManager,
        notif_mgr: NotificationManager,
        ssh_mgr: SshConnectionManager,
    ) -> Self {
        Self {
            scanner: Mutex::new(scanner),
            config_mgr: Mutex::new(config_mgr),
            notif_mgr: Mutex::new(notif_mgr),
            ssh_mgr: Mutex::new(ssh_mgr),
        }
    }
}

#[async_trait]
impl DataApi for LocalDataApi {
    // =========================================================================
    // 项目 + 会话
    // =========================================================================

    async fn list_projects(&self) -> Result<Vec<ProjectInfo>, ApiError> {
        let mut scanner = self.scanner.lock().await;
        let projects = scanner
            .scan()
            .await
            .map_err(|e| ApiError::internal(format!("scan error: {e}")))?;

        Ok(projects
            .into_iter()
            .map(|p| ProjectInfo {
                id: p.id.clone(),
                path: p.path.to_string_lossy().into_owned(),
                display_name: p.name.clone(),
                session_count: p.sessions.len(),
            })
            .collect())
    }

    async fn list_sessions(
        &self,
        project_id: &str,
        pagination: &PaginatedRequest,
    ) -> Result<PaginatedResponse<SessionSummary>, ApiError> {
        let scanner = self.scanner.lock().await;
        let sessions = scanner
            .list_sessions(project_id, &std::collections::BTreeSet::new())
            .await
            .map_err(|e| ApiError::internal(format!("list sessions error: {e}")))?;

        let offset = pagination
            .cursor
            .as_deref()
            .and_then(|c| c.parse::<usize>().ok())
            .unwrap_or(0);
        let total = sessions.len();
        let page_sessions: Vec<_> = sessions
            .into_iter()
            .skip(offset)
            .take(pagination.page_size)
            .collect();

        let projects_dir = path_decoder::get_projects_base_path();
        let base_dir = cdt_discover::path_decoder::extract_base_dir(project_id);
        let dir = projects_dir.join(base_dir);

        let mut page = Vec::with_capacity(page_sessions.len());
        for s in page_sessions {
            let jsonl_path = dir.join(format!("{}.jsonl", s.id));
            let meta = extract_session_metadata(&jsonl_path).await;
            page.push(SessionSummary {
                session_id: s.id.clone(),
                project_id: project_id.to_owned(),
                timestamp: s.last_modified,
                message_count: meta.message_count,
                title: meta.title,
            });
        }

        let next_cursor = if offset + page.len() < total {
            Some((offset + page.len()).to_string())
        } else {
            None
        };

        Ok(PaginatedResponse {
            items: page,
            next_cursor,
            total,
        })
    }

    async fn get_session_detail(
        &self,
        project_id: &str,
        session_id: &str,
    ) -> Result<SessionDetail, ApiError> {
        let scanner = self.scanner.lock().await;
        let sessions = scanner
            .list_sessions(project_id, &std::collections::BTreeSet::new())
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;

        let session = sessions
            .iter()
            .find(|s| s.id == session_id)
            .ok_or_else(|| ApiError::not_found(format!("session {session_id}")))?;

        // 构建 JSONL 文件路径
        let projects_dir = path_decoder::get_projects_base_path();
        let jsonl_path = projects_dir
            .join(project_id)
            .join(format!("{session_id}.jsonl"));

        let messages = parse_file(&jsonl_path)
            .await
            .map_err(|e| ApiError::internal(format!("parse error: {e}")))?;

        let chunks = build_chunks(&messages);

        // 从 session cwd 扫描实际 CLAUDE.md 文件
        let project_root = messages.iter().find_map(|m| m.cwd.as_deref()).unwrap_or("");
        let initial_claude_md = build_claude_md_from_filesystem(project_root).await;

        // 调用 context-tracking 计算完整的 context injections
        let empty_cmd = std::collections::HashMap::new();
        let empty_mf = std::collections::HashMap::new();
        let token_dicts = cdt_analyze::context::TokenDictionaries::new(
            Path::new(""),
            &empty_cmd,
            &empty_cmd,
            &empty_mf,
        );
        let ctx_result = cdt_analyze::context::process_session_context_with_phases(
            &chunks,
            &cdt_analyze::context::ProcessSessionParams {
                project_root: Path::new(""),
                token_dictionaries: token_dicts,
                initial_claude_md_injections: &initial_claude_md,
            },
        );

        // 取最后一个 phase 的最后一个 AI group 的 accumulated_injections
        let context_injections = ctx_result
            .phase_info
            .phases
            .last()
            .and_then(|phase| ctx_result.stats_map.get(&phase.last_ai_group_id))
            .map(|stats| &stats.accumulated_injections)
            .and_then(|inj| serde_json::to_value(inj).ok())
            .unwrap_or(serde_json::Value::Array(Vec::new()));

        Ok(SessionDetail {
            session_id: session_id.to_owned(),
            project_id: project_id.to_owned(),
            chunks: serde_json::to_value(&chunks).unwrap_or_default(),
            metrics: serde_json::json!({"message_count": messages.len()}),
            metadata: serde_json::json!({
                "last_modified": session.last_modified,
                "size": session.size,
            }),
            context_injections,
        })
    }

    async fn get_sessions_by_ids(
        &self,
        session_ids: &[String],
    ) -> Result<Vec<SessionDetail>, ApiError> {
        let mut results = Vec::new();
        for sid in session_ids {
            // 简化：尝试查找，找不到就跳过
            match self.get_session_detail("", sid).await {
                Ok(detail) => results.push(detail),
                Err(_) => results.push(SessionDetail {
                    session_id: sid.clone(),
                    project_id: String::new(),
                    chunks: serde_json::Value::Null,
                    metrics: serde_json::Value::Null,
                    metadata: serde_json::json!({"status": "not_found"}),
                    context_injections: serde_json::Value::Array(Vec::new()),
                }),
            }
        }
        Ok(results)
    }

    // =========================================================================
    // 搜索
    // =========================================================================

    async fn search(&self, request: &SearchRequest) -> Result<serde_json::Value, ApiError> {
        // 简化实现：返回空结果
        // 完整实现需要 SessionSearcher + SearchTextCache
        Ok(serde_json::json!({
            "query": request.query,
            "results": [],
        }))
    }

    // =========================================================================
    // 配置 + 通知
    // =========================================================================

    async fn get_config(&self) -> Result<serde_json::Value, ApiError> {
        let mgr = self.config_mgr.lock().await;
        let config = mgr.get_config();
        serde_json::to_value(&config).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn update_config(
        &self,
        request: &ConfigUpdateRequest,
    ) -> Result<serde_json::Value, ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        let result = match request.section.as_str() {
            "general" => mgr.update_general(request.data.clone()).await,
            "display" => mgr.update_display(request.data.clone()).await,
            "notifications" => mgr.update_notifications(request.data.clone()).await,
            "httpServer" => mgr.update_http_server(request.data.clone()).await,
            _ => {
                return Err(ApiError::validation(format!(
                    "unknown section: {}",
                    request.section
                )));
            }
        };
        let config = result.map_err(|e| ApiError::internal(format!("{e}")))?;
        serde_json::to_value(&config).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn get_notifications(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<serde_json::Value, ApiError> {
        let mgr = self.notif_mgr.lock().await;
        let result = mgr.get_notifications(limit, offset);
        serde_json::to_value(&result).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn mark_notification_read(&self, notification_id: &str) -> Result<bool, ApiError> {
        let mut mgr = self.notif_mgr.lock().await;
        mgr.mark_as_read(notification_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    // =========================================================================
    // SSH + Context
    // =========================================================================

    async fn list_contexts(&self) -> Result<Vec<ContextInfo>, ApiError> {
        let mgr = self.ssh_mgr.lock().await;
        let active = mgr.get_active_context();

        let mut contexts = vec![ContextInfo {
            id: "local".into(),
            kind: "local".into(),
            is_active: matches!(active, ActiveContext::Local),
            host: None,
        }];

        for status in mgr.get_all_statuses() {
            contexts.push(ContextInfo {
                id: status.context_id.clone(),
                kind: "ssh".into(),
                is_active: matches!(active, ActiveContext::Ssh(id) if id == &status.context_id),
                host: status.host.clone(),
            });
        }

        Ok(contexts)
    }

    async fn switch_context(&self, context_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.ssh_mgr.lock().await;
        if context_id == "local" {
            mgr.set_active_context(ActiveContext::Local);
        } else {
            mgr.set_active_context(ActiveContext::Ssh(context_id.to_owned()));
        }
        Ok(())
    }

    async fn ssh_connect(
        &self,
        request: &SshConnectRequest,
    ) -> Result<serde_json::Value, ApiError> {
        let config_path = cdt_ssh::default_ssh_config_path();
        let configs = parse_ssh_config_file(&config_path).await;
        let host_config = resolve_host(&configs, &request.host_alias)
            .ok_or_else(|| ApiError::not_found(format!("SSH host: {}", request.host_alias)))?;

        let context_id = request
            .context_id
            .clone()
            .unwrap_or_else(|| request.host_alias.clone());

        let mut mgr = self.ssh_mgr.lock().await;
        let status = mgr.register_connection(&context_id, &host_config);
        serde_json::to_value(status).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn ssh_disconnect(&self, context_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.ssh_mgr.lock().await;
        mgr.disconnect(context_id);
        Ok(())
    }

    async fn resolve_ssh_host(&self, alias: &str) -> Result<serde_json::Value, ApiError> {
        let config_path = cdt_ssh::default_ssh_config_path();
        let configs = parse_ssh_config_file(&config_path).await;
        let host = resolve_host(&configs, alias)
            .ok_or_else(|| ApiError::not_found(format!("SSH host: {alias}")))?;
        Ok(serde_json::json!({
            "hostname": host.hostname,
            "user": host.user,
            "port": host.port,
            "identityFiles": host.identity_files,
        }))
    }

    // =========================================================================
    // 文件 + 验证
    // =========================================================================

    async fn validate_path(
        &self,
        path: &str,
        project_root: Option<&str>,
    ) -> Result<serde_json::Value, ApiError> {
        let result = validate_file_path(path, project_root.map(Path::new));
        Ok(serde_json::json!({
            "valid": result.valid,
            "error": result.error,
            "normalizedPath": result.normalized_path.map(|p| p.to_string_lossy().into_owned()),
        }))
    }

    async fn read_claude_md_files(
        &self,
        project_root: &str,
    ) -> Result<serde_json::Value, ApiError> {
        let result = read_all_claude_md_files(Path::new(project_root)).await;
        serde_json::to_value(&result).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn read_mentioned_file(
        &self,
        path: &str,
        project_root: &str,
    ) -> Result<serde_json::Value, ApiError> {
        let result = config_read_mentioned_file(path, Path::new(project_root), None)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        Ok(serde_json::to_value(&result).unwrap_or(serde_json::Value::Null))
    }

    // =========================================================================
    // 辅助
    // =========================================================================

    async fn read_agent_configs(&self, _project_root: &str) -> Result<serde_json::Value, ApiError> {
        // 简化：agent config 读取需要文件扫描，暂返回空
        Ok(serde_json::json!({}))
    }

    async fn get_worktree_sessions(&self, _group_id: &str) -> Result<serde_json::Value, ApiError> {
        // 简化：worktree session 需要 WorktreeGrouper，暂返回空
        Ok(serde_json::json!([]))
    }
}

/// 从文件系统扫描 CLAUDE.md 文件，构建 `ClaudeMdContextInjection` 列表。
async fn build_claude_md_from_filesystem(project_root: &str) -> Vec<cdt_core::ContextInjection> {
    use cdt_config::claude_md::Scope;

    let files = read_all_claude_md_files(Path::new(project_root)).await;
    files
        .into_iter()
        .filter(|(_, info)| info.exists)
        .map(|(scope, info)| {
            let core_scope = match scope {
                Scope::Enterprise => cdt_core::ClaudeMdScope::Enterprise,
                Scope::User | Scope::UserRules | Scope::AutoMemory => cdt_core::ClaudeMdScope::User,
                Scope::Project | Scope::ProjectAlt | Scope::ProjectRules | Scope::ProjectLocal => {
                    cdt_core::ClaudeMdScope::Project
                }
            };
            let display_name = info
                .path
                .rsplit('/')
                .next()
                .unwrap_or(&info.path)
                .to_owned();
            cdt_core::ContextInjection::ClaudeMd(cdt_core::ClaudeMdContextInjection {
                id: format!("claude-md-{}", info.path),
                path: info.path,
                display_name,
                scope: core_scope,
                estimated_tokens: u64::try_from(info.estimated_tokens).unwrap_or(0),
                first_seen_turn_index: 0,
            })
        })
        .collect()
}
