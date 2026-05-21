//! claude-devtools-rs binary entrypoint.
//!
//! 初始化各 manager → 构造带 `FileWatcher` 的 `LocalDataApi` → spawn watcher
//! 与 SSE event bridge → 启动 HTTP server。

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::Semaphore;

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
    // change `simplify-repository-as-project::D4`：生产代码 SHALL 用
    // `new_with_semaphore` 注入共享 semaphore，避免 N 个 scanner × 64 fd 击穿。
    // CLI 此处仅创建 1 个 scanner，但保持 spec 约束以便 build-time grep 拦回归。
    let scanner_semaphore = Arc::new(Semaphore::new(64));
    let scanner = ProjectScanner::new_with_semaphore(fs, projects_dir.clone(), scanner_semaphore);

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

    // 与 src-tauri/server_mode 保持一致：page_size=50 默认 × 多 SSE
    // subscriber 时 256 容量易被打满（codex 二审 issue 2）。1024 给约 20×
    // headroom；仍 lag 时由 SSE handler 的 `sse_lagged` sentinel 兜底。
    let state = AppState::new(api, 1024);

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
