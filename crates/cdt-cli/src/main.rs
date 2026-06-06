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
use cdt_query::{ChunkKindFilter, QueryEngine, QueryFilter, SessionQueryOptions};
use cdt_query::{cost, stats, summary};
use cdt_ssh::SshConnectionManager;

mod mcp;
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

        /// 限定到单个 session（intra-session search）
        #[arg(long)]
        session: Option<String>,
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
        #[arg(add = ArgValueCompleter::new(completions::SessionCompleter))]
        id: String,
    },
    /// 显示会话详情（chunk 流）
    Detail {
        /// 会话 ID
        #[arg(add = ArgValueCompleter::new(completions::SessionCompleter))]
        id: String,

        /// 指定 chunk 区间（如 10:30），与 --tail 互斥
        #[arg(long, conflicts_with = "tail")]
        range: Option<String>,

        /// 仅显示最后 N 条 chunks，与 --range 互斥
        #[arg(long, conflicts_with = "range")]
        tail: Option<usize>,

        /// 过滤条件：`errors_only` 或 `tool_calls`
        #[arg(long)]
        filter: Option<String>,

        /// 返回全部 chunk，禁用默认 tail=20
        #[arg(long, visible_alias = "full")]
        all: bool,

        /// 按内容匹配过滤 chunks（case-insensitive literal substring）
        #[arg(long)]
        grep: Option<String>,

        /// grep 命中周围的 context chunk 数（默认 1）
        #[arg(long, default_value = "1")]
        grep_context: usize,

        /// JSON/JSONL 输出的内容模式：omit（结构概览）或 full（完整内容）
        #[arg(long)]
        content: Option<String>,
    },
    /// 聚合会话中的所有错误
    Errors {
        /// 会话 ID
        #[arg(add = ArgValueCompleter::new(completions::SessionCompleter))]
        id: String,
    },
    /// 会话结构化诊断摘要
    Summary {
        /// 会话 ID
        #[arg(add = ArgValueCompleter::new(completions::SessionCompleter))]
        id: String,
    },
    /// 会话 token 费用估算
    Cost {
        /// 会话 ID
        #[arg(add = ArgValueCompleter::new(completions::SessionCompleter))]
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

async fn cmd_sessions_list(
    format: &OutputFormat,
    project_filter: Option<&str>,
    limit: usize,
    since: Option<&str>,
    grep: Option<&str>,
    min_messages: Option<usize>,
    json_fields: Option<&str>,
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
            let fixed = 38 + 10 + 8 + 8 + 8; // ID + DURATION + STATUS + MESSAGES + padding
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
// sessions show
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_sessions_show(
    format: &OutputFormat,
    session_id: &str,
    json_fields: Option<&str>,
) -> Result<()> {
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
        OutputFormat::Json => emit_json(&detail, json_fields)?,
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
    json_fields: Option<&str>,
) -> Result<()> {
    let content_mode = match content {
        None => None,
        Some("omit") => Some(view::ContentMode::Omit),
        Some("full") => Some(view::ContentMode::Full),
        Some(other) => {
            anyhow::bail!("invalid --content value: '{other}'. Supported: omit, full");
        }
    };

    let api = build_local_data_api().await?;
    let engine = QueryEngine::new(api);

    let project_id = engine
        .find_session_project(session_id)
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
        .get_session_detail(&project_id, session_id, &options)
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

    if windowed.is_empty() && range.is_some() {
        let range_str = range.unwrap_or("");
        eprintln!(
            "hint: 0 chunks matched. --range uses [start, end) semantics (left-inclusive, \
             right-exclusive by chunkIndex). \"{range_str}\" → try \
             adjusting end to be at least start+1."
        );
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
        // --content 指定时构造结构化 ChunkView
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
                },
                "chunks": views,
            });
            emit_json(&output, json_fields)?;
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
// sessions errors
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_sessions_errors(
    format: &OutputFormat,
    session_id: &str,
    json_fields: Option<&str>,
) -> Result<()> {
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
        return Ok(());
    }

    match format {
        OutputFormat::Json => emit_json(&errors, json_fields)?,
        OutputFormat::Jsonl => {
            for e in &errors {
                println!("{}", serde_json::to_string(e)?);
            }
        }
        OutputFormat::Table => {
            let tw = term_width();
            let fixed = 6 + 20 + 4; // CHUNK + TOOL + padding
            let error_w = tw.saturating_sub(fixed).max(20);
            println!("{:>6} {:<20} {:<error_w$}", "CHUNK", "TOOL", "ERROR");
            println!("{}", "-".repeat(tw));
            for e in &errors {
                let msg = e.error_message.as_deref().unwrap_or("(no message)");
                println!(
                    "{:>6} {:<20} {:<error_w$}",
                    e.chunk_index,
                    truncate(&e.tool_name, 19),
                    truncate(msg, error_w.saturating_sub(1)),
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
    session_filter: Option<&str>,
    json_fields: Option<&str>,
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
        .search(query, project_id.as_deref(), session_filter)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

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
            ],
            SessionsAction::Show { .. } => &[
                "sessionId",
                "projectId",
                "title",
                "isOngoing",
                "metrics",
                "metadata",
            ],
            SessionsAction::Detail { .. } => &["sessionId", "chunks", "totalChunks", "contentMode"],
            SessionsAction::Errors { .. } => {
                &["chunkIndex", "toolName", "toolUseId", "errorMessage"]
            }
            SessionsAction::Summary { .. } => &[
                "sessionId",
                "messageCount",
                "errorCount",
                "cost",
                "phases",
                "toolUsage",
                "topFiles",
            ],
            SessionsAction::Cost { .. } => &[
                "model",
                "totalTokens",
                "totalCost",
                "inputTokens",
                "outputTokens",
                "cacheReadTokens",
                "cacheCreationTokens",
            ],
        },
        Command::Search { .. } => &["sessionId", "sessionTitle", "totalMatches", "hits"],
        Command::Stats { .. } => &[
            "sessionCount",
            "totalMessages",
            "totalTokens",
            "totalCost",
            "modelUsage",
            "toolFrequency",
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

/// 解析 `7d` / `24h` / `30m` 格式的 duration 为截止时间戳（毫秒）。
fn parse_duration_to_ms(s: &str) -> Result<i64> {
    let s = s.trim();
    let split_pos = s.char_indices().next_back().map_or(0, |(i, _)| i);
    let (num_str, unit) = s.split_at(split_pos);
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
// sessions summary
// ─────────────────────────────────────────────────────────────────────────────

async fn cmd_sessions_summary(
    format: &OutputFormat,
    session_id: &str,
    json_fields: Option<&str>,
) -> Result<()> {
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
        OutputFormat::Json => emit_json(&output, json_fields)?,
        OutputFormat::Jsonl => {
            println!("{}", serde_json::to_string(&output)?);
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

async fn cmd_sessions_cost(
    format: &OutputFormat,
    session_id: &str,
    json_fields: Option<&str>,
) -> Result<()> {
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
        OutputFormat::Json => emit_json(&output, json_fields)?,
        OutputFormat::Jsonl => {
            println!("{}", serde_json::to_string(&output)?);
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
    json_fields: Option<&str>,
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
            OutputFormat::Json => println!("{{\"sessionCount\": 0}}"),
            OutputFormat::Jsonl => println!("{{\"sessionCount\":0}}"),
            OutputFormat::Table => eprintln!("No sessions found in the given period."),
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
                grep,
                min_messages,
            } => {
                cmd_sessions_list(
                    &effective_format,
                    cli.project.as_deref(),
                    limit,
                    since.as_deref(),
                    grep.as_deref(),
                    min_messages,
                    json_fields,
                )
                .await
            }
            SessionsAction::Show { id } => {
                cmd_sessions_show(&effective_format, &id, json_fields).await
            }
            SessionsAction::Detail {
                id,
                range,
                tail,
                filter,
                all,
                grep,
                grep_context,
                content,
            } => {
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
                    json_fields,
                )
                .await
            }
            SessionsAction::Errors { id } => {
                cmd_sessions_errors(&effective_format, &id, json_fields).await
            }
            SessionsAction::Summary { id } => {
                cmd_sessions_summary(&effective_format, &id, json_fields).await
            }
            SessionsAction::Cost { id } => {
                cmd_sessions_cost(&effective_format, &id, json_fields).await
            }
        },
        Command::Search {
            query,
            limit,
            offset,
            session,
        } => {
            cmd_search(
                &effective_format,
                cli.project.as_deref(),
                &query,
                limit,
                offset,
                session.as_deref(),
                json_fields,
            )
            .await
        }
        Command::Stats { period, project } => {
            let proj = project.as_deref().or(cli.project.as_deref());
            cmd_stats(&effective_format, &period, proj, json_fields).await
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
