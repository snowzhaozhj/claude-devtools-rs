//! claude-devtools-rs CLI entrypoint.
//!
//! Clap 子命令结构：projects / sessions / search / stats / serve / mcp。
//! `serve` 启动 HTTP server；其余子命令 in-process 调用 `QueryEngine`。

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::engine::{ArgValueCandidates, ArgValueCompleter};
use tokio::sync::Semaphore;

mod completions;

use cdt_api::http::spawn_event_bridge;
use cdt_api::{AppState, DataApi, LocalDataApi, StaticServe, start_server};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{ProjectScanner, local_handle, new_cwd_cache, path_decoder};
use cdt_query::stats;
use cdt_query::{ChunkKindFilter, QueryEngine, QueryFilter, SessionQueryOptions};
use cdt_ssh::SshConnectionManager;

mod export;
mod mcp;
mod time_expr;
mod update;
mod view;

// ─────────────────────────────────────────────────────────────────────────────
// CLI 定义
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "cdt", about = "claude-devtools CLI", version)]
struct Cli {
    /// 输出格式
    #[arg(long, global = true, default_value = "table")]
    format: OutputFormat,

    /// 限定项目范围（项目名或 ID；编码 ID 需用 --project=<id> 形式）
    #[arg(long, global = true, add = ArgValueCandidates::new(completions::ProjectCompleter))]
    project: Option<String>,

    /// JSON 字段选择（逗号分隔），隐含 --format json + 紧凑输出。
    /// 无参数时列出可用字段。使用 --json=field1,field2 或 --json 不带值。
    #[arg(long, global = true, num_args = 0..=1, default_missing_value = "", require_equals = true)]
    json: Option<String>,

