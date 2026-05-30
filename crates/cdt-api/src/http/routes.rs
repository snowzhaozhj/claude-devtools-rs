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

use crate::ipc::traits::CorrectnessEventItem;
use crate::ipc::{
    ApiError, ApiErrorCode, ConfigUpdateRequest, PaginatedRequest, SearchRequest, SshConnectRequest,
};

use super::StaticServe;
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
            // SshError + ExternalApp 都属"依赖外部资源失败"——SSH spawn / editor /
            // terminal CLI 缺失或 OS 拒绝；BAD_GATEWAY 比 INTERNAL_SERVER_ERROR 更准。
            // 前端按 message 弹 toast 引导用户去 Settings 修。
            ApiErrorCode::SshError | ApiErrorCode::ExternalApp => StatusCode::BAD_GATEWAY,
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

/// 构建完整的 `/api` 路由 + 可选静态文件 serve / redirect。
///
/// `static_serve` 决定未命中 `/api/*` 的请求如何处理（详 `StaticServe` doc）：
/// - `None`：直接 404
/// - `Dir(p)`：SPA `ServeDir` fallback；路径无效仅 `tracing::warn!` 后退化为 `None`
/// - `Redirect(base)`：HTTP 307 重定向到 `<base><path>?<query>&http=1&apiBase=...`
///
/// CORS：所有路由统一 layer `localhost_cors_layer`，仅放行 localhost / 127.0.0.1
/// origin（详 `crate::http::cors`）。
pub fn build_router(state: AppState, static_serve: StaticServe) -> Router {
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
        .route(
            "/api/repository-groups/{group_id}/sessions",
            get(list_group_sessions),
        )
        .route(
            "/api/repository-groups/{group_id}/search",
            post(search_group_sessions),
        )
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
        // Telemetry (cdt-telemetry Phase 1)
        .route("/api/telemetry/snapshot", get(get_telemetry_snapshot_route))
        .route(
            "/api/telemetry/correctness-events",
            post(record_correctness_events_route),
        )
        // Phase 2 frontend-context-menu：右键菜单"在终端 / 编辑器打开"+ Settings dropdown
        // 详 openspec/specs/frontend-context-menu/spec.md 三个 Requirement
        .route("/api/external-app/terminal", post(open_in_terminal_route))
        .route("/api/external-app/editor", post(open_in_editor_route))
        .route(
            "/api/external-app/terminals",
            get(list_available_terminals_route),
        )
        // SSE
        .route("/api/events", get(sse_handler))
        .with_state(state);

    // 顺序：CORS layer 在最外层（先看 Origin）；fallback 按 StaticServe 选分支：
    // - Dir 走 SPA static_fallback；路径无效退化为 None
    // - Redirect 走 redirect_to_upstream（dev 重定向浏览器到 vite）
    // - None 不挂 fallback，未命中 `/api/*` 自然 404（与本 change 之前行为一致）
    let router_with_static = match static_serve {
        StaticServe::Dir(p) if p.is_dir() => {
            // 用 closure + Arc::clone 捕获 dir，避免引入第二个 with_state 与
            // api_router 已设的 AppState 冲突（axum 0.8 一个 Router 只能持有
            // 单一 State 类型）。
            let dir = Arc::new(p);
            api_router.fallback(move |uri: axum::http::Uri| {
                let dir = Arc::clone(&dir);
                async move { static_fallback(dir, uri).await }
            })
        }
        StaticServe::Dir(p) => {
            tracing::warn!(
                target: "cdt_api::http",
                path = %p.display(),
                "static_dir does not exist or is not a directory; serving /api/* only"
            );
            api_router
        }
        StaticServe::Redirect(base) => {
            let base = Arc::new(base);
            api_router.fallback(
                move |uri: axum::http::Uri, headers: axum::http::HeaderMap| {
                    let base = Arc::clone(&base);
                    async move { redirect_to_upstream(&base, &uri, &headers) }
                },
            )
        }
        StaticServe::None => api_router,
    };

    router_with_static.layer(localhost_cors_layer())
}

