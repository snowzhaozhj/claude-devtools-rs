use std::sync::Arc;

use cdt_api::{ConfigUpdateRequest, DataApi, LocalDataApi, PaginatedRequest, SearchRequest};
use cdt_config::{ConfigManager, NotificationManager, NotificationTrigger};
use cdt_discover::{local_handle, path_decoder, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use cdt_watch::FileWatcher;
use tauri::{Emitter, Manager, State};

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
async fn get_session_detail(
    data: State<'_, AppData>,
    project_id: String,
    session_id: String,
) -> Result<serde_json::Value, String> {
    let detail = data
        .api
        .get_session_detail(&project_id, &session_id)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&detail).map_err(|e| e.to_string())
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
    data.api
        .search(&request)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_config(data: State<'_, AppData>) -> Result<serde_json::Value, String> {
    data.api.get_config().await.map_err(|e| e.to_string())
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
    data.api
        .update_config(&request)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_notifications(
    data: State<'_, AppData>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<serde_json::Value, String> {
    data.api
        .get_notifications(limit.unwrap_or(50), offset.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let (api, watcher) = rt.block_on(async {
        let mut config_mgr = ConfigManager::new(None);
        let _ = config_mgr.load().await;

        let mut notif_mgr = NotificationManager::new(None);
        let _ = notif_mgr.load().await;

        let fs = local_handle();
        let projects_dir = path_decoder::get_projects_base_path();
        let scanner = ProjectScanner::new(fs, projects_dir.clone());
        let ssh_mgr = SshConnectionManager::new();

        let watcher = Arc::new(FileWatcher::new());
        let api = Arc::new(LocalDataApi::new_with_watcher(
            scanner,
            config_mgr,
            notif_mgr,
            ssh_mgr,
            watcher.as_ref(),
            projects_dir,
        ));

        (api, watcher)
    });

    tauri::Builder::default()
        .manage(AppData { api: api.clone() })
        .setup(move |app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
                // 打开 WebView devtools 供调试
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            // 启动 FileWatcher：监听 `~/.claude/projects/` + `~/.claude/todos/`，
            // 将 file 变更广播给自动通知管线
            let watcher_for_task = watcher.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = watcher_for_task.start().await {
                    log::warn!("FileWatcher terminated: {err}");
                }
            });

            // 把自动通知管线产出的 DetectedError 桥到前端 `notification-added` 事件
            let mut error_rx = api.subscribe_detected_errors();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match error_rx.recv().await {
                        Ok(err) => {
                            let _ = app_handle.emit("notification-added", &err);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            list_sessions,
            get_session_detail,
            search_sessions,
            get_config,
            update_config,
            get_notifications,
            mark_notification_read,
            add_trigger,
            remove_trigger,
            read_agent_configs,
            pin_session,
            unpin_session,
            hide_session,
            unhide_session,
            get_project_session_prefs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
