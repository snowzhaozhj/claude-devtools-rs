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
use cdt_core::{Chunk, message::MessageContent, tool_execution::ToolOutput};
use cdt_query::{ChunkKindFilter, QueryEngine, QueryFilter, SessionQueryOptions};

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
    #[schemars(description = "Project name or ID.")]
    pub project: Option<String>,
    #[schemars(description = "Only sessions since this time period (e.g. '7d', '24h', '1h')")]
    pub since: Option<String>,
    #[schemars(description = "Filter by title keyword (case-insensitive)")]
    pub grep: Option<String>,
    #[schemars(description = "Maximum number of sessions to return (default 20, max 100)")]
    pub limit: Option<usize>,
    #[schemars(description = "Pagination cursor from previous response")]
    pub cursor: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SessionIdParams {
    #[schemars(description = "Session ID (full or short prefix)")]
    pub session: String,
    #[schemars(description = "Project name or ID (auto-resolved if omitted)")]
    pub project: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SessionDetailParams {
    #[schemars(description = "Session ID (full or short prefix)")]
    pub session: String,
    #[schemars(description = "Project name or ID (auto-resolved if omitted)")]
    pub project: Option<String>,
    #[schemars(
        description = "Window selection: chunk range, e.g. '10:30'. Mutually exclusive with cursor and tail."
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
        description = "Content mode: 'omit' (default) returns structure + size metadata; 'full' includes content. Do NOT use 'full' without range/tail except for export — it returns the entire session."
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
pub struct SessionErrorsParams {
    #[schemars(description = "Session ID (full or short prefix)")]
    pub session: String,
    #[schemars(description = "Project name or ID (auto-resolved if omitted)")]
    pub project: Option<String>,
    #[schemars(description = "Maximum errors to return (default 20, max 100)")]
    pub limit: Option<usize>,
    #[schemars(description = "Pagination cursor from previous response")]
    pub cursor: Option<String>,
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
    #[schemars(description = "Pagination cursor from previous response")]
    pub cursor: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[allow(dead_code)]
pub struct StatsParams {
    #[schemars(description = "Time period: 'today', 'week', '7d', '24h', '30d' (default '7d')")]
    pub period: Option<String>,
    #[schemars(description = "Project name or ID to limit stats scope")]
    pub project: Option<String>,
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
struct SessionDetailResponse {
    session_id: String,
    total_chunks: usize,
    returned_chunks: usize,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
    is_ongoing: bool,
    content_mode: String,
    chunks: Vec<ChunkEnvelope>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChunkEnvelope {
    chunk_index: usize,
    chunk_id: String,
    #[serde(rename = "type")]
    kind: String,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_executions: Vec<ToolExecEnvelope>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    responses: Vec<ResponseEnvelope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_content: Option<ContentField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_content: Option<ContentField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compact_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    grep_hit: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolExecEnvelope {
    tool_name: String,
    tool_use_id: String,
    is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<serde_json::Value>,
    output_omitted: bool,
    output_chars: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResponseEnvelope {
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    content_omitted: bool,
    content_chars: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContentField {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    omitted: bool,
    chars: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEntry {
    chunk_index: usize,
    tool_name: String,
    tool_use_id: String,
    is_error: bool,
    error_message: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    message_truncated: bool,
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
        description = "List all Claude Code projects (repository groups) with session counts and last active time. Call this first to discover available projects.",
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
        description = "List sessions for a project. Returns paginated results (default 20 per page). Check `hasMore` and use `cursor` for next page. Supports filtering by time range (since), title keyword (grep).",
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
        let project_id = self
            .resolve_project_id(params.project.as_deref(), None)
            .await?;

        let since_ms = params.since.as_deref().and_then(parse_duration_to_epoch_ms);
        let limit = params
            .limit
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, MAX_PAGE_SIZE);
        let offset = parse_cursor_offset(params.cursor.as_deref());

        let filter = QueryFilter {
            since: since_ms,
            grep: params.grep,
            limit: None,
            ..Default::default()
        };
        let all_sessions = self
            .engine
            .list_sessions(&project_id, &filter)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let total = all_sessions.len();
        let page: Vec<_> = all_sessions.into_iter().skip(offset).take(limit).collect();
        let returned = page.len();
        let has_more = offset + returned < total;

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
        name = "get_session_summary",
        description = "Structured diagnostic summary (~2K tokens): phases, tool stats, errors, idle gaps, top files, cost, and toolActivity (commands, files edited, git ops, CLI tools). Good starting point for session overview.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_session_summary(
        &self,
        Parameters(params): Parameters<SessionIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let project_id = self
            .resolve_project_id(params.project.as_deref(), Some(&params.session))
            .await?;
        let options = SessionQueryOptions::default();
        let detail = self
            .engine
            .get_session_detail(&project_id, &params.session, &options)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let summary_output = cdt_query::summary::build_summary(&detail);
        self.emit_json(&summary_output)
    }

    #[tool(
        name = "get_session_detail",
        description = "Inspect chunks for a known session. Defaults to structure-only (`outputChars`/`contentChars` show omitted sizes). \
            `chunkIndex` is absolute and stable. Window: range, tail, or cursor. Content: 'omit' or 'full' \
            (avoid 'full' without range/tail — returns entire session). \
            `grep`: case-insensitive literal filter across text, tool inputs/outputs, tool names, error messages; \
            hits auto-expand to full with `grepHit` flag. Not for cross-session discovery — use search_sessions first.",
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
            Some(other) => {
                return Err(McpError::invalid_params(
                    format!("Invalid content_mode '{other}'. Supported: 'omit', 'full'"),
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
                    format!("Invalid range '{s}'. Expected: 'start:end' (e.g. '10:30')"),
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
                .filter(|(_, chunk)| chunk_matches_grep(chunk, matcher))
                .map(|(idx, _)| *idx)
                .collect()
        } else {
            std::collections::HashSet::new()
        };

        let indexed_chunks: Vec<(usize, &Chunk)> = if grep_matcher.is_some() {
            let ctx = params.grep_context.unwrap_or(1);
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

        let envelopes: Vec<ChunkEnvelope> = page_chunks
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
                build_chunk_envelope(*abs_idx, chunk, effective_mode, hit_flag)
            })
            .collect();

        let response = SessionDetailResponse {
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
            },
            chunks: envelopes,
        };

        self.emit_json(&response)
    }

    #[tool(
        name = "get_session_errors",
        description = "Get errors from a session. Returns paginated results (default 20). Long error messages are truncated to 500 chars (check `messageTruncated` flag). Use get_session_detail with range + content_mode='full' for full error output.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_session_errors(
        &self,
        Parameters(params): Parameters<SessionErrorsParams>,
    ) -> Result<CallToolResult, McpError> {
        let project_id = self
            .resolve_project_id(params.project.as_deref(), Some(&params.session))
            .await?;
        let all_errors = self
            .engine
            .get_session_errors(&project_id, &params.session)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let limit = params
            .limit
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, MAX_PAGE_SIZE);
        let offset = parse_cursor_offset(params.cursor.as_deref());
        let total = all_errors.len();

        let page: Vec<ErrorEntry> = all_errors
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|e| {
                let (msg, truncated) = truncate_error_message(e.error_message);
                ErrorEntry {
                    chunk_index: e.chunk_index,
                    tool_name: e.tool_name,
                    tool_use_id: e.tool_use_id,
                    is_error: true,
                    error_message: msg,
                    message_truncated: truncated,
                }
            })
            .collect();

        let returned = page.len();
        let has_more = offset + returned < total;

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
        name = "search_sessions",
        description = "Full-text discovery across session search index. Returns grouped session hits with preview snippets, not chunk envelopes. \
            Use `session` for intra-session search. Use get_session_detail with grep/range for chunk-level content.",
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
            .search(
                &params.query,
                project_id.as_deref(),
                params.session.as_deref(),
                offset,
                limit,
            )
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let total_results = results.results.len();
        let returned = total_results.saturating_sub(offset).min(limit);
        let has_more = offset + returned < total_results;

        let response = SearchResponse {
            results: results.results,
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
        name = "get_session_cost",
        description = "Get token usage and estimated cost for a session. Returns aggregated breakdown by input/output/cache tokens with per-model pricing.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_session_cost(
        &self,
        Parameters(params): Parameters<SessionIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let project_id = self
            .resolve_project_id(params.project.as_deref(), Some(&params.session))
            .await?;
        let options = SessionQueryOptions::default();
        let detail = self
            .engine
            .get_session_detail(&project_id, &params.session, &options)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let cost = cdt_query::cost::compute_session_cost(&detail);
        self.emit_json(&cost)
    }

    #[tool(
        name = "get_stats",
        description = "Get aggregated statistics across sessions for a time period. Note: not yet implemented in MCP mode. Use `cdt stats` CLI command directly.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_stats(
        &self,
        Parameters(_params): Parameters<StatsParams>,
    ) -> Result<CallToolResult, McpError> {
        Err(McpError::internal_error(
            "get_stats is not yet implemented in MCP mode. Use `cdt stats` CLI command directly.",
            None,
        ))
    }
}

#[tool_handler]
impl ServerHandler for CdtMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "Claude DevTools — read-only session intelligence.\n\n\
             QUICK START:\n\
             - Overview: get_session_summary → phases, tool stats, toolActivity, cost\n\
             - Discover: search_sessions(query, session?) → grouped hit previews across search index\n\
             - Inspect: get_session_detail(session, grep?, range?) → chunk envelopes with chunkIndex\n\
             - search_sessions finds WHICH session/content; get_session_detail inspects WHAT's inside\n\
             - Avoid content_mode='full' without range/tail; use grep for filtered browsing\n\
             - All lists paginated (hasMore + cursor). chunkIndex is absolute."
                    .to_string(),
            )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Content mode
// ─────────────────────────────────────────────────────────────────────────────

enum ContentMode {
    Omit,
    Full,
}

// ─────────────────────────────────────────────────────────────────────────────
// Chunk envelope builder
// ─────────────────────────────────────────────────────────────────────────────

fn chunk_matches_grep(chunk: &Chunk, matcher: &cdt_discover::search_text::GrepMatcher) -> bool {
    match chunk {
        Chunk::Ai(ai) => {
            ai.responses
                .iter()
                .any(|r| matcher.matches(&message_content_text(&r.content)))
                || ai.tool_executions.iter().any(|te| {
                    matcher.matches(&te.tool_name)
                        || matcher.matches_json_value(&te.input)
                        || match &te.output {
                            cdt_core::tool_execution::ToolOutput::Text { text } => {
                                matcher.matches(text)
                            }
                            cdt_core::tool_execution::ToolOutput::Structured { value } => {
                                matcher.matches_json_value(value)
                            }
                            cdt_core::tool_execution::ToolOutput::Missing => false,
                        }
                        || te
                            .error_message
                            .as_deref()
                            .is_some_and(|m| matcher.matches(m))
                })
        }
        Chunk::User(u) => matcher.matches(&message_content_text(&u.content)),
        Chunk::System(s) => matcher.matches(&s.content_text),
        Chunk::Compact(c) => matcher.matches(&c.summary_text),
    }
}

fn build_chunk_envelope(
    abs_index: usize,
    chunk: &Chunk,
    mode: &ContentMode,
    grep_hit: Option<bool>,
) -> ChunkEnvelope {
    match chunk {
        Chunk::Ai(ai) => {
            let tool_execs: Vec<ToolExecEnvelope> = ai
                .tool_executions
                .iter()
                .map(|te| build_tool_exec_envelope(te, mode))
                .collect();

            let responses: Vec<ResponseEnvelope> = ai
                .responses
                .iter()
                .map(|r| {
                    let text = message_content_text(&r.content);
                    let content_chars = text.chars().count();
                    // Upstream IPC layer may have already omitted content
                    let upstream_omitted = r.content_omitted;
                    match mode {
                        ContentMode::Omit => ResponseEnvelope {
                            model: r.model.clone(),
                            content: None,
                            content_omitted: true,
                            content_chars,
                        },
                        ContentMode::Full => ResponseEnvelope {
                            model: r.model.clone(),
                            content: if upstream_omitted { None } else { Some(text) },
                            content_omitted: upstream_omitted,
                            content_chars,
                        },
                    }
                })
                .collect();

            ChunkEnvelope {
                chunk_index: abs_index,
                chunk_id: ai.chunk_id.clone(),
                kind: "ai".to_string(),
                timestamp: ai.timestamp.to_rfc3339(),
                duration_ms: ai.duration_ms,
                tool_executions: tool_execs,
                responses,
                user_content: None,
                system_content: None,
                compact_summary: None,
                grep_hit,
            }
        }
        Chunk::User(user) => {
            let text = message_content_text(&user.content);
            let chars = text.chars().count();
            let user_content = match mode {
                ContentMode::Omit => ContentField {
                    text: if chars <= 200 { Some(text) } else { None },
                    omitted: chars > 200,
                    chars,
                },
                ContentMode::Full => ContentField {
                    text: Some(text),
                    omitted: false,
                    chars,
                },
            };
            ChunkEnvelope {
                chunk_index: abs_index,
                chunk_id: user.chunk_id.clone(),
                kind: "user".to_string(),
                timestamp: user.timestamp.to_rfc3339(),
                duration_ms: user.duration_ms,
                tool_executions: vec![],
                responses: vec![],
                user_content: Some(user_content),
                system_content: None,
                compact_summary: None,
                grep_hit,
            }
        }
        Chunk::System(sys) => {
            let chars = sys.content_text.chars().count();
            let system_content = match mode {
                ContentMode::Omit => ContentField {
                    text: if chars <= 200 {
                        Some(sys.content_text.clone())
                    } else {
                        None
                    },
                    omitted: chars > 200,
                    chars,
                },
                ContentMode::Full => ContentField {
                    text: Some(sys.content_text.clone()),
                    omitted: false,
                    chars,
                },
            };
            ChunkEnvelope {
                chunk_index: abs_index,
                chunk_id: sys.chunk_id.clone(),
                kind: "system".to_string(),
                timestamp: sys.timestamp.to_rfc3339(),
                duration_ms: sys.duration_ms,
                tool_executions: vec![],
                responses: vec![],
                user_content: None,
                system_content: Some(system_content),
                compact_summary: None,
                grep_hit,
            }
        }
        Chunk::Compact(compact) => ChunkEnvelope {
            chunk_index: abs_index,
            chunk_id: compact.chunk_id.clone(),
            kind: "compact".to_string(),
            timestamp: compact.timestamp.to_rfc3339(),
            duration_ms: compact.duration_ms,
            tool_executions: vec![],
            responses: vec![],
            user_content: None,
            system_content: None,
            compact_summary: Some(compact.summary_text.clone()),
            grep_hit,
        },
    }
}

fn tool_output_to_value(output: &ToolOutput) -> serde_json::Value {
    match output {
        ToolOutput::Text { text } => serde_json::Value::String(text.clone()),
        ToolOutput::Structured { value } => value.clone(),
        ToolOutput::Missing => serde_json::Value::Null,
    }
}

fn build_tool_exec_envelope(te: &cdt_core::ToolExecution, mode: &ContentMode) -> ToolExecEnvelope {
    let output_text = tool_output_text(&te.output);
    // If upstream omitted output, use output_bytes as approximate char count
    let upstream_omitted = te.output_omitted;
    let output_chars = if upstream_omitted {
        te.output_bytes
            .map_or(0, |b| usize::try_from(b).unwrap_or(usize::MAX))
    } else {
        output_text.chars().count()
    };

    match mode {
        ContentMode::Omit => ToolExecEnvelope {
            tool_name: te.tool_name.clone(),
            tool_use_id: te.tool_use_id.clone(),
            is_error: te.is_error,
            input_summary: Some(summarize_input(&te.input)),
            input: None,
            output: None,
            output_omitted: true,
            output_chars,
            error_message: te.error_message.clone(),
        },
        ContentMode::Full => ToolExecEnvelope {
            tool_name: te.tool_name.clone(),
            tool_use_id: te.tool_use_id.clone(),
            is_error: te.is_error,
            input_summary: None,
            input: Some(te.input.clone()),
            output: if upstream_omitted {
                None
            } else {
                Some(tool_output_to_value(&te.output))
            },
            output_omitted: upstream_omitted,
            output_chars,
            error_message: te.error_message.clone(),
        },
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}...")
}

fn summarize_input(input: &serde_json::Value) -> String {
    match input {
        serde_json::Value::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .take(3)
                .map(|(k, v)| {
                    let val_str = match v {
                        serde_json::Value::String(s) => truncate_str(s, 57),
                        other => truncate_str(&other.to_string(), 57),
                    };
                    format!("{k}: {val_str}")
                })
                .collect();
            if map.len() > 3 {
                format!("{} (+{} more)", parts.join(", "), map.len() - 3)
            } else {
                parts.join(", ")
            }
        }
        other => truncate_str(&other.to_string(), 117),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn message_content_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut parts = Vec::new();
            for block in blocks {
                match block {
                    cdt_core::message::ContentBlock::Text { text } => parts.push(text.as_str()),
                    cdt_core::message::ContentBlock::Thinking { thinking, .. } => {
                        parts.push(thinking.as_str());
                    }
                    _ => {}
                }
            }
            parts.join("\n")
        }
    }
}