/// dev 模式下把非 `/api/*` 的请求 307 重定向到 vite dev server 的同路径，并
/// 自动追加 `http=1` + `apiBase=<incoming origin>` query。
///
/// - `http=1` 触发前端 `main.ts` 走 `BrowserTransport`
/// - `apiBase` 让前端 `getServerBaseUrl()` 锁定**真实 API server origin**——
///   server-mode 用户改端口（如启动到 `:4000`）时不会因 vite proxy 写死的
///   `:3456` target 错连（codex PR review 必修）
///
/// `base` 例如 `http://127.0.0.1:5173`。path / query 透传，避免破坏 vite HMR
/// 自动建链路径（`/@vite/client` 等）。**path traversal 不做 guard**——redirect
/// handler 本身不读本地文件，仅把 path 反映到 Location header 让浏览器跳到
/// 上游 vite，由 vite 自行决定如何处理。
///
/// 用 `Redirect::temporary`（307）而非 302：保留请求方法对非 GET fallback 安全；
/// 实际 HMR / nav 都是 GET，行为等价。
fn redirect_to_upstream(
    base: &str,
    uri: &axum::http::Uri,
    headers: &axum::http::HeaderMap,
) -> axum::response::Response {
    let path = uri.path();
    let mut parts: Vec<String> = Vec::new();
    if let Some(q) = uri.query() {
        parts.push(q.to_string());
    }
    if !parts.iter().any(|p| p.split('&').any(|kv| kv == "http=1")) {
        parts.push("http=1".to_string());
    }
    if let Some(api_base) = api_base_from_host(headers)
        && !parts
            .iter()
            .any(|p| p.split('&').any(|kv| kv.starts_with("apiBase=")))
    {
        parts.push(format!(
            "apiBase={}",
            percent_encoding::utf8_percent_encode(&api_base, percent_encoding::NON_ALPHANUMERIC)
        ));
    }
    let merged_query = parts.join("&");
    let target = if merged_query.is_empty() {
        format!("{base}{path}")
    } else {
        format!("{base}{path}?{merged_query}")
    };
    axum::response::Redirect::temporary(&target).into_response()
}

/// 从 `Host` header 推 server origin（HTTP server 走 plain HTTP，监听
/// 127.0.0.1）。Host 缺失 / 非 localhost 形态返 `None`——前端 fallback
/// `window.location.origin`（vite domain）+ vite proxy 默认 target 仍能
///服务默认 `:3456` 场景。
///
/// 形态白名单：`localhost[:port]` / `127.0.0.1[:port]` / IPv6 `[::1][:port]`。
/// 拒绝带路径 / 字母端口 / 其它 host 防御 Host header 注入污染前端 base URL
/// 拼接（codex CR 第 3 点）。
fn api_base_from_host(headers: &axum::http::HeaderMap) -> Option<String> {
    let host = headers.get(axum::http::header::HOST)?.to_str().ok()?;
    if !is_localhost_host(host) {
        return None;
    }
    Some(format!("http://{host}"))
}

fn is_localhost_host(host: &str) -> bool {
    if host.is_empty() || host.contains('/') || host.contains('?') || host.contains('#') {
        return false;
    }
    let (hostname, port_part) = if let Some(rest) = host.strip_prefix('[') {
        // IPv6 literal: `[::1]` / `[::1]:port`
        let Some(close) = rest.find(']') else {
            return false;
        };
        let bracket = &rest[..close];
        let after = &rest[close + 1..];
        if after.is_empty() {
            (bracket, None)
        } else if let Some(port) = after.strip_prefix(':') {
            (bracket, Some(port))
        } else {
            return false;
        }
    } else {
        match host.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (host, None),
        }
    };
    if !matches!(hostname, "localhost" | "127.0.0.1" | "::1") {
        return false;
    }
    if let Some(p) = port_part
        && (p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return false;
    }
    true
}

#[cfg(test)]
mod host_validation_tests {
    use super::is_localhost_host;

    #[test]
    fn allows_localhost_variants() {
        assert!(is_localhost_host("localhost"));
        assert!(is_localhost_host("localhost:3456"));
        assert!(is_localhost_host("127.0.0.1"));
        assert!(is_localhost_host("127.0.0.1:4000"));
        assert!(is_localhost_host("[::1]"));
        assert!(is_localhost_host("[::1]:8080"));
    }

