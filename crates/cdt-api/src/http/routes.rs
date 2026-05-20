//! 路由注册。
//!
//! Spec：`openspec/specs/http-data-api/spec.md`。
//! 每个 handler 委托给 `AppState.api` 的对应方法。

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};

use crate::ipc::{
    ApiError, ApiErrorCode, ConfigUpdateRequest, PaginatedRequest, SearchRequest, SshConnectRequest,
};

use super::cors::localhost_cors_layer;
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

/// 构建完整的 `/api` 路由 + 可选静态文件 serve。
///
/// `static_dir` 为 `Some(<existing dir>)` 时挂 `ServeDir`：未命中 `/api/*`
/// 路由的 GET 请求 SHALL fallback 到该目录（前端 client-side router）。
/// 路径不存在或非目录时仅 `tracing::warn!` 后跳过 ServeDir，行为退化为
/// 之前的"仅 API"模式。
///
/// CORS：所有路由统一 layer `localhost_cors_layer`，仅放行 localhost / 127.0.0.1
/// origin（详 `crate::http::cors`）。
pub fn build_router(state: AppState, static_dir: Option<PathBuf>) -> Router {
    let api_router = Router::new()
        // 项目 + 会话
        .route("/api/projects", get(list_projects))
        .route("/api/projects/{project_id}/sessions", get(list_sessions))
        .route(
            "/api/projects/{project_id}/session-summaries/batch",
            post(get_session_summaries_by_ids),
        )
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
            delete(delete_notification),
        )
        .route(
            "/api/notifications/mark-all-read",
            post(mark_all_notifications_read),
        )
        .route("/api/notifications/clear", post(clear_notifications))
        // 通知 trigger CRUD（spec http-data-api::Mirror lazy and auxiliary）
        .route("/api/notifications/triggers", post(add_trigger))
        .route(
            "/api/notifications/triggers/{trigger_id}",
            delete(remove_trigger),
        )
        // SSH + context
        .route("/api/contexts", get(list_contexts))
        .route("/api/contexts/active", get(get_active_context))
        .route("/api/contexts/switch", post(switch_context))
        .route("/api/ssh/connect", post(ssh_connect))
        .route("/api/ssh/disconnect", post(ssh_disconnect))
        .route("/api/ssh/test-connection", post(ssh_test_connection))
        .route("/api/ssh/state", get(ssh_get_state))
        .route("/api/ssh/config-hosts", get(ssh_get_config_hosts))
        .route(
            "/api/ssh/last-connection",
            get(ssh_get_last_connection).post(ssh_save_last_connection),
        )
        .route("/api/ssh/resolve-host", get(resolve_ssh_host))
        // 文件 + 验证
        .route("/api/validate/path", post(validate_path))
        .route("/api/claude-md", get(read_claude_md_files))
        .route("/api/mentioned-file", post(read_mentioned_file))
        // 辅助
        .route("/api/agent-configs", get(read_agent_configs))
        .route("/api/repository-groups", get(list_repository_groups))
        .route("/api/wsl-distros", get(list_wsl_distros))
        .route(
            "/api/worktrees/{group_id}/sessions",
            get(get_worktree_sessions),
        )
        // Project memory（lazy mirror）
        .route("/api/projects/{project_id}/memory", get(get_project_memory))
        .route(
            "/api/projects/{project_id}/memory-files",
            post(read_memory_file),
        )
        // Session prefs / pin / hide（lazy mirror）
        .route(
            "/api/projects/{project_id}/session-prefs",
            get(get_project_session_prefs),
        )
        .route(
            "/api/projects/{project_id}/sessions/{session_id}/pin",
            post(pin_session).delete(unpin_session),
        )
        .route(
            "/api/projects/{project_id}/sessions/{session_id}/hide",
            post(hide_session).delete(unhide_session),
        )
        // Subagent trace / image asset / tool output（lazy mirror）
        .route(
            "/api/sessions/{root_session_id}/subagents/{subagent_session_id}/trace",
            get(get_subagent_trace),
        )
        .route(
            "/api/sessions/{root_session_id}/subagents/{session_id}/blocks/{block_id}/image",
            get(get_image_asset),
        )
        .route(
            "/api/sessions/{root_session_id}/subagents/{session_id}/tools/{tool_use_id}/output",
            get(get_tool_output),
        )
        // SSE
        .route("/api/events", get(sse_handler))
        .with_state(state);

    // 顺序：CORS layer 在最外层（先看 Origin）；静态文件 fallback 仅在传入
    // static_dir 且其存在时挂载；不存在 / None 时未命中 `/api/*` 的请求自然
    // 返 404（与本 change 之前行为一致）。
    let router_with_static = match static_dir {
        Some(p) if p.is_dir() => {
            // 用 closure + Arc::clone 捕获 dir，避免引入第二个 with_state 与
            // api_router 已设的 AppState 冲突（axum 0.8 一个 Router 只能持有
            // 单一 State 类型）。
            let dir = Arc::new(p);
            api_router.fallback(move |uri: axum::http::Uri| {
                let dir = Arc::clone(&dir);
                async move { static_fallback(dir, uri).await }
            })
        }
        Some(p) => {
            tracing::warn!(
                target: "cdt_api::http",
                path = %p.display(),
                "static_dir does not exist or is not a directory; serving /api/* only"
            );
            api_router
        }
        None => api_router,
    };

    router_with_static.layer(localhost_cors_layer())
}

