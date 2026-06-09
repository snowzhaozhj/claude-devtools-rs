//! MCP Server implementation for claude-devtools session intelligence.

pub mod redact;

use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};
use serde::Serialize;

use cdt_api::DataApi;
use cdt_core::Chunk;
use cdt_query::{ChunkKindFilter, QueryEngine, QueryFilter, SessionQueryOptions};

use crate::view::{self, ChunkView, ContentMode, build_chunk_view};
use redact::Redactor;

const DEFAULT_PAGE_SIZE: usize = 20;
const MAX_PAGE_SIZE: usize = 100;
const DEFAULT_LIST_LIMIT: usize = 20;
const ERROR_MESSAGE_MAX_CHARS: usize = 500;

// ─────────────────────────────────────────────────────────────────────────────
// Parameter types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListSessionsParams {
    #[schemars(description = "Project name or ID. Omit for cross-project query.")]
    pub project: Option<String>,
    #[schemars(
        description = "Only sessions since this time. Formats: relative (7d, 24h, 30m), named (today, yesterday, week), absolute (2026-06-06, ISO 8601)"
    )]
    pub since: Option<String>,
    #[schemars(description = "Only sessions until this time. Same formats as 'since'.")]
    pub until: Option<String>,
    #[schemars(description = "Filter by git branch (case-insensitive substring match)")]
    pub branch: Option<String>,
    #[schemars(description = "Filter to only ongoing/active sessions")]
    pub is_ongoing: Option<bool>,
    #[schemars(description = "Filter by title keyword (case-insensitive)")]
    pub grep: Option<String>,
    #[schemars(description = "Maximum number of sessions to return (default 20, max 100)")]
    pub limit: Option<usize>,
    #[schemars(
        description = "Group results by dimension: 'none' (default flat list), 'project' (by project name), 'day' (by date)"
    )]
    pub group_by: Option<String>,
    #[schemars(description = "Pagination cursor from previous response")]
    pub cursor: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetSessionParams {
    #[schemars(
        description = "Session ID (full or short prefix). Use 'latest' for most recent session."
    )]
    pub session: String,
    #[schemars(description = "Project name or ID (auto-resolved if omitted)")]
    pub project: Option<String>,
    #[schemars(
        description = "Comma-separated list of additional facets to include: phases, tools, activity, idle_gaps, files"
    )]
    pub include: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SessionDetailParams {
    #[schemars(description = "Session ID (full or short prefix)")]
    pub session: String,
    #[schemars(description = "Project name or ID (auto-resolved if omitted)")]
    pub project: Option<String>,
    #[schemars(
        description = "Window selection: chunk range [start, end) by chunkIndex, e.g. '10:30' or '10:' (open-ended). Mutually exclusive with cursor and tail."
    )]
    pub range: Option<String>,
    #[schemars(
        description = "Window selection: return only the last N chunks. Mutually exclusive with cursor and range."
    )]
    pub tail: Option<usize>,
    #[schemars(
        description = "Window selection: pagination cursor from previous response. Mutually exclusive with range and tail."
    )]
    pub cursor: Option<String>,
    #[schemars(description = "Filter: 'errors_only' or 'tool_calls'")]
    pub filter: Option<String>,
    #[schemars(
        description = "Content mode: 'omit' (default) returns structure + size metadata; 'overview' returns one-line per chunk summary; 'full' includes complete content. Do NOT use 'full' without range/tail except for export."
    )]
    pub content_mode: Option<String>,
    #[schemars(
        description = "Max chunks per page (default 20, max 100). Ignored when content_mode='full' without range/tail (returns all)."
    )]
    pub max_chunks: Option<usize>,
    #[schemars(
        description = "Case-insensitive literal chunk filter. Matches text, tool inputs/outputs, tool names, error messages. Empty string ignored."
    )]
    pub grep: Option<String>,
    #[schemars(
        description = "Number of context chunks around each grep hit (default 1). Only used with grep."
    )]
    pub grep_context: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    #[schemars(description = "Search query text")]
    pub query: String,
    #[schemars(description = "Maximum results to return (default 20, max 100)")]
    pub limit: Option<usize>,
    #[schemars(description = "Project name or ID to limit search scope")]
    pub project: Option<String>,
    #[schemars(
        description = "Session ID to scope search to a single session (intra-session search). Auto-resolves project if omitted."
    )]
    pub session: Option<String>,
    #[schemars(
        description = "Only search sessions since this time. Same formats as list_sessions since."
    )]
    pub since: Option<String>,
    #[schemars(description = "Pagination cursor from previous response")]
    pub cursor: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct StatsParams {
    #[schemars(description = "Time period: 'today', 'week', '7d', '24h', '30d' (default '7d')")]
    pub period: Option<String>,
    #[schemars(description = "Project name or ID to limit stats scope")]
    pub project: Option<String>,
    #[schemars(description = "Group results by dimension: 'none' (default), 'model', 'day'")]
    pub group_by: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Response envelope types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PaginatedResponse<T: Serialize> {
    items: T,
    total: usize,
    returned: usize,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResponse {
    results: Vec<cdt_core::SessionSearchResult>,
    total: usize,
    returned: usize,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
    total_matches: usize,
    sessions_searched: usize,
    query: String,
    is_partial: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionDetailMcpResponse {
    session_id: String,
    total_chunks: usize,
    returned_chunks: usize,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
    is_ongoing: bool,
    content_mode: String,
    chunks: Vec<ChunkView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct McpErrorEntry {
    chunk_index: usize,
    tool_name: String,
    tool_use_id: String,
    is_error: bool,
    error_message: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    message_summarized: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Server struct
// ─────────────────────────────────────────────────────────────────────────────

pub struct CdtMcpServer {
    engine: Arc<QueryEngine>,
    redactor: Redactor,
}

impl CdtMcpServer {
    pub fn new(engine: Arc<QueryEngine>, allow_sensitive: bool) -> Self {
        Self {
            engine,
            redactor: Redactor::new(!allow_sensitive),
        }
    }

    async fn resolve_project_id(
        &self,
        project: Option<&str>,
        session: Option<&str>,
    ) -> Result<String, McpError> {
        if let Some(p) = project {
            self.engine
                .resolve_project(p)
                .await
                .map_err(|e| McpError::invalid_params(e.to_string(), None))
        } else if let Some(sid) = session {
            self.engine.find_session_project(sid).await.map_err(|e| {
                McpError::invalid_params(
                    format!("Cannot auto-resolve project for session '{sid}': {e}"),
                    None,
                )
            })
        } else {
            Err(McpError::invalid_params(
                "Either 'project' or 'session' must be provided",
                None,
            ))
        }
    }

    async fn resolve_session_latest(
        &self,
        session: &str,
        project: Option<&str>,
    ) -> Result<String, McpError> {
        if session != "latest" {
            return Ok(session.to_string());
        }
        let filter = QueryFilter {
            since: None,
            until: None,
            grep: None,
            min_messages: None,
            limit: Some(1),
        };
        let sessions = if let Some(p) = project {
            let project_id = self
                .engine
                .resolve_project(p)
                .await
                .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
            self.engine
                .list_sessions(&project_id, &filter)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            self.engine
                .list_sessions_cross_project(&filter)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };
        sessions
            .first()
            .map(|s| s.session_id.clone())
            .ok_or_else(|| McpError::invalid_params("No sessions found for 'latest'", None))
    }

    fn emit_json<T: Serialize>(&self, value: &T) -> Result<CallToolResult, McpError> {
        let json = serde_json::to_string(value)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let (text, redacted_count) = self.redactor.redact(&json);

        if redacted_count > 0 {
            let wrapper = serde_json::json!({
                "data": serde_json::from_str::<serde_json::Value>(&text)
                    .unwrap_or(serde_json::Value::String(text)),
                "redacted": true,
                "redactedCount": redacted_count,
            });
            Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string(&wrapper).unwrap_or_default(),
            )]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(text)]))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool implementations
// ─────────────────────────────────────────────────────────────────────────────

#[tool_router]
impl CdtMcpServer {
    #[tool(
        name = "list_projects",
        description = "List all Claude Code projects (name, path, session count). Rarely needed — prefer list_sessions without project.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_projects(&self) -> Result<CallToolResult, McpError> {
        let groups = self
            .engine
            .api()
            .list_repository_groups()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        self.emit_json(&groups)
    }

    #[tool(
        name = "list_sessions",
        description = "List sessions with filtering. Omit 'project' for cross-project query (defaults since='7d').",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_sessions(
        &self,
        Parameters(params): Parameters<ListSessionsParams>,
    ) -> Result<CallToolResult, McpError> {
        let is_cross_project = params.project.is_none();

        let since_ms = params
            .since
            .as_deref()
            .or(if is_cross_project { Some("7d") } else { None })
            .map(super::time_expr::parse_time_expr_local)
            .transpose()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let until_ms = params
            .until
            .as_deref()
            .map(super::time_expr::parse_time_expr_local)
            .transpose()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let limit = params
            .limit
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, MAX_PAGE_SIZE);
        let offset = parse_cursor_offset(params.cursor.as_deref());

        let filter = QueryFilter {
            since: since_ms,
            until: until_ms,
            grep: params.grep,
            min_messages: None,
            limit: None,
        };

        let all_sessions = if is_cross_project {
            self.engine
                .list_sessions_cross_project(&filter)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            let project_name = params.project.clone();
            let project_id = self
                .resolve_project_id(params.project.as_deref(), None)
                .await?;
            let mut items = self
                .engine
                .list_sessions(&project_id, &filter)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            if let Some(ref name) = project_name {
                for s in &mut items {
                    if s.project_name.is_none() {
                        s.project_name = Some(name.clone());
                    }
                }
            }
            items
        };

        let mut sessions = all_sessions;

        if let Some(ref branch) = params.branch {
            let lower = branch.to_lowercase();
            sessions.retain(|s| {
                s.git_branch
                    .as_deref()
                    .is_some_and(|b| b.to_lowercase().contains(&lower))
            });
        }

        if let Some(ongoing) = params.is_ongoing {
            sessions.retain(|s| s.is_ongoing == ongoing);
        }

        let total = sessions.len();
        let page: Vec<_> = sessions.into_iter().skip(offset).take(limit).collect();
        let returned = page.len();
        let has_more = offset + returned < total;

        let group_by = params.group_by.as_deref().unwrap_or("none");
        if !["none", "project", "day"].contains(&group_by) {
            return Err(McpError::invalid_params(
                format!("Invalid group_by '{group_by}'. Supported: 'none', 'project', 'day'"),
                None,
            ));
        }

        if group_by != "none" {
            let groups = group_sessions(&page, group_by);
            let response = serde_json::json!({
                "groups": groups,
                "total": total,
                "returned": returned,
                "hasMore": has_more,
                "cursor": if has_more { Some(format!("{}", offset + returned)) } else { None },
            });
            return self.emit_json(&response);
        }

        let response = PaginatedResponse {
            items: page,
            total,
            returned,
            has_more,
            cursor: if has_more {
                Some(format!("{}", offset + returned))
            } else {
                None
            },
        };
        self.emit_json(&response)
    }

    #[tool(
        name = "get_session_chunks",
        description = "Get chunk-level content of a session. Use range/tail to window, grep to filter.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_session_detail(
        &self,
        Parameters(params): Parameters<SessionDetailParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate mutually exclusive window params (before resolving project)
        let window_count = u8::from(params.range.is_some())
            + u8::from(params.tail.is_some())
            + u8::from(params.cursor.is_some());
        if window_count > 1 {
            return Err(McpError::invalid_params(
                "Parameters 'range', 'tail', and 'cursor' are mutually exclusive. Pick one or none.",
                None,
            ));
        }

        let content_mode = match params.content_mode.as_deref() {
            None | Some("omit") => ContentMode::Omit,
            Some("full") => ContentMode::Full,
            Some("overview") => ContentMode::Overview,
            Some(other) => {
                return Err(McpError::invalid_params(
                    format!(
                        "Invalid content_mode '{other}'. Supported: 'omit', 'full', 'overview'"
                    ),
                    None,
                ));
            }
        };

        let project_id = self
            .resolve_project_id(params.project.as_deref(), Some(&params.session))
            .await?;

        let kind_filter = match params.filter.as_deref() {
            None => None,
            Some("errors_only") => Some(ChunkKindFilter::ErrorsOnly),
            Some("tool_calls") => Some(ChunkKindFilter::ToolCalls),
            Some(other) => {
                return Err(McpError::invalid_params(
                    format!("Invalid filter '{other}'. Supported: 'errors_only', 'tool_calls'"),
                    None,
                ));
            }
        };

        // Parse range if provided (absolute chunk indices)
        let range = match params.range.as_deref() {
            None => None,
            Some(s) => Some(parse_range(s).ok_or_else(|| {
                McpError::invalid_params(
                    format!("Invalid range '{s}'. Expected: 'start:end' [start, end) e.g. '10:30' or '10:' (open-ended). start must be <= end."),
                    None,
                )
            })?),
        };

        // Fetch ALL chunks (no range/tail applied at query layer) so we keep absolute indices
        let options = SessionQueryOptions {
            range: None,
            tail: None,
            kind_filter: None,
            errors_only: false,
        };

        let detail = self
            .engine
            .get_session_detail(&project_id, &params.session, &options)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let is_ongoing = detail.is_ongoing;

        // Build indexed chunks with absolute indices, then apply filter
        let indexed_chunks: Vec<(usize, &Chunk)> = detail
            .chunks
            .iter()
            .enumerate()
            .filter(|(_, chunk)| match kind_filter {
                None => true,
                Some(ChunkKindFilter::ErrorsOnly) => matches!(chunk, Chunk::Ai(ai) if ai.tool_executions.iter().any(|te| te.is_error)),
                Some(ChunkKindFilter::ToolCalls) => matches!(chunk, Chunk::Ai(ai) if !ai.tool_executions.is_empty()),
            })
            .collect();

        // Reject empty grep (W3: empty string matches everything)
        let grep_param = params.grep.as_deref().filter(|s| !s.trim().is_empty());

        // Apply grep filter + context expansion (D7: kind_filter → grep → context → range)
        let grep_matcher = grep_param.map(cdt_discover::search_text::GrepMatcher::literal);
        let grep_hits: std::collections::HashSet<usize> = if let Some(ref matcher) = grep_matcher {
            indexed_chunks
                .iter()
                .filter(|(_, chunk)| cdt_discover::search_text::chunk_matches_grep(chunk, matcher))
                .map(|(idx, _)| *idx)
                .collect()
        } else {
            std::collections::HashSet::new()
        };

        let indexed_chunks: Vec<(usize, &Chunk)> = if grep_matcher.is_some() {
            let ctx = params.grep_context.unwrap_or(1).min(50);
            let visible: std::collections::HashSet<usize> = grep_hits
                .iter()
                .flat_map(|&i| {
                    let lo = i.saturating_sub(ctx);
                    let hi = i + ctx;
                    lo..=hi
                })
                .collect();
            indexed_chunks
                .into_iter()
                .filter(|(idx, _)| visible.contains(idx))
                .collect()
        } else {
            indexed_chunks
        };

        // Apply window selection (range/tail) on filtered set, preserving absolute indices
        let windowed: Vec<(usize, &Chunk)> = if let Some((start, end)) = range {
            indexed_chunks
                .into_iter()
                .filter(|(abs_idx, _)| *abs_idx >= start && *abs_idx < end)
                .collect()
        } else if let Some(tail) = params.tail {
            let len = indexed_chunks.len();
            if tail < len {
                indexed_chunks[len - tail..].to_vec()
            } else {
                indexed_chunks
            }
        } else {
            indexed_chunks
        };

        let total_chunks = windowed.len();

        // Pagination logic:
        // - range/tail: explicit window, return all of it (no further pagination)
        // - content_mode=full without window: return all (documented behavior for export)
        // - otherwise: paginate with cursor + max_chunks
        let has_explicit_window = range.is_some() || params.tail.is_some();
        let return_all = has_explicit_window
            || (matches!(content_mode, ContentMode::Full) && params.cursor.is_none());

        let (page_chunks, offset): (Vec<(usize, &Chunk)>, usize) = if return_all {
            (windowed, 0)
        } else {
            let off = parse_cursor_offset(params.cursor.as_deref());
            let page_size = params
                .max_chunks
                .unwrap_or(DEFAULT_PAGE_SIZE)
                .clamp(1, MAX_PAGE_SIZE);
            let page: Vec<_> = windowed.into_iter().skip(off).take(page_size).collect();
            (page, off)
        };

        let returned_chunks = page_chunks.len();
        let has_more = !return_all && (offset + returned_chunks < total_chunks);

        if matches!(content_mode, ContentMode::Overview) {
            let overview_chunks: Vec<serde_json::Value> = page_chunks
                .iter()
                .map(|(abs_idx, chunk)| build_overview_entry(*abs_idx, chunk))
                .collect();

            let response = serde_json::json!({
                "sessionId": detail.session_id,
                "totalChunks": total_chunks,
                "returnedChunks": returned_chunks,
                "hasMore": has_more,
                "cursor": if has_more { Some(format!("{}", offset + returned_chunks)) } else { None },
                "isOngoing": is_ongoing,
                "contentMode": "overview",
                "chunks": overview_chunks,
            });
            return self.emit_json(&response);
        }

        let envelopes: Vec<ChunkView> = page_chunks
            .iter()
            .map(|(abs_idx, chunk)| {
                let is_grep_mode = grep_matcher.is_some();
                let is_hit = grep_hits.contains(abs_idx);
                let effective_mode = if is_hit {
                    &ContentMode::Full
                } else {
                    &content_mode
                };
                let hit_flag = if is_grep_mode { Some(is_hit) } else { None };
                build_chunk_view(*abs_idx, chunk, effective_mode, hit_flag)
            })
            .collect();

        let response = SessionDetailMcpResponse {
            session_id: detail.session_id.clone(),
            total_chunks,
            returned_chunks,
            has_more,
            cursor: if has_more {
                Some(format!("{}", offset + returned_chunks))
            } else {
                None
            },
            is_ongoing,
            content_mode: match content_mode {
                ContentMode::Omit => "omit".to_string(),
                ContentMode::Full => "full".to_string(),
                ContentMode::Overview => unreachable!(),
            },
            chunks: envelopes,
        };

        self.emit_json(&response)
    }

    #[tool(
        name = "search_sessions",
        description = "Full-text search across sessions. Returns lightweight snippets grouped by session.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn search_sessions(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let limit = params
            .limit
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, MAX_PAGE_SIZE);
        let offset = parse_cursor_offset(params.cursor.as_deref());
        let since_ms = params
            .since
            .as_deref()
            .map(super::time_expr::parse_time_expr_local)
            .transpose()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let project_id = match params.project.as_deref() {
            Some(p) => Some(
                self.engine
                    .resolve_project(p)
                    .await
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?,
            ),
            None => {
                if let Some(ref sid) = params.session {
                    Some(
                        self.engine
                            .find_session_project(sid)
                            .await
                            .map_err(|e| McpError::invalid_params(e.to_string(), None))?,
                    )
                } else {
                    None
                }
            }
        };

        let results = self
            .engine
            .search_with_since(
                &params.query,
                project_id.as_deref(),
                params.session.as_deref(),
                since_ms,
            )
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let total_results = results.results.len();
        let page: Vec<_> = results
            .results
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();
        let returned = page.len();
        let has_more = offset + returned < total_results;

        let response = SearchResponse {
            results: page,
            total: total_results,
            returned,
            has_more,
            cursor: if has_more {
                Some(format!("{}", offset + returned))
            } else {
                None
            },
            total_matches: results.total_matches,
            sessions_searched: results.sessions_searched,
            query: results.query,
            is_partial: results.is_partial,
        };
        self.emit_json(&response)
    }

    #[tool(
        name = "get_session",
        description = "Session summary + cost + errors in one call. Use 'include' to add phases/tools/activity/idle_gaps/files.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_session(
        &self,
        Parameters(params): Parameters<GetSessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let session_id = self
            .resolve_session_latest(&params.session, params.project.as_deref())
            .await?;
        let project_id = self
            .resolve_project_id(params.project.as_deref(), Some(&session_id))
            .await?;

        let options = SessionQueryOptions::default();
        let detail = self
            .engine
            .get_session_detail(&project_id, &session_id, &options)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let summary = cdt_query::summary::build_summary(&detail);
        let cost = cdt_query::cost::compute_session_cost(&detail);

        let indexed: Vec<(usize, &cdt_core::Chunk)> = detail.chunks.iter().enumerate().collect();
        let error_entries = cdt_query::extract::extract_errors(&indexed);
        let error_count = error_entries.len();
        let top_errors: Vec<McpErrorEntry> = error_entries
            .into_iter()
            .take(10)
            .map(|e| {
                let (msg, truncated) = summarize_error_message(e.error_summary);
                McpErrorEntry {
                    chunk_index: e.chunk_index,
                    tool_name: e.tool_name,
                    tool_use_id: e.tool_use_id,
                    is_error: true,
                    error_message: msg,
                    message_summarized: truncated,
                }
            })
            .collect();

        let include_set: std::collections::HashSet<&str> = params
            .include
            .as_deref()
            .map(|s| s.split(',').map(str::trim).collect())
            .unwrap_or_default();

        let chunk_count = detail.chunks.len();

        let mut result = serde_json::json!({
            "sessionId": session_id,
            "projectId": project_id,
            "messageCount": summary.message_count,
            "chunkCount": chunk_count,
            "durationMs": summary.total_duration_ms,
            "cost": cost,
            "errorCount": error_count,
            "errors": top_errors,
        });

        if include_set.contains("phases") {
            result["phases"] = serde_json::to_value(&summary.phases).unwrap_or_default();
        }
        if include_set.contains("tools") {
            result["toolUsage"] = serde_json::to_value(&summary.tool_usage).unwrap_or_default();
        }
        if include_set.contains("activity") {
            result["toolActivity"] =
                serde_json::to_value(&summary.tool_activity).unwrap_or_default();
        }
        if include_set.contains("idle_gaps") {
            result["idleGaps"] = serde_json::to_value(&summary.idle_gaps).unwrap_or_default();
        }
        if include_set.contains("files") {
            result["topFiles"] = serde_json::to_value(&summary.top_files).unwrap_or_default();
        }

        self.emit_json(&result)
    }

    #[tool(
        name = "get_stats",
        description = "Aggregated statistics: cost, tokens, cache hit rate, tool frequency, model usage, languages. Default period='7d'.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_stats(
        &self,
        Parameters(params): Parameters<StatsParams>,
    ) -> Result<CallToolResult, McpError> {
        let period = params.period.as_deref().unwrap_or("7d");
        let since_ms = super::time_expr::parse_time_expr_local(period)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let since_dt = chrono::DateTime::from_timestamp_millis(since_ms).unwrap_or_default();

        let project_ids: Vec<String> = if let Some(ref p) = params.project {
            vec![
                self.engine
                    .resolve_project(p)
                    .await
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?,
            ]
        } else {
            let groups = self
                .engine
                .api()
                .list_repository_groups()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            groups
                .iter()
                .flat_map(|g| g.worktrees.iter().map(|w| w.id.clone()))
                .collect()
        };

        let pagination = cdt_api::PaginatedRequest {
            page_size: 500,
            cursor: None,
        };
        let mut session_data_list = Vec::new();

        for pid in &project_ids {
            let resp = match self.engine.api().list_sessions_sync(pid, &pagination).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(project_id = %pid, error = %e, "get_stats: skipping project");
                    continue;
                }
            };

            for session in &resp.items {
                if session.timestamp < since_ms {
                    continue;
                }
                match self
                    .engine
                    .api()
                    .get_session_detail(pid, &session.session_id, None)
                    .await
                {
                    Ok(cdt_api::SessionDetailResponse::Full { detail, .. }) => {
                        session_data_list.push(cdt_query::stats::build_session_data(&detail));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(
                            session_id = %session.session_id,
                            error = %e,
                            "get_stats: skipping session"
                        );
                    }
                }
            }
        }

        if session_data_list.is_empty() {
            return self.emit_json(&serde_json::json!({"sessionCount": 0}));
        }

        let group_by = params.group_by.as_deref().unwrap_or("none");
        if !["none", "model", "day"].contains(&group_by) {
            return Err(McpError::invalid_params(
                format!("Invalid group_by '{group_by}'. Supported: 'none', 'model', 'day'"),
                None,
            ));
        }
        if group_by != "none" {
            let grouped = group_stats_data(&session_data_list, group_by, since_dt);
            return self.emit_json(&grouped);
        }

        let result = cdt_query::stats::aggregate(&session_data_list, since_dt);
        self.emit_json(&result)
    }
}