    #[test]
    fn rejects_other_hosts() {
        assert!(!is_localhost_host(""));
        assert!(!is_localhost_host("evil.com"));
        assert!(!is_localhost_host("localhost.evil.com"));
        assert!(!is_localhost_host("8.8.8.8:3456"));
        assert!(!is_localhost_host("0.0.0.0:3456"));
    }

    #[test]
    fn rejects_malformed() {
        assert!(!is_localhost_host("localhost/path"));
        assert!(!is_localhost_host("localhost?x=1"));
        assert!(!is_localhost_host("localhost:abc"));
        assert!(!is_localhost_host("localhost:"));
    }
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
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, ApiError> {
    let project_id = s
        .api
        .find_session_project(&session_id)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("session {session_id}")))?;
    let known_fp = params.get("fingerprint").map(String::as_str);
    let result = s
        .api
        .get_session_detail(&project_id, &session_id, known_fp)
        .await?;
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

async fn search_group_sessions(
    State(s): State<AppState>,
    Path(group_id): Path<String>,
    Json(body): Json<SearchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s.api.search_group_sessions(&group_id, &body.query).await?;
    Ok(Json(result))
}

async fn get_config(State(s): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let (config, version) = s.api.get_config_versioned().await?;
    let mut value = serde_json::to_value(&config)
        .map_err(|e| ApiError::internal(format!("serialize config: {e}")))?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert("_version".to_string(), serde_json::Value::from(version));
    }
    Ok(Json(value))
}

async fn update_config(
    State(s): State<AppState>,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let (config, version) = s.api.update_config_versioned(&request).await?;
    let mut value = serde_json::to_value(&config)
        .map_err(|e| ApiError::internal(format!("serialize config: {e}")))?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert("_version".to_string(), serde_json::Value::from(version));
    }
    Ok(Json(value))
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
    #[serde(alias = "context_id")]
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
    #[serde(alias = "context_id")]
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

/// change `simplify-repository-as-project::D3`：k-way merge cursor 分页桥接。
///
/// Server-mode 远端 UI 必须能拿到 group 维度的 sessions 合并视图，否则切到
/// `RepositoryGroup` 入口后无法走 `list_group_sessions` IPC（远端没 Tauri runtime）
/// 只能 fallback `get_worktree_sessions`（单 worktree，丢 group merge 语义）。
/// `pageSize` 与 `cursor` 走 query string。
async fn list_group_sessions(
    State(s): State<AppState>,
    Path(group_id): Path<String>,
    Query(pagination): Query<PaginatedRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let result = s
        .api
        .list_group_sessions(
            &group_id,
            pagination.page_size,
            pagination.cursor.as_deref(),
        )
        .await?;
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

async fn get_telemetry_snapshot_route(
    State(s): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let snap = s.api.get_telemetry_snapshot().await?;
    Ok(Json(snap))
}

#[derive(serde::Deserialize)]
struct RecordCorrectnessEventsRequest {
    items: Vec<CorrectnessEventItem>,
}

async fn record_correctness_events_route(
    State(s): State<AppState>,
    Json(payload): Json<RecordCorrectnessEventsRequest>,
) -> Result<impl IntoResponse, ApiError> {
    s.api.record_correctness_events(payload.items).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

// =============================================================================
// Phase 2 frontend-context-menu：外部应用交互 HTTP 镜像
// 详 openspec/specs/frontend-context-menu/spec.md 三个 Requirement
// =============================================================================

#[derive(serde::Deserialize)]
struct OpenInTerminalRequest {
    path: String,
}

async fn open_in_terminal_route(
    State(s): State<AppState>,
    Json(payload): Json<OpenInTerminalRequest>,
) -> Result<impl IntoResponse, ApiError> {
    s.api.open_in_terminal(&payload.path).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenInEditorRequest {
    path: String,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    column: Option<u32>,
}

async fn open_in_editor_route(
    State(s): State<AppState>,
    Json(payload): Json<OpenInEditorRequest>,
) -> Result<impl IntoResponse, ApiError> {
    s.api
        .open_in_editor(&payload.path, payload.line, payload.column)
        .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn list_available_terminals_route(
    State(s): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let list = s.api.list_available_terminals().await?;
    Ok(Json(list))
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