/// 静态文件 + SPA fallback handler。
///
/// 三种情况：
/// 1. 磁盘上对应文件存在 → serve（手动 guess mime）
/// 2. navigation 请求（路径无 `.` 扩展名 / 根路径）→ 返回 `index.html`
/// 3. 带扩展名但磁盘上不存在的资源 → 404（**不**得 fallback 到 HTML 否则
///    浏览器把 HTML 当 JS 解析 + CDN 缓存脏数据，codex review 指出的 SPA
///    部署经典坑）
async fn static_fallback(dir: Arc<PathBuf>, uri: axum::http::Uri) -> axum::response::Response {
    let raw = uri.path().trim_start_matches('/');
    // path traversal 防御：URL-encoded `%2e%2e`、Windows backslash、percent-decode
    // 后含 `..` 段，三种形态都拦。`Uri::path()` 在 axum 0.8 不自动 percent-decode
    // （codex review 第二轮指出），裸 `..` 段比对会被 `%2e%2e/etc/passwd` 绕过。
    if !is_path_safe(raw) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let candidate = dir.join(raw);
    if candidate.is_file() {
        return serve_file(&candidate).await;
    }
    // navigation 请求：根路径 `/` 或路径中无 `.`（前端 client-side router 路径）
    let is_navigation = raw.is_empty() || !raw.contains('.');
    if is_navigation {
        return serve_file(&dir.join("index.html")).await;
    }
    StatusCode::NOT_FOUND.into_response()
}

/// 检查 path 是否安全（无 traversal 痕迹）。
///
/// 三道闸门：
/// 1. raw 含 `\` → 拒（Windows 路径分隔符，HTTP 路径不应出现）
/// 2. percent-decode 后 仍含 `\` → 拒（`%5c` 形态）
/// 3. percent-decode 后按 `/` 切段，任一段为 `..` → 拒（覆盖 `..` 与 `%2e%2e` 形态）
fn is_path_safe(raw: &str) -> bool {
    if raw.contains('\\') {
        return false;
    }
    let decoded = percent_encoding::percent_decode_str(raw).decode_utf8_lossy();
    if decoded.contains('\\') {
        return false;
    }
    !decoded.split('/').any(|seg| seg == "..")
}

