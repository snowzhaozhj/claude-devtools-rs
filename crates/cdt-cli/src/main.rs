//! claude-devtools-rs CLI entrypoint.
//!
//! Clap 子命令结构：projects / sessions / search / stats / serve / mcp。
//! `serve` 启动 HTTP server；其余子命令 in-process 调用 `QueryEngine`。

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use tokio::sync::Semaphore;

use cdt_api::http::spawn_event_bridge;
use cdt_api::{AppState, DataApi, LocalDataApi, StaticServe, start_server};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{ProjectScanner, local_handle, path_decoder};
use cdt_query::{ChunkKindFilter, QueryEngine, QueryFilter, SessionQueryOptions};
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
    Jsonl,
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
    /// 全文搜索
    Search {
        /// 搜索关键词
        query: String,

        /// 最多返回 N 条结果
        #[arg(long, default_value = "50")]
        limit: usize,

        /// 偏移（分页）
        #[arg(long, default_value = "0")]
        offset: usize,
    },
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

        /// 标题关键词过滤（大小写不敏感）
        #[arg(long)]
        grep: Option<String>,

        /// 仅显示消息数 >= N 的会话
        #[arg(long)]
        min_messages: Option<usize>,
    },
    /// 显示会话元数据（不含 chunks）
    Show {
        /// 会话 ID
        id: String,
    },
    /// 显示会话详情（chunk 流）
    Detail {
        /// 会话 ID
        id: String,

        /// 指定 chunk 区间（如 10:30）
        #[arg(long)]
        range: Option<String>,

        /// 仅显示最后 N 条 chunks
        #[arg(long)]
        tail: Option<usize>,

        /// 过滤条件：`errors_only` 或 `tool_calls`
        #[arg(long)]
        filter: Option<String>,

        /// 输出完整 chunks（不截断）
        #[arg(long)]
        full: bool,
    },
    /// 聚合会话中的所有错误
    Errors {
        /// 会话 ID
        id: String,
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
        OutputFormat::Jsonl => {
            for group in &groups {
                println!("{}", serde_json::to_string(group)?);
            }
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
    grep: Option<&str>,
    min_messages: Option<usize>,
) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let project_id = match project_filter {
        Some(name) => engine
            .resolve_project(name)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        None => {
            anyhow::bail!(
                "--project is required for `sessions list`. Use `projects list` to see available projects."
            );
        }
    };

    let since_ms = since.map(parse_duration_to_ms).transpose()?;
    let filter = QueryFilter {
        since: since_ms,
        grep: grep.map(ToOwned::to_owned),
        min_messages,
        limit: Some(limit),
        ..Default::default()
    };

    let items = engine
        .list_sessions(&project_id, &filter)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if items.is_empty() {
        match format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Jsonl => {}
            OutputFormat::Table => eprintln!("No sessions found."),
        }
        std::process::exit(2);
    }

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&items)?;
            println!("{json}");
        }
        OutputFormat::Jsonl => {
            for item in &items {
                println!("{}", serde_json::to_string(item)?);
            }
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

// ─────────────────────────────────────────────────────────────────────────────
// sessions show
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_sessions_show(format: &OutputFormat, session_id: &str) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let project_id = engine
        .find_session_project(session_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let detail = engine
        .get_session_show(&project_id, session_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&detail)?;
            println!("{json}");
        }
        OutputFormat::Jsonl => {
            println!("{}", serde_json::to_string(&detail)?);
        }
        OutputFormat::Table => {
            println!("Session:   {}", detail.session_id);
            println!("Project:   {}", detail.project_id);
            println!(
                "Title:     {}",
                detail.title.as_deref().unwrap_or("(untitled)")
            );
            println!("Messages:  {}", detail.metrics.message_count);
            println!(
                "Status:    {}",
                if detail.is_ongoing { "active" } else { "done" }
            );
            if let Some(cwd) = &detail.metadata.cwd {
                println!("CWD:       {cwd}");
            }
            if let Some(modified) = detail.metadata.last_modified {
                println!("Modified:  {}", format_timestamp(modified));
            }
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// sessions detail
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_sessions_detail(
    format: &OutputFormat,
    session_id: &str,
    range: Option<&str>,
    tail: Option<usize>,
    filter: Option<&str>,
    full: bool,
) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let project_id = engine
        .find_session_project(session_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let options = if full {
        SessionQueryOptions::full()
    } else {
        let parsed_range = range.map(parse_range).transpose()?;
        let kind_filter = filter.map(parse_kind_filter).transpose()?;
        SessionQueryOptions {
            range: parsed_range,
            tail: tail.or(if parsed_range.is_some() {
                None
            } else {
                Some(20)
            }),
            kind_filter,
            errors_only: false,
        }
    };

    let detail = engine
        .get_session_detail(&project_id, session_id, &options)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&detail)?;
            println!("{json}");
        }
        OutputFormat::Jsonl => {
            for chunk in &detail.chunks {
                println!("{}", serde_json::to_string(chunk)?);
            }
        }
        OutputFormat::Table => {
            println!(
                "Session: {} ({} chunks)",
                detail.session_id,
                detail.chunks.len()
            );
            println!("{}", "-".repeat(60));
            for (i, chunk) in detail.chunks.iter().enumerate() {
                print_chunk_summary(i, chunk);
            }
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// sessions errors
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_sessions_errors(format: &OutputFormat, session_id: &str) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let project_id = engine
        .find_session_project(session_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let errors = engine
        .get_session_errors(&project_id, session_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if errors.is_empty() {
        match format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Jsonl => {}
            OutputFormat::Table => eprintln!("No errors found."),
        }
        std::process::exit(2);
    }

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&errors)?;
            println!("{json}");
        }
        OutputFormat::Jsonl => {
            for e in &errors {
                println!("{}", serde_json::to_string(e)?);
            }
        }
        OutputFormat::Table => {
            println!("{:>6} {:<20} {:<50}", "CHUNK", "TOOL", "ERROR");
            println!("{}", "-".repeat(78));
            for e in &errors {
                let msg = e.error_message.as_deref().unwrap_or("(no message)");
                println!(
                    "{:>6} {:<20} {:<50}",
                    e.chunk_index,
                    truncate(&e.tool_name, 19),
                    truncate(msg, 49),
                );
            }
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// search
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_search(
    format: &OutputFormat,
    project_filter: Option<&str>,
    query: &str,
    limit: usize,
    offset: usize,
) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let project_id = match project_filter {
        Some(name) => Some(
            engine
                .resolve_project(name)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        ),
        None => None,
    };

    let result = engine
        .search(query, project_id.as_deref(), offset, limit)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if result.results.is_empty() {
        match format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Jsonl => {}
            OutputFormat::Table => eprintln!("No results found."),
        }
        std::process::exit(2);
    }

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&result.results)?;
            println!("{json}");
        }
        OutputFormat::Jsonl => {
            for r in &result.results {
                println!("{}", serde_json::to_string(r)?);
            }
        }
        OutputFormat::Table => {
            println!(
                "{:<12} {:<30} {:>8} {:<40}",
                "SESSION", "TITLE", "MATCHES", "PREVIEW"
            );
            println!("{}", "-".repeat(92));
            for r in &result.results {
                let short_id: String = r.session_id.chars().take(10).collect();
                let preview = r.hits.first().map_or("", |h| h.preview.as_str());
                println!(
                    "{:<12} {:<30} {:>8} {:<40}",
                    short_id,
                    truncate(&r.session_title, 29),
                    r.total_matches,
                    truncate(preview, 39),
                );
            }
        }
    }
    Ok(())
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