#[tool_handler]
impl ServerHandler for CdtMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "Claude DevTools — read-only session intelligence.\n\
\n\
Pick by intent:\n\
- \"What did I do?\" → list_sessions(since='yesterday')\n\
- \"Summarize session X\" → get_session(session=X)\n\
- \"Deep dive\" → get_session_chunks(session=X, content_mode='overview') then range/grep\n\
- \"Find sessions about Y\" → search_sessions(query=Y)\n\
- \"How much did I spend?\" → get_stats(period='7d')\n\
\n\
Rules:\n\
- 'latest' as session ID = most recent session\n\
- project is always optional — omit for cross-project\n\
- since/until: relative (7d, 24h), named (today, yesterday, week), absolute (2026-06-06)\n\
- search_sessions finds WHICH session; get_session_chunks grep filters WITHIN a session"
                    .to_string(),
            )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn parse_cursor_offset(cursor: Option<&str>) -> usize {
    cursor
        .and_then(|s| s.split(':').next())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0)
}

fn summarize_error_message(msg: Option<String>) -> (Option<String>, bool) {
    match msg {
        None => (None, false),
        Some(s) if s.chars().count() <= ERROR_MESSAGE_MAX_CHARS => (Some(s), false),
        Some(s) => {
            let chars: Vec<char> = s.chars().collect();
            let head_len = ERROR_MESSAGE_MAX_CHARS * 3 / 5;
            let tail_len = ERROR_MESSAGE_MAX_CHARS * 2 / 5;
            let head: String = chars[..head_len].iter().collect();
            let tail: String = chars[chars.len() - tail_len..].iter().collect();
            (Some(format!("{head}\n…\n{tail}")), true)
        }
    }
}