    /// table 模式不截断任何字段
    #[arg(long, global = true)]
    no_truncate: bool,

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
    /// 会话列表
    Sessions {
        #[command(subcommand)]
        action: SessionsAction,
    },
    /// 单 session 复合视图（summary + cost + errors）
    Session {
        /// 会话 ID（支持 'latest'）
        #[arg(add = ArgValueCompleter::new(completions::SessionCompleter))]
        id: String,

        /// 切换到 chunk 模式（显示 chunk 流而非复合视图）
        #[arg(long)]
        chunks: bool,

        /// 追加重数据（逗号分隔）
        #[arg(long, add = ArgValueCandidates::new(completions::IncludeCompleter))]
        include: Option<String>,

        /// chunk 模式：指定 chunk 区间（如 10:30），与 --tail 互斥
        #[arg(long, conflicts_with = "tail", requires = "chunks")]
        range: Option<String>,

        /// chunk 模式：仅显示最后 N 条 chunks
        #[arg(long, conflicts_with = "range", requires = "chunks")]
        tail: Option<usize>,

        /// chunk 模式：过滤条件
        #[arg(long, requires = "chunks", add = ArgValueCandidates::new(completions::FilterCompleter))]
        filter: Option<String>,

        /// chunk 模式：内容模式 omit / overview / full
        #[arg(long, requires = "chunks", add = ArgValueCandidates::new(completions::ContentModeCompleter))]
        content: Option<String>,

        /// chunk 模式：grep 过滤
        #[arg(long, requires = "chunks")]
        grep: Option<String>,

        /// chunk 模式：grep context 数
        #[arg(long, default_value = "1", requires = "chunks")]
        grep_context: usize,

        /// chunk 模式：返回全部 chunk
        #[arg(long, visible_alias = "full", requires = "chunks")]
        all: bool,

        /// chunk 模式：展平提取 overview/errors/tools
        #[arg(long, conflicts_with = "content", requires = "chunks")]
        extract: Option<String>,
    },
    /// 导出会话为 Markdown / JSON
    Export {
        /// 会话 ID（支持 'latest'）
        #[arg(add = ArgValueCompleter::new(completions::SessionCompleter))]
        id: String,

        /// 导出格式：md（默认）/ json
        #[arg(long = "export-format", default_value = "md")]
        export_format: String,

        /// 输出文件路径（默认 stdout）
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// 工具输出详略：full（默认）/ summary / name-only
        #[arg(long, default_value = "full")]
        detail: String,

        /// 排除 thinking blocks
        #[arg(long)]
        no_thinking: bool,

        /// 排除子代理卡片
        #[arg(long)]
        no_subagents: bool,

        /// 指定 chunk 区间（如 10:30），与 --tail 互斥
        #[arg(long, conflicts_with = "tail")]
        range: Option<String>,

        /// 仅导出最后 N 条 chunks
        #[arg(long, conflicts_with = "range")]
        tail: Option<usize>,

        /// grep 过滤
        #[arg(long)]
        grep: Option<String>,

        /// grep context 数
        #[arg(long, default_value = "1")]
        grep_context: usize,

        /// 过滤条件（`errors_only` / `tool_calls`）
        #[arg(long)]
        filter: Option<String>,

        /// 导出全部 chunk（禁用默认 tail）
        #[arg(long)]
        all: bool,
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

        /// 限定到单个 session（intra-session search）
        #[arg(long)]
        session: Option<String>,

        /// 仅搜索此时间之后的 session
        #[arg(long, add = ArgValueCandidates::new(completions::SinceCompleter))]
        since: Option<String>,
    },
    /// 聚合统计
    Stats {
        /// 时间范围（today / week / 7d / 24h / 30d）
        #[arg(default_value = "7d")]
        period: String,

        /// 限定项目
        #[arg(long)]
        project: Option<String>,

        /// 按维度分组：none / model / day
        #[arg(long, default_value = "none", add = ArgValueCandidates::new(completions::GroupByStatsCompleter))]
        group_by: String,
    },
    /// 启动 HTTP API server
    Serve,
    /// MCP server 模式
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
    /// 一键配置（MCP 注册 + Skills 安装）
    Setup {
        #[command(subcommand)]
        action: Option<SetupAction>,

        /// 配置范围：local（个人私有）、project（团队共享 .mcp.json）、user（全局）
        #[arg(long, short, global = true, default_value = "local")]
        scope: SetupScope,

        /// 仅打印将执行的操作，不实际执行
        #[arg(long, global = true)]
        dry_run: bool,

        /// 强制覆盖已有文件（Skills）
        #[arg(long, global = true)]
        force: bool,
    },
    /// 生成 shell 补全脚本
    Completions {
        /// 目标 shell
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// 自更新到最新版本
    #[command(name = "self-update")]
    SelfUpdate {
        /// 仅检查是否有新版本，不执行更新
        #[arg(long)]
        check: bool,

        /// 指定目标版本（如 v0.5.14）
        #[arg(long)]
        version: Option<String>,

        /// 指定安装路径（默认替换当前可执行文件）
        #[arg(long)]
        install_path: Option<PathBuf>,
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

        /// 仅显示指定时间范围内的会话（如 7d、24h、30m、today、yesterday、2026-06-06）
        #[arg(long, add = ArgValueCandidates::new(completions::SinceCompleter))]
        since: Option<String>,

        /// 仅显示此时间之前的会话（格式同 --since）
        #[arg(long, add = ArgValueCandidates::new(completions::SinceCompleter))]
        until: Option<String>,

        /// 按 git 分支过滤（大小写不敏感子串匹配）
        #[arg(long)]
        branch: Option<String>,

        /// 仅显示活跃 session
        #[arg(long)]
        is_ongoing: bool,

        /// 标题关键词过滤（大小写不敏感）
        #[arg(long)]
        grep: Option<String>,

        /// 仅显示消息数 >= N 的会话
        #[arg(long)]
        min_messages: Option<usize>,

        /// 按维度分组：none / project / day
        #[arg(long, default_value = "none", add = ArgValueCandidates::new(completions::GroupBySessionsCompleter))]
        group_by: String,
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

#[derive(Clone, ValueEnum)]
enum SetupScope {
    /// 个人私有（~/.claude/settings.local.json），不入版本控制
    Local,
    /// 团队共享（.mcp.json / .claude/skills/），可 git commit
    Project,
    /// 全局（~/.claude/settings.json / ~/.claude/skills/），所有项目可用
    User,
}

#[derive(Subcommand)]
enum SetupAction {
    /// 注册 MCP server 到 Claude Code
    Mcp,
    /// 安装示例 Skills
    Skills,
    /// 安装 shell 补全
    Completions,
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
    let scanner =
        ProjectScanner::new_with_cwd_cache(fs, projects_dir, scanner_semaphore, new_cwd_cache());

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
    let jobs_rx = api.subscribe_jobs();

    let state = AppState::new(api, 1024);

    spawn_event_bridge(
        state.events_tx.clone(),
        file_rx,
        todo_rx,
        error_rx,
        metadata_rx,
        context_rx,
        jobs_rx,
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

async fn cmd_projects_list(format: &OutputFormat, json_fields: Option<&str>) -> Result<()> {
    let api = build_local_data_api().await?;
    let groups = api
        .list_repository_groups()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    match format {
        OutputFormat::Json => emit_json(&groups, json_fields)?,
        OutputFormat::Jsonl => {
            for group in &groups {
                println!("{}", serde_json::to_string(group)?);
            }
        }
        OutputFormat::Table => {
            let tw = term_width();
            let fixed = 8 + 20 + 6; // SESSIONS + LAST ACTIVE + padding
            let flex = tw.saturating_sub(fixed);
            let name_w = flex * 2 / 5;
            let path_w = flex * 3 / 5;
            println!(
                "{:<name_w$} {:<path_w$} {:>8} {:>20}",
                "NAME", "PATH", "SESSIONS", "LAST ACTIVE",
            );
            println!("{}", "-".repeat(tw));
            for group in &groups {
                let raw_path = group
                    .worktrees
                    .first()
                    .map(|w| w.path.display().to_string())
                    .unwrap_or_default();
                let path = shorten_path(&raw_path);
                let last_active = group
                    .most_recent_session
                    .map_or_else(|| "-".to_string(), format_timestamp);
                println!(
                    "{:<name_w$} {:<path_w$} {:>8} {:>20}",
                    truncate(&group.name, name_w.saturating_sub(1)),
                    truncate(&path, path_w.saturating_sub(1)),
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

#[allow(clippy::too_many_arguments)]
async fn cmd_sessions_list(
    format: &OutputFormat,
    project_filter: Option<&str>,
    limit: usize,
    since: Option<&str>,
    until: Option<&str>,
    branch: Option<&str>,
    is_ongoing: bool,
    grep: Option<&str>,
    min_messages: Option<usize>,
    group_by: &str,
    json_fields: Option<&str>,
) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let is_cross_project = project_filter.is_none();
    let since_ms = since
        .or(if is_cross_project { Some("7d") } else { None })
        .map(cdt_cli::time_expr::parse_time_expr_local)
        .transpose()
        .with_context(|| "invalid --since value")?;
    let until_ms = until
        .map(cdt_cli::time_expr::parse_time_expr_local)
        .transpose()
        .with_context(|| "invalid --until value")?;
    let filter = QueryFilter {
        since: since_ms,
        until: until_ms,
        grep: grep.map(ToOwned::to_owned),
        min_messages,
        limit: Some(limit),
    };

    let items = if is_cross_project {
        engine
            .list_sessions_cross_project(&filter)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?
    } else {
        let project_id = engine
            .resolve_project(project_filter.unwrap())
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        engine
            .list_sessions(&project_id, &filter)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?
    };

    let mut items = items;
    if let Some(b) = branch {
        let lower = b.to_lowercase();
        items.retain(|s| {
            s.git_branch
                .as_deref()
                .is_some_and(|gb| gb.to_lowercase().contains(&lower))
        });
    }
    if is_ongoing {
        items.retain(|s| s.is_ongoing);
    }

    if items.is_empty() {
        match format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Jsonl => {}
            OutputFormat::Table => eprintln!("No sessions found."),
        }
        return Ok(());
    }

    if group_by != "none" {
        let grouped = group_sessions_cli(&items, group_by);
        match format {
            OutputFormat::Json => emit_json(&grouped, json_fields)?,
            OutputFormat::Jsonl => {
                for g in &grouped {
                    println!("{}", serde_json::to_string(g)?);
                }
            }
            OutputFormat::Table => {
                let tw = term_width();
                let fixed = 38 + 10 + 8 + 8 + 8;
                let title_w = tw.saturating_sub(fixed).max(10);
                for g in &grouped {
                    let key = g["key"].as_str().unwrap_or("?");
                    let count = g["count"].as_u64().unwrap_or(0);
                    println!("\n=== {key} ({count} sessions) ===");
                    println!(
                        "{:<38} {:<title_w$} {:>10} {:>8} {:>8}",
                        "ID", "TITLE", "DURATION", "STATUS", "MESSAGES"
                    );
                    println!("{}", "-".repeat(tw));
                    if let Some(sessions) = g["sessions"].as_array() {
                        for sv in sessions {
                            let sid = sv["sessionId"].as_str().unwrap_or("");
                            let title = sv["title"].as_str().unwrap_or("(untitled)");
                            let ongoing = sv["isOngoing"].as_bool().unwrap_or(false);
                            let status = if ongoing { "active" } else { "done" };
                            let ts = sv["timestamp"].as_i64().unwrap_or(0);
                            let duration = format_duration(ts);
                            let msgs = sv["messageCount"].as_u64().unwrap_or(0);
                            println!(
                                "{:<38} {:<title_w$} {:>10} {:>8} {:>8}",
                                sid,
                                truncate(title, title_w.saturating_sub(1)),
                                duration,
                                status,
                                msgs,
                            );
                        }
                    }
                }
            }
        }
        return Ok(());
    }

    match format {
        OutputFormat::Json => emit_json(&items, json_fields)?,
        OutputFormat::Jsonl => {
            for item in &items {
                println!("{}", serde_json::to_string(item)?);
            }
        }
        OutputFormat::Table => {
            let tw = term_width();
            let fixed = 38 + 10 + 8 + 8 + 8;
            let title_w = tw.saturating_sub(fixed).max(10);
            println!(
                "{:<38} {:<title_w$} {:>10} {:>8} {:>8}",
                "ID", "TITLE", "DURATION", "STATUS", "MESSAGES"
            );
            println!("{}", "-".repeat(tw));
            for s in &items {
                let title = s.title.as_deref().unwrap_or("(untitled)");
                let status = if s.is_ongoing { "active" } else { "done" };
                let duration = format_duration(s.timestamp);
                println!(
                    "{:<38} {:<title_w$} {:>10} {:>8} {:>8}",
                    &s.session_id,
                    truncate(title, title_w.saturating_sub(1)),
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
// session inspect (composite view)
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_session_inspect(
    format: &OutputFormat,
    session_id: &str,
    include: Option<&str>,
    json_fields: Option<&str>,
) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let session_id = resolve_latest_cli(&engine, session_id).await?;

    let project_id = engine
        .find_session_project(&session_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let options = SessionQueryOptions::default();
    let detail = engine
        .get_session_detail(&project_id, &session_id, &options)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let summary = cdt_query::summary::build_summary(&detail);
    let cost = cdt_query::cost::compute_session_cost(&detail);

    let indexed: Vec<(usize, &cdt_core::Chunk)> = detail.chunks.iter().enumerate().collect();
    let error_entries = cdt_query::extract::extract_errors(&indexed);
    let error_count = error_entries.len();
    let top_errors: Vec<serde_json::Value> = error_entries
        .into_iter()
        .take(10)
        .map(|e| {
            serde_json::json!({
                "chunkIndex": e.chunk_index,
                "toolName": e.tool_name,
                "errorMessage": e.error_summary,
            })
        })
        .collect();

    let include_set: std::collections::HashSet<&str> = include
        .map(|s| s.split(',').map(str::trim).collect())
        .unwrap_or_default();

    let mut result = serde_json::json!({
        "sessionId": &session_id,
        "projectId": project_id,
        "messageCount": summary.message_count,
        "chunkCount": detail.chunks.len(),
        "durationMs": summary.total_duration_ms,
        "cost": cost,
        "errorCount": error_count,
        "errors": top_errors,
    });

    if include_set.contains("phases") {
        result["phases"] = serde_json::to_value(&summary.phases).unwrap_or_default();
    }
    if include_set.contains("tools") {
        result["toolUsage"] = serde_json::to_value(&summary.tool_usage).unwrap_or_default();
    }
    if include_set.contains("activity") {
        result["toolActivity"] = serde_json::to_value(&summary.tool_activity).unwrap_or_default();
    }
    if include_set.contains("idle_gaps") {
        result["idleGaps"] = serde_json::to_value(&summary.idle_gaps).unwrap_or_default();
    }
    if include_set.contains("files") {
        result["topFiles"] = serde_json::to_value(&summary.top_files).unwrap_or_default();
    }

    match format {
        OutputFormat::Json => emit_json(&result, json_fields)?,
        OutputFormat::Jsonl => println!("{}", serde_json::to_string(&result)?),
        OutputFormat::Table => {
            println!("Session: {session_id}");
            println!(
                "Messages: {}  Chunks: {}  Duration: {}s",
                summary.message_count,
                detail.chunks.len(),
                summary.total_duration_ms / 1000
            );
            println!(
                "Cost: ${:.4} ({} tokens, {})",
                cost.total_cost, cost.total_tokens, cost.model
            );
            println!("Errors: {error_count}");
            if !top_errors.is_empty() {
                println!();
                let tw = term_width();
                let fixed = 6 + 20 + 4;
                let error_w = tw.saturating_sub(fixed).max(20);
                println!("{:>6} {:<20} {:<error_w$}", "CHUNK", "TOOL", "ERROR");
                println!("{}", "-".repeat(tw));
                for e in &top_errors {
                    let msg = e["errorMessage"].as_str().unwrap_or("(no details)");
                    println!(
                        "{:>6} {:<20} {:<error_w$}",
                        e["chunkIndex"],
                        truncate(e["toolName"].as_str().unwrap_or(""), 19),
                        truncate(msg, error_w.saturating_sub(1)),
                    );
                }
            }
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// export
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn cmd_export(
    session_id: &str,
    format: &str,
    output_path: Option<&std::path::Path>,
    detail: &str,
    no_thinking: bool,
    no_subagents: bool,
    range: Option<&str>,
    tail: Option<usize>,
    grep: Option<&str>,
    grep_context: usize,
    filter: Option<&str>,
    all: bool,
) -> Result<()> {
    let export_format = match format {
        "md" | "markdown" => export::ExportFormat::Markdown,
        "json" => export::ExportFormat::Json,
        other => anyhow::bail!("invalid export format: '{other}'. Supported: md, json"),
    };

    let detail_mode = match detail {
        "full" => export::ToolDetailMode::Full,
        "summary" => export::ToolDetailMode::Summary,
        "name-only" => export::ToolDetailMode::NameOnly,
        other => {
            anyhow::bail!("invalid --detail value: '{other}'. Supported: full, summary, name-only")
        }
    };

    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(Arc::clone(&api));

    let session_id = resolve_latest_cli(&engine, session_id).await?;

    let project_id = engine
        .find_session_project(&session_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let options = SessionQueryOptions::default();
    let session_detail = engine
        .get_session_detail(&project_id, &session_id, &options)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // Apply chunk filters (same pipeline as session --chunks)
    let kind_filter = filter.map(parse_kind_filter).transpose()?;
    let indexed: Vec<(usize, &cdt_core::Chunk)> = session_detail
        .chunks
        .iter()
        .enumerate()
        .filter(|(_, chunk)| match kind_filter {
            None => true,
            Some(ChunkKindFilter::ErrorsOnly) => {
                matches!(chunk, cdt_core::Chunk::Ai(ai) if ai.tool_executions.iter().any(|te| te.is_error))
            }
            Some(ChunkKindFilter::ToolCalls) => {
                matches!(chunk, cdt_core::Chunk::Ai(ai) if !ai.tool_executions.is_empty())
            }
        })
        .collect();

    let grep_needle = grep.filter(|s| !s.trim().is_empty());
    let grep_matcher = grep_needle.map(cdt_discover::search_text::GrepMatcher::literal);
    let grep_hits: std::collections::HashSet<usize> = if let Some(ref matcher) = grep_matcher {
        indexed
            .iter()
            .filter(|(_, chunk)| cdt_discover::search_text::chunk_matches_grep(chunk, matcher))
            .map(|(i, _)| *i)
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let filtered: Vec<(usize, &cdt_core::Chunk)> = if grep_matcher.is_some() {
        let visible: std::collections::HashSet<usize> = grep_hits
            .iter()
            .flat_map(|&i| i.saturating_sub(grep_context)..=i + grep_context)
            .collect();
        indexed
            .into_iter()
            .filter(|(i, _)| visible.contains(i))
            .collect()
    } else {
        indexed
    };

    let windowed: Vec<&cdt_core::Chunk> = if all {
        filtered.into_iter().map(|(_, c)| c).collect()
    } else {
        let parsed_range = range.map(parse_range).transpose()?;
        if let Some((start, end)) = parsed_range {
            filtered
                .into_iter()
                .filter(|(i, _)| *i >= start && *i < end)
                .map(|(_, c)| c)
                .collect()
        } else {
            let effective_tail = tail.unwrap_or(usize::MAX);
            let items: Vec<&cdt_core::Chunk> = filtered.into_iter().map(|(_, c)| c).collect();
            let len = items.len();
            if effective_tail < len {
                items.into_iter().skip(len - effective_tail).collect()
            } else {
                items
            }
        }
    };

    // Build a filtered SessionDetail with only the selected chunks
    let filtered_detail = cdt_api::SessionDetail {
        session_id: session_detail.session_id.clone(),
        project_id: session_detail.project_id.clone(),
        chunks: windowed.into_iter().cloned().collect(),
        metrics: session_detail.metrics.clone(),
        metadata: session_detail.metadata.clone(),
        context_injections: vec![],
        injections_by_phase: std::collections::BTreeMap::new(),
        phase_info: cdt_core::ContextPhaseInfo::default(),
        turn_context_stats: std::collections::HashMap::new(),
        is_ongoing: session_detail.is_ongoing,
        title: session_detail.title.clone(),
        workflow_items: vec![],
    };

    let summary = cdt_query::summary::build_summary(&filtered_detail);
    let cost = cdt_query::cost::compute_session_cost(&session_detail);

    let export_options = export::ExportOptions {
        format: export_format,
        detail: detail_mode,
        include_thinking: !no_thinking,
        include_subagents: !no_subagents,
    };

    let content = export::export_session(&filtered_detail, &summary, &cost, &export_options);

    if let Some(path) = output_path {
        std::fs::write(path, &content)
            .with_context(|| format!("failed to write export to {}", path.display()))?;
        eprintln!("Exported to {}", path.display());
    } else {
        print!("{content}");
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// session chunks (was sessions detail)
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn cmd_sessions_detail(
    format: &OutputFormat,
    session_id: &str,
    range: Option<&str>,
    tail: Option<usize>,
    filter: Option<&str>,
    all: bool,
    grep: Option<&str>,
    grep_context: usize,
    content: Option<&str>,
    extract: Option<&str>,
    json_fields: Option<&str>,
) -> Result<()> {
    let content_mode = match content {
        None => None,
        Some("omit") => Some(view::ContentMode::Omit),
        Some("full") => Some(view::ContentMode::Full),
        Some("overview") => Some(view::ContentMode::Overview),
        Some(other) => {
            anyhow::bail!("invalid --content value: '{other}'. Supported: omit, overview, full");
        }
    };

    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let session_id = resolve_latest_cli(&engine, session_id).await?;

    let project_id = engine
        .find_session_project(&session_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let kind_filter = filter.map(parse_kind_filter).transpose()?;

    // Fetch ALL chunks; apply kind_filter + grep + range/tail in-process to preserve absolute indices
    let options = SessionQueryOptions {
        range: None,
        tail: None,
        kind_filter: None,
        errors_only: false,
    };

    let detail = engine
        .get_session_detail(&project_id, &session_id, &options)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // Enumerate on full set for absolute indices, then apply kind_filter
    let indexed_chunks: Vec<(usize, &cdt_core::Chunk)> = detail
        .chunks
        .iter()
        .enumerate()
        .filter(|(_, chunk)| match kind_filter {
            None => true,
            Some(ChunkKindFilter::ErrorsOnly) => matches!(chunk, cdt_core::Chunk::Ai(ai) if ai.tool_executions.iter().any(|te| te.is_error)),
            Some(ChunkKindFilter::ToolCalls) => matches!(chunk, cdt_core::Chunk::Ai(ai) if !ai.tool_executions.is_empty()),
        })
        .collect();

    // grep on full set (after kind_filter, before range/tail)
    let grep_needle = grep.filter(|s| !s.trim().is_empty());
    let grep_matcher = grep_needle.map(cdt_discover::search_text::GrepMatcher::literal);
    let grep_hits: std::collections::HashSet<usize> = if let Some(ref matcher) = grep_matcher {
        indexed_chunks
            .iter()
            .filter(|(_, chunk)| cdt_discover::search_text::chunk_matches_grep(chunk, matcher))
            .map(|(i, _)| *i)
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let filtered: Vec<(usize, &cdt_core::Chunk)> = if grep_matcher.is_some() {
        let visible: std::collections::HashSet<usize> = grep_hits
            .iter()
            .flat_map(|&i| i.saturating_sub(grep_context)..=i + grep_context)
            .collect();
        indexed_chunks
            .into_iter()
            .filter(|(i, _)| visible.contains(i))
            .collect()
    } else {
        indexed_chunks
    };

    // range/tail applied after grep
    let windowed: Vec<(usize, &cdt_core::Chunk)> = if all {
        filtered
    } else {
        let parsed_range = range.map(parse_range).transpose()?;
        if let Some((start, end)) = parsed_range {
            filtered
                .into_iter()
                .filter(|(i, _)| *i >= start && *i < end)
                .collect()
        } else {
            let effective_tail = tail.unwrap_or(20);
            let len = filtered.len();
            if effective_tail < len {
                filtered.into_iter().skip(len - effective_tail).collect()
            } else {
                filtered
            }
        }
    };

    if windowed.is_empty() && range.is_some() && !all && filter.is_none() && grep.is_none() {
        let range_str = range.unwrap_or("");
        eprintln!(
            "hint: 0 chunks in range \"{range_str}\". --range uses [start, end) semantics \
             (left-inclusive, right-exclusive by chunkIndex). For a single chunk at index N, \
             use N:N+1."
        );
    }

    if let Some(extract_mode) = extract {
        return cmd_extract(&windowed, extract_mode, format, json_fields);
    }

    if matches!(format, OutputFormat::Table) {
        let tw = term_width();
        let content_w = tw.saturating_sub(16).max(20);
        println!("Session: {} ({} chunks)", detail.session_id, windowed.len());
        println!("{}", "-".repeat(tw));
        for (i, (_, chunk)) in windowed.iter().enumerate() {
            print_chunk_summary(i, chunk, content_w);
        }
    } else if let Some(ref mode) = content_mode {
        if matches!(mode, view::ContentMode::Overview) {
            let overview_entries: Vec<serde_json::Value> = windowed
                .iter()
                .map(|(abs_idx, chunk)| build_overview_entry_cli(*abs_idx, chunk))
                .collect();
            if matches!(format, OutputFormat::Jsonl) {
                for e in &overview_entries {
                    println!("{}", serde_json::to_string(e)?);
                }
            } else {
                let output = serde_json::json!({
                    "sessionId": detail.session_id,
                    "totalChunks": detail.chunks.len(),
                    "returnedChunks": overview_entries.len(),
                    "contentMode": "overview",
                    "chunks": overview_entries,
                });
                emit_json(&output, json_fields)?;
            }
        } else {
            let views: Vec<view::ChunkView> = windowed
                .iter()
                .map(|(abs_idx, chunk)| {
                    let is_hit = grep_hits.contains(abs_idx);
                    let effective_mode = if is_hit {
                        &view::ContentMode::Full
                    } else {
                        mode
                    };
                    let hit_flag = grep_matcher.as_ref().map(|_| is_hit);
                    view::build_chunk_view(*abs_idx, chunk, effective_mode, hit_flag)
                })
                .collect();

            if matches!(format, OutputFormat::Jsonl) {
                for v in &views {
                    println!("{}", serde_json::to_string(v)?);
                }
            } else {
                let output = serde_json::json!({
                    "sessionId": detail.session_id,
                    "totalChunks": detail.chunks.len(),
                    "returnedChunks": views.len(),
                    "contentMode": match mode {
                        view::ContentMode::Omit => "omit",
                        view::ContentMode::Full => "full",
                        view::ContentMode::Overview => unreachable!(),
                    },
                    "chunks": views,
                });
                emit_json(&output, json_fields)?;
            }
        }
    } else if matches!(format, OutputFormat::Jsonl) {
        for (_, chunk) in &windowed {
            println!("{}", serde_json::to_string(chunk)?);
        }
    } else {
        let filtered_chunks: Vec<&cdt_core::Chunk> = windowed.iter().map(|(_, c)| *c).collect();
        let output = serde_json::json!({
            "sessionId": detail.session_id,
            "projectId": detail.project_id,
            "isOngoing": detail.is_ongoing,
            "metrics": detail.metrics,
            "metadata": detail.metadata,
            "chunks": filtered_chunks,
        });
        emit_json(&output, json_fields)?;
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// --extract dispatch
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_extract(
    windowed: &[(usize, &cdt_core::Chunk)],
    mode: &str,
    format: &OutputFormat,
    json_fields: Option<&str>,
) -> Result<()> {
    match mode {
        "overview" => {
            let entries = cdt_query::extract::extract_overview(windowed);
            if matches!(format, OutputFormat::Json) {
                emit_json(&serde_json::to_value(&entries)?, json_fields)?;
            } else if matches!(format, OutputFormat::Jsonl) {
                for e in &entries {
                    println!("{}", serde_json::to_string(e)?);
                }
            } else {
                for e in &entries {
                    let tools_str = if e.tool_names.is_empty() {
                        String::new()
                    } else {
                        format!("  {}", e.tool_names.join(","))
                    };
                    let dur = e
                        .duration_ms
                        .map_or(String::new(), |ms| format!("  {}", format_ms(ms)));
                    println!(
                        "[{:>3}] {:<8} tools={:<3} err={}{}{dur}",
                        e.chunk_index, e.kind, e.tool_count, e.error_count, tools_str,
                    );
                }
            }
        }
        "errors" => {
            let entries = cdt_query::extract::extract_errors(windowed);
            if matches!(format, OutputFormat::Json) {
                emit_json(&serde_json::to_value(&entries)?, json_fields)?;
            } else if matches!(format, OutputFormat::Jsonl) {
                for e in &entries {
                    println!("{}", serde_json::to_string(e)?);
                }
            } else {
                for e in &entries {
                    let msg = e.error_summary.as_deref().unwrap_or("(no details)");
                    println!(
                        "[{:>3}] {}  {}",
                        e.chunk_index,
                        truncate(&e.tool_name, 19),
                        truncate(msg, 80),
                    );
                }
            }
        }
        "tools" => {
            let entries = cdt_query::extract::extract_tool_executions(windowed);
            if matches!(format, OutputFormat::Json) {
                emit_json(&serde_json::to_value(&entries)?, json_fields)?;
            } else if matches!(format, OutputFormat::Jsonl) {
                for e in &entries {
                    println!("{}", serde_json::to_string(e)?);
                }
            } else {
                for e in &entries {
                    let status = if e.is_error { "ERR" } else { "ok " };
                    println!(
                        "[{:>3}.{:<2}] {:<20} {}  {}",
                        e.chunk_index,
                        e.tool_index,
                        truncate(&e.tool_name, 19),
                        status,
                        truncate(&e.input_summary, 60),
                    );
                }
            }
        }
        other => {
            anyhow::bail!("invalid --extract value: '{other}'. Supported: overview, errors, tools");
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// search
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn cmd_search(
    format: &OutputFormat,
    project_filter: Option<&str>,
    query: &str,
    limit: usize,
    offset: usize,
    session_filter: Option<&str>,
    since: Option<&str>,
    json_fields: Option<&str>,
) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let since_ms = since
        .map(cdt_cli::time_expr::parse_time_expr_local)
        .transpose()
        .with_context(|| "invalid --since value")?;

    let project_id = match project_filter {
        Some(name) => Some(
            engine
                .resolve_project(name)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        ),
        None => {
            if let Some(sid) = session_filter {
                Some(
                    engine
                        .find_session_project(sid)
                        .await
                        .map_err(|e| anyhow::anyhow!("{e}"))?,
                )
            } else {
                None
            }
        }
    };

    let mut result = engine
        .search_with_since(query, project_id.as_deref(), session_filter, since_ms)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if result.is_partial {
        eprintln!(
            "warning: search results may be incomplete (some projects could not be searched)"
        );
    }

    if offset > 0 || result.results.len() > limit {
        result.results = result
            .results
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();
    }

    if result.results.is_empty() {
        match format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Jsonl => {}
            OutputFormat::Table => eprintln!("No results found."),
        }
        return Ok(());
    }

    match format {
        OutputFormat::Json => emit_json(&result.results, json_fields)?,
        OutputFormat::Jsonl => {
            for r in &result.results {
                println!("{}", serde_json::to_string(r)?);
            }
        }
        OutputFormat::Table => {
            let tw = term_width();
            let fixed = 38 + 8 + 4; // SESSION + MATCHES + padding
            let flex = tw.saturating_sub(fixed);
            let title_w = flex * 2 / 5;
            let preview_w = flex * 3 / 5;
            println!(
                "{:<38} {:<title_w$} {:>8} {:<preview_w$}",
                "SESSION", "TITLE", "MATCHES", "PREVIEW"
            );
            println!("{}", "-".repeat(tw));
            for r in &result.results {
                let preview = r.hits.first().map_or("", |h| h.preview.as_str());
                println!(
                    "{:<38} {:<title_w$} {:>8} {:<preview_w$}",
                    &r.session_id,
                    truncate(&r.session_title, title_w.saturating_sub(1)),
                    r.total_matches,
                    truncate(preview, preview_w.saturating_sub(1)),
                );
            }
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具函数
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::atomic::{AtomicBool, Ordering};

static NO_TRUNCATE: AtomicBool = AtomicBool::new(false);

fn term_width() -> usize {
    terminal_size::terminal_size().map_or(120, |(w, _)| w.0 as usize)
}

fn shorten_path(path: &str) -> String {
    if let Some(home) = cdt_discover::home_dir() {
        let home_str = home.display().to_string();
        if path.starts_with(&home_str) {
            return format!("~{}", &path[home_str.len()..]);
        }
    }
    path.to_string()
}

fn truncate(s: &str, max_width: usize) -> String {
    if NO_TRUNCATE.load(Ordering::Relaxed) {
        return s.to_string();
    }
    view::truncate_display(s, max_width)
}

fn list_available_fields(command: &Command) {
    let fields: &[&str] = match command {
        Command::Projects { .. } => &["name", "worktrees", "totalSessions", "mostRecentSession"],
        Command::Sessions { action } => match action {
            SessionsAction::List { .. } => &[
                "sessionId",
                "title",
                "timestamp",
                "messageCount",
                "isOngoing",
                "model",
                "gitBranch",
                "cwd",
                "projectId",
                "projectName",
                "userIntents",
                "lastActive",
                "durationMs",
                "totalCost",
                "totalInputTokens",
                "totalOutputTokens",
                "toolErrorCount",
                "filesTouched",
                "gitSummary",
            ],
        },
        Command::Session { chunks: true, .. } => {
            &["sessionId", "chunks", "totalChunks", "contentMode"]
        }
        Command::Session { .. } => &[
            "sessionId",
            "messageCount",
            "chunkCount",
            "durationMs",
            "cost",
            "errorCount",
            "errors",
        ],
        Command::Search { .. } => &["sessionId", "sessionTitle", "totalMatches", "hits"],
        Command::Stats { .. } => &[
            "sessionCount",
            "totalMessages",
            "totalTokens",
            "totalCost",
            "cacheHitRate",
            "avgCostPerSession",
            "avgMessagesPerSession",
            "modelUsage",
            "toolFrequency",
            "languages",
        ],
        _ => &[],
    };
    if fields.is_empty() {
        eprintln!("No JSON fields available for this command.");
    } else {
        for f in fields {
            println!("{f}");
        }
    }
}

fn emit_json(value: &impl serde::Serialize, json_fields: Option<&str>) -> Result<()> {
    let serialized = serde_json::to_value(value)?;
    match json_fields {
        Some(fields_str) if !fields_str.is_empty() => {
            let fields: Vec<&str> = fields_str.split(',').map(str::trim).collect();
            let projected = view::project_fields(serialized, &fields);
            println!("{}", serde_json::to_string(&projected)?);
        }
        _ => {
            println!("{}", serde_json::to_string_pretty(&serialized)?);
        }
    }
    Ok(())
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

/// 解析 `10:30` 格式的 range 为 `(start, end)`。
fn parse_range(s: &str) -> Result<(usize, usize)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        anyhow::bail!(
            "invalid range format: {s} (expected start:end, e.g. 10:30 or 10: for open-ended)"
        );
    }
    let start: usize = parts[0]
        .parse()
        .with_context(|| format!("invalid range start: {}", parts[0]))?;
    let end: usize = if parts[1].is_empty() {
        usize::MAX
    } else {
        parts[1]
            .parse()
            .with_context(|| format!("invalid range end: {}", parts[1]))?
    };
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

fn print_chunk_summary(index: usize, chunk: &cdt_core::Chunk, content_width: usize) {
    match chunk {
        cdt_core::Chunk::User(u) => {
            let text = match &u.content {
                cdt_core::MessageContent::Text(t) => truncate(t, content_width),
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
                    truncate(&tools.join(", "), content_width)
                );
            } else {
                println!(
                    "[{index:>4}] AI: tools=[{}]",
                    truncate(&tools.join(", "), content_width)
                );
            }
        }
        cdt_core::Chunk::System(s) => {
            println!(
                "[{index:>4}] SYSTEM: {}",
                truncate(&s.content_text, content_width)
            );
        }
        cdt_core::Chunk::Compact(c) => {
            println!(
                "[{index:>4}] COMPACT: {}",
                truncate(&c.summary_text, content_width)
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// setup
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_setup_mcp(scope: &SetupScope, dry_run: bool) -> Result<()> {
    let scope_flag = match scope {
        SetupScope::Local => "local",
        SetupScope::Project => "project",
        SetupScope::User => "user",
    };

    let cdt_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| "cdt".to_string());
    let cmd = format!("claude mcp add -s {scope_flag} cdt-devtools -- {cdt_bin} mcp serve");

    if dry_run {
        println!("[dry-run] Would run:\n  {cmd}");
        return Ok(());
    }

    eprintln!("Running: {cmd}");
    let status = std::process::Command::new("claude")
        .args([
            "mcp",
            "add",
            "-s",
            scope_flag,
            "cdt-devtools",
            "--",
            &cdt_bin,
            "mcp",
            "serve",
        ])
        .status()
        .context("Failed to run `claude`. Make sure Claude Code CLI is installed and in PATH.")?;

    if status.success() {
        let location = match scope {
            SetupScope::Local => "~/.claude/settings.local.json",
            SetupScope::Project => ".mcp.json",
            SetupScope::User => "~/.claude/settings.json",
        };
        eprintln!(
            "MCP server \"cdt-devtools\" registered (scope: {scope_flag}, stored in {location})"
        );
        Ok(())
    } else {
        anyhow::bail!("claude mcp add exited with {status}");
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

fn skills_target_dir(scope: &SetupScope) -> Result<PathBuf> {
    match scope {
        SetupScope::Local | SetupScope::Project => Ok(PathBuf::from(".claude/skills")),
        SetupScope::User => {
            let home = cdt_discover::home_dir()
                .context("Cannot determine home directory for user-scope skills installation")?;
            Ok(home.join(".claude/skills"))
        }
    }
}

fn cmd_setup_skills(scope: &SetupScope, dry_run: bool, force: bool) -> Result<()> {
    let target_dir = skills_target_dir(scope)?;
    let count = SKILL_TEMPLATES.len();
    let noun = if count == 1 { "skill" } else { "skills" };
    let scope_label = match scope {
        SetupScope::Local | SetupScope::Project => "project",
        SetupScope::User => "user (~/.claude/skills/)",
    };

    println!(
        "Installing {count} session-aware {noun} to {} (scope: {scope_label})\n",
        target_dir.display()
    );

    if dry_run {
        for skill in SKILL_TEMPLATES {
            let skill_file = target_dir.join(skill.name).join("SKILL.md");
            let exists = skill_file.exists();
            if exists && !force {
                println!("  [dry-run] SKIP  {}/SKILL.md (already exists)", skill.name);
            } else {
                let verb = if exists { "FORCE" } else { "WRITE" };
                println!("  [dry-run] {verb}  {}/SKILL.md", skill.name);
            }
        }
        println!("\nUse without --dry-run to apply.");
        return Ok(());
    }

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
// stats
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_stats(
    format: &OutputFormat,
    period: &str,
    project_filter: Option<&str>,
    group_by: &str,
    json_fields: Option<&str>,
) -> Result<()> {
    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(Arc::clone(&api));

    let since_ms = cdt_cli::time_expr::parse_time_expr_local(period)
        .with_context(|| format!("invalid period: {period}"))?;
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
        let resp = match api.list_sessions_sync(pid, &pagination).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("warning: skipping project {pid}: {e}");
                continue;
            }
        };

        for session in &resp.items {
            if session.timestamp < since_ms {
                continue;
            }
            match api.get_session_detail(pid, &session.session_id, None).await {
                Ok(cdt_api::SessionDetailResponse::Full { detail, .. }) => {
                    session_data_list.push(stats::build_session_data(&detail));
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("warning: skipping session {}: {e}", session.session_id);
                }
            }
        }
    }

    if session_data_list.is_empty() {
        match format {
            OutputFormat::Json => println!("{{\"sessionCount\": 0}}"),
            OutputFormat::Jsonl => println!("{{\"sessionCount\":0}}"),
            OutputFormat::Table => eprintln!("No sessions found in the given period."),
        }
        return Ok(());
    }

    if group_by != "none" {
        let grouped = group_stats_data_cli(&session_data_list, group_by, since_dt);
        match format {
            OutputFormat::Json => emit_json(&grouped, json_fields)?,
            OutputFormat::Jsonl => println!("{}", serde_json::to_string(&grouped)?),
            OutputFormat::Table => {
                if let Some(groups) = grouped["groups"].as_array() {
                    for g in groups {
                        let key = g["key"].as_str().unwrap_or("?");
                        println!("\n=== {key} ===");
                        if let Ok(stats_val) =
                            serde_json::from_value::<stats::AggregatedStats>(g["stats"].clone())
                        {
                            print_stats_table(&stats_val);
                        }
                    }
                }
            }
        }
        return Ok(());
    }

    let result = stats::aggregate(&session_data_list, since_dt);

    match format {
        OutputFormat::Json => emit_json(&result, json_fields)?,
        OutputFormat::Jsonl => {
            println!("{}", serde_json::to_string(&result)?);
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
        "Sessions: {}  Messages: {} (avg {:.1}/session)",
        s.session_count, s.total_messages, s.avg_messages_per_session,
    );
    println!(
        "Tokens: {} (input: {}, output: {}, cache_read: {}, cache_write: {})",
        s.total_tokens,
        s.input_tokens,
        s.output_tokens,
        s.cache_read_tokens,
        s.cache_creation_tokens,
    );
    println!("Cache Hit Rate: {:.1}%", s.cache_hit_rate * 100.0);
    println!(
        "Total Cost: ${:.4} (avg ${:.4}/session)",
        s.total_cost, s.avg_cost_per_session,
    );
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

    if !s.languages.is_empty() {
        println!("Languages:");
        for l in s.languages.iter().take(10) {
            println!("  {:<25} {:>5} files", l.language, l.file_count);
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

async fn resolve_latest_cli(engine: &QueryEngine, session_id: &str) -> Result<String> {
    if session_id != "latest" {
        return Ok(session_id.to_string());
    }
    let filter = QueryFilter {
        limit: Some(1),
        ..Default::default()
    };
    let sessions = engine
        .list_sessions_cross_project(&filter)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    sessions
        .first()
        .map(|s| s.session_id.clone())
        .ok_or_else(|| anyhow::anyhow!("No sessions found for 'latest'"))
}

fn group_stats_data_cli(
    sessions: &[stats::SessionData],
    group_by: &str,
    since: chrono::DateTime<chrono::Utc>,
) -> serde_json::Value {
    let mut keys: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<&stats::SessionData>> =
        std::collections::HashMap::new();
    for s in sessions {
        let key = match group_by {
            "model" => s.model.clone(),
            "day" => chrono::DateTime::from_timestamp_millis(s.timestamp).map_or_else(
                || "unknown".to_string(),
                |dt| dt.format("%Y-%m-%d").to_string(),
            ),
            _ => "all".to_string(),
        };
        if !groups.contains_key(&key) {
            keys.push(key.clone());
        }
        groups.entry(key).or_default().push(s);
    }

    let total = stats::aggregate(sessions, since);
    let group_results: Vec<serde_json::Value> = keys
        .iter()
        .filter_map(|key| {
            groups.get(key).map(|items| {
                let owned: Vec<stats::SessionData> = items.iter().map(|s| (*s).clone()).collect();
                let agg = stats::aggregate(&owned, since);
                serde_json::json!({
                    "key": key,
                    "stats": agg,
                })
            })
        })
        .collect();

    serde_json::json!({
        "groupBy": group_by,
        "total": total,
        "groups": group_results,
    })
}

fn build_overview_entry_cli(abs_index: usize, chunk: &cdt_core::Chunk) -> serde_json::Value {
    match chunk {
        cdt_core::Chunk::Ai(ai) => {
            let tool_names: Vec<&str> = ai
                .tool_executions
                .iter()
                .map(|te| te.tool_name.as_str())
                .collect();
            let error_count = ai.tool_executions.iter().filter(|te| te.is_error).count();
            let headline = ai
                .responses
                .first()
                .map(|r| {
                    let text = view::message_content_text(&r.content);
                    text.chars().take(100).collect::<String>()
                })
                .unwrap_or_default();
            serde_json::json!({
                "chunkIndex": abs_index,
                "kind": "ai",
                "timestamp": ai.timestamp.to_rfc3339(),
                "toolNames": tool_names,
                "errorCount": error_count,
                "headline": headline,
            })
        }
        cdt_core::Chunk::User(user) => {
            let text = view::message_content_text(&user.content);
            let headline: String = text.chars().take(100).collect();
            serde_json::json!({
                "chunkIndex": abs_index,
                "kind": "user",
                "timestamp": user.timestamp.to_rfc3339(),
                "toolNames": [],
                "errorCount": 0,
                "headline": headline,
            })
        }
        cdt_core::Chunk::System(sys) => {
            let headline: String = sys.content_text.chars().take(100).collect();
            serde_json::json!({
                "chunkIndex": abs_index,
                "kind": "system",
                "timestamp": sys.timestamp.to_rfc3339(),
                "toolNames": [],
                "errorCount": 0,
                "headline": headline,
            })
        }
        cdt_core::Chunk::Compact(compact) => {
            serde_json::json!({
                "chunkIndex": abs_index,
                "kind": "compact",
                "timestamp": compact.timestamp.to_rfc3339(),
                "toolNames": [],
                "errorCount": 0,
                "headline": compact.summary_text.chars().take(100).collect::<String>(),
            })
        }
    }
}

fn group_sessions_cli(
    sessions: &[cdt_api::SessionSummary],
    group_by: &str,
) -> Vec<serde_json::Value> {
    let mut keys: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<&cdt_api::SessionSummary>> =
        std::collections::HashMap::new();
    for s in sessions {
        let key = match group_by {
            "project" => s.project_name.as_deref().unwrap_or("(unknown)").to_string(),
            "day" => chrono::DateTime::from_timestamp_millis(s.timestamp).map_or_else(
                || "unknown".to_string(),
                |dt| dt.format("%Y-%m-%d").to_string(),
            ),
            _ => "all".to_string(),
        };
        if !groups.contains_key(&key) {
            keys.push(key.clone());
        }
        groups.entry(key).or_default().push(s);
    }
    keys.iter()
        .filter_map(|key| {
            groups.get(key).map(|items| {
                serde_json::json!({
                    "key": key,
                    "count": items.len(),
                    "sessions": items,
                })
            })
        })
        .collect()
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
    clap_complete::CompleteEnv::with_factory(Cli::command)
        .var(completions::ENV_VAR)
        .complete();
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

    if cli.no_truncate {
        NO_TRUNCATE.store(true, Ordering::Relaxed);
    }

    // --json overrides format; empty string means "list available fields"
    let (effective_format, json_fields) = if let Some(ref json_arg) = cli.json {
        if json_arg.is_empty() {
            list_available_fields(&cli.command);
            return Ok(());
        }
        (OutputFormat::Json, Some(json_arg.as_str()))
    } else {
        (cli.format, None)
    };

    match cli.command {
        Command::Completions { shell } => {
            let script = completions::generate_script(shell)?;
            std::io::Write::write_all(&mut std::io::stdout(), &script)?;
            Ok(())
        }
        Command::Serve => run_serve().await,
        Command::Projects { action } => match action {
            ProjectsAction::List => cmd_projects_list(&effective_format, json_fields).await,
        },
        Command::Sessions { action } => match action {
            SessionsAction::List {
                limit,
                since,
                until,
                branch,
                is_ongoing,
                grep,
                min_messages,
                group_by,
            } => {
                cmd_sessions_list(
                    &effective_format,
                    cli.project.as_deref(),
                    limit,
                    since.as_deref(),
                    until.as_deref(),
                    branch.as_deref(),
                    is_ongoing,
                    grep.as_deref(),
                    min_messages,
                    &group_by,
                    json_fields,
                )
                .await
            }
        },
        Command::Session {
            id,
            chunks,
            include,
            range,
            tail,
            filter,
            content,
            grep,
            grep_context,
            all,
            extract,
        } => {
            if chunks {
                cmd_sessions_detail(
                    &effective_format,
                    &id,
                    range.as_deref(),
                    tail,
                    filter.as_deref(),
                    all,
                    grep.as_deref(),
                    grep_context,
                    content.as_deref(),
                    extract.as_deref(),
                    json_fields,
                )
                .await
            } else {
                cmd_session_inspect(&effective_format, &id, include.as_deref(), json_fields).await
            }
        }
        Command::Export {
            id,
            export_format,
            output,
            detail,
            no_thinking,
            no_subagents,
            range,
            tail,
            grep,
            grep_context,
            filter,
            all,
        } => {
            cmd_export(
                &id,
                &export_format,
                output.as_deref(),
                &detail,
                no_thinking,
                no_subagents,
                range.as_deref(),
                tail,
                grep.as_deref(),
                grep_context,
                filter.as_deref(),
                all,
            )
            .await
        }
        Command::Search {
            query,
            limit,
            offset,
            session,
            since,
        } => {
            cmd_search(
                &effective_format,
                cli.project.as_deref(),
                &query,
                limit,
                offset,
                session.as_deref(),
                since.as_deref(),
                json_fields,
            )
            .await
        }
        Command::Stats {
            period,
            project,
            group_by,
        } => {
            let proj = project.as_deref().or(cli.project.as_deref());
            cmd_stats(&effective_format, &period, proj, &group_by, json_fields).await
        }
        Command::Mcp { action } => match action {
            McpAction::Serve { allow_sensitive } => {
                let api = build_local_data_api().await?;
                let engine = Arc::new(QueryEngine::new(api));
                mcp::run_mcp_server(engine, allow_sensitive).await
            }
        },
        Command::Setup {
            action,
            scope,
            dry_run,
            force,
        } => match action {
            Some(SetupAction::Mcp) => cmd_setup_mcp(&scope, dry_run),
            Some(SetupAction::Skills) => cmd_setup_skills(&scope, dry_run, force),
            Some(SetupAction::Completions) => completions::install(dry_run),
            None => {
                let mcp_result = cmd_setup_mcp(&scope, dry_run);
                let skills_result = cmd_setup_skills(&scope, dry_run, force);
                if let Err(e) = completions::install(dry_run) {
                    eprintln!("Note: shell completions skipped ({e:#})");
                }
                mcp_result.and(skills_result)
            }
        },
        Command::SelfUpdate {
            check,
            version,
            install_path,
        } => {
            update::run(update::UpdateOptions {
                check_only: check,
                target_version: version,
                install_path,
            })
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_normal() {
        assert_eq!(parse_range("10:20").unwrap(), (10, 20));
    }

    #[test]
    fn parse_range_single_chunk() {
        assert_eq!(parse_range("5:6").unwrap(), (5, 6));
    }

    #[test]
    fn parse_range_open_ended() {
        assert_eq!(parse_range("10:").unwrap(), (10, usize::MAX));
    }

    #[test]
    fn parse_range_zero_start() {
        assert_eq!(parse_range("0:5").unwrap(), (0, 5));
    }

    #[test]
    fn parse_range_rejects_inverted() {
        assert!(parse_range("20:10").is_err());
    }

    #[test]
    fn parse_range_rejects_empty_start() {
        assert!(parse_range(":10").is_err());
    }

    #[test]
    fn parse_range_rejects_non_numeric() {
        assert!(parse_range("abc:10").is_err());
    }

    #[test]
    fn parse_range_rejects_no_colon() {
        assert!(parse_range("1020").is_err());
    }
}
