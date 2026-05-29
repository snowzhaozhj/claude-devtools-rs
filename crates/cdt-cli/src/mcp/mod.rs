//! MCP Server implementation for claude-devtools session intelligence.

pub mod redact;
pub mod truncate;

use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};
use serde::Serialize;

use cdt_api::DataApi;
use cdt_query::{
    CharRatioEstimator, ChunkKindFilter, QueryEngine, QueryFilter, SessionQueryOptions,
    TokenEstimator,
};

use redact::Redactor;
use truncate::truncate_chunks_to_budget;

// ─────────────────────────────────────────────────────────────────────────────
// Parameter types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListSessionsParams {
    #[schemars(description = "Project name or ID. Required unless --project is global.")]
    pub project: Option<String>,
    #[schemars(description = "Only sessions since this time period (e.g. '7d', '24h', '1h')")]
    pub since: Option<String>,
    #[schemars(description = "Filter by title keyword (case-insensitive)")]
    pub grep: Option<String>,
    #[schemars(description = "Maximum number of sessions to return")]
    pub limit: Option<usize>,
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
    #[schemars(description = "Chunk range, e.g. '10:30'")]
    pub range: Option<String>,
    #[schemars(description = "Return only the last N chunks")]
    pub tail: Option<usize>,
    #[schemars(description = "Filter: 'errors_only' or 'tool_calls'")]
    pub filter: Option<String>,
    #[schemars(description = "Maximum estimated tokens in response (truncates by chunk boundary)")]
    pub max_tokens: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    #[schemars(description = "Search query text")]
    pub query: String,
    #[schemars(description = "Maximum results to return (default 50)")]
    pub limit: Option<usize>,
    #[schemars(description = "Project name or ID to limit search scope")]
    pub project: Option<String>,
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
// Server struct
// ─────────────────────────────────────────────────────────────────────────────

pub struct CdtMcpServer {
    engine: Arc<QueryEngine>,
    redactor: Redactor,
    estimator: Arc<dyn TokenEstimator>,
}

