//! 路由注册。
//!
//! Spec：`openspec/specs/http-data-api/spec.md`。
//! 每个 handler 委托给 `AppState.api` 的对应方法。

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::ipc::{
    ApiError, ApiErrorCode, ConfigUpdateRequest, PaginatedRequest, SearchRequest, SshConnectRequest,
};

use super::sse::sse_handler;
use super::state::AppState;

// =============================================================================
// ApiError → IntoResponse
// =============================================================================

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.code {
            ApiErrorCode::ValidationError | ApiErrorCode::ConfigError => StatusCode::BAD_REQUEST,
            ApiErrorCode::NotFound => StatusCode::NOT_FOUND,
            ApiErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorCode::SshError => StatusCode::BAD_GATEWAY,
        };
        (
            status,
            Json(serde_json::json!({"code": self.code, "message": self.message})),
        )
            .into_response()
    }
}

// =============================================================================
// Router builder
// =============================================================================

/// 构建完整的 `/api` 路由。
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // 项目 + 会话
        .route("/api/projects", get(list_projects))
        .route("/api/projects/{project_id}/sessions", get(list_sessions))
        .route("/api/sessions/{session_id}", get(get_session_detail))
        .route("/api/sessions/batch", post(get_sessions_by_ids))
        // 搜索
        .route("/api/search", post(search))
        // 配置
        .route("/api/config", get(get_config).patch(update_config))
        // 通知
        .route("/api/notifications", get(get_notifications))
        .route(
            "/api/notifications/{notification_id}/read",
            post(mark_notification_read),
        )
        .route(
            "/api/notifications/{notification_id}",
            axum::routing::delete(delete_notification),
        )
        .route(
            "/api/notifications/mark-all-read",
            post(mark_all_notifications_read),
        )
        .route("/api/notifications/clear", post(clear_notifications))
        // SSH + context
        .route("/api/contexts", get(list_contexts))
        .route("/api/contexts/switch", post(switch_context))
        .route("/api/ssh/connect", post(ssh_connect))
        .route("/api/ssh/disconnect", post(ssh_disconnect))
        .route("/api/ssh/resolve-host", get(resolve_ssh_host))
        // 文件 + 验证
        .route("/api/validate/path", post(validate_path))
        .route("/api/claude-md", get(read_claude_md_files))
        .route("/api/mentioned-file", post(read_mentioned_file))
        // 辅助
        .route("/api/agent-configs", get(read_agent_configs))
        .route(
            "/api/worktrees/{group_id}/sessions",
            get(get_worktree_sessions),
        )
        // SSE
        .route("/api/events", get(sse_handler))
        .with_state(state)
}

// =============================================================================
// Handlers — 每个委托给 DataApi trait
// =============================================================================

async fn list_projects(State(s): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let projects = s.api.list_projects().await?;
    Ok(Json(projects))
}

async fn list_sessions(
    State(s): State<AppState>,
    Path(project_id): Path<String>,
    Query(pagination): Query<PaginatedRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // HTTP 无 push 通道，保留同步完整返回语义（spec ipc-data-api §"HTTP
    // list_sessions 保留同步完整返回"）。IPC 路径的骨架化由 trait 方法
    // `list_sessions` 提供，这里显式走 `list_sessions_sync`。
    let result = s.api.list_sessions_sync(&project_id, &pagination).await?;
    Ok(Json(result))
}

async fn get_session_detail(
    State(s): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    // session detail 需要 project_id，这里简化为空字符串
    // 完整实现会从 query param 或 path 中获取
    let result = s.api.get_session_detail("", &session_id).await?;
    Ok(Json(result))
}

async fn get_sessions_by_ids(
    State(s): State<AppState>,
    Json(ids): Json<Vec<String>>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.get_sessions_by_ids(&ids).await?;
    Ok(Json(result))
}

async fn search(
    State(s): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.search(&request).await?;
    Ok(Json(result))
}

async fn get_config(State(s): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let config = s.api.get_config().await?;
    Ok(Json(config))
}

async fn update_config(
    State(s): State<AppState>,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let config = s.api.update_config(&request).await?;
    Ok(Json(config))
}

#[derive(serde::Deserialize)]
struct NotifQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

fn default_limit() -> usize {
    50
}

