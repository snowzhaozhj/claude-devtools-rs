//! claude-devtools-rs binary entrypoint.
//!
//! 初始化各 manager → 构造带 `FileWatcher` 的 `LocalDataApi` → spawn watcher
//! 与 SSE event bridge → 启动 HTTP server。

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use cdt_api::http::spawn_event_bridge;
use cdt_api::{AppState, LocalDataApi, start_server};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{ProjectScanner, home_dir, local_handle, path_decoder};
use cdt_ssh::SshConnectionManager;
use cdt_watch::FileWatcher;

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

    // 解析 home 与监听目录。`cdt_discover::home_dir()` 走四级 fallback
    // (HOME → USERPROFILE → HOMEDRIVE+HOMEPATH → dirs::home_dir())，跨平台
    // 行为统一；不要直接 `dirs::home_dir()`（CLAUDE.md 跨平台路径硬约束）。
    let home: PathBuf = home_dir().unwrap_or_else(|| PathBuf::from("."));
    let projects_dir = home.join(".claude").join("projects");
    let todos_dir = home.join(".claude").join("todos");

    // 初始化 scanner——沿用 `path_decoder::get_projects_base_path()` 入参，
    // 与 watcher / HTTP detail 路径共享同一份默认 home 解析（实际值与
    // `projects_dir` 等价，path_decoder 保留作为单一真相源入口）。
    let fs = local_handle();
    let scanner = ProjectScanner::new(fs, path_decoder::get_projects_base_path());

    // FileWatcher 监听 projects + todos
    let watcher = Arc::new(FileWatcher::with_paths(projects_dir.clone(), todos_dir));

    // SSH manager
    let ssh_mgr = SshConnectionManager::new();

    // 组装带 watcher 的 LocalDataApi（自动通知管线接通）
    let api = LocalDataApi::new_with_watcher(
        scanner,
        config_mgr,
        notif_mgr,
        ssh_mgr,
        watcher.as_ref(),
        projects_dir,
    );

    // 抽出 SSE bridge 三类 receiver；api 之后装入 Arc 给 AppState
    let file_rx = watcher.subscribe_files();
    let todo_rx = watcher.subscribe_todos();
    let error_rx = api.subscribe_detected_errors();

    let api: Arc<LocalDataApi> = Arc::new(api);
    let state = AppState::new(api, 256);

    // spawn FileWatcher 主循环
    let watcher_for_task = watcher.clone();
    tokio::spawn(async move {
        if let Err(err) = watcher_for_task.start().await {
            tracing::warn!(error = %err, "FileWatcher terminated");
        }
    });

    // 把 file / todo / detected-error 桥到 AppState.events_tx，供 SSE 推送
    spawn_event_bridge(state.events_tx.clone(), file_rx, todo_rx, error_rx);

    tracing::info!("Starting claude-devtools-rs on port {port}");
    start_server(state, port)
        .await
        .context("HTTP server failed")?;

    Ok(())
}
