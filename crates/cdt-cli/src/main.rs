//! claude-devtools-rs CLI entrypoint.
//!
//! Clap 子命令结构：projects / sessions / search / stats / serve / mcp。
//! `serve` 启动 HTTP server；其余子命令 in-process 调用 `LocalDataApi`。

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use tokio::sync::Semaphore;

use cdt_api::http::spawn_event_bridge;
use cdt_api::{AppState, DataApi, LocalDataApi, PaginatedRequest, StaticServe, start_server};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{ProjectScanner, local_handle, path_decoder};
use cdt_ssh::SshConnectionManager;

// ─────────────────────────────────────────────────────────────────────────────
// CLI 定义
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "cdt", about = "claude-devtools CLI", version)]
struct Cli {
    /// 输出格式
    #[arg(long, global = true, default_value = "table")]
    format: OutputFormat,

    /// 限定项目范围（项目名或 ID）
    #[arg(long, global = true)]
    project: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Json,
    Table,
}

#[derive(Subcommand)]
enum Command {
    /// 项目相关操作
    Projects {
        #[command(subcommand)]
        action: ProjectsAction,
    },
    /// 会话相关操作
    Sessions {
        #[command(subcommand)]
        action: SessionsAction,
    },
    /// 全文搜索（未实现）
    Search,
    /// 统计信息（未实现）
    Stats,
    /// 启动 HTTP API server
    Serve,
    /// MCP server 模式
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
    /// 配置激活（MCP 注册、Skills 安装等）
    Setup {
        #[command(subcommand)]
        action: SetupAction,
    },
}

#[derive(Subcommand)]
enum ProjectsAction {
    /// 列出所有项目（按 repository group 聚合）
    List,
}

#[derive(Subcommand)]
enum SessionsAction {
    /// 列出会话
    List {
        /// 最多返回 N 条
        #[arg(long, default_value = "100")]
        limit: usize,

        /// 仅显示指定时间范围内的会话（如 7d、24h、30m）
        #[arg(long)]
        since: Option<String>,
    },
}

#[derive(Subcommand)]
enum McpAction {
    /// 启动 MCP stdio server（未实现）
    Serve,
}

#[derive(Subcommand)]
enum SetupAction {
    /// 注册 MCP server 到 Claude Code
    Mcp {
        /// 自动执行注册（否则仅打印命令）
        #[arg(long)]
        apply: bool,
    },
    /// 安装示例 Skills
    Skills {
        /// 强制覆盖已修改的文件
        #[arg(long)]
        force: bool,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared query layer
// ─────────────────────────────────────────────────────────────────────────────

/// 构造 `LocalDataApi`（不启动 watcher），CLI 子命令 in-process 使用。
async fn build_local_data_api() -> Result<Arc<LocalDataApi>> {
    let mut config_mgr = ConfigManager::new(None);
    config_mgr.load().await.context("failed to load config")?;

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

    let scanner_semaphore = Arc::new(Semaphore::new(64));
    let scanner = ProjectScanner::new_with_semaphore(fs, projects_dir, scanner_semaphore);

    let ssh_mgr = SshConnectionManager::new();
    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);

    Ok(Arc::new(api))
}

// ─────────────────────────────────────────────────────────────────────────────
// Serve 子命令（原 main 逻辑）
// ─────────────────────────────────────────────────────────────────────────────

async fn run_serve() -> Result<()> {
    let mut config_mgr = ConfigManager::new(None);
    config_mgr.load().await.context("failed to load config")?;

    let port = config_mgr.get_config().http_server.port;

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
    let scanner_semaphore = Arc::new(Semaphore::new(64));
    let scanner = ProjectScanner::new_with_semaphore(fs, projects_dir.clone(), scanner_semaphore);

    let ssh_mgr = SshConnectionManager::new();

    let api = LocalDataApi::new_with_watcher(
        scanner,
        config_mgr,
        notif_mgr,
        ssh_mgr,
        &cdt_watch::FileWatcher::with_paths(projects_dir.clone(), todos_dir),
        projects_dir,
    );

    let api = Arc::new(api);
    let file_rx = api.subscribe_file_changes();
    let todo_rx = api.subscribe_todo_changes();
    let error_rx = api.subscribe_detected_errors();
    let metadata_rx = api.subscribe_session_metadata();
    let context_rx = api.subscribe_context_changed();

    let state = AppState::new(api, 1024);

    spawn_event_bridge(
        state.events_tx.clone(),
        file_rx,
        todo_rx,
        error_rx,
        metadata_rx,
        context_rx,
    );

    tracing::info!("Starting claude-devtools-rs on port {port}");
    start_server(state, port, StaticServe::None)
        .await
        .context("HTTP server failed")?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// projects list
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_projects_list(format: &OutputFormat) -> Result<()> {
    let api = build_local_data_api().await?;
    let groups = api
        .list_repository_groups()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&groups)?;
            println!("{json}");
        }
        OutputFormat::Table => {
            println!(
                "{:<40} {:<50} {:>8} {:>20}",
                "NAME", "PATH", "SESSIONS", "LAST ACTIVE"
            );
            println!("{}", "-".repeat(120));
            for group in &groups {
                let path = group
                    .worktrees
                    .first()
                    .map(|w| w.path.display().to_string())
                    .unwrap_or_default();
                let last_active = group
                    .most_recent_session
                    .map_or_else(|| "-".to_string(), format_timestamp);
                println!(
                    "{:<40} {:<50} {:>8} {:>20}",
                    truncate(&group.name, 39),
                    truncate(&path, 49),
                    group.total_sessions,
                    last_active,
                );
            }
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// sessions list
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_sessions_list(
    format: &OutputFormat,
    project_filter: Option<&str>,
    limit: usize,
    since: Option<&str>,
) -> Result<()> {
    let api = build_local_data_api().await?;

    let project_id = match project_filter {
        Some(name) => resolve_project_id(&api, name).await?,
        None => {
            anyhow::bail!(
                "--project is required for `sessions list`. Use `projects list` to see available projects."
            );
        }
    };

    let pagination = PaginatedRequest {
        page_size: limit,
        cursor: None,
    };
    let resp = api
        .list_sessions_sync(&project_id, &pagination)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let since_ms = since.map(parse_duration_to_ms).transpose()?;
    let items: Vec<_> = if let Some(cutoff) = since_ms {
        resp.items
            .into_iter()
            .filter(|s| s.timestamp >= cutoff)
            .collect()
    } else {
        resp.items
    };

    if items.is_empty() {
        match format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Table => eprintln!("No sessions found."),
        }
        std::process::exit(2);
    }

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&items)?;
            println!("{json}");
        }
        OutputFormat::Table => {
            println!(
                "{:<12} {:<40} {:>10} {:>8} {:>8}",
                "ID", "TITLE", "DURATION", "STATUS", "MESSAGES"
            );
            println!("{}", "-".repeat(80));
            for s in &items {
                let short_id: String = s.session_id.chars().take(10).collect();
                let title = s.title.as_deref().unwrap_or("(untitled)");
                let status = if s.is_ongoing { "active" } else { "done" };
                let duration = format_duration(s.timestamp);
                println!(
                    "{:<12} {:<40} {:>10} {:>8} {:>8}",
                    short_id,
                    truncate(title, 39),
                    duration,
                    status,
                    s.message_count,
                );
            }
        }
    }
    Ok(())
}