fn tool_output_text(output: &ToolOutput) -> String {
    match output {
        ToolOutput::Text { text } => text.clone(),
        ToolOutput::Structured { value } => serde_json::to_string(value).unwrap_or_default(),
        ToolOutput::Missing => String::new(),
    }
}

fn parse_cursor_offset(cursor: Option<&str>) -> usize {
    cursor
        .and_then(|s| s.split(':').next())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0)
}

fn truncate_error_message(msg: Option<String>) -> (Option<String>, bool) {
    match msg {
        None => (None, false),
        Some(s) if s.chars().count() <= ERROR_MESSAGE_MAX_CHARS => (Some(s), false),
        Some(s) => (Some(truncate_str(&s, ERROR_MESSAGE_MAX_CHARS)), true),
    }
}

fn parse_duration_to_epoch_ms(s: &str) -> Option<i64> {
    let now = chrono::Utc::now().timestamp_millis();
    let s = s.trim();
    if s == "today" {
        let start_of_day = chrono::Utc::now().date_naive().and_hms_opt(0, 0, 0)?;
        return Some(start_of_day.and_utc().timestamp_millis());
    }
    if s == "week" {
        return Some(now - 7 * 24 * 3600 * 1000);
    }

    let split_pos = s.char_indices().next_back().map_or(0, |(i, _)| i);
    let (num_str, unit) = s.split_at(split_pos);
    let num: i64 = num_str.parse().ok()?;
    let ms = match unit {
        "m" => num.checked_mul(60 * 1000)?,
        "h" => num.checked_mul(3600 * 1000)?,
        "d" => num.checked_mul(24 * 3600 * 1000)?,
        _ => return None,
    };
    now.checked_sub(ms)
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
    Some((start, end))
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
