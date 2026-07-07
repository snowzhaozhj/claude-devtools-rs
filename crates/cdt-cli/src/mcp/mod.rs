//! MCP Server implementation for claude-devtools session intelligence.
//!
//! 7 tools: `list_projects`, `list_sessions`, `get_session`, `get_turn`,
//! `get_tool_output`, `search`, `get_stats`.

pub mod redact;

use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};
use serde::Serialize;

use cdt_api::DataApi;
use cdt_api::SessionListFilter;
use cdt_query::QueryEngine;
use cdt_query::turn_view::{attribute_grep_match, build_turn_detail, build_turn_overviews};

use crate::turn_api::{
    MetricsView, SessionOverviewResponse, StepView, ToolOutputFullResponse, ToolOutputView,
    TurnCompactView, TurnDetailResponse, TurnSearchHit, TurnSearchResponse, next_cursor,
    paginate_cursor,
};
use redact::Redactor;

const DEFAULT_PAGE_SIZE: usize = 20;
const MAX_PAGE_SIZE: usize = 100;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListSessionsResponse {
    total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
    items: Vec<cdt_api::SessionSummary>,
}

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
    #[schemars(description = "Filter by title keyword (case-insensitive)")]
    pub grep: Option<String>,
    #[schemars(description = "Results per page (default 20, max 100)")]
    pub page_size: Option<usize>,
    #[schemars(description = "Pagination cursor from previous response")]
    pub cursor: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetSessionParams {
    #[schemars(
        description = "Session ID (full or short prefix). Use 'latest' for most recent session."
    )]
    pub session: String,
    #[schemars(
        description = "Case-insensitive grep filter. Only turns matching this text are returned, with matchedIn attribution."
    )]
    pub grep: Option<String>,
    #[schemars(description = "Turns per page (default 20, max 100)")]
    pub page_size: Option<usize>,
    #[schemars(description = "Pagination cursor from previous response")]
    pub cursor: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetTurnParams {
    #[schemars(description = "Session ID (full or short prefix). Use 'latest' for most recent.")]
    pub session: String,
    #[schemars(description = "Turn index (0-based)")]
    pub turn: u32,
    #[schemars(description = "Steps per page (default 50, max 100)")]
    pub page_size: Option<usize>,
    #[schemars(description = "Pagination cursor from previous response")]
    pub cursor: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetToolOutputParams {
    #[schemars(description = "Session ID (full or short prefix)")]
    pub session: String,
    #[schemars(description = "The toolUseId from a truncated tool step")]
    pub tool_use_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    #[schemars(description = "Search query text")]
    pub query: String,
    #[schemars(description = "Project name or ID to limit search scope")]
    pub project: Option<String>,
    #[schemars(
        description = "Only search sessions since this time. Same formats as list_sessions since."
    )]
    pub since: Option<String>,
    #[schemars(description = "Results per page (default 20, max 100)")]
    pub page_size: Option<usize>,
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

    async fn resolve_project_for_session(&self, session: &str) -> Result<String, McpError> {
        self.engine
            .find_session_project(session)
            .await
            .map_err(|e| {
                McpError::invalid_params(
                    format!("Cannot find project for session '{session}': {e}"),
                    None,
                )
            })
    }

    async fn resolve_session_latest(
        &self,
        session: &str,
        project: Option<&str>,
    ) -> Result<String, McpError> {
        if session != "latest" {
            return Ok(session.to_string());
        }
        let filter = SessionListFilter {
            since: None,
            until: None,
            grep: None,
            branch: None,
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

    async fn load_session_chunks(
        &self,
        session_id: &str,
    ) -> Result<(String, Vec<cdt_core::Chunk>, bool), McpError> {
        let project_id = self.resolve_project_for_session(session_id).await?;
        let options = cdt_query::SessionQueryOptions::default();
        let detail = self
            .engine
            .get_session_detail(&project_id, session_id, &options)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok((detail.session_id, detail.chunks, detail.is_ongoing))
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
            Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string(&wrapper).unwrap_or_default(),
            )]))
        } else {
            Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
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
        description = "List sessions with filtering. Omit 'project' for cross-project query (defaults since='7d'). Each session includes filesModified for file-based lookup.",
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
        let page_size = params
            .page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        let offset = paginate_cursor(params.cursor.as_deref());

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

        let filter = SessionListFilter {
            since: since_ms,
            until: until_ms,
            grep: params.grep,
            branch: None,
            limit: None,
        };

        let sessions = if is_cross_project {
            self.engine
                .list_sessions_cross_project(&filter)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            let project_name = params.project.clone();
            let project_id = self
                .engine
                .resolve_project(params.project.as_deref().unwrap_or(""))
                .await
                .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
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

        let total = sessions.len();
        let page: Vec<_> = sessions.into_iter().skip(offset).take(page_size).collect();

        let response = ListSessionsResponse {
            total,
            next_cursor: next_cursor(offset, page_size, total),
            items: page,
        };
        self.emit_json(&response)
    }

    #[tool(
        name = "get_session",
        description = "Compact overview of a session as turns. Each turn has question, answer, tool usage summary, metrics. Use grep to filter turns. Drill into a turn with get_turn.",
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
        let session_id = self.resolve_session_latest(&params.session, None).await?;
        let (real_session_id, chunks, _is_ongoing) = self.load_session_chunks(&session_id).await?;

        let overviews = build_turn_overviews(&chunks);
        let grep = params.grep.as_deref().filter(|s| !s.trim().is_empty());

        let page_size = params
            .page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        let offset = paginate_cursor(params.cursor.as_deref());

        let turns_with_match: Vec<TurnCompactView> = if let Some(needle) = grep {
            let chunk_map = cdt_query::step::build_chunk_map(&chunks);
            let all_turns = cdt_analyze::derive_turns(&chunks);

            overviews
                .iter()
                .filter_map(|o| {
                    let turn = all_turns.iter().find(|t| t.index == o.index)?;
                    let steps =
                        cdt_query::step::build_steps_for_turn(&turn.member_chunk_ids, &chunk_map);
                    let matched_in = attribute_grep_match(
                        needle,
                        o.question.as_deref(),
                        o.answer.as_deref(),
                        &steps,
                    );
                    matched_in.map(|m| TurnCompactView::from_overview(o, Some(m)))
                })
                .collect()
        } else {
            overviews
                .iter()
                .map(|o| TurnCompactView::from_overview(o, None))
                .collect()
        };

        let total = turns_with_match.len();
        let page: Vec<_> = turns_with_match
            .into_iter()
            .skip(offset)
            .take(page_size)
            .collect();

        let session_model = overviews.first().and_then(|o| o.metrics.model.clone());
        let total_cost = cdt_query::turn_view::compute_session_cost_from_chunks(&chunks);
        let duration_ms = cdt_query::turn_view::compute_session_duration_ms(&chunks);
        let files_modified = cdt_query::turn_view::extract_files_modified(&chunks);

        let response = SessionOverviewResponse {
            session_id: real_session_id,
            model: session_model,
            total_cost,
            duration_ms,
            files_modified,
            total,
            next_cursor: next_cursor(offset, page_size, total),
            turns: page,
        };
        self.emit_json(&response)
    }

    #[tool(
        name = "get_turn",
        description = "Get a single turn's complete steps (thinking, tool calls with input/output, text, subagent, etc). Tool outputs >=5KB are truncated — use get_tool_output to fetch full text.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_turn(
        &self,
        Parameters(params): Parameters<GetTurnParams>,
    ) -> Result<CallToolResult, McpError> {
        let session_id = self.resolve_session_latest(&params.session, None).await?;
        let (real_session_id, chunks, _) = self.load_session_chunks(&session_id).await?;

        let detail = build_turn_detail(&chunks, params.turn).ok_or_else(|| {
            McpError::invalid_params(
                format!(
                    "Turn index {} not found in session '{}'",
                    params.turn, real_session_id
                ),
                None,
            )
        })?;

        let page_size = params.page_size.unwrap_or(50).clamp(1, MAX_PAGE_SIZE);
        let offset = paginate_cursor(params.cursor.as_deref());

        let steps_total = detail.steps.len();
        let page_steps: Vec<StepView> = detail
            .steps
            .iter()
            .enumerate()
            .skip(offset)
            .take(page_size)
            .map(|(i, s)| StepView::from_step(s, i))
            .collect();

        let response = TurnDetailResponse {
            session_id: real_session_id,
            turn_index: detail.index,
            question: detail.question,
            answer: detail.answer,
            steps_total,
            next_cursor: next_cursor(offset, page_size, steps_total),
            metrics: MetricsView::from(&detail.metrics),
            steps: page_steps,
        };
        self.emit_json(&response)
    }

    #[tool(
        name = "get_tool_output",
        description = "Get the full, untruncated output of a tool call. Use when get_turn shows outputTruncated=true.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_tool_output(
        &self,
        Parameters(params): Parameters<GetToolOutputParams>,
    ) -> Result<CallToolResult, McpError> {
        let (_real_session_id, chunks, _) = self.load_session_chunks(&params.session).await?;

        let mut tool_name = String::new();
        let mut found_output = cdt_core::ToolOutput::Missing;
        for chunk in &chunks {
            if let cdt_core::Chunk::Ai(ai) = chunk {
                for exec in &ai.tool_executions {
                    if exec.tool_use_id == params.tool_use_id {
                        tool_name.clone_from(&exec.tool_name);
                        found_output = exec.output.clone();
                    }
                }
            }
        }

        let output_bytes = match &found_output {
            cdt_core::ToolOutput::Text { text } => text.len() as u64,
            cdt_core::ToolOutput::Structured { value } => {
                serde_json::to_string(value).map_or(0, |s| s.len() as u64)
            }
            cdt_core::ToolOutput::Missing => 0,
        };

        let response = ToolOutputFullResponse {
            session_id: params.session,
            tool_use_id: params.tool_use_id,
            tool_name,
            output_bytes,
            output: ToolOutputView::from(&found_output),
        };
        self.emit_json(&response)
    }

    #[tool(
        name = "search",
        description = "Full-text search across sessions. Returns turn-level hits with turnIndex — use get_turn to drill into a hit.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn search(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let page_size = params
            .page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        let offset = paginate_cursor(params.cursor.as_deref());
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
            None => None,
        };

        let results = self
            .engine
            .search_with_since(&params.query, project_id.as_deref(), None, since_ms)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut hits: Vec<TurnSearchHit> = Vec::new();
        for session_result in &results.results {
            let sid = &session_result.session_id;
            let project_name = Some(
                cdt_discover::decode_path(&session_result.project_id)
                    .to_string_lossy()
                    .into_owned(),
            );

            let chunks_result = self.load_session_chunks(sid).await;
            let Ok((_real_id, chunks, _)) = chunks_result else {
                tracing::warn!(session_id = %sid, "search: skipping session (load failed)");
                continue;
            };

            let overviews = build_turn_overviews(&chunks);
            let chunk_map = cdt_query::step::build_chunk_map(&chunks);
            let all_turns = cdt_analyze::derive_turns(&chunks);

            for overview in &overviews {
                let Some(turn) = all_turns.iter().find(|t| t.index == overview.index) else {
                    continue;
                };
                let steps =
                    cdt_query::step::build_steps_for_turn(&turn.member_chunk_ids, &chunk_map);
                let matched = attribute_grep_match(
                    &params.query,
                    overview.question.as_deref(),
                    overview.answer.as_deref(),
                    &steps,
                );
                if matched.is_some() {
                    let snippet = overview
                        .question
                        .as_deref()
                        .or(overview.answer.as_deref())
                        .unwrap_or("")
                        .chars()
                        .take(200)
                        .collect();
                    let turn_ts = chunks
                        .iter()
                        .find(|c| {
                            let cid = match c {
                                cdt_core::Chunk::User(u) => &u.chunk_id,
                                cdt_core::Chunk::Ai(a) => &a.chunk_id,
                                cdt_core::Chunk::System(s) => &s.chunk_id,
                                cdt_core::Chunk::Compact(co) => &co.chunk_id,
                            };
                            turn.member_chunk_ids.first().is_some_and(|id| id == cid)
                        })
                        .map_or(0, |c| c.timestamp().timestamp_millis());
                    hits.push(TurnSearchHit {
                        session_id: sid.clone(),
                        turn_index: overview.index,
                        question: overview.question.clone(),
                        match_snippet: snippet,
                        timestamp: turn_ts,
                        project_name: project_name.clone(),
                    });
                }
            }
        }

        let total = hits.len();
        let page: Vec<_> = hits.into_iter().skip(offset).take(page_size).collect();

        let response = TurnSearchResponse {
            total,
            next_cursor: next_cursor(offset, page_size, total),
            results: page,
        };
        self.emit_json(&response)
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
                "Claude DevTools — read-only session intelligence (turn model).\n\
\n\
Workflow (2-3 calls for full context):\n\
1. get_session(session) → compact overview with turns (question/answer/tools/metrics)\n\
2. get_turn(session, turn) → full steps (thinking, tool calls with input/output)\n\
3. get_tool_output(session, toolUseId) → full text of truncated tool output\n\
\n\
Other tools:\n\
- list_sessions(since='yesterday') → find sessions\n\
- search(query) → turn-level hits across sessions\n\
- get_stats(period='7d') → aggregated cost/token stats\n\
\n\
Tips:\n\
- 'latest' as session ID = most recent session\n\
- project is always optional — auto-resolved from session\n\
- since/until: relative (7d, 24h), named (today, yesterday, week), absolute (2026-06-06)\n\
- grep in get_session filters turns and adds matchedIn attribution"
                    .to_string(),
            )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

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
    use crate::turn_api::{next_cursor, paginate_cursor};

    #[test]
    fn pagination_helpers() {
        assert_eq!(paginate_cursor(None), 0);
        assert_eq!(paginate_cursor(Some("10")), 10);
        assert_eq!(next_cursor(0, 20, 50), Some("20".into()));
        assert_eq!(next_cursor(40, 20, 50), None);
    }
}