/// 按名称或 ID 解析 `project_id`。
async fn resolve_project_id(api: &LocalDataApi, name: &str) -> Result<String> {
    let groups = api
        .list_repository_groups()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    for group in &groups {
        if group.name.eq_ignore_ascii_case(name) || group.id == name {
            if let Some(wt) = group.worktrees.first() {
                return Ok(wt.id.clone());
            }
            return Ok(group.id.clone());
        }
        for wt in &group.worktrees {
            if wt.name.eq_ignore_ascii_case(name) || wt.id == name {
                return Ok(wt.id.clone());
            }
        }
    }
    anyhow::bail!("project not found: {name}");
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具函数
// ─────────────────────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{truncated}…")
    }
}

fn format_timestamp(ts: i64) -> String {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let diff = (now_ms - ts) / 1000;
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

fn format_duration(session_ts: i64) -> String {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let diff = (now_ms - session_ts) / 1000;
    if diff < 60 {
        format!("{diff}s")
    } else if diff < 3600 {
        format!("{}m", diff / 60)
    } else if diff < 86400 {
        format!("{}h", diff / 3600)
    } else {
        format!("{}d", diff / 86400)
    }
}

/// 解析 `7d` / `24h` / `30m` 格式的 duration 为截止时间戳（毫秒）。
fn parse_duration_to_ms(s: &str) -> Result<i64> {
    let s = s.trim();
    let (num_str, unit) = s.split_at(s.len().saturating_sub(1));
    let num: i64 = num_str
        .parse()
        .with_context(|| format!("invalid duration: {s}"))?;
    let seconds = match unit {
        "m" => num * 60,
        "h" => num * 3600,
        "d" => num * 86400,
        "w" => num * 604_800,
        _ => anyhow::bail!("unsupported duration unit: {s} (use m/h/d/w)"),
    };
    let now_ms = chrono::Utc::now().timestamp_millis();
    Ok(now_ms - seconds * 1000)
}

// ─────────────────────────────────────────────────────────────────────────────
// setup
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_setup_mcp(apply: bool) {
    let cmd = "claude mcp add cdt-devtools -- cdt mcp serve";
    if apply {
        eprintln!("Running: {cmd}");
        let status = std::process::Command::new("claude")
            .args(["mcp", "add", "cdt-devtools", "--", "cdt", "mcp", "serve"])
            .status();
        match status {
            Ok(s) if s.success() => {
                eprintln!("Registered MCP server \"cdt-devtools\" via `claude mcp add`");
            }
            Ok(s) => {
                eprintln!("claude mcp add exited with {s}");
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("Failed to run `claude`: {e}");
                eprintln!("Make sure Claude Code CLI is installed and in PATH.");
                std::process::exit(1);
            }
        }
    } else {
        println!("To register the MCP server, run:\n");
        println!("  {cmd}\n");
        println!("Or use --apply to do it automatically:\n");
        println!("  cdt setup mcp --apply");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// main
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Serve => run_serve().await,
        Command::Projects { action } => match action {
            ProjectsAction::List => cmd_projects_list(&cli.format).await,
        },
        Command::Sessions { action } => match action {
            SessionsAction::List { limit, since } => {
                cmd_sessions_list(&cli.format, cli.project.as_deref(), limit, since.as_deref())
                    .await
            }
        },
        Command::Search => {
            anyhow::bail!("search: not yet implemented");
        }
        Command::Stats => {
            anyhow::bail!("stats: not yet implemented");
        }
        Command::Mcp { action } => match action {
            McpAction::Serve => {
                anyhow::bail!("mcp serve: not yet implemented (see #366)");
            }
        },
        Command::Setup { action } => match action {
            SetupAction::Mcp { apply } => {
                cmd_setup_mcp(apply);
                Ok(())
            }
            SetupAction::Skills { .. } => {
                anyhow::bail!("setup skills: not yet implemented (see #367)");
            }
        },
    }
}
