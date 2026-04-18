use std::sync::Arc;

use cdt_api::{ConfigUpdateRequest, DataApi, LocalDataApi, PaginatedRequest, SearchRequest};
use cdt_config::{ConfigManager, NotificationManager, NotificationTrigger};
use cdt_discover::{local_handle, path_decoder, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use cdt_watch::FileWatcher;
use tauri::{
    Emitter, Manager, State,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_notification::NotificationExt;

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
        .plugin(tauri_plugin_notification::init())
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

            // 启动 FileWatcher：监听 `~/.claude/projects/` + `~/.claude/todos/`，
            // 将 file 变更广播给自动通知管线
            let watcher_for_task = watcher.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = watcher_for_task.start().await {
                    log::warn!("FileWatcher terminated: {err}");
                }
            });

            // 把 FileWatcher 的 FileChangeEvent 桥到前端 `file-change` 事件，
            // 让 SessionDetail 与 Sidebar 自动刷新。
            let mut file_rx = watcher.subscribe_files();
            let app_handle_for_files = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match file_rx.recv().await {
                        Ok(event) => {
                            let _ = app_handle_for_files.emit("file-change", &event);
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
                            let cfg = api_for_notif.get_config().await.ok();
                            let enabled = cfg
                                .as_ref()
                                .and_then(|c| c.get("notifications"))
                                .and_then(|n| n.get("enabled"))
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(true);
                            let sound_enabled = cfg
                                .as_ref()
                                .and_then(|c| c.get("notifications"))
                                .and_then(|n| n.get("soundEnabled"))
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(true);
                            let snoozed_until = cfg
                                .as_ref()
                                .and_then(|c| c.get("notifications"))
                                .and_then(|n| n.get("snoozedUntil"))
                                .and_then(serde_json::Value::as_i64);
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