fn parse_range(s: &str) -> Option<(usize, usize)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let start: usize = parts[0].parse().ok()?;
    let end: usize = if parts[1].is_empty() {
        usize::MAX
    } else {
        parts[1].parse().ok()?
    };
    if start > end {
        return None;
    }
    Some((start, end))
}

fn group_stats_data(
    sessions: &[cdt_query::stats::SessionData],
    group_by: &str,
    since: chrono::DateTime<chrono::Utc>,
) -> serde_json::Value {
    let mut keys: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<&cdt_query::stats::SessionData>> =
        std::collections::HashMap::new();
    for s in sessions {
        let key = match group_by {
            "model" => s.model.clone(),
            "day" => chrono::DateTime::from_timestamp_millis(s.timestamp).map_or_else(
                || "unknown".to_string(),
                |dt| dt.format("%Y-%m-%d").to_string(),
            ),
            _ => "all".to_string(),
        };
        if !groups.contains_key(&key) {
            keys.push(key.clone());
        }
        groups.entry(key).or_default().push(s);
    }

    let total = cdt_query::stats::aggregate(sessions, since);
    let group_results: Vec<serde_json::Value> = keys
        .iter()
        .filter_map(|key| {
            groups.get(key).map(|items| {
                let owned: Vec<cdt_query::stats::SessionData> =
                    items.iter().map(|s| (*s).clone()).collect();
                let stats = cdt_query::stats::aggregate(&owned, since);
                serde_json::json!({
                    "key": key,
                    "stats": stats,
                })
            })
        })
        .collect();

    serde_json::json!({
        "groupBy": group_by,
        "total": total,
        "groups": group_results,
    })
}

