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
use cdt_query::{cost, stats, summary};
use cdt_ssh::SshConnectionManager;

mod mcp;

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
    /// 聚合统计
    Stats {
        /// 时间范围（today / week / 7d / 24h / 30d）
        #[arg(default_value = "7d")]
        period: String,

        /// 限定项目
        #[arg(long)]
        project: Option<String>,
    },
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
    /// 会话结构化诊断摘要
    Summary {
        /// 会话 ID
        id: String,
    },
    /// 会话 token 费用估算
    Cost {
        /// 会话 ID
        id: String,
    },
}

#[derive(Subcommand)]
enum McpAction {
    /// 启动 MCP stdio server
    Serve {
        /// 跳过 secret redaction（默认启用脱敏）
        #[arg(long)]
        allow_sensitive: bool,
    },
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
// setup skills
// ─────────────────────────────────────────────────────────────────────────────

struct SkillTemplate {
    name: &'static str,
    content: &'static str,
}

const SKILL_TEMPLATES: &[SkillTemplate] = &[SkillTemplate {
    name: "session-insights",
    content: include_str!("../assets/skills/session-insights/SKILL.md"),
}];

fn cmd_setup_skills(force: bool) -> Result<()> {
    let target_dir = std::path::PathBuf::from(".claude/skills");
    let count = SKILL_TEMPLATES.len();
    let noun = if count == 1 { "skill" } else { "skills" };

    println!(
        "Installing {count} session-aware {noun} to {}/\n",
        target_dir.display()
    );

    let mut installed = 0;
    let mut skipped = 0;
    let mut errors = 0;

    for skill in SKILL_TEMPLATES {
        let skill_dir = target_dir.join(skill.name);
        let skill_file = skill_dir.join("SKILL.md");
        let exists = skill_file.exists();

        if exists && !force {
            println!(
                "  SKIP  {}/SKILL.md (already exists, use --force to overwrite)",
                skill.name
            );
            skipped += 1;
            continue;
        }

        if let Err(e) = std::fs::create_dir_all(&skill_dir) {
            eprintln!("  ERROR creating {}: {e}", skill_dir.display());
            errors += 1;
            continue;
        }

        if let Err(e) = std::fs::write(&skill_file, skill.content) {
            eprintln!("  ERROR writing {}: {e}", skill_file.display());
            errors += 1;
            continue;
        }

        let verb = if exists { "FORCE" } else { "WRITE" };
        println!("  {verb}  {}/SKILL.md", skill.name);
        installed += 1;
    }

    println!("\nDone: {installed} installed, {skipped} skipped.");
    if skipped > 0 {
        println!("Hint: use `cdt setup skills --force` to overwrite existing files.");
    }
    if errors > 0 {
        anyhow::bail!("{errors} skill(s) failed to install");
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// sessions summary
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_sessions_summary(format: &OutputFormat, session_id: &str) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let project_id = engine
        .find_session_project(session_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let detail = engine
        .api()
        .get_session_detail(&project_id, session_id, None)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let detail = match detail {
        cdt_api::SessionDetailResponse::Full { detail, .. } => *detail,
        cdt_api::SessionDetailResponse::Unchanged { .. } => {
            anyhow::bail!("unexpected unchanged response");
        }
    };

    let output = summary::build_summary(&detail);

    match format {
        OutputFormat::Json | OutputFormat::Jsonl => {
            let json = serde_json::to_string_pretty(&output)?;
            println!("{json}");
        }
        OutputFormat::Table => {
            print_summary_table(&output);
        }
    }
    Ok(())
}

fn print_summary_table(s: &summary::SessionSummaryOutput) {
    println!("Session: {}", s.session_id);
    println!(
        "Duration: {}  Messages: {}  Errors: {}  Compactions: {}",
        format_ms(s.total_duration_ms),
        s.message_count,
        s.error_count,
        s.compaction_count,
    );
    println!(
        "Cost: ${:.4} ({} tokens)",
        s.cost.total_cost, s.cost.total_tokens
    );
    println!();

    if !s.phases.is_empty() {
        println!("Phases ({}):", s.phases.len());
        for p in &s.phases {
            println!(
                "  #{}: {} | {} chunks, {} tools, {} errors | {}",
                p.index,
                format_ms(p.duration_ms),
                p.chunk_count,
                p.tool_count,
                p.error_count,
                p.top_tools.join(", "),
            );
        }
        println!();
    }

    if !s.tool_usage.is_empty() {
        println!("Tool Usage:");
        for t in s.tool_usage.iter().take(10) {
            println!(
                "  {:<20} {:>5}x  ({:.0}% success)",
                t.name,
                t.count,
                t.success_rate * 100.0,
            );
        }
        println!();
    }

    if !s.top_files.is_empty() {
        println!("Top Files:");
        for f in s.top_files.iter().take(5) {
            println!("  {:<60} {:>3}x", truncate(&f.path, 59), f.count);
        }
        println!();
    }

    if !s.idle_gaps.is_empty() {
        println!("Idle Gaps (>2min):");
        for g in s.idle_gaps.iter().take(5) {
            println!(
                "  {} idle at {}",
                format_ms(g.gap_ms),
                g.after_ts.format("%H:%M")
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// sessions cost
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_sessions_cost(format: &OutputFormat, session_id: &str) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let project_id = engine
        .find_session_project(session_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let detail = engine
        .api()
        .get_session_detail(&project_id, session_id, None)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let detail = match detail {
        cdt_api::SessionDetailResponse::Full { detail, .. } => *detail,
        cdt_api::SessionDetailResponse::Unchanged { .. } => {
            anyhow::bail!("unexpected unchanged response");
        }
    };

    let output = cost::compute_session_cost(&detail);

    match format {
        OutputFormat::Json | OutputFormat::Jsonl => {
            let json = serde_json::to_string_pretty(&output)?;
            println!("{json}");
        }
        OutputFormat::Table => {
            println!(
                "Model: {} (pricing: {})",
                output.model, output.model_pricing_used
            );
            println!();
            println!("{:<22} {:>12} {:>12}", "CATEGORY", "TOKENS", "COST");
            println!("{}", "-".repeat(48));
            println!(
                "{:<22} {:>12} {:>12}",
                "Input",
                output.input_tokens,
                format!("${:.4}", output.input_cost)
            );
            println!(
                "{:<22} {:>12} {:>12}",
                "Output",
                output.output_tokens,
                format!("${:.4}", output.output_cost)
            );
            println!(
                "{:<22} {:>12} {:>12}",
                "Cache Read",
                output.cache_read_tokens,
                format!("${:.4}", output.cache_read_cost)
            );
            println!(
                "{:<22} {:>12} {:>12}",
                "Cache Creation",
                output.cache_creation_tokens,
                format!("${:.4}", output.cache_creation_cost)
            );
            println!("{}", "-".repeat(48));
            println!(
                "{:<22} {:>12} {:>12}",
                "TOTAL",
                output.total_tokens,
                format!("${:.4}", output.total_cost)
            );
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// stats
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_stats(
    format: &OutputFormat,
    period: &str,
    project_filter: Option<&str>,
) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(Arc::clone(&api));

    let since_str = match period {
        "today" => "24h",
        "week" => "7d",
        other => other,
    };
    let since_ms = parse_duration_to_ms(since_str)?;
    let since_dt = chrono::DateTime::from_timestamp_millis(since_ms).unwrap_or_default();

    let project_ids = if let Some(name) = project_filter {
        vec![
            engine
                .resolve_project(name)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        ]
    } else {
        let groups = api
            .list_repository_groups()
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        groups
            .iter()
            .flat_map(|g| g.worktrees.iter().map(|w| w.id.clone()))
            .collect()
    };

    let mut session_data_list: Vec<stats::SessionData> = Vec::new();
    let pagination = cdt_api::PaginatedRequest {
        page_size: 500,
        cursor: None,
    };

    for pid in &project_ids {
        let resp = api
            .list_sessions_sync(pid, &pagination)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        for session in &resp.items {
            if session.timestamp < since_ms {
                continue;
            }
            let detail = api
                .get_session_detail(pid, &session.session_id, None)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            if let cdt_api::SessionDetailResponse::Full { detail, .. } = detail {
                session_data_list.push(stats::build_session_data(&detail));
            }
        }
    }

    if session_data_list.is_empty() {
        match format {
            OutputFormat::Json | OutputFormat::Jsonl => {
                println!("{{\"sessionCount\": 0}}");
            }
            OutputFormat::Table => eprintln!("No sessions found in the given period."),
        }
        std::process::exit(2);
    }

    let result = stats::aggregate(&session_data_list, since_dt);

    match format {
        OutputFormat::Json | OutputFormat::Jsonl => {
            let json = serde_json::to_string_pretty(&result)?;
            println!("{json}");
        }
        OutputFormat::Table => {
            print_stats_table(&result);
        }
    }
    Ok(())
}

fn print_stats_table(s: &stats::AggregatedStats) {
    println!(
        "Period: {} to {}",
        s.period_start.format("%Y-%m-%d %H:%M"),
        s.period_end.format("%Y-%m-%d %H:%M"),
    );
    println!();
    println!(
        "Sessions: {}  Messages: {}",
        s.session_count, s.total_messages
    );
    println!(
        "Tokens: {} (input: {}, output: {}, cache_read: {}, cache_write: {})",
        s.total_tokens,
        s.input_tokens,
        s.output_tokens,
        s.cache_read_tokens,
        s.cache_creation_tokens,
    );
    println!("Total Cost: ${:.4}", s.total_cost);
    println!("Error Rate: {:.1}%", s.error_rate * 100.0);
    println!();

    if !s.model_usage.is_empty() {
        println!("Model Usage:");
        for m in &s.model_usage {
            println!(
                "  {:<30} {:>3} sessions  ${:.4}",
                m.model, m.session_count, m.total_cost,
            );
        }
        println!();
    }

    if !s.tool_frequency.is_empty() {
        println!("Top Tools:");
        for t in s.tool_frequency.iter().take(10) {
            println!("  {:<25} {:>5}x", t.name, t.count);
        }
        println!();
    }

    if !s.active_hours.is_empty() {
        println!("Active Hours (UTC):");
        for h in &s.active_hours {
            println!(
                "  {:02}:00  {:>3} sessions, {:>5} messages",
                h.hour, h.session_count, h.message_count,
            );
        }
    }
}

fn format_ms(ms: i64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// main
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let default_filter = if cfg!(debug_assertions) {
        "info"
    } else {
        "info,cdt_api::perf=warn"
    };
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| default_filter.into()),
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
            SessionsAction::Summary { id } => cmd_sessions_summary(&cli.format, &id).await,
            SessionsAction::Cost { id } => cmd_sessions_cost(&cli.format, &id).await,
        },
        Command::Search {
            query,
            limit,
            offset,
        } => cmd_search(&cli.format, cli.project.as_deref(), &query, limit, offset).await,
        Command::Stats { period, project } => {
            let proj = project.as_deref().or(cli.project.as_deref());
            cmd_stats(&cli.format, &period, proj).await
        }
        Command::Mcp { action } => match action {
            McpAction::Serve { allow_sensitive } => {
                let api = build_local_data_api().await?;
                let engine = Arc::new(QueryEngine::new(api));
                mcp::run_mcp_server(engine, allow_sensitive).await
            }
        },
        Command::Setup { action } => match action {
            SetupAction::Mcp { apply } => {
                cmd_setup_mcp(apply);
                Ok(())
            }
            SetupAction::Skills { force } => cmd_setup_skills(force),
        },
    }
}
