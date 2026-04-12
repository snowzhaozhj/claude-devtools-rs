//! claude-devtools-rs binary entrypoint.
//!
//! 初始化各 manager → 构造 `LocalDataApi` → 启动 HTTP server。

use std::sync::Arc;

use anyhow::{Context, Result};

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

    // 初始化 scanner
    let fs = local_handle();
    let projects_dir = path_decoder::get_projects_base_path();
    let scanner = ProjectScanner::new(fs, projects_dir);

    // SSH manager
    let ssh_mgr = SshConnectionManager::new();

    // 组装 LocalDataApi
    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);
    let state = AppState::new(Arc::new(api), 256);

    tracing::info!("Starting claude-devtools-rs on port {port}");
    start_server(state, port)
        .await
        .context("HTTP server failed")?;

    Ok(())
}
