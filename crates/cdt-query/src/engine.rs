use std::sync::Arc;

use cdt_api::{
    DataApi, LocalDataApi, SearchRequest, SessionDetail, SessionDetailResponse, SessionListFilter,
    SessionSummary,
};

use crate::error::QueryError;
use crate::filter::QueryFilter;
use crate::options::SessionQueryOptions;

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

    /// List sessions with optional filter (single project).
    ///
    /// Delegates to `LocalDataApi::list_sessions_filtered` (streaming scan).
    pub async fn list_sessions(
        &self,
        project_id: &str,
        filter: &QueryFilter,
    ) -> Result<Vec<SessionSummary>, QueryError> {
        let f = filter.to_session_list_filter();
        Ok(self.api.list_sessions_filtered(project_id, &f).await?)
    }

    /// List sessions across all projects, with filter applied.
    ///
    /// Fan-out per project → collect filtered results → global mtime sort →
    /// truncate to limit.
    pub async fn list_sessions_cross_project(
        &self,
        filter: &QueryFilter,
    ) -> Result<Vec<SessionSummary>, QueryError> {
        let groups = self
            .api
            .list_repository_groups()
            .await
            .map_err(|e| QueryError::Api(e.to_string()))?;

        let mut all_sessions = Vec::new();
        // 内容过滤（grep / branch）SHALL 在 per-project 阶段做，让全局 limit 截断
        // 发生在过滤**之后**——否则 limit 先截会漏掉后续 project 的匹配项。
        // limit 留到全局排序后再 truncate。
        let per_project_filter = SessionListFilter {
            since: filter.since,
            until: filter.until,
            grep: filter.grep.clone(),
            branch: filter.branch.clone(),
            limit: None,
        };

        for group in &groups {
            if let Some(since) = filter.since {
                if group.most_recent_session.is_some_and(|mtime| mtime < since) {
                    continue;
                }
            }
            for wt in &group.worktrees {
                match self
                    .api
                    .list_sessions_filtered(&wt.id, &per_project_filter)
                    .await
                {
                    Ok(mut sessions) => {
                        for s in &mut sessions {
                            if s.project_name.is_none() {
                                s.project_name = Some(group.name.clone());
                            }
                        }
                        all_sessions.extend(sessions);
                    }
                    Err(e) => {
                        tracing::warn!(
                            project_id = %wt.id,
                            error = %e,
                            "cross-project list_sessions: skipping worktree"
                        );
                    }
                }
            }
        }

        all_sessions.sort_by_key(|s| std::cmp::Reverse(s.timestamp));

        if let Some(limit) = filter.limit {
            all_sessions.truncate(limit);
        }

        Ok(all_sessions)
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

    /// Search across sessions.
    pub async fn search(
        &self,
        query: &str,
        project_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<cdt_core::SearchSessionsResult, QueryError> {
        self.search_with_since(query, project_id, session_id, None)
            .await
    }

    pub async fn search_with_since(
        &self,
        query: &str,
        project_id: Option<&str>,
        session_id: Option<&str>,
        since_ms: Option<i64>,
    ) -> Result<cdt_core::SearchSessionsResult, QueryError> {
        let project_id_resolved = project_id.map(ToOwned::to_owned);

        if let Some(ref pid) = project_id_resolved {
            let request = SearchRequest {
                query: query.to_owned(),
                project_id: Some(pid.clone()),
                session_id: session_id.map(ToOwned::to_owned),
            };
            return Ok(self.api.search(&request).await?);
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
        let mut has_error = false;

        for group in &groups {
            if let Some(since) = since_ms {
                if group.most_recent_session.is_some_and(|mtime| mtime < since) {
                    continue;
                }
            }
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
                        has_error = true;
                        tracing::warn!(project_id = %wt.id, error = %e, "search failed for project");
                    }
                }
            }
        }

        Ok(cdt_core::SearchSessionsResult {
            results: all_results,
            total_matches,
            sessions_searched,
            query: query.to_owned(),
            is_partial: has_error,
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
