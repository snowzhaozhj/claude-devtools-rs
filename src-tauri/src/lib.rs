use std::sync::Arc;

use cdt_api::{DataApi, LocalDataApi, PaginatedRequest, SearchRequest};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{local_handle, path_decoder, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use tauri::State;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let api = rt.block_on(async {
        let mut config_mgr = ConfigManager::new(None);
        let _ = config_mgr.load().await;

        let mut notif_mgr = NotificationManager::new(None);
        let _ = notif_mgr.load().await;

        let fs = local_handle();
        let projects_dir = path_decoder::get_projects_base_path();
        let scanner = ProjectScanner::new(fs, projects_dir);
        let ssh_mgr = SshConnectionManager::new();

        Arc::new(LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr))
    });

    tauri::Builder::default()
        .manage(AppData { api })
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            list_sessions,
            get_session_detail,
            search_sessions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