async fn get_notifications(
    State(s): State<AppState>,
    Query(q): Query<NotifQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.get_notifications(q.limit, q.offset).await?;
    Ok(Json(result))
}

async fn mark_notification_read(
    State(s): State<AppState>,
    Path(notification_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let ok = s.api.mark_notification_read(&notification_id).await?;
    Ok(Json(serde_json::json!({"success": ok})))
}

async fn delete_notification(
    State(s): State<AppState>,
    Path(notification_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let removed = s.api.delete_notification(&notification_id).await?;
    Ok(Json(serde_json::json!({"removed": removed})))
}

async fn mark_all_notifications_read(
    State(s): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    s.api.mark_all_notifications_read().await?;
    Ok(Json(serde_json::json!({"success": true})))
}

#[derive(serde::Deserialize, Default)]
struct ClearNotificationsBody {
    #[serde(default)]
    trigger_id: Option<String>,
}

async fn clear_notifications(
    State(s): State<AppState>,
    body: Option<Json<ClearNotificationsBody>>,
) -> Result<impl IntoResponse, ApiError> {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let removed = s
        .api
        .clear_notifications(body.trigger_id.as_deref())
        .await?;
    Ok(Json(serde_json::json!({"removed": removed})))
}

async fn list_contexts(State(s): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let contexts = s.api.list_contexts().await?;
    Ok(Json(contexts))
}

#[derive(serde::Deserialize)]
struct SwitchContextBody {
    context_id: String,
}

async fn switch_context(
    State(s): State<AppState>,
    Json(body): Json<SwitchContextBody>,
) -> Result<impl IntoResponse, ApiError> {
    s.api.switch_context(&body.context_id).await?;
    Ok(Json(serde_json::json!({"success": true})))
}

async fn ssh_connect(
    State(s): State<AppState>,
    Json(request): Json<SshConnectRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.ssh_connect(&request).await?;
    Ok(Json(result))
}

#[derive(serde::Deserialize)]
struct DisconnectBody {
    context_id: String,
}

async fn ssh_disconnect(
    State(s): State<AppState>,
    Json(body): Json<DisconnectBody>,
) -> Result<impl IntoResponse, ApiError> {
    s.api.ssh_disconnect(&body.context_id).await?;
    Ok(Json(serde_json::json!({"success": true})))
}

#[derive(serde::Deserialize)]
struct ResolveHostQuery {
    alias: String,
}

async fn resolve_ssh_host(
    State(s): State<AppState>,
    Query(q): Query<ResolveHostQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.resolve_ssh_host(&q.alias).await?;
    Ok(Json(result))
}

#[derive(serde::Deserialize)]
struct ValidatePathBody {
    path: String,
    project_root: Option<String>,
}

async fn validate_path(
    State(s): State<AppState>,
    Json(body): Json<ValidatePathBody>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s
        .api
        .validate_path(&body.path, body.project_root.as_deref())
        .await?;
    Ok(Json(result))
}

#[derive(serde::Deserialize)]
struct ClaudeMdQuery {
    project_root: String,
}

async fn read_claude_md_files(
    State(s): State<AppState>,
    Query(q): Query<ClaudeMdQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.read_claude_md_files(&q.project_root).await?;
    Ok(Json(result))
}

#[derive(serde::Deserialize)]
struct MentionBody {
    path: String,
    project_root: String,
}

async fn read_mentioned_file(
    State(s): State<AppState>,
    Json(body): Json<MentionBody>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s
        .api
        .read_mentioned_file(&body.path, &body.project_root)
        .await?;
    Ok(Json(result))
}

async fn read_agent_configs(
    State(s): State<AppState>,
    Query(q): Query<ClaudeMdQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.read_agent_configs(&q.project_root).await?;
    Ok(Json(result))
}

async fn get_worktree_sessions(
    State(s): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.get_worktree_sessions(&group_id).await?;
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_validation_maps_to_400() {
        let err = ApiError::validation("missing field");
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn api_error_not_found_maps_to_404() {
        let err = ApiError::not_found("session xyz");
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn api_error_internal_maps_to_500() {
        let err = ApiError::internal("unexpected");
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn api_error_ssh_maps_to_502() {
        let err = ApiError::ssh("connection refused");
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    }
}