fn group_sessions(sessions: &[cdt_api::SessionSummary], group_by: &str) -> Vec<serde_json::Value> {
    let mut keys: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<&cdt_api::SessionSummary>> =
        std::collections::HashMap::new();
    for s in sessions {
        let key = match group_by {
            "project" => s.project_name.as_deref().unwrap_or("(unknown)").to_string(),
            "day" => chrono::DateTime::from_timestamp_millis(s.timestamp).map_or_else(
                || "unknown".to_string(),
                |dt| dt.format("%Y-%m-%d").to_string(),
            ),
            _ => "all".to_string(),
        };
        if !groups.contains_key(&key) {
            keys.push(key.clone());
        }
        groups.entry(key).or_default().push(s);
    }
    keys.iter()
        .filter_map(|key| {
            groups.get(key).map(|items| {
                serde_json::json!({
                    "key": key,
                    "count": items.len(),
                    "sessions": items,
                })
            })
        })
        .collect()
}

fn build_overview_entry(abs_index: usize, chunk: &Chunk) -> serde_json::Value {
    match chunk {
        Chunk::Ai(ai) => {
            let tool_names: Vec<&str> = ai
                .tool_executions
                .iter()
                .map(|te| te.tool_name.as_str())
                .collect();
            let error_count = ai.tool_executions.iter().filter(|te| te.is_error).count();
            let headline = ai
                .responses
                .first()
                .map(|r| {
                    let text = view::message_content_text(&r.content);
                    text.chars().take(100).collect::<String>()
                })
                .unwrap_or_default();
            serde_json::json!({
                "chunkIndex": abs_index,
                "kind": "ai",
                "timestamp": ai.timestamp.to_rfc3339(),
                "toolNames": tool_names,
                "errorCount": error_count,
                "headline": headline,
            })
        }
        Chunk::User(user) => {
            let text = view::message_content_text(&user.content);
            let headline: String = text.chars().take(100).collect();
            serde_json::json!({
                "chunkIndex": abs_index,
                "kind": "user",
                "timestamp": user.timestamp.to_rfc3339(),
                "toolNames": [],
                "errorCount": 0,
                "headline": headline,
            })
        }
        Chunk::System(sys) => {
            let headline: String = sys.content_text.chars().take(100).collect();
            serde_json::json!({
                "chunkIndex": abs_index,
                "kind": "system",
                "timestamp": sys.timestamp.to_rfc3339(),
                "toolNames": [],
                "errorCount": 0,
                "headline": headline,
            })
        }
        Chunk::Compact(compact) => {
            serde_json::json!({
                "chunkIndex": abs_index,
                "kind": "compact",
                "timestamp": compact.timestamp.to_rfc3339(),
                "toolNames": [],
                "errorCount": 0,
                "headline": compact.summary_text.chars().take(100).collect::<String>(),
            })
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Server startup
// ─────────────────────────────────────────────────────────────────────────────

pub async fn run_mcp_server(engine: Arc<QueryEngine>, allow_sensitive: bool) -> anyhow::Result<()> {
    use rmcp::{ServiceExt, transport::stdio};

    let server = CdtMcpServer::new(engine, allow_sensitive);
    let service = server
        .serve(stdio())
        .await
        .map_err(|e| anyhow::anyhow!("MCP server initialization failed: {e}"))?;

    service
        .waiting()
        .await
        .map_err(|e| anyhow::anyhow!("MCP server error: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_parse_range_normal() {
        assert_eq!(parse_range("10:20"), Some((10, 20)));
    }

    #[test]
    fn mcp_parse_range_open_ended() {
        assert_eq!(parse_range("10:"), Some((10, usize::MAX)));
    }

    #[test]
    fn mcp_parse_range_rejects_inverted() {
        assert_eq!(parse_range("20:10"), None);
    }

    #[test]
    fn mcp_parse_range_rejects_non_numeric() {
        assert_eq!(parse_range("abc:10"), None);
    }

    #[test]
    fn mcp_parse_range_rejects_no_colon() {
        assert_eq!(parse_range("1020"), None);
    }

    #[test]
    fn summarize_short_message_unchanged() {
        let (msg, summarized) = summarize_error_message(Some("short error".to_string()));
        assert_eq!(msg.unwrap(), "short error");
        assert!(!summarized);
    }

    #[test]
    fn summarize_none_message() {
        let (msg, summarized) = summarize_error_message(None);
        assert!(msg.is_none());
        assert!(!summarized);
    }

    #[test]
    fn summarize_long_message_uses_head_tail() {
        let long_msg: String = (0..1000u32)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let (msg, summarized) = summarize_error_message(Some(long_msg.clone()));
        assert!(summarized);
        let result = msg.unwrap();
        assert!(result.contains('\n'));
        assert!(result.len() < long_msg.len());
    }

    #[test]
    fn group_sessions_by_project() {
        let sessions = vec![
            make_test_session("s1", Some("proj-a"), 1000),
            make_test_session("s2", Some("proj-a"), 2000),
            make_test_session("s3", Some("proj-b"), 3000),
        ];
        let groups = group_sessions(&sessions, "project");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0]["key"], "proj-a");
        assert_eq!(groups[0]["count"], 2);
        assert_eq!(groups[1]["key"], "proj-b");
        assert_eq!(groups[1]["count"], 1);
    }

    #[test]
    fn group_sessions_by_day() {
        let sessions = vec![
            make_test_session("s1", None, 1_717_718_400_000), // 2024-06-07
            make_test_session("s2", None, 1_717_718_400_000), // 2024-06-07
            make_test_session("s3", None, 1_717_804_800_000), // 2024-06-08
        ];
        let groups = group_sessions(&sessions, "day");
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn overview_entry_user_chunk() {
        let chunk = Chunk::User(cdt_core::UserChunk {
            chunk_id: "u1".into(),
            uuid: "u1".into(),
            timestamp: chrono::Utc::now(),
            duration_ms: None,
            content: cdt_core::MessageContent::Text("hello world".into()),
            metrics: cdt_core::ChunkMetrics::default(),
        });
        let entry = build_overview_entry(0, &chunk);
        assert_eq!(entry["kind"], "user");
        assert_eq!(entry["chunkIndex"], 0);
        assert_eq!(entry["headline"], "hello world");
        assert_eq!(entry["errorCount"], 0);
    }

    #[test]
    fn overview_entry_ai_chunk_with_tools() {
        let chunk = Chunk::Ai(cdt_core::AIChunk {
            chunk_id: "ai1".into(),
            timestamp: chrono::Utc::now(),
            duration_ms: None,
            responses: Vec::new(),
            metrics: cdt_core::ChunkMetrics::default(),
            semantic_steps: Vec::new(),
            tool_executions: vec![
                cdt_core::ToolExecution {
                    tool_use_id: "t1".into(),
                    tool_name: "Bash".into(),
                    input: serde_json::json!({}),
                    output: cdt_core::ToolOutput::Missing,
                    is_error: false,
                    start_ts: chrono::Utc::now(),
                    end_ts: None,
                    source_assistant_uuid: "a1".into(),
                    result_agent_id: None,
                    error_message: None,
                    output_omitted: false,
                    output_bytes: None,
                    teammate_spawn: None,
                    workflow_run_id: None,
                    workflow_script_path: None,
                },
                cdt_core::ToolExecution {
                    tool_use_id: "t2".into(),
                    tool_name: "Read".into(),
                    input: serde_json::json!({}),
                    output: cdt_core::ToolOutput::Missing,
                    is_error: true,
                    start_ts: chrono::Utc::now(),
                    end_ts: None,
                    source_assistant_uuid: "a1".into(),
                    result_agent_id: None,
                    error_message: Some("file not found".into()),
                    output_omitted: false,
                    output_bytes: None,
                    teammate_spawn: None,
                    workflow_run_id: None,
                    workflow_script_path: None,
                },
            ],
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        });
        let entry = build_overview_entry(5, &chunk);
        assert_eq!(entry["kind"], "ai");
        assert_eq!(entry["chunkIndex"], 5);
        assert_eq!(entry["errorCount"], 1);
        let tools = entry["toolNames"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0], "Bash");
        assert_eq!(tools[1], "Read");
    }

    fn make_test_session(id: &str, project: Option<&str>, ts: i64) -> cdt_api::SessionSummary {
        cdt_api::SessionSummary {
            session_id: id.to_owned(),
            project_id: "p1".to_owned(),
            timestamp: ts,
            created: 0,
            message_count: 5,
            title: Some("test".to_owned()),
            is_ongoing: false,
            git_branch: None,
            worktree_id: None,
            worktree_name: None,
            group_id: None,
            cwd_relative_to_repo_root: None,
            cwd: None,
            project_name: project.map(ToOwned::to_owned),
        }
    }
}
