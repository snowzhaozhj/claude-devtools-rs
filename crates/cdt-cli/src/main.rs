//! claude-devtools-rs binary entrypoint.
//!
//! 初始化各 manager → 构造带 `FileWatcher` 的 `LocalDataApi` → spawn watcher
//! 与 SSE event bridge → 启动 HTTP server。

use std::path::PathBuf;

use anyhow::{Context, Result};

use cdt_api::http::spawn_event_bridge;
use cdt_api::{AppState, LocalDataApi, start_server};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{ProjectScanner, local_handle, path_decoder};
use cdt_ssh::SshConnectionManager;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // 加载配置
    let mut config_mgr = ConfigManager::new(None);
    config_mgr.load().await.context("failed to load config")?;

    let port = config_mgr.get_config().http_server.port;

    // 加载通知
    let mut notif_mgr = NotificationManager::new(None);
    notif_mgr
        .load()
        .await
        .context("failed to load notifications")?;

    let fs = local_handle();
    let projects_dir = path_decoder::projects_base_path_for(
        config_mgr
            .get_config()
            .general
            .claude_root_path
            .as_deref()
            .map(PathBuf::from)
            .as_deref(),
    );
    let todos_dir = path_decoder::todos_base_path_for(
        config_mgr
            .get_config()
            .general
            .claude_root_path
            .as_deref()
            .map(PathBuf::from)
            .as_deref(),
    );
    let scanner = ProjectScanner::new(fs, projects_dir.clone());

    // SSH manager
    let ssh_mgr = SshConnectionManager::new();

    // 组装带 watcher 的 LocalDataApi（自动通知管线接通）
    let api = LocalDataApi::new_with_watcher(
        scanner,
        config_mgr,
        notif_mgr,
        ssh_mgr,
        &cdt_watch::FileWatcher::with_paths(projects_dir.clone(), todos_dir),
        projects_dir,
    );

    let api = std::sync::Arc::new(api);
    let file_rx = api.subscribe_file_changes();
    let todo_rx = api.subscribe_todo_changes();
    let error_rx = api.subscribe_detected_errors();
    let metadata_rx = api.subscribe_session_metadata();

    let state = AppState::new(api, 256);

    // 把 file / todo / detected-error / metadata 桥到 AppState.events_tx，供 SSE 推送
    spawn_event_bridge(
        state.events_tx.clone(),
        file_rx,
        todo_rx,
        error_rx,
        metadata_rx,
    );

    tracing::info!("Starting claude-devtools-rs on port {port}");
    // CLI 不挂静态文件 serve（CLI 只是 API server 用途；UI 走 Tauri runtime
    // 或浏览器打开 Tauri build 出的 bundle），传 None 让 router 仅 serve `/api/*`。
    start_server(state, port, None)
        .await
        .context("HTTP server failed")?;

    Ok(())
}