async fn serve_file(path: &std::path::Path) -> axum::response::Response {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let mime = guess_mime(path);
            ([(axum::http::header::CONTENT_TYPE, mime)], bytes).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// 极简 mime 推断——仅覆盖前端 bundle 常用扩展名。
///
/// 不引入 `mime_guess` 依赖：本场景 bundle 内容固定（Vite 产物），扩展名
/// 集合可枚举；任何未列入的扩展名 fallback 到 `application/octet-stream`，
/// 浏览器会按文件名提示用户下载——比错配 mime 更安全。
fn guess_mime(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
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
    // 走与 IPC 共用的骨架 + spawn 后台扫描 + `broadcast::Sender<SessionMetadataUpdate>`
    // emit 路径（spec ipc-data-api §"Expose project and session queries" 段落
    // "HTTP `list_sessions` 复用 IPC 骨架 + push 实现"）。后台扫描产物经
    // `http::bridge::forward_session_metadata` 桥接到 `/api/events` SSE，
    // 浏览器 client 按 `session_metadata_update` event 收到与 IPC 路径同形的 patch。
    let result = s.api.list_sessions(&project_id, &pagination).await?;
    Ok(Json(result))
}

async fn get_session_summaries_by_ids(
    State(s): State<AppState>,
    Path(project_id): Path<String>,
    Json(ids): Json<Vec<String>>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s
        .api
        .get_session_summaries_by_ids(&project_id, &ids)
        .await?;
    Ok(Json(result))
}

async fn get_session_detail(
    State(s): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    // spec：`GET /api/sessions/:id` 不携带 project_id；先反查所属 project
    // 再走 `DataApi::get_session_detail(project_id, session_id)` 标准路径。
    // 反查未命中按 spec `Return safe defaults on lookup failures` 返 404。
    let project_id = s
        .api
        .find_session_project(&session_id)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("session {session_id}")))?;
    let result = s.api.get_session_detail(&project_id, &session_id).await?;
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
#[serde(rename_all = "camelCase")]
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

async fn ssh_test_connection(
    State(s): State<AppState>,
    Json(request): Json<SshConnectRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.ssh_test_connection(&request).await?;
    Ok(Json(result))
}

async fn ssh_get_state(State(s): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.ssh_get_state().await?;
    Ok(Json(result))
}

async fn ssh_get_config_hosts(State(s): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.ssh_get_config_hosts().await?;
    Ok(Json(result))
}

async fn ssh_save_last_connection(
    State(s): State<AppState>,
    Json(request): Json<SshConnectRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.ssh_save_last_connection(&request).await?;
    Ok(Json(result))
}

async fn ssh_get_last_connection(State(s): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.ssh_get_last_connection().await?;
    Ok(Json(result))
}