/// 解析 `10:30` 格式的 range 为 `(start, end)`。
fn parse_range(s: &str) -> Result<(usize, usize)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        anyhow::bail!("invalid range format: {s} (expected start:end, e.g. 10:30)");
    }
    let start: usize = parts[0]
        .parse()
        .with_context(|| format!("invalid range start: {}", parts[0]))?;
    let end: usize = parts[1]
        .parse()
        .with_context(|| format!("invalid range end: {}", parts[1]))?;
    if start > end {
        anyhow::bail!("invalid range: start ({start}) > end ({end})");
    }
    Ok((start, end))
}

fn parse_kind_filter(s: &str) -> Result<ChunkKindFilter> {
    match s {
        "errors_only" | "errors" => Ok(ChunkKindFilter::ErrorsOnly),
        "tool_calls" | "tools" => Ok(ChunkKindFilter::ToolCalls),
        _ => anyhow::bail!("unknown filter: {s} (expected: errors_only, tool_calls)"),
    }
}

fn print_chunk_summary(index: usize, chunk: &cdt_core::Chunk) {
    match chunk {
        cdt_core::Chunk::User(u) => {
            let text = match &u.content {
                cdt_core::MessageContent::Text(t) => truncate(t, 60),
                cdt_core::MessageContent::Blocks(_) => "(non-text)".to_string(),
            };
            println!("[{index:>4}] USER: {text}");
        }
        cdt_core::Chunk::Ai(ai) => {
            let tools: Vec<&str> = ai
                .tool_executions
                .iter()
                .map(|t| t.tool_name.as_str())
                .collect();
            let errors = ai.tool_executions.iter().filter(|t| t.is_error).count();
            if tools.is_empty() {
                println!("[{index:>4}] AI: ({} steps)", ai.semantic_steps.len());
            } else if errors > 0 {
                println!(
                    "[{index:>4}] AI: tools=[{}] ({errors} errors)",
                    tools.join(", ")
                );
            } else {
                println!("[{index:>4}] AI: tools=[{}]", tools.join(", "));
            }
        }
        cdt_core::Chunk::System(s) => {
            println!("[{index:>4}] SYSTEM: {}", truncate(&s.content_text, 60));
        }
        cdt_core::Chunk::Compact(c) => {
            println!("[{index:>4}] COMPACT: {}", truncate(&c.summary_text, 60));
        }
    }
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
            SessionsAction::List {
                limit,
                since,
                grep,
                min_messages,
            } => {
                cmd_sessions_list(
                    &cli.format,
                    cli.project.as_deref(),
                    limit,
                    since.as_deref(),
                    grep.as_deref(),
                    min_messages,
                )
                .await
            }
            SessionsAction::Show { id } => cmd_sessions_show(&cli.format, &id).await,
            SessionsAction::Detail {
                id,
                range,
                tail,
                filter,
                full,
            } => {
                cmd_sessions_detail(
                    &cli.format,
                    &id,
                    range.as_deref(),
                    tail,
                    filter.as_deref(),
                    full,
                )
                .await
            }
            SessionsAction::Errors { id } => cmd_sessions_errors(&cli.format, &id).await,
        },
        Command::Search {
            query,
            limit,
            offset,
        } => cmd_search(&cli.format, cli.project.as_deref(), &query, limit, offset).await,
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
