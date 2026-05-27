mod server_mode;

use std::sync::Arc;

use cdt_api::{
    ConfigUpdateRequest, DataApi, LocalDataApi, PaginatedRequest, SearchRequest, SshConnectRequest,
};
use cdt_config::{ConfigManager, NotificationManager, NotificationTrigger};
use cdt_discover::{ProjectScanner, local_handle, path_decoder};
use cdt_ssh::SshConnectionManager;
use cdt_watch::FileWatcher;
use tauri::{
    Emitter, Manager, RunEvent, State,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

use server_mode::{ServerState, ServerStatus};

struct AppData {
    api: Arc<LocalDataApi>,
}

#[tauri::command]
async fn list_projects(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    let projects = data.api.list_projects().await.map_err(|e| e.to_string())?;
    serde_json::to_value(&projects).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_sessions(
    data: State<'_, AppData>,
    project_id: String,
    page_size: Option<usize>,
    cursor: Option<String>,
) -> Result<serde_json::Value, String> {
    let pagination = PaginatedRequest {
        page_size: page_size.unwrap_or(50),
        cursor,
    };
    let result = data
        .api
        .list_sessions(&project_id, &pagination)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_session_summaries_by_ids(
    data: State<'_, AppData>,
    project_id: String,
    session_ids: Vec<String>,
) -> Result<serde_json::Value, String> {
    let summaries = data
        .api
        .get_session_summaries_by_ids(&project_id, &session_ids)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&summaries).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_session_detail(
    data: State<'_, AppData>,
    project_id: String,
    session_id: String,
    known_fingerprint: Option<String>,
) -> Result<serde_json::Value, String> {
    let resp = data
        .api
        .get_session_detail(
            &project_id,
            &session_id,
            known_fingerprint.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&resp).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_project_memory(
    data: State<'_, AppData>,
    project_id: String,
) -> Result<serde_json::Value, String> {
    let memory = data
        .api
        .get_project_memory(&project_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&memory).map_err(|e| e.to_string())
}

#[tauri::command]
async fn read_memory_file(
    data: State<'_, AppData>,
    project_id: String,
    file: String,
) -> Result<serde_json::Value, String> {
    let content = data
        .api
        .read_memory_file(&project_id, &file)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&content).map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_memory(
    data: State<'_, AppData>,
    project_id: String,
    file: String,
    content: String,
) -> Result<serde_json::Value, String> {
    let memory = data
        .api
        .add_memory(&project_id, &file, &content)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&memory).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_memory(
    data: State<'_, AppData>,
    project_id: String,
    file: String,
) -> Result<serde_json::Value, String> {
    let memory = data
        .api
        .delete_memory(&project_id, &file)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&memory).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_subagent_trace(
    data: State<'_, AppData>,
    root_session_id: String,
    subagent_session_id: String,
) -> Result<serde_json::Value, String> {
    // change `typed-ipc-payload`：trait 返回 typed `Vec<Chunk>`，Tauri command
    // 仍 wrap 为 `serde_json::Value` 透传（wire 形状不变；前端 typed 由
    // `ui/src/lib/api.ts` 端独立保证）。
    let chunks = data
        .api
        .get_subagent_trace(&root_session_id, &subagent_session_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&chunks).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_image_asset(
    data: State<'_, AppData>,
    root_session_id: String,
    session_id: String,
    block_id: String,
) -> Result<String, String> {
    data.api
        .get_image_asset(&root_session_id, &session_id, &block_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_tool_output(
    data: State<'_, AppData>,
    root_session_id: String,
    session_id: String,
    tool_use_id: String,
) -> Result<serde_json::Value, String> {
    let output = data
        .api
        .get_tool_output(&root_session_id, &session_id, &tool_use_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&output).map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_sessions(
    data: State<'_, AppData>,
    project_id: String,
    query: String,
) -> Result<serde_json::Value, String> {
    let request = SearchRequest {
        query,
        project_id: Some(project_id),
        session_id: None,
    };
    let result = data
        .api
        .search(&request)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_config(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    let config = data.api.get_config().await.map_err(|e| e.to_string())?;
    let version = data.api.config_version().await.map_err(|e| e.to_string())?;
    let mut value = serde_json::to_value(&config).map_err(|e| e.to_string())?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert("_version".to_string(), serde_json::Value::from(version));
    }
    Ok(value)
}

#[tauri::command]
async fn update_config(
    data: State<'_, AppData>,
    section: String,
    config_data: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let request = ConfigUpdateRequest {
        section,
        data: config_data,
    };
    let config = data
        .api
        .update_config(&request)
        .await
        .map_err(|e| e.to_string())?;
    let version = data.api.config_version().await.map_err(|e| e.to_string())?;
    let mut value = serde_json::to_value(&config).map_err(|e| e.to_string())?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert("_version".to_string(), serde_json::Value::from(version));
    }
    Ok(value)
}

#[tauri::command]
async fn get_notifications(
    data: State<'_, AppData>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<serde_json::Value, String> {
    let result = data
        .api
        .get_notifications(limit.unwrap_or(50), offset.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn mark_notification_read(
    app: tauri::AppHandle,
    data: State<'_, AppData>,
    notification_id: String,
) -> Result<bool, String> {
    let result = data
        .api
        .mark_notification_read(&notification_id)
        .await
        .map_err(|e| e.to_string())?;
    // 通知前端刷新 badge
    let _ = app.emit("notification-update", ());
    Ok(result)
}

#[tauri::command]
async fn delete_notification(
    app: tauri::AppHandle,
    data: State<'_, AppData>,
    notification_id: String,
) -> Result<bool, String> {
    let removed = data
        .api
        .delete_notification(&notification_id)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit("notification-update", ());
    Ok(removed)
}

#[tauri::command]
async fn mark_all_notifications_read(
    app: tauri::AppHandle,
    data: State<'_, AppData>,
) -> Result<(), String> {
    data.api
        .mark_all_notifications_read()
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit("notification-update", ());
    Ok(())
}

#[tauri::command]
async fn clear_notifications(
    app: tauri::AppHandle,
    data: State<'_, AppData>,
    trigger_id: Option<String>,
) -> Result<usize, String> {
    let removed = data
        .api
        .clear_notifications(trigger_id.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit("notification-update", ());
    Ok(removed)
}

#[tauri::command]
async fn add_trigger(
    data: State<'_, AppData>,
    trigger: NotificationTrigger,
) -> Result<serde_json::Value, String> {
    data.api
        .add_trigger(trigger)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_trigger(
    data: State<'_, AppData>,
    trigger_id: String,
) -> Result<serde_json::Value, String> {
    data.api
        .remove_trigger(&trigger_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn read_agent_configs(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    let configs = data
        .api
        .read_agent_configs()
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&configs).map_err(|e| e.to_string())
}

#[tauri::command]
async fn ssh_connect(
    data: State<'_, AppData>,
    request: SshConnectRequest,
) -> Result<serde_json::Value, String> {
    tracing::info!(
        target: "cdt_tauri::ssh",
        host = %request.host,
        username = ?request.username,
        auth_method = ?request.auth_method,
        "ssh connect requested"
    );
    data.api
        .ssh_connect(&request)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn ssh_disconnect(data: State<'_, AppData>, context_id: String) -> Result<(), String> {
    data.api
        .ssh_disconnect(&context_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn ssh_test_connection(
    data: State<'_, AppData>,
    request: SshConnectRequest,
) -> Result<serde_json::Value, String> {
    tracing::info!(
        target: "cdt_tauri::ssh",
        host = %request.host,
        username = ?request.username,
        auth_method = ?request.auth_method,
        "ssh test connection requested"
    );
    data.api
        .ssh_test_connection(&request)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn ssh_get_state(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    data.api.ssh_get_state().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn ssh_get_config_hosts(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    data.api
        .ssh_get_config_hosts()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn ssh_resolve_host(
    data: State<'_, AppData>,
    alias: String,
) -> Result<serde_json::Value, String> {
    data.api
        .resolve_ssh_host(&alias)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn ssh_save_last_connection(
    data: State<'_, AppData>,
    request: SshConnectRequest,
) -> Result<serde_json::Value, String> {
    data.api
        .ssh_save_last_connection(&request)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn ssh_get_last_connection(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    data.api
        .ssh_get_last_connection()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_contexts(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    let contexts = data.api.list_contexts().await.map_err(|e| e.to_string())?;
    serde_json::to_value(contexts).map_err(|e| e.to_string())
}

#[tauri::command]
async fn switch_context(data: State<'_, AppData>, context_id: String) -> Result<(), String> {
    data.api
        .switch_context(&context_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_active_context(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    let context = data
        .api
        .get_active_context()
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(context).map_err(|e| e.to_string())
}

// =============================================================================
// Sidebar Pin/Hide 持久化
// =============================================================================

#[tauri::command]
async fn pin_session(
    data: State<'_, AppData>,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    data.api
        .pin_session(&project_id, &session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn unpin_session(
    data: State<'_, AppData>,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    data.api
        .unpin_session(&project_id, &session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn hide_session(
    data: State<'_, AppData>,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    data.api
        .hide_session(&project_id, &session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn unhide_session(
    data: State<'_, AppData>,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    data.api
        .unhide_session(&project_id, &session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_project_session_prefs(
    data: State<'_, AppData>,
    project_id: String,
) -> Result<serde_json::Value, String> {
    let prefs = data
        .api
        .get_project_session_prefs(&project_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&prefs).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_telemetry_snapshot(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    let snap = data
        .api
        .get_telemetry_snapshot()
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&snap).map_err(|e| e.to_string())
}

#[tauri::command]
async fn record_correctness_events(
    data: State<'_, AppData>,
    items: Vec<cdt_api::CorrectnessEventItem>,
) -> Result<serde_json::Value, String> {
    data.api
        .record_correctness_events(items)
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ok": true}))
}

// =============================================================================
// 外部应用交互（Phase 2 frontend-context-menu 右键菜单）
// 详 openspec/specs/frontend-context-menu/spec.md 三个 Requirement +
// openspec/changes/frontend-context-menu-phase-2/design.md::D1-D5
//
// 安全不变量：
// - command 入参 SHALL 不接受 shell command 字符串，仅接受 path / line / column
// - 后端从 ConfigManager 读 terminal_app / external_editor，前端**无法**指定任意程序
// - capabilities/default.json 无需新增条目（自定义 commands 默认对 default capability
//   下所有 windows 可用，Tauri 2 capabilities 仅管控 plugin 权限；详 design.md::D4）
// =============================================================================

#[tauri::command]
async fn open_in_terminal(data: State<'_, AppData>, path: String) -> Result<(), String> {
    data.api
        .open_in_terminal(&path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_in_editor(
    data: State<'_, AppData>,
    path: String,
    line: Option<u32>,
    column: Option<u32>,
) -> Result<(), String> {
    data.api
        .open_in_editor(&path, line, column)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_available_terminals(data: State<'_, AppData>) -> Result<Vec<String>, String> {
    data.api
        .list_available_terminals()
        .await
        .map_err(|e| e.to_string())
}

// =============================================================================
// Repository Groups / Worktree Sessions
// =============================================================================

#[tauri::command]
async fn list_repository_groups(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    let groups = data
        .api
        .list_repository_groups()
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&groups).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_worktree_sessions(
    data: State<'_, AppData>,
    group_id: String,
    page_size: Option<usize>,
    cursor: Option<String>,
) -> Result<serde_json::Value, String> {
    let pagination = PaginatedRequest {
        page_size: page_size.unwrap_or(50),
        cursor,
    };
    let result = data
        .api
        .get_worktree_sessions(&group_id, &pagination)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_group_sessions(
    data: State<'_, AppData>,
    group_id: String,
    page_size: Option<usize>,
    cursor: Option<String>,
) -> Result<serde_json::Value, String> {
    let result = data
        .api
        .list_group_sessions(&group_id, page_size.unwrap_or(50), cursor.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

// =============================================================================
// WSL Distro Discovery (Windows 平台)
// =============================================================================

#[tauri::command]
async fn list_wsl_distros(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    let report = data
        .api
        .list_wsl_distros()
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&report).map_err(|e| e.to_string())
}

// =============================================================================
// Server-mode 控制（详 openspec/specs/server-mode/spec.md）
// =============================================================================

#[tauri::command]
async fn http_server_start(state: State<'_, Arc<ServerState>>, port: u16) -> Result<(), String> {
    state.start(port).await
}

#[tauri::command]
async fn http_server_stop(state: State<'_, Arc<ServerState>>) -> Result<(), String> {
    state.stop().await
}

#[tauri::command]
async fn http_server_status(state: State<'_, Arc<ServerState>>) -> Result<ServerStatus, String> {
    Ok(state.status().await)
}

// =============================================================================
// Auto Updater
// =============================================================================

/// 手动检查更新 IPC 返回结构。
///
/// 与 spec `app-auto-update::Requirement: 手动检查更新 IPC` 对齐：
/// `status` 是 internally-tagged 的 enum tag，前端按 `result.status` switch。
#[derive(serde::Serialize)]
#[serde(
    tag = "status",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum CheckUpdateResult {
    UpToDate {
        current_version: String,
    },
    Available {
        current_version: String,
        new_version: String,
        notes: String,
        signature_ok: bool,
    },
    Error {
        message: String,
    },
}

/// macOS 上探测当前进程是否被 Rosetta 2 翻译执行。
///
/// 仅当 `sysctl.proc_translated` 返回 `1` 时认为正在 Rosetta 下；
/// 任何 I/O 失败 / 非 macOS 平台均返回 `false`（保守不打扰用户）。
///
/// 用 `std::process::Command` 调系统 `sysctl` 二进制是为了避免引入 `libc`
/// 依赖——本仓 src-tauri 已经避开 `libc` / `nix`，保持 Cargo 依赖最小化。
fn detect_rosetta_translation() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sysctl")
            .args(["-n", "sysctl.proc_translated"])
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.trim() == "1")
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// IPC：当前进程是否在 Rosetta 翻译下运行（Apple Silicon 上跑 x86_64 binary）。
///
/// 前端启动时调用一次：true 时提示用户改装 ARM 版本以获得原生性能。
/// 非 macOS 平台始终 `false`。
#[tauri::command]
fn is_running_under_rosetta() -> bool {
    detect_rosetta_translation()
}

/// 手动检查更新的整体超时——超过即返回友好的"网络超时"文案。
///
/// 默认 plugin-updater 内部 reqwest 没设上限，山区 / 弱网 / DNS 超时叠加可达 30s+，
/// 用户在设置页看到 spinner 转半天。8s 已经覆盖正常 GitHub release CDN 往返 + 双 DNS
/// 兜底；超过即放弃，让用户手动重试。
const CHECK_UPDATE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

/// 启动后台检查的整体超时——非阻塞用户操作，但避免无限挂着 task。
const STARTUP_UPDATE_CHECK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// 把 plugin-updater / reqwest 原始错误映射成对用户友好的中文文案。
///
/// 原始错误（含完整 URL / reqwest 内部链路）只写入 tracing，**不**返回给前端——
/// 截图反馈过完整 URL 直接 leak 到 banner 既不友好也暴露发行渠道细节。
fn friendly_update_error(raw: &str) -> &'static str {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("deadline")
        || lower.contains("operation timed out")
    {
        "网络超时，请稍后重试"
    } else if lower.contains("dns")
        || lower.contains("failed to lookup")
        || lower.contains("name resolution")
        || lower.contains("no such host")
    {
        "无法解析更新服务器域名，请检查网络"
    } else if lower.contains("connect")
        || lower.contains("connection")
        || lower.contains("network")
        || lower.contains("error sending request")
        || lower.contains("tls")
        || lower.contains("certificate")
    {
        "无法连接到更新服务器，请检查网络"
    } else if lower.contains("404") || lower.contains("not found") {
        "暂无可用版本信息"
    } else {
        "检查更新失败，请稍后重试"
    }
}

#[tauri::command]
async fn check_for_update(app: tauri::AppHandle) -> Result<CheckUpdateResult, String> {
    let current_version = app.package_info().version.to_string();
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(target: "cdt_tauri::updater", error = %e, "updater init failed");
            return Ok(CheckUpdateResult::Error {
                message: friendly_update_error(&e.to_string()).to_string(),
            });
        }
    };
    let result = tokio::time::timeout(CHECK_UPDATE_TIMEOUT, updater.check()).await;
    match result {
        Ok(Ok(Some(update))) => Ok(CheckUpdateResult::Available {
            current_version,
            new_version: update.version.clone(),
            notes: update.body.clone().unwrap_or_default(),
            signature_ok: true,
        }),
        Ok(Ok(None)) => Ok(CheckUpdateResult::UpToDate { current_version }),
        Ok(Err(e)) => {
            tracing::warn!(target: "cdt_tauri::updater", error = %e, "manual update check failed");
            Ok(CheckUpdateResult::Error {
                message: friendly_update_error(&e.to_string()).to_string(),
            })
        }
        Err(_) => {
            tracing::warn!(
                target: "cdt_tauri::updater",
                timeout_secs = CHECK_UPDATE_TIMEOUT.as_secs(),
                "manual update check timed out"
            );
            Ok(CheckUpdateResult::Error {
                message: "网络超时，请稍后重试".to_string(),
            })
        }
    }
}

/// 启动后台静默检查的实现。
///
/// 节拍：读 config gate → 调 `updater().check()` → 与 `skipped_update_version` 比 semver
/// → 命中跳过则 return；否则 emit `updater://available`。
/// 任意环节失败均静默吞掉（启动检查不打扰用户），仅 `tracing::warn!` 记录。
async fn run_startup_update_check(api: Arc<LocalDataApi>, app: tauri::AppHandle) {
    let cfg = match api.get_config().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                target: "cdt_tauri::updater",
                error = %e,
                "failed to read config for updater gate"
            );
            return;
        }
    };
    let auto_check = cfg.updater.auto_update_check_enabled;
    if !auto_check {
        tracing::debug!(
            target: "cdt_tauri::updater",
            "auto check disabled, skip startup check"
        );
        return;
    }
    let skipped_version = cfg.updater.skipped_update_version.clone();

    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(
                target: "cdt_tauri::updater",
                error = %e,
                "failed to acquire updater"
            );
            return;
        }
    };

    let check_outcome = tokio::time::timeout(STARTUP_UPDATE_CHECK_TIMEOUT, updater.check()).await;
    let check_outcome = match check_outcome {
        Ok(inner) => inner,
        Err(_) => {
            tracing::warn!(
                target: "cdt_tauri::updater",
                timeout_secs = STARTUP_UPDATE_CHECK_TIMEOUT.as_secs(),
                "startup update check timed out"
            );
            return;
        }
    };
    match check_outcome {
        Ok(Some(update)) => {
            // 与 skipped_version 比较：仅当解析为合法 semver 且新版本 ≤ 跳过版本时才跳过
            if let Some(skipped) = &skipped_version {
                if let (Ok(skipped_v), Ok(new_v)) = (
                    semver::Version::parse(skipped),
                    semver::Version::parse(&update.version),
                ) {
                    if new_v <= skipped_v {
                        tracing::info!(
                            target: "cdt_tauri::updater",
                            skipped_version = %skipped,
                            new_version = %update.version,
                            "new version skipped by user"
                        );
                        return;
                    }
                }
            }
            let payload = serde_json::json!({
                "currentVersion": app.package_info().version.to_string(),
                "newVersion": update.version,
                "notes": update.body.clone().unwrap_or_default(),
                "signatureOk": true,
            });
            let _ = app.emit("updater://available", payload);
        }
        Ok(None) => {
            tracing::debug!(
                target: "cdt_tauri::updater",
                current_version = %app.package_info().version,
                "no update available"
            );
        }
        Err(e) => {
            tracing::warn!(
                target: "cdt_tauri::updater",
                error = %e,
                "startup update check failed"
            );
        }
    }
}

/// 装一个 tracing_subscriber，含 EnvFilter + fmt 层 + cdt-telemetry 桥 layer。
///
/// 注意：EnvFilter 仅挂到 fmt layer（per-layer filter），TelemetryLayer 不受
/// `RUST_LOG` 过滤——否则 `RUST_LOG=cdt_api=error` 会让 cdt_api.warn event 永远
/// 到不了 telemetry，破坏 spec 契约（tracing layer 自动归类 ERROR + WARN）。
///
/// TelemetryLayer 走 per-layer `LevelFilter::WARN`：subscriber 分发层在 INFO/
/// DEBUG/TRACE event 路径直接 short-circuit，函数调用本身都不发起；layer 内部
/// `on_event` 仍保留 ERROR/WARN guard 作双保险（issue #255：v0.5.6 → v0.5.8 引入
/// 的 idle CPU 回归直接相关的修法之一）。Phase 2 若要监控 INFO event 需先调宽。
///
/// `init` 一次幂等；多次调用后续无效（tracing global subscriber 单例语义）。
fn install_tracing_subscriber() {
    use tracing_subscriber::Layer;
    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_filter(env_filter);
    let telemetry_layer = cdt_telemetry::TelemetryLayer::new().with_filter(LevelFilter::WARN);
    let _ = tracing_subscriber::registry()
        .with(fmt_layer)
        .with(telemetry_layer)
        .try_init();
}

/// 注册 panic hook：先 take 既有 hook（保留 Tauri/Tokio 默认行为），再用闭包包装：
/// 既有 hook → counter inc → critical channel push。
fn install_telemetry_panic_hook() {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        prev_hook(info);
        cdt_telemetry::counter!("panic.recovered").inc();
        let location = info.location().map_or_else(
            || "unknown".to_string(),
            |l| format!("{}:{}", l.file(), l.line()),
        );
        let msg = panic_payload_str(info.payload());
        let truncated_msg = if msg.chars().count() > 1024 {
            // 按字符边界截断（非 ASCII panic message 超 1024 bytes 用 byte 切会
            // 落在 UTF-8 中间触发 double-panic 把 telemetry 也丢掉）。
            let mut out: String = msg.chars().take(1024).collect();
            out.push_str("...(truncated)");
            out
        } else {
            msg
        };
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("unnamed").to_string();
        let ev = cdt_telemetry::Event {
            kind: "panic.recovered",
            ts_unix_ms: u64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis()),
            )
            .unwrap_or(0),
            fields: vec![
                cdt_telemetry::EventField::Str("location", location),
                cdt_telemetry::EventField::Str("panic_message", truncated_msg),
                cdt_telemetry::EventField::Str("thread_name", thread_name),
            ],
        };
        cdt_telemetry::registry().panic_events().push(ev);
    }));
}

fn panic_payload_str(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Telemetry：启动期一次性 init Registry + 读 CDT_TELEMETRY_ENABLED env。
    // 必须在 tracing_subscriber 装 layer 之前调（layer 内会 lookup Registry）。
    cdt_telemetry::init_registry();
    install_telemetry_panic_hook();
    install_tracing_subscriber();

    // 单一 multi-thread runtime + 显式 Tauri 共享：避免 lib.rs 自建 + Tauri 内置
    // `default_runtime` 双 runtime 并存（issue #257：sample 287 线程 / 10s 内 140 次
    // pthread_join 周期销毁）。
    //
    // - `worker_threads(4)`：base user/real ratio = 0.13-0.17（强 I/O-bound），4 worker
    //   已覆盖；未来若转 CPU-bound（chunk-building 大批解析）按 telemetry 实测调
    // - `max_blocking_threads(64)`：严格等于 `cdt_discover::project_scanner::FILE_READ_CONCURRENCY=64`。
    //   不能小：应用层 Semaphore 放行 64 个任务后每个走 `tokio::fs` 隐式 spawn_blocking，
    //   pool 容量 < 64 → permit 持有者在 blocking FIFO 排队 → 吞噬其他 waiter 的 permit
    //   → 反而引入隐性延迟。根治 fan-out 走 issue #262（显式有界队列替代隐式 spawn_blocking）
    // - `thread_keep_alive(60s)`：让 idle blocking 线程真正复用，消除 create/destroy 循环。
    //   Tokio 默认 10s 在 sidebar 切换 / metadata 扫描的 burst 节奏下持续制造短生命线程；
    //   60s 让 pool 跨多次切换复用。验收：warm 30s 内线程不会回落，75s 后才稳态
    // - `thread_stack_size(2 MiB)`：显式锁栈预算，避免平台/未来 tokio 默认变化
    // - `thread_name_fn`：每个 runtime 线程独立序号 `cdt-rt-N`，sample / Activity
    //   Monitor / `ps -M` 输出能数出 worker 0..3 + blocking 4..67 各做什么；同名 `cdt-rt`
    //   会丢失 287→~90 这类线程构成的诊断信号
    // - `enable_all()`：裸 Builder 默认 timer/IO 全 off，少这一行 `tokio::time::sleep` /
    //   `tokio::fs` 直接 panic（`Runtime::new()` 隐式 enable_all）
    //
    // 因果分两条不要混淆：
    // 1. `rt.block_on(...)` 进入 tokio context → 覆盖 cdt-api 初始化期裸 `tokio::spawn`
    //    （`tokio::spawn` 看 thread context，与 `tauri::async_runtime::set` 无关）
    // 2. `tauri::async_runtime::set(handle)` → 覆盖后续 `tauri::async_runtime::*` API，
    //    防 Tauri lazy-init 自己的 default runtime
    //
    // `set` 必须紧跟 `rt` 创建之后、任何 `tauri::async_runtime::*` 或
    // `tauri::Builder::default()` 之前——`set` 是 OnceLock，二次调用 panic；若 Tauri
    // 默认 runtime 已 lazy-init，`set` 也 panic。`rt` 必须活到 `.run(...)` 返回
    // （drop runtime 会让 set 注册的 handle 失效）。
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .max_blocking_threads(64)
        .thread_keep_alive(std::time::Duration::from_secs(60))
        .thread_stack_size(2 * 1024 * 1024)
        .thread_name_fn(|| {
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let id = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            format!("cdt-rt-{id}")
        })
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");
    tauri::async_runtime::set(rt.handle().clone());

    let api = rt.block_on(async {
        let mut config_mgr = ConfigManager::new(None);
        let _ = config_mgr.load().await;

        let mut notif_mgr = NotificationManager::new(None);
        let _ = notif_mgr.load().await;

        let claude_root_path = config_mgr.get_config().general.claude_root_path.clone();
        let claude_root = claude_root_path.as_deref().map(std::path::Path::new);
        let fs = local_handle();
        let projects_dir = path_decoder::projects_base_path_for(claude_root);
        let todos_dir = path_decoder::todos_base_path_for(claude_root);
        let scanner = ProjectScanner::new(fs, projects_dir.clone());
        let ssh_mgr = SshConnectionManager::new();

        // phase 3 image asset cache：用 OS 标准 cache 目录 + app 子目录。
        // dirs::cache_dir() 同步且跨平台，无需 Tauri app handle。
        let image_cache_dir =
            dirs::cache_dir().map(|d| d.join("claude-devtools-rs").join("cdt-images"));
        let watcher = FileWatcher::with_paths(projects_dir.clone(), todos_dir);
        let mut api_inner = LocalDataApi::new_with_watcher(
            scanner,
            config_mgr,
            notif_mgr,
            ssh_mgr,
            &watcher,
            projects_dir,
        );
        if let Some(dir) = image_cache_dir {
            api_inner = api_inner.with_image_cache(dir);
        }
        Arc::new(api_inner)
    });

    let api_for_window_event = api.clone();
    let api_for_run_event = api.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppData { api: api.clone() })
        .setup({
            let api = api.clone();
            move |app| {
                #[cfg(debug_assertions)]
                {
                    // 注：不再注册 tauri_plugin_log——`tracing_subscriber::try_init()`
                    // 默认开 `tracing-log` feature，启动期已经把 `log::set_logger`
                    // 占走（LogTracer 转发到 tracing），二次 set 会 panic。
                    // `log::*` 宏的输出通过 LogTracer 桥接到 tracing fmt layer。
                    if let Some(window) = app.get_webview_window("main") {
                        window.open_devtools();
                    }
                }

                // 系统托盘：左键点击 toggle 主窗口；菜单 Show / Quit
                let show_item = MenuItemBuilder::with_id("show", "显示窗口").build(app)?;
                let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;
                let tray_menu = MenuBuilder::new(app)
                    .items(&[&show_item, &quit_item])
                    .build()?;
                let _tray = TrayIconBuilder::with_id("main-tray")
                    .icon(
                        app.default_window_icon()
                            .cloned()
                            .expect("app should have default icon"),
                    )
                    .tooltip("Claude DevTools")
                    .menu(&tray_menu)
                    .on_menu_event(|app, event| match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let is_visible = window.is_visible().unwrap_or(false);
                                if is_visible {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.unminimize();
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    })
                    .build(app)?;

                // 把 LocalDataApi 的稳定 FileChangeEvent 广播桥到前端 `file-change` 事件。
                // Claude root 运行时重配会重建内部 watcher，但此订阅保持有效。
                //
                // Lagged 路径 SHALL 显式 emit `sse-lagged` event 让前端 silent
                // refresh 兜底（change `enrich-file-change-with-session-list-changed::D6`）。
                // 原实现 `continue` 不通知前端，lag 期间错过的 structural 信号会让
                // `totalSessions` 滞后到 LOCAL_CACHE_TTL=5min 才被动恢复。形态与
                // HTTP `PushEvent::SseLagged` 对齐，前端 transport 走同一 handler。
                let mut file_rx = api.subscribe_file_changes();
                let app_handle_for_files = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        match file_rx.recv().await {
                            Ok(event) => {
                                let _ = app_handle_for_files.emit("file-change", &event);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                let _ = app_handle_for_files.emit(
                                    "sse-lagged",
                                    &serde_json::json!({
                                        "source": "file-change",
                                        "missed": n,
                                    }),
                                );
                                continue;
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                });

                let mut ssh_status_rx = api.subscribe_ssh_status();
                let app_handle_for_ssh_status = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        match ssh_status_rx.recv().await {
                            Ok(event) => {
                                let _ = app_handle_for_ssh_status.emit("ssh_status", &event);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                });

                let mut context_rx = api.subscribe_context_changed();
                let app_handle_for_context = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        match context_rx.recv().await {
                            Ok(event) => {
                                let _ = app_handle_for_context.emit("context_changed", &event);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                });

                // 把 list_sessions 后台元数据扫描的 SessionMetadataUpdate 桥到前端
                // `session-metadata-update` 事件，供 Sidebar 增量 patch 列表项。
                // 详见 openspec/specs/ipc-data-api/spec.md §"Emit session metadata updates"。
                let mut metadata_rx = api.subscribe_session_metadata();
                let app_handle_for_metadata = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        match metadata_rx.recv().await {
                            Ok(update) => {
                                let _ = app_handle_for_metadata
                                    .emit("session-metadata-update", &update);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                });

                // 启动 5 秒后台静默检查更新
                // 详见 openspec/specs/app-auto-update/spec.md `Requirement: 启动后台静默检查`
                let api_for_updater = api.clone();
                let app_handle_for_updater = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    run_startup_update_check(api_for_updater, app_handle_for_updater).await;
                });

                // 把自动通知管线产出的 DetectedError 桥到前端 `notification-added` 事件
                // 同时按 config.notifications.{enabled,soundEnabled} 发 OS native 通知
                let mut error_rx = api.subscribe_detected_errors();
                let app_handle = app.handle().clone();
                let api_for_notif = api.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        match error_rx.recv().await {
                            Ok(err) => {
                                let _ = app_handle.emit("notification-added", &err);

                                // 读最新 config 判断是否发 OS 通知
                                // change `typed-ipc-payload`：get_config 返回
                                // typed AppConfig，直接 field 访问取代 JSON path。
                                let cfg = api_for_notif.get_config().await.ok();
                                let enabled = cfg
                                    .as_ref()
                                    .map(|c| c.notifications.enabled)
                                    .unwrap_or(true);
                                let sound_enabled = cfg
                                    .as_ref()
                                    .map(|c| c.notifications.sound_enabled)
                                    .unwrap_or(true);
                                let snoozed_until = cfg
                                    .as_ref()
                                    .and_then(|c| c.notifications.snoozed_until);
                                let now_ms = i64::try_from(
                                    std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis(),
                                )
                                .unwrap_or(i64::MAX);
                                let snoozed = snoozed_until.is_some_and(|until| until > now_ms);

                                if enabled && !snoozed {
                                    let body: String = err.message.chars().take(200).collect();
                                    let mut builder = app_handle
                                        .notification()
                                        .builder()
                                        .title("Claude Code Error")
                                        .body(format!("[{}] {}", err.context.project_name, body));
                                    if sound_enabled {
                                        builder = builder.sound("default");
                                    }
                                    if let Err(e) = builder.show() {
                                        log::warn!("failed to show OS notification: {e}");
                                    }
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                });

                // server-mode：构建 ServerState + 自动恢复（详
                // openspec/specs/server-mode/spec.md::Tauri 桌面应用启动时 SHALL
                // 按持久化配置自动恢复 server）。
                //
                // static_serve 解析：dev mode 默认 Redirect 到 vite dev server 让浏览器
                // 与 Tauri 窗口共享同一份热重载 UI（见 `resolve_static_serve` doc）；
                // release 走 resource_dir ServeDir 提供完整 bundle。
                let static_serve = resolve_static_serve(app.handle());
                let server_state = Arc::new(ServerState::new(
                    api.clone(),
                    static_serve,
                    app.handle().clone(),
                ));
                app.manage(server_state.clone());

                let server_state_for_restore = server_state.clone();
                tauri::async_runtime::spawn(async move {
                    server_state_for_restore.restore_if_enabled().await;
                });

                Ok(())
            }
        })
        .on_window_event({
            move |_window, event| {
                if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                    let api = api_for_window_event.clone();
                    tauri::async_runtime::spawn(async move {
                        api.shutdown_ssh_all(cdt_ssh::SHUTDOWN_TIMEOUT).await;
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            list_sessions,
            get_session_summaries_by_ids,
            get_session_detail,
            get_project_memory,
            read_memory_file,
            add_memory,
            delete_memory,
            get_subagent_trace,
            get_image_asset,
            get_tool_output,
            search_sessions,
            get_config,
            update_config,
            get_notifications,
            mark_notification_read,
            delete_notification,
            mark_all_notifications_read,
            clear_notifications,
            add_trigger,
            remove_trigger,
            read_agent_configs,
            ssh_connect,
            ssh_disconnect,
            ssh_test_connection,
            ssh_get_state,
            ssh_get_config_hosts,
            ssh_resolve_host,
            ssh_save_last_connection,
            ssh_get_last_connection,
            list_contexts,
            switch_context,
            get_active_context,
            pin_session,
            unpin_session,
            hide_session,
            unhide_session,
            get_project_session_prefs,
            check_for_update,
            is_running_under_rosetta,
            list_repository_groups,
            get_worktree_sessions,
            list_group_sessions,
            list_wsl_distros,
            http_server_start,
            http_server_stop,
            http_server_status,
            get_telemetry_snapshot,
            record_correctness_events,
            open_in_terminal,
            open_in_editor,
            list_available_terminals,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(move |app_handle, event| {
            if let RunEvent::Exit = event {
                // 应用退出：abort 已运行的 HTTP server task 让 TCP listener
                // 立即释放（详 openspec/specs/server-mode/spec.md::Scenario
                // "应用退出时关闭 server"）。
                let api = api_for_run_event.clone();
                tauri::async_runtime::block_on(async move {
                    api.shutdown_ssh_all(cdt_ssh::SHUTDOWN_TIMEOUT).await;
                });
                if let Some(state) = app_handle.try_state::<Arc<ServerState>>() {
                    let state = state.inner().clone();
                    tauri::async_runtime::block_on(async move {
                        state.shutdown_runtime_only().await;
                    });
                }
            }
        });
}

/// 解析前端 bundle 静态文件目录，供 HTTP server static fallback 挂载。
///
/// **release 模式**：从 `resource_dir()` 取——`tauri.conf.json::build.frontendDist =
/// "../ui/dist"` 在 release 打包时被 tauri-build 拷贝到 resource_dir 根。各平台
/// 实测子路径见 task 6.3，必要时改成 `resource_dir().join("<subpath>")`。
/// **未来分叉风险**（codex D5）：若 tauri-action 改成嵌套子路径（如
/// `resource_dir().join("dist")`），dev 端基于 `<repo>/ui/dist` 的硬编码会与
/// release 分叉——届时**两个 `resolve_static_serve` 必须同时改**。另：以 resource
/// 根作为静态根会同时暴露同目录其他打包资源，加入非前端资源时应收窄静态根。
///
/// **dev 模式**（`cfg!(debug_assertions)`）：默认返回
/// `StaticServe::Redirect("http://127.0.0.1:5173")`——浏览器访问 Tauri 内置
/// HTTP server 的根路径会被 302 跳到 vite dev server，让浏览器和桌面 Tauri
/// 窗口共享同一份 HMR UI（消除"浏览器看 dist 旧 bundle / 桌面看 vite 新代码"
/// 的两端分叉）。`/api/*` / `/api/events` 仍由 axum 处理保证 HTTP 后端行为
/// 真实可测。
///
/// 设 `CDT_DEV_USE_PREBUILT_DIST=1` 切回 ServeDir(`<repo>/ui/dist`) 验证
/// release 形态（path traversal / mime 推断 / SPA fallback 真实链路）；缺失
/// `ui/dist` 时降级到 `None` + warn 引导跑 `pnpm --dir ui build`。
///
/// **worktree 路径绑定的是编译时源树**（codex D2）：`env!("CARGO_MANIFEST_DIR")`
/// 在编译期固定。正常 worktree 内 `cargo tauri dev` → 指向本 worktree 自己的
/// `ui/dist`，互相隔离没问题；但若复用主 checkout 编译产物在 worktree 跑，仍
/// serve 主 checkout 的旧 dist——排查 UI 不一致时记得 `which claude-devtools-tauri`
/// 看 binary 来源。
///
/// **`#[cfg(debug_assertions)]` 属性 gate 而非 `if cfg!(...)` 运行时判断**（codex
/// D1）：后者依赖 LLVM DCE 才能从 release `.rodata` 段剔除 `env!()` 编译期展开
/// 的开发机绝对路径字面量（如 `/Users/<dev>/.../src-tauri`）；属性 gate 让整段
/// 未启用分支不进 HIR/codegen，路径字面量**保证**不进入 release binary。`_app`
/// underscore-prefix 让 dev 构建里不触发 unused warning（release 构建里读
/// `_app.path()`）。
///
/// **分支选择按 `cfg(debug_assertions)` 不按 profile 名**（codex Q2）：若在
/// 自定义 cargo profile 里强制 `debug-assertions = true`（如 release-with-debug-
/// info 类配置），release-name profile 仍走 dev 路径——dev 字面量会进 binary。
/// 标准 `cargo build --release` 不受影响。**结果速查**：release binary 走
/// `resource_dir()` ServeDir；dev / debug-assertions=on 的 binary 默认走
/// Redirect to vite。
#[cfg(debug_assertions)]
fn resolve_static_serve(_app: &tauri::AppHandle) -> cdt_api::StaticServe {
    if std::env::var_os("CDT_DEV_USE_PREBUILT_DIST").is_some() {
        let src_tauri_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        return match resolve_dev_static_dir_from(&src_tauri_dir) {
            Some(p) => {
                tracing::info!(
                    target: "cdt_tauri::server_mode",
                    path = %p.display(),
                    "CDT_DEV_USE_PREBUILT_DIST set; dev mode serving ui/dist directly"
                );
                cdt_api::StaticServe::Dir(p)
            }
            None => {
                tracing::warn!(
                    target: "cdt_tauri::server_mode",
                    src_tauri_dir = %src_tauri_dir.display(),
                    "CDT_DEV_USE_PREBUILT_DIST set but ui/dist missing; serving /api/* only"
                );
                cdt_api::StaticServe::None
            }
        };
    }
    let upstream = "http://127.0.0.1:5173".to_string();
    tracing::info!(
        target: "cdt_tauri::server_mode",
        upstream = %upstream,
        "dev mode redirecting non-/api/* to vite dev server (set CDT_DEV_USE_PREBUILT_DIST=1 to test ui/dist)"
    );
    cdt_api::StaticServe::Redirect(upstream)
}

#[cfg(not(debug_assertions))]
fn resolve_static_serve(app: &tauri::AppHandle) -> cdt_api::StaticServe {
    match app.path().resource_dir() {
        Ok(dir) => cdt_api::StaticServe::Dir(dir),
        Err(e) => {
            tracing::warn!(
                target: "cdt_tauri::server_mode",
                error = %e,
                "failed to resolve resource_dir for static serve"
            );
            cdt_api::StaticServe::None
        }
    }
}

/// 从 src-tauri manifest dir 推 `<repo>/ui/dist`，仅当目标存在且是目录时返
/// `Some`。pure function——不读 env / 不打 log，让单测能注入 fixture root 验证
/// 三种形态（存在 / 不存在 / 是文件不是目录）。dev-only：release 构建里属性
/// gate 完全剔除（含 env!() 字面量），不进 codegen。
#[cfg(debug_assertions)]
fn resolve_dev_static_dir_from(src_tauri_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let dist = src_tauri_dir.parent()?.join("ui").join("dist");
    if dist.is_dir() { Some(dist) } else { None }
}

#[cfg(test)]
#[cfg(debug_assertions)]
mod tests {
    use super::resolve_dev_static_dir_from;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn dev_static_dir_resolves_when_ui_dist_is_a_directory() {
        let repo = TempDir::new().unwrap();
        let src_tauri = repo.path().join("src-tauri");
        let ui_dist = repo.path().join("ui").join("dist");
        fs::create_dir_all(&src_tauri).unwrap();
        fs::create_dir_all(&ui_dist).unwrap();

        let resolved = resolve_dev_static_dir_from(&src_tauri).unwrap();
        assert_eq!(resolved, ui_dist);
    }

    #[test]
    fn dev_static_dir_returns_none_when_ui_dist_missing() {
        let repo = TempDir::new().unwrap();
        let src_tauri = repo.path().join("src-tauri");
        fs::create_dir_all(&src_tauri).unwrap();
        // 故意不建 ui/dist——模拟用户没跑 `pnpm --dir ui build`

        assert!(resolve_dev_static_dir_from(&src_tauri).is_none());
    }

    #[test]
    fn dev_static_dir_returns_none_when_ui_dist_is_a_file() {
        let repo = TempDir::new().unwrap();
        let src_tauri = repo.path().join("src-tauri");
        let ui = repo.path().join("ui");
        fs::create_dir_all(&src_tauri).unwrap();
        fs::create_dir_all(&ui).unwrap();
        fs::write(ui.join("dist"), b"oops").unwrap();

        assert!(resolve_dev_static_dir_from(&src_tauri).is_none());
    }
}
