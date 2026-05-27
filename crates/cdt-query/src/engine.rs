use std::sync::Arc;

use cdt_api::{
    DataApi, LocalDataApi, PaginatedRequest, SearchRequest, SessionDetail, SessionDetailResponse,
    SessionSummary,
};

use crate::error::QueryError;
use crate::filter::QueryFilter;
use crate::options::SessionQueryOptions;

/// Error detail extracted from a session's chunks.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEntry {
    pub chunk_index: usize,
    pub tool_name: String,
    pub tool_use_id: String,
    pub error_message: Option<String>,
}

/// High-level query orchestration over `LocalDataApi`.
pub struct QueryEngine {
    api: Arc<LocalDataApi>,
}

impl QueryEngine {
    pub fn new(api: Arc<LocalDataApi>) -> Self {
        Self { api }
    }

    pub fn api(&self) -> &LocalDataApi {
        &self.api
    }

    /// Resolve a project name/id to a `project_id`.
    pub async fn resolve_project(&self, name: &str) -> Result<String, QueryError> {
        let groups = self
            .api
            .list_repository_groups()
            .await
            .map_err(|e| QueryError::Api(e.to_string()))?;

        for group in &groups {
            if group.name.eq_ignore_ascii_case(name) || group.id == name {
                if let Some(wt) = group.worktrees.first() {
                    return Ok(wt.id.clone());
                }
                return Ok(group.id.clone());
            }
            for wt in &group.worktrees {
                if wt.name.eq_ignore_ascii_case(name) || wt.id == name {
                    return Ok(wt.id.clone());
                }
            }
        }
        Err(QueryError::NotFound(format!("project not found: {name}")))
    }

    /// List sessions with optional filter.
    pub async fn list_sessions(
        &self,
        project_id: &str,
        filter: &QueryFilter,
    ) -> Result<Vec<SessionSummary>, QueryError> {
        let pagination = PaginatedRequest {
            page_size: filter.limit.unwrap_or(1000),
            cursor: None,
        };
        let resp = self.api.list_sessions_sync(project_id, &pagination).await?;

        Ok(filter.apply(resp.items))
    }

    /// Get session detail (full parse + build), then apply query options.
    pub async fn get_session_detail(
        &self,
        project_id: &str,
        session_id: &str,
        options: &SessionQueryOptions,
    ) -> Result<SessionDetail, QueryError> {
        let resp = self
            .api
            .get_session_detail(project_id, session_id, None)
            .await?;

        let detail = match resp {
            SessionDetailResponse::Full { detail, .. } => *detail,
            SessionDetailResponse::Unchanged { .. } => {
                return Err(QueryError::Api(
                    "unexpected unchanged response without fingerprint".into(),
                ));
            }
        };

        let chunks = options.apply(detail.chunks);

        Ok(SessionDetail { chunks, ..detail })
    }

    /// Get session metadata (no chunks).
    pub async fn get_session_show(
        &self,
        project_id: &str,
        session_id: &str,
    ) -> Result<SessionDetail, QueryError> {
        let resp = self
            .api
            .get_session_detail(project_id, session_id, None)
            .await?;

        let detail = match resp {
            SessionDetailResponse::Full { detail, .. } => *detail,
            SessionDetailResponse::Unchanged { .. } => {
                return Err(QueryError::Api(
                    "unexpected unchanged response without fingerprint".into(),
                ));
            }
        };

        Ok(SessionDetail {
            chunks: Vec::new(),
            ..detail
        })
    }

    /// Extract all errors from a session.
    pub async fn get_session_errors(
        &self,
        project_id: &str,
        session_id: &str,
    ) -> Result<Vec<ErrorEntry>, QueryError> {
        let resp = self
            .api
            .get_session_detail(project_id, session_id, None)
            .await?;

        let detail = match resp {
            SessionDetailResponse::Full { detail, .. } => *detail,
            SessionDetailResponse::Unchanged { .. } => {
                return Err(QueryError::Api(
                    "unexpected unchanged response without fingerprint".into(),
                ));
            }
        };

        let mut errors = Vec::new();
        for (i, chunk) in detail.chunks.iter().enumerate() {
            if let cdt_core::Chunk::Ai(ai) = chunk {
                for te in &ai.tool_executions {
                    if te.is_error {
                        errors.push(ErrorEntry {
                            chunk_index: i,
                            tool_name: te.tool_name.clone(),
                            tool_use_id: te.tool_use_id.clone(),
                            error_message: te.error_message.clone(),
                        });
                    }
                }
            }
        }

        Ok(errors)
    }

    /// Search across sessions.
    pub async fn search(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: Option<usize>,
    ) -> Result<cdt_core::SearchSessionsResult, QueryError> {
        let project_id_resolved = project_id.map(ToOwned::to_owned);

        if let Some(ref pid) = project_id_resolved {
            let request = SearchRequest {
                query: query.to_owned(),
                project_id: Some(pid.clone()),
                session_id: None,
            };
            let mut result = self.api.search(&request).await?;
            if let Some(lim) = limit {
                result.results.truncate(lim);
            }
            return Ok(result);
        }

        // 全局搜索
        let groups = self
            .api
            .list_repository_groups()
            .await
            .map_err(|e| QueryError::Api(e.to_string()))?;

        let mut all_results = Vec::new();
        let mut total_matches = 0usize;
        let mut sessions_searched = 0usize;

        for group in &groups {
            for wt in &group.worktrees {
                let request = SearchRequest {
                    query: query.to_owned(),
                    project_id: Some(wt.id.clone()),
                    session_id: None,
                };
                match self.api.search(&request).await {
                    Ok(r) => {
                        total_matches += r.total_matches;
                        sessions_searched += r.sessions_searched;
                        all_results.extend(r.results);
                    }
                    Err(e) => {
                        tracing::warn!(project_id = %wt.id, error = %e, "search failed for project");
                    }
                }
            }
        }

        if let Some(lim) = limit {
            all_results.truncate(lim);
        }

        Ok(cdt_core::SearchSessionsResult {
            results: all_results,
            total_matches,
            sessions_searched,
            query: query.to_owned(),
            is_partial: false,
        })
    }

    /// Find which project a session belongs to (global lookup).
    pub async fn find_session_project(&self, session_id: &str) -> Result<String, QueryError> {
        self.api
            .find_session_project(session_id)
            .await
            .map_err(|e| QueryError::Api(e.to_string()))?
            .ok_or_else(|| QueryError::NotFound(format!("session not found: {session_id}")))
    }
}