async fn get_active_context(State(s): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.get_active_context().await?;
    Ok(Json(result))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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

async fn list_repository_groups(State(s): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.list_repository_groups().await?;
    Ok(Json(result))
}

async fn list_wsl_distros(State(s): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.list_wsl_distros().await?;
    Ok(Json(result))
}

async fn get_worktree_sessions(
    State(s): State<AppState>,
    Path(group_id): Path<String>,
    Query(pagination): Query<PaginatedRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.get_worktree_sessions(&group_id, &pagination).await?;
    Ok(Json(result))
}

// =============================================================================
// Lazy / 辅助 IPC 镜像（spec http-data-api::Mirror lazy and auxiliary IPC commands）
// =============================================================================

async fn get_project_memory(
    State(s): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let memory = s.api.get_project_memory(&project_id).await?;
    Ok(Json(memory))
}

#[derive(serde::Deserialize)]
struct ReadMemoryBody {
    file: String,
}

async fn read_memory_file(
    State(s): State<AppState>,
    Path(project_id): Path<String>,
    Json(body): Json<ReadMemoryBody>,
) -> Result<impl IntoResponse, ApiError> {
    let content = s.api.read_memory_file(&project_id, &body.file).await?;
    Ok(Json(content))
}

async fn get_subagent_trace(
    State(s): State<AppState>,
    Path((root_session_id, subagent_session_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let trace = s
        .api
        .get_subagent_trace(&root_session_id, &subagent_session_id)
        .await?;
    Ok(Json(trace))
}

async fn get_image_asset(
    State(s): State<AppState>,
    Path((root_session_id, session_id, block_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let asset = s
        .api
        .get_image_asset(&root_session_id, &session_id, &block_id)
        .await?;
    Ok(Json(browser_safe_image_asset(&asset).await))
}

async fn browser_safe_image_asset(asset: &str) -> String {
    let Some(path) = asset.strip_prefix("asset://localhost/") else {
        return asset.to_owned();
    };
    let decoded = percent_encoding::percent_decode_str(path).decode_utf8_lossy();
    let normalized = asset_url_path_to_platform_path(&decoded);
    let path = std::path::Path::new(normalized.as_ref());
    let media_type = path
        .extension()
        .and_then(|e| e.to_str())
        .map_or("application/octet-stream", image_ext_to_mime);
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
            format!("data:{media_type};base64,{b64}")
        }
        Err(e) => {
            tracing::warn!(
                target: "cdt_api::http",
                error = %e,
                path = %path.display(),
                "failed to read image asset file for browser response"
            );
            "data:application/octet-stream;base64,".to_owned()
        }
    }
}

fn asset_url_path_to_platform_path(path: &str) -> std::borrow::Cow<'_, str> {
    #[cfg(windows)]
    {
        std::borrow::Cow::Owned(path.replace('/', "\\"))
    }
    #[cfg(not(windows))]
    {
        std::borrow::Cow::Borrowed(path)
    }
}

fn image_ext_to_mime(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

async fn get_tool_output(
    State(s): State<AppState>,
    Path((root_session_id, session_id, tool_use_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let output = s
        .api
        .get_tool_output(&root_session_id, &session_id, &tool_use_id)
        .await?;
    Ok(Json(output))
}

async fn add_trigger(
    State(s): State<AppState>,
    Json(trigger): Json<cdt_config::NotificationTrigger>,
) -> Result<impl IntoResponse, ApiError> {
    let config = s.api.add_trigger(trigger).await?;
    Ok(Json(config))
}

async fn remove_trigger(
    State(s): State<AppState>,
    Path(trigger_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let config = s.api.remove_trigger(&trigger_id).await?;
    Ok(Json(config))
}

async fn pin_session(
    State(s): State<AppState>,
    Path((project_id, session_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    s.api.pin_session(&project_id, &session_id).await?;
    Ok(Json(serde_json::json!({"success": true})))
}

async fn unpin_session(
    State(s): State<AppState>,
    Path((project_id, session_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    s.api.unpin_session(&project_id, &session_id).await?;
    Ok(Json(serde_json::json!({"success": true})))
}

async fn hide_session(
    State(s): State<AppState>,
    Path((project_id, session_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    s.api.hide_session(&project_id, &session_id).await?;
    Ok(Json(serde_json::json!({"success": true})))
}

async fn unhide_session(
    State(s): State<AppState>,
    Path((project_id, session_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    s.api.unhide_session(&project_id, &session_id).await?;
    Ok(Json(serde_json::json!({"success": true})))
}

async fn get_project_session_prefs(
    State(s): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let prefs = s.api.get_project_session_prefs(&project_id).await?;
    Ok(Json(prefs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn browser_safe_image_asset_converts_asset_url_to_data_uri() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("image.png");
        tokio::fs::write(&path, [1_u8, 2, 3]).await.unwrap();
        let asset = format!("asset://localhost/{}", path.display());

        let result = browser_safe_image_asset(&asset).await;

        assert_eq!(result, "data:image/png;base64,AQID");
    }

    #[tokio::test]
    async fn browser_safe_image_asset_decodes_url_path() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("space image.PNG");
        tokio::fs::write(&path, [1_u8, 2, 3]).await.unwrap();
        let raw_path = path.to_string_lossy().replace(' ', "%20");
        let asset = format!("asset://localhost/{raw_path}");

        let result = browser_safe_image_asset(&asset).await;

        assert_eq!(result, "data:image/png;base64,AQID");
    }

    #[tokio::test]
    async fn browser_safe_image_asset_keeps_data_uri() {
        let data = "data:image/png;base64,AQID";
        assert_eq!(browser_safe_image_asset(data).await, data);
    }

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