impl CdtMcpServer {
    pub fn new(engine: Arc<QueryEngine>, allow_sensitive: bool) -> Self {
        Self {
            engine,
            redactor: Redactor::new(!allow_sensitive),
            estimator: Arc::new(CharRatioEstimator::default()),
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

    fn redact_json<T: Serialize>(&self, value: &T) -> Result<CallToolResult, McpError> {
        let json = serde_json::to_string_pretty(value)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let (text, redacted_count) = self.redactor.redact(&json);

        if redacted_count > 0 {
            let wrapper = serde_json::json!({
                "data": serde_json::from_str::<serde_json::Value>(&text).unwrap_or(serde_json::Value::String(text)),
                "redacted": true,
                "redactedCount": redacted_count,
            });
            Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&wrapper).unwrap_or_default(),
            )]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(json)]))
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
        self.redact_json(&groups)
    }

    #[tool(
        name = "list_sessions",
        description = "List sessions for a project. Supports filtering by time range (since), title keyword (grep), and limit. Returns session ID, title, duration, status, and message count.",
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

        let filter = QueryFilter {
            since: since_ms,
            grep: params.grep,
            limit: params.limit,
            ..Default::default()
        };
        let sessions = self
            .engine
            .list_sessions(&project_id, &filter)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        self.redact_json(&sessions)
    }

    #[tool(
        name = "get_session_summary",
        description = "Get a structured diagnostic summary of a session. Returns: time phases, tool usage stats, error density, idle gaps, top files touched, and estimated cost. ALWAYS call this FIRST before get_session_detail — it's compact (~2K tokens) and tells you whether you need to drill deeper.",
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
        self.redact_json(&summary_output)
    }

    #[tool(
        name = "get_session_detail",
        description = "Get detailed chunk data from a session. Supports range ('10:30'), tail (last N chunks), filter ('errors_only' or 'tool_calls'), and max_tokens (truncates by chunk boundary to fit context budget). Use get_session_summary first to understand the session before requesting full detail.",
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
        let project_id = self
            .resolve_project_id(params.project.as_deref(), Some(&params.session))
            .await?;

        let kind_filter = match params.filter.as_deref() {
            None => None,
            Some("errors_only") => Some(ChunkKindFilter::ErrorsOnly),
            Some("tool_calls") => Some(ChunkKindFilter::ToolCalls),
            Some(other) => {
                return Err(McpError::invalid_params(
                    format!(
                        "Invalid filter '{other}'. Supported values: 'errors_only', 'tool_calls'"
                    ),
                    None,
                ));
            }
        };

        let range = match params.range.as_deref() {
            None => None,
            Some(s) => Some(parse_range(s).ok_or_else(|| {
                McpError::invalid_params(
                    format!("Invalid range '{s}'. Expected format: 'start:end' (e.g. '10:30')"),
                    None,
                )
            })?),
        };

        let options = SessionQueryOptions {
            range,
            tail: params.tail,
            kind_filter,
            errors_only: false,
        };

        let detail = self
            .engine
            .get_session_detail(&project_id, &params.session, &options)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if let Some(budget) = params.max_tokens {
            let truncated =
                truncate_chunks_to_budget(&detail.chunks, self.estimator.as_ref(), budget);
            let (text, redacted_count) = self.redactor.redact(
                &serde_json::to_string_pretty(&truncated)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?,
            );
            if redacted_count > 0 {
                let wrapper = serde_json::json!({
                    "data": serde_json::from_str::<serde_json::Value>(&text).unwrap_or(serde_json::Value::String(text.clone())),
                    "redacted": true,
                    "redactedCount": redacted_count,
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&wrapper).unwrap_or_default(),
                )]))
            } else {
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
        } else {
            self.redact_json(&detail)
        }
    }

    #[tool(
        name = "get_session_errors",
        description = "Get all errors from a session. Returns chunk index, tool name, tool_use_id, and error message for each failed tool execution.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_session_errors(
        &self,
        Parameters(params): Parameters<SessionIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let project_id = self
            .resolve_project_id(params.project.as_deref(), Some(&params.session))
            .await?;
        let errors = self
            .engine
            .get_session_errors(&project_id, &params.session)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        self.redact_json(&errors)
    }

    #[tool(
        name = "search_sessions",
        description = "Full-text search across all sessions. Returns matching session IDs, titles, and match context. Useful for finding sessions that discussed a specific topic or encountered a specific error.",
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
        let limit = params.limit.unwrap_or(50);
        let project_id = match params.project.as_deref() {
            Some(p) => Some(
                self.engine
                    .resolve_project(p)
                    .await
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?,
            ),
            None => None,
        };
        let results = self
            .engine
            .search(&params.query, project_id.as_deref(), 0, limit)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        self.redact_json(&results)
    }

    #[tool(
        name = "get_session_cost",
        description = "Get token usage and estimated cost for a session. Breaks down by input/output/cache tokens with per-model pricing (Opus $5/$25, Sonnet $3/$15, Haiku $1/$5 per million tokens).",
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
        self.redact_json(&cost)
    }

    #[tool(
        name = "get_stats",
        description = "Get aggregated statistics across sessions for a time period. Note: this requires loading all sessions in the period and may be slow for large datasets. Consider using get_session_cost for individual sessions instead.",
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
            "get_stats is not yet implemented in MCP mode. Use `cdt stats` CLI command directly. \
             This tool will be fully implemented in a follow-up PR.",
            None,
        ))
    }
}

#[tool_handler]
impl ServerHandler for CdtMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_instructions(
            "Claude DevTools session intelligence server. Provides read-only access to Claude Code session history, diagnostics, and cost analysis.\n\n\
             USAGE PATTERN: Always call get_session_summary FIRST before get_session_detail to avoid context overflow. \
             Summary gives you phases, tool usage, errors, and cost in ~2K tokens. \
             Only request detail (with range/tail/max_tokens) when you need specific chunks.\n\n\
             All tools are read-only and safe to call repeatedly."
                .to_string(),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

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
        "m" => num * 60 * 1000,
        "h" => num * 3600 * 1000,
        "d" => num * 24 * 3600 * 1000,
        _ => return None,
    };
    Some(now - ms)
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
