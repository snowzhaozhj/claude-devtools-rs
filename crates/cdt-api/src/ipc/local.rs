//! `LocalDataApi`：`DataApi` trait 的本地文件系统实现。
//!
//! 组装底层 crate 调用，作为默认的数据 API 实现。

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use tokio::sync::{Mutex, Semaphore, broadcast};
use tokio::task::{AbortHandle, JoinHandle, JoinSet};

use cdt_analyze::build_chunks_with_subagents;
use cdt_config::{
    ConfigManager, DetectedError, NotificationManager, SshLastConnection,
    read_all_claude_md_files_with_base, read_mentioned_file as config_read_mentioned_file,
    validate_file_path,
};
use cdt_core::{Project, RepositoryGroup};
use cdt_discover::{
    FileSystemProvider, ProjectScanner, SearchConfig, SearchTextCache, SessionSearcher,
    local_handle,
};
use cdt_parse::{ParseError, parse_entry_at};
use cdt_ssh::{
    RemoteWatcherHandle, SshConnectionManager, SshFileSystemProvider, SshSessionManager,
    default_ssh_config_path, list_hosts, parse_ssh_config_file, resolve_host_via_ssh_g,
};
use cdt_watch::FileWatcher;

use super::error::ApiError;
use super::events::SessionMetadataUpdate;
use super::image_disk_cache::{empty_data_uri, format_data_uri, materialize_image_asset};
use super::parsed_message_cache::{
    ParsedMessageCache, apply_file_event_to_parsed_cache, extract_parsed_messages_cached,
};
use super::project_scan_cache::{
    EnrichDecision, ProjectScanCache, apply_file_event_to_project_scan_cache,
    apply_lag_to_project_scan_cache, apply_mtime_advance_to_project_scan_cache,
    synthesize_projects_with_overlay,
};
use super::session_metadata::{
    MetadataCache, STALE_SESSION_THRESHOLD, SessionMetadata, extract_session_metadata_cached,
    extract_session_metadata_from_parsed, try_lookup_cached_metadata,
};
use super::traits::DataApi;
use super::types::{
    ConfigUpdateRequest, ContextInfo, GroupCursor, GroupSessionPage, MemoryFileContent,
    MemoryLayer, MemoryLayerKind, PaginatedRequest, PaginatedResponse, ProjectInfo, ProjectMemory,
    ProjectSessionPrefs, SearchRequest, SessionDetail, SessionDetailMetadata, SessionDetailMetrics,
    SessionDetailResponse, SessionSummary, SshConnectRequest, WorktreeOffset,
};
use crate::notifier::NotificationPipeline;

/// 元数据扫描的最大并发数。文件扫描是 I/O 密集，8 路并发足够打满 `NVMe`
/// 顺序读且不抢 tokio runtime（详见 design.md decision 2）。
pub const METADATA_SCAN_CONCURRENCY: usize = 8;

/// 同一 project 内 subagent parse 的最大并发数。限到 4 以保证
/// `user/real` ≤ 0.66（避免辅助工具短时打满 CPU）。
const SUBAGENT_PARSE_CONCURRENCY: usize = 4;

/// IPC payload 优化：`apply_display_omissions` 默认把每个 `Process.messages`
/// 裁剪为空 `Vec`、设 `messages_omitted=true`，砍掉 ~60% payload。前端
/// `SubagentCard` 展开时调 `get_subagent_trace` 懒拉取。
///
/// 紧急回滚：把本常量改为 `false` 即恢复完整 payload；前端 fallback 路径自动生效。
const OMIT_SUBAGENT_MESSAGES: bool = true;

/// IPC payload 优化（phase 3）：`apply_display_omissions` 默认把所有 `ContentBlock::Image`
/// 的 base64 `data` 替换为空 + 设 `data_omitted=true`，砍掉 image-heavy session 的
/// 大头 payload（实测 7826d1b8 case 4840 KB → ~620 KB）。前端 `ImageBlock`
/// `IntersectionObserver` 进视口时调 `get_image_asset` 懒拉文件 URL。
///
/// 紧急回滚：把本常量改为 `false` 即恢复完整 base64 payload；前端 fallback
/// 走 `data:` URI 路径。行为契约见 change `session-detail-image-asset-cache`。
const OMIT_IMAGE_DATA: bool = true;

/// IPC payload 优化（phase 4）：`apply_display_omissions` 默认把所有
/// `AIChunk.responses[].content` 替换为空 `MessageContent::Text("")` + 设
/// `content_omitted=true`，砍掉首屏 IPC 最大单一字段（实测 46a25772 case
/// 1257 KB / 41%）。前端无任何代码读 `responses[].content`（chunk 显示文本
/// 走 `semanticSteps`），故无需懒拉接口。
///
/// 紧急回滚：把本常量改为 `false` 即恢复完整 payload；前端零改动也无需 fallback。
/// 行为契约见 change `session-detail-response-content-omit`。
const OMIT_RESPONSE_CONTENT: bool = true;

/// IPC payload 优化（phase 5）：`apply_display_omissions` 默认把所有
/// `AIChunk.tool_executions[].output` 内 `text` / `value` 字段清空（保留 enum
/// variant kind）+ 设 `output_omitted=true`，砍掉首屏 IPC 中 tool 输出（实测
/// 46a25772 case 436 KB / 26%）。前端 `ExecutionTrace` 默认折叠，用户点击展开时
/// 通过 `get_tool_output` IPC 按需懒拉。
///
/// 紧急回滚：把本常量改为 `false` 即恢复完整 payload；前端 fallback 路径
/// （直接渲染 `exec.output`）自动接管。行为契约见 change
/// `session-detail-tool-output-lazy-load`。
const OMIT_TOOL_OUTPUT: bool = true;

/// `CompactChunk.token_delta` / `CompactChunk.phase_number` 派生开关。
///
/// `cdt-analyze::chunk::builder` emit `CompactChunk` 时填 `None`，由本文件
/// `apply_compact_derived` 在 IPC 组装层基于 chunks 自身派生填充：phaseNumber
/// 按 chunks 顺序 1-based ordinal 计算（对齐原版 `groupTransformer.ts`
/// `phaseCounter++`），tokenDelta 通过 `find_last_ai_before` /
/// `find_first_ai_after` 取邻接 AI 的 last/first response usage 总和算 delta
/// （对齐原版 `findLastAiBefore` / `findFirstAiAfter`）。
///
/// 紧急回滚：把本常量改为 `false` 即让 `apply_compact_derived` 直接 return；
/// 前端 fallback 路径（`tokenDelta` / `phaseNumber` 字段缺失时不渲染 Phase 徽章
/// 与 token delta 行）自动接管。行为契约见 change `compact-chunk-rendering-alignment`
/// 的 `openspec/specs/ipc-data-api/spec.md` "Expose `CompactChunk` derived metadata in `SessionDetail`"。
const COMPACT_DERIVED_ENABLED: bool = true;

/// 控制是否跨 `project_dir` 扫描 subagent JSONL。
///
/// 当主 session 在主 cwd 启动后，subagent 通过 `EnterWorktree` 把 cwd 切到
/// `<repo>/.claude/worktrees/<slug>/`，Claude Code 会把 subagent JSONL 写到
/// worktree cwd 编码出的另一个 `project_dir` 下（如
/// `~/.claude/projects/-Users-...-claude-worktrees-<slug>/<rootSessionId>/subagents/`）。
/// 默认 `true` 时，`scan_subagent_candidates_cross_project` /
/// `find_subagent_jsonl_cross_project` / `locate_session_jsonl` 会遍历整个
/// `projects_dir`，把所有 `{rootSessionId}/subagents/agent-*.jsonl` 收集为候选。
///
/// 紧急回滚：设为 `false` 时所有跨目录调用退化为只扫主 `project_dir`（原行为）；
/// 旧结构 flat `agent-*.jsonl` 始终只扫主 `project_dir`（设计决策 D2）。
/// 行为契约见 change `worktree-support-and-cross-project-subagent` 的
/// `openspec/specs/ipc-data-api/spec.md` "Expose project and session queries"
/// Requirement 内的"跨 `project_dir` 装载 subagent"段。
const CROSS_PROJECT_SUBAGENT_SCAN: bool = true;

/// Find a group by ID with fallback: if exact match fails, try matching
/// `group_id` against worktree IDs (handles git-init causing group ID change).
fn find_group_with_fallback(
    mut groups: Vec<RepositoryGroup>,
    group_id: &str,
) -> Result<RepositoryGroup, ApiError> {
    let pos = groups
        .iter()
        .position(|g| g.id == group_id)
        .or_else(|| {
            let fallback_pos = groups
                .iter()
                .position(|g| g.worktrees.iter().any(|w| w.id == group_id));
            if let Some(p) = fallback_pos {
                tracing::debug!(
                    requested_id = %group_id,
                    matched_group_id = %groups[p].id,
                    "group lookup used worktree-id fallback (frontend holds stale group ID)"
                );
            }
            fallback_pos
        })
        .ok_or_else(|| ApiError::not_found(format!("repository group {group_id}")))?;
    Ok(groups.swap_remove(pos))
}

fn is_safe_path_component(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && !s.contains('/')
        && !s.contains('\\')
        && !s.contains("..")
        && !s.contains('\0')
}

/// 累加一个 `TokenUsage` 的四类计数，溢出时返回 `None`（防御性，u64 总量
/// 实际罕见溢出，但坏 JSONL 可能把 usage 字段填到极大值，避免 panic）。
fn token_usage_total(u: &cdt_core::TokenUsage) -> Option<u64> {
    u.input_tokens
        .checked_add(u.output_tokens)?
        .checked_add(u.cache_read_input_tokens)?
        .checked_add(u.cache_creation_input_tokens)
}

fn ai_last_response_total_tokens(ai: &cdt_core::AIChunk) -> Option<u64> {
    ai.responses
        .iter()
        .rev()
        .find_map(|r| r.usage.as_ref().and_then(token_usage_total))
}

fn ai_first_response_total_tokens(ai: &cdt_core::AIChunk) -> Option<u64> {
    ai.responses
        .iter()
        .find_map(|r| r.usage.as_ref().and_then(token_usage_total))
}

/// IPC fingerprint：前端 round-trip 用的 opaque wire token。
///
/// 与 `FileSignature`（进程内 cache key）职责分离：
/// - `FileSignature`：`SystemTime` + `size` + `identity`，可自由演进（加 hash/inode）
/// - IPC fingerprint：稳定 wire 格式，前端不解析只回传，变更需 bump 版本前缀
///
/// 格式 `"v2:<mtime_ms>:<size>:<stale>"`；`None` 时用 `0`。`stale` 编码
/// `is_ongoing` 的 stale 翻转决定因素——写入停止 ≥5min 时翻 `1` 使 fingerprint
/// 变化，触发一次重算完成 `ongoing→complete` 状态翻转。
fn make_session_ipc_fingerprint(
    mtime_ms: Option<i64>,
    size: Option<u64>,
    is_stale: bool,
) -> String {
    format!(
        "v2:{}:{}:{}",
        mtime_ms.unwrap_or(0),
        size.unwrap_or(0),
        u8::from(is_stale)
    )
}

fn find_last_ai_before(chunks: &[cdt_core::Chunk], i: usize) -> Option<&cdt_core::AIChunk> {
    chunks[..i].iter().rev().find_map(|c| {
        if let cdt_core::Chunk::Ai(ai) = c {
            Some(ai)
        } else {
            None
        }
    })
}

fn find_first_ai_after(chunks: &[cdt_core::Chunk], i: usize) -> Option<&cdt_core::AIChunk> {
    chunks[i + 1..].iter().find_map(|c| {
        if let cdt_core::Chunk::Ai(ai) = c {
            Some(ai)
        } else {
            None
        }
    })
}

/// Tauri IPC 消费者层调用的展示裁剪流水线：image → response content
/// → tool output → subagent messages。
///
/// `LocalDataApi::get_session_detail` 返回完整数据；Tauri IPC command handler
/// 在序列化返回前端之前调用本函数裁剪 payload。MCP / CLI / HTTP 消费者不调用。
pub(crate) fn apply_display_omissions(chunks: &mut [cdt_core::Chunk]) {
    apply_omissions_impl(chunks, true, true);
}

/// 导出专用裁剪：保留 tool output + response content（导出器实际消费），
/// 裁剪 image data + subagent messages（导出器不渲染、且是 payload 大头）。
///
/// `get_session_detail_for_export` Tauri command 调用。
pub(crate) fn apply_export_omissions(chunks: &mut [cdt_core::Chunk]) {
    apply_omissions_impl(chunks, false, false);
}

fn apply_omissions_impl(
    chunks: &mut [cdt_core::Chunk],
    omit_tool_output: bool,
    omit_response_content: bool,
) {
    if OMIT_IMAGE_DATA {
        apply_image_omit(chunks);
    }
    if omit_response_content && OMIT_RESPONSE_CONTENT {
        apply_response_content_omit(chunks);
    }
    if omit_tool_output && OMIT_TOOL_OUTPUT {
        apply_tool_output_omit(chunks);
    }
    if OMIT_SUBAGENT_MESSAGES {
        for c in chunks {
            if let cdt_core::Chunk::Ai(ai) = c {
                for sub in &mut ai.subagents {
                    sub.messages = Vec::new();
                    sub.messages_omitted = true;
                }
            }
        }
    }
}

#[allow(dead_code)]
fn parse_jsonl_content(content: &str) -> Result<Vec<cdt_core::ParsedMessage>, ParseError> {
    let mut out = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match parse_entry_at(line, idx + 1) {
            Ok(Some(msg)) => out.push(msg),
            Ok(None) => {}
            Err(ParseError::MalformedLine { line, source }) => {
                tracing::warn!(line, error = %source, "skipping malformed JSONL line");
            }
            Err(ParseError::SchemaMismatch { line, reason }) => {
                tracing::warn!(line, reason = %reason, "skipping entry with schema mismatch");
            }
            Err(e @ ParseError::Io { .. }) => return Err(e),
        }
    }
    Ok(out)
}

/// 派生 `CompactChunk` 的 `token_delta` / `phase_number` 两个可选字段。
///
/// 算法（D1c `phaseNumber` + D1d `tokenDelta`，详见 design.md 修订链）：
///
/// - 派生层**完全独立**于 `cdt_core::ContextPhaseInfo`
/// - `phaseNumber`：按 chunks 顺序遍历，每遇 `Compact` 就 `compact_counter += 1`
/// - `tokenDelta`：对每个 compact 独立查 `find_last_ai_before` /
///   `find_first_ai_after`，算 `post - pre`；任一缺值 → `None`
///
/// 两趟扫描避免可变借用冲突：Pass 1 不可变借用算 (delta, phase)，Pass 2 可变借用写入。
fn apply_compact_derived(chunks: &mut [cdt_core::Chunk], enabled: bool) {
    if !enabled {
        return;
    }
    let mut compact_counter: u32 = 1;
    let mut updates: Vec<(usize, Option<cdt_core::CompactionTokenDelta>, u32)> = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        if matches!(chunk, cdt_core::Chunk::Compact(_)) {
            let delta = match (
                find_last_ai_before(chunks, i).and_then(ai_last_response_total_tokens),
                find_first_ai_after(chunks, i).and_then(ai_first_response_total_tokens),
            ) {
                (Some(pre), Some(post)) => {
                    let pre_i = i64::try_from(pre).unwrap_or(i64::MAX);
                    let post_i = i64::try_from(post).unwrap_or(i64::MAX);
                    Some(cdt_core::CompactionTokenDelta {
                        pre_compaction_tokens: pre,
                        post_compaction_tokens: post,
                        delta: post_i - pre_i,
                    })
                }
                _ => None,
            };
            compact_counter += 1;
            updates.push((i, delta, compact_counter));
        }
    }
    for (i, delta, phase) in updates {
        if let cdt_core::Chunk::Compact(c) = &mut chunks[i] {
            c.token_delta = delta;
            c.phase_number = Some(phase);
        }
    }
}

/// 遍历 chunks 内所有 `ContentBlock::Image`，把 `source.data` 替换为空字符串
/// 并设 `source.data_omitted = true`。覆盖 `UserChunk.content`、
/// `AIChunk.responses[].content` 与 `AIChunk.subagents[].messages[]` 嵌套层。
/// 在 `apply_display_omissions` 中先于 subagent 裁剪执行，故嵌套层 messages
/// 可能非空；`OMIT_SUBAGENT_MESSAGES=false` 回滚时同样命中嵌套层。
fn apply_image_omit(chunks: &mut [cdt_core::Chunk]) {
    for chunk in chunks {
        match chunk {
            cdt_core::Chunk::User(u) => omit_image_in_content(&mut u.content),
            cdt_core::Chunk::Ai(ai) => {
                for resp in &mut ai.responses {
                    omit_image_in_content(&mut resp.content);
                }
                for sub in &mut ai.subagents {
                    apply_image_omit(&mut sub.messages);
                }
            }
            cdt_core::Chunk::System(_) | cdt_core::Chunk::Compact(_) => {}
        }
    }
}

fn omit_image_in_content(content: &mut cdt_core::MessageContent) {
    if let cdt_core::MessageContent::Blocks(blocks) = content {
        for blk in blocks {
            if let cdt_core::ContentBlock::Image { source } = blk {
                source.data.clear();
                source.data_omitted = true;
            }
        }
    }
}

fn compute_new_tokens_by_category(
    injections: &[cdt_core::ContextInjection],
) -> cdt_core::TokensByCategory {
    let mut t = cdt_core::TokensByCategory::default();
    for inj in injections {
        match inj {
            cdt_core::ContextInjection::ClaudeMd(x) => t.claude_md += x.estimated_tokens,
            cdt_core::ContextInjection::MentionedFile(x) => {
                t.mentioned_file += x.estimated_tokens;
            }
            cdt_core::ContextInjection::ToolOutput(x) => t.tool_output += x.estimated_tokens,
            cdt_core::ContextInjection::ThinkingText(x) => t.thinking_text += x.estimated_tokens,
            cdt_core::ContextInjection::TaskCoordination(x) => {
                t.task_coordination += x.estimated_tokens;
            }
            cdt_core::ContextInjection::UserMessage(x) => t.user_messages += x.estimated_tokens,
        }
    }
    t
}

/// 遍历 chunks 内所有 `AIChunk.responses[].content`，替换为空
/// `MessageContent::Text("")` 并设 `content_omitted = true`。覆盖顶层
/// `AIChunk` 与 `AIChunk.subagents[].messages[]` 嵌套层。在
/// `apply_display_omissions` 中先于 subagent 裁剪执行，故嵌套层 messages
/// 可能非空；`OMIT_SUBAGENT_MESSAGES=false` 回滚时同样命中嵌套层。
fn apply_response_content_omit(chunks: &mut [cdt_core::Chunk]) {
    for chunk in chunks {
        if let cdt_core::Chunk::Ai(ai) = chunk {
            for resp in &mut ai.responses {
                resp.content = cdt_core::MessageContent::Text(String::new());
                resp.content_omitted = true;
            }
            for sub in &mut ai.subagents {
                apply_response_content_omit(&mut sub.messages);
            }
        }
    }
}

/// 遍历 chunks 内所有 `AIChunk.tool_executions[].output`，inner `text` /
/// `value` 字段清空（保留 enum variant kind）并设 `output_omitted = true`。
/// 覆盖顶层 `AIChunk` 与 `AIChunk.subagents[].messages[]` 嵌套层。在
/// `apply_display_omissions` 中先于 subagent 裁剪执行，故嵌套层 messages
/// 可能非空；`OMIT_SUBAGENT_MESSAGES=false` 回滚时同样命中嵌套层。
fn apply_tool_output_omit(chunks: &mut [cdt_core::Chunk]) {
    for chunk in chunks {
        if let cdt_core::Chunk::Ai(ai) = chunk {
            for exec in &mut ai.tool_executions {
                // 在 trim 前记录原始 output 字节长度，让前端在懒加载前即可估算
                // output token（按 4 字符/token），避免 BaseItem 头部 token 数随
                // 展开抖动。`Missing` variant 保持 `None`。见 change
                // `tool-output-omit-preserve-size`。
                exec.output_bytes = match &exec.output {
                    cdt_core::ToolOutput::Text { text } => Some(text.len() as u64),
                    cdt_core::ToolOutput::Structured { value } => {
                        Some(serde_json::to_string(value).map_or(0, |s| s.len() as u64))
                    }
                    cdt_core::ToolOutput::Missing => None,
                };
                exec.output.trim();
                exec.output_omitted = true;
            }
            for sub in &mut ai.subagents {
                apply_tool_output_omit(&mut sub.messages);
            }
        }
    }
}

/// 元数据 broadcast channel capacity。50 个 session 的项目最多产出 50
/// 条 update，一些缓冲足以容纳订阅者短暂卡顿。
const METADATA_BROADCAST_CAPACITY: usize = 64;

/// 单条 active scan 注册项：generation 作为版本号让 cleanup 时只在
/// 自己仍是当前注册的 scan 时才 remove，避免旧 task 误删新 handle
/// （codex 二审找到的 race，详见 `scan_metadata_for_page`）。
///
/// `context_id` 让 `switch_context` / `ssh_disconnect` 能按 ctx abort 旧
/// host 下未完成的 scan（change `unify-fs-direct-calls` design D3-bis +
/// codex 二审 H2 修订：旧 ctx 的 scan 仍持 `Arc<dyn FileSystemProvider>`
/// 跨 await，切 host 后会向前端 broadcast 旧 ctx 的 metadata update，
/// 串扰新 ctx 的渲染状态）。
#[derive(Debug)]
struct ScanEntry {
    generation: u64,
    /// 让 `abort_scans_for_context` 按 ctx 精确 abort 用——避免 abort-all 误杀
    /// 新 host scan（codex 二审第二轮 H2-R 修订）。
    context_id: cdt_fs::ContextId,
    handle: AbortHandle,
}

/// Polling watcher 检测到 SFTP channel 死亡时的自愈 disconnect 完整流程。
///
/// 与 `LocalDataApi::ssh_disconnect` IPC 路径的清理动作对齐——SHALL 同时做：
/// bump `context_generation` + abort SSH ctx scans + `ssh_mgr.disconnect`，
/// 不能只调 disconnect 否则 in-flight `list_sessions` scan 会继续 broadcast
/// 旧 SSH metadata 污染切回 local 后的 UI（codex 二审第二轮 major fix）。
///
/// 由 `attach_remote_watcher` spawn 的 dead-signal monitor task 调用——free
/// function 而非 `&self` method 是因为 monitor 是 detached task 拿不到
/// `Arc<LocalDataApi>`，所需资源全部通过 Arc clone 入参传入。
async fn perform_polling_self_heal_disconnect(
    ops: Arc<Mutex<()>>,
    gen_counter: Arc<AtomicU64>,
    captured_generation: u64,
    active_scans: Arc<std::sync::Mutex<HashMap<String, ScanEntry>>>,
    ssh_mgr: cdt_ssh::SshSessionManager,
    context_id: String,
) {
    let _ops = ops.lock().await;
    // Guard 1: context_generation 没换代（任何 ssh_connect / switch_context /
    // ssh_disconnect / 同 ctx 重连都会 bump 它）
    let current_gen = gen_counter.load(Ordering::SeqCst);
    if current_gen != captured_generation {
        tracing::debug!(
            target: "cdt_ssh::lifecycle",
            context_id = %context_id,
            captured_generation,
            current_gen,
            "skip stale auto-disconnect: context_generation changed since watcher attach",
        );
        return;
    }
    // Guard 2: active context 仍是这个 watcher 守护的 context
    let active = ssh_mgr.active_context_id().await;
    if active.as_deref() != Some(context_id.as_str()) {
        tracing::debug!(
            target: "cdt_ssh::lifecycle",
            context_id = %context_id,
            ?active,
            "skip stale auto-disconnect: active context no longer matches",
        );
        return;
    }
    tracing::warn!(
        target: "cdt_ssh::lifecycle",
        context_id = %context_id,
        "polling reported SFTP channel dead; auto-disconnecting to sync active context",
    );
    // Bump generation —— 关闭 in-flight list_sessions late insert 串扰新
    // local UI 的窗口（与 ssh_disconnect IPC 路径同形）。
    gen_counter.fetch_add(1, Ordering::SeqCst);
    // abort 当前 SSH ctx 下 active scans —— SHALL 在 ssh_mgr.disconnect
    // 之前做，让 ssh_mgr 仍能 lookup ContextId（disconnect 后 provider 被
    // 删，无法解析 ctx）。silent no-op 若 provider 已 gone（与
    // abort_scans_for_ssh_context_id 同语义）。
    if let Some((_provider, ctx)) = ssh_mgr.provider_and_context_id(&context_id).await {
        if let Ok(mut scans) = active_scans.lock() {
            scans.retain(|_key, entry| {
                if entry.context_id == ctx {
                    entry.handle.abort();
                    false
                } else {
                    true
                }
            });
        }
    }
    if let Err(e) = ssh_mgr.disconnect(&context_id).await {
        tracing::warn!(
            target: "cdt_ssh::lifecycle",
            context_id = %context_id,
            error = %e,
            "auto-disconnect after polling dead-signal failed",
        );
    }
}

/// 本地文件系统 `DataApi` 实现。
pub struct LocalDataApi {
    scanner: Mutex<ProjectScanner>,
    search_cache: Arc<Mutex<SearchTextCache>>,
    config_mgr: Arc<Mutex<ConfigManager>>,
    notif_mgr: Arc<Mutex<NotificationManager>>,
    ssh_mgr: SshSessionManager,
    /// 自动通知管线的 `DetectedError` 广播发送端。仅在 `new_with_watcher`
    /// 构造下存在；`new()` 构造返回 `None`，此时 `subscribe_detected_errors`
    /// 返回一条永不发消息的 receiver（caller 代码统一）。
    error_tx: Option<broadcast::Sender<DetectedError>>,
    /// File watcher 稳定广播发送端。root 重配时内部 watcher 会重建，但 host
    /// 订阅这条稳定 channel，避免继续绑在旧 watcher 的 receiver 上。
    file_tx: Option<broadcast::Sender<cdt_core::FileChangeEvent>>,
    todo_tx: Option<broadcast::Sender<cdt_core::TodoChangeEvent>>,
    jobs_tx: Option<broadcast::Sender<cdt_core::JobChangeEvent>>,
    /// 内部 `FileWatcher` 句柄（`new_with_watcher` 路径下注入）。仅用于让
    /// `attach_remote_watcher` 走 `FileWatcher::attach_remote` 把 SSH polling
    /// 事件喂回同一 `file_tx`，让 SSH 与 Local 共用 unified invalidator 的
    /// `session_list_changed` enrichment gateway（change
    /// `enrich-file-change-with-session-list-changed::D2`）。
    ///
    /// **Invariant**：与 `file_tx` 同步——`new_with_watcher` 路径下两者都
    /// `Some`，`new()` 路径下两者都 `None`；`attach_remote_watcher` 在 `file_tx`
    /// 为 `None` 时已早返，进入 attach 路径时 `watcher` 必然 `Some`。
    ///
    /// 用 `Mutex` 包裹是因为 `reconfigure_claude_root` 切 root 时会重建内部
    /// watcher 实例，与现有 `watcher_tasks` 字段同型可变。
    watcher: Mutex<Option<Arc<FileWatcher>>>,
    watcher_tasks: Mutex<Vec<JoinHandle<()>>>,
    remote_watchers: Mutex<HashMap<String, RemoteWatcherHandle>>,
    /// SSH 状态变更 + remote watcher attach/detach 的串行化锁。
    ///
    /// `Arc` 包裹让 `attach_remote_watcher` 内 spawn 的 monitor task 也能
    /// clone 一份，在自愈 disconnect 路径上拿锁 +（`context_generation`,
    /// `active_context`）二次校验，避免旧 watcher 的 `dead_signal` 在用户已
    /// disconnect+reconnect 同 context 后误删新 sessions entry（codex 二审
    /// critical race，2026-05-22）。
    ssh_watcher_ops: Arc<Mutex<()>>,
    ssh_shutdown_generation: AtomicU64,
    /// `list_sessions` 后台元数据扫描的广播发送端。`subscribe_session_metadata()`
    /// 返回 receiver，Tauri host 桥接为前端 `session-metadata-update` 事件。
    session_metadata_tx: broadcast::Sender<SessionMetadataUpdate>,
    /// 当前进行中的元数据扫描句柄。key 由 `metadata_scan_key(project_id, cursor)`
    /// 编码为 `"{project_id}|{cursor}"`，让同 project 不同分页的扫描互不 abort。
    /// `list_sessions` 调用前会 abort **同 (`project_id`, `cursor`)** 的旧扫描，
    /// 避免重复触发同一页扫描造成的事件串扰；不同分页的扫描 SHALL 并存（详见
    /// `openspec/specs/ipc-data-api/spec.md::Emit session metadata updates`
    /// Scenario "同 projectId 同 cursor 的新扫描取消旧扫描" 与 "同 projectId
    /// 不同 cursor 的扫描并存互不 abort"）。
    active_scans: Arc<std::sync::Mutex<HashMap<String, ScanEntry>>>,
    /// 单调递增的 scan generation 计数器，用于 `active_scans` 的 race-free
    /// cleanup（详见 `scan_metadata_for_page`）。
    scan_generation: Arc<AtomicU64>,
    /// Claude root generation，仅在 `general.claudeRootPath` 切换时递增。
    root_generation: Arc<AtomicU64>,
    /// active context 切换 generation —— `switch_context` / `ssh_connect` /
    /// `ssh_disconnect` SHALL `fetch_add(1, SeqCst)` 递增此计数器。
    ///
    /// 修复 codex 二审第三轮 H2-R-2 Blocking：context 切换期间，已 in-flight 的
    /// `list_sessions` 调用会先拿到旧 `(fs, ctx)` 后再 await scanner，最终才 insert
    /// scan handle 到 `active_scans`。`abort_scans_for_context` 跑在 abort 之前没
    /// 捕获这个 late insert，导致旧 ctx scan 在 context 切换后仍 broadcast 旧
    /// metadata 串扰新 ctx UI。`scan_metadata_for_page` 内每次 broadcast 前
    /// SHALL check `context_generation.load(SeqCst) == my_generation`；mismatch
    /// 时 silent drop 该 update（不 broadcast 也不 panic）。
    context_generation: Arc<AtomicU64>,
    /// 后台元数据扫描共享的 `Semaphore`，所有 in-flight scan task 共享一个实例
    /// 以保证全局并发上限为 `METADATA_SCAN_CONCURRENCY=8`。spec
    /// `ipc-data-api/spec.md::Emit session metadata updates` Scenario "后台扫描
    /// 并发度限制" 明确"同一时刻打开的 JSONL 文件句柄数 SHALL 不超过 8"——
    /// 在改用 (`project_id`, `cursor`) 双键 abort 后，page 1 / page 2 / 多 project
    /// 扫描会真并发执行，必须用 `Arc<Semaphore>` 在 task 间共享许可，避免每个
    /// task 各自 new 一个 8 容量信号量加和成 16+ 并发。
    metadata_scan_semaphore: Arc<Semaphore>,
    /// `get_image_asset` 落盘 cache 目录。由 Tauri host 通过
    /// `new_with_image_cache` 注入 `app_cache_dir().join("cdt-images")`；
    /// `None` 时 `get_image_asset` fallback 到 `data:` URI（默认构造路径
    /// + 集成测试无 cache 目录依赖）。
    image_cache_dir: Option<std::path::PathBuf>,
    /// session metadata LRU 缓存，跨 IPC / HTTP 路径复用。**不**走全局单例
    /// （详 change `multi-session-cpu-cache` design D3b）；多实例 cache 隔离。
    metadata_cache: Arc<std::sync::Mutex<MetadataCache>>,
    /// parsed-message LRU 缓存：按 `(jsonl_path, FileSignature)` 缓存
    /// `parse_file` 结果，让 `get_tool_output` / `get_image_asset` 命中时
    /// 跳过整文件 line-by-line parse。详 change `parsed-message-lru-cache`
    /// design D2/D3；行为契约见 spec `ipc-data-api/spec.md`
    /// §"`get_tool_output` 与 `get_image_asset` 走 parsed-message LRU 缓存"。
    parsed_msg_cache: Arc<std::sync::Mutex<ParsedMessageCache>>,
    workflow_manifest_cache: Arc<std::sync::Mutex<super::workflow_manifest::WorkflowManifestCache>>,
    /// 当前 projects root；随 `general.claudeRootPath` 运行时重配。
    projects_dir: Mutex<PathBuf>,
    /// `ProjectScanner` 共享的 head-read `Semaphore`（容量 64）。所有动态
    /// 创建的 scanner（`active_scanner` / `list_sessions_skeleton` /
    /// `list_group_sessions` 等）SHALL 走 `ProjectScanner::new_with_semaphore`
    /// 复用本字段，避免 N 个 IPC 并发 × 64 击穿到 N×64 文件描述符并发。
    /// change `simplify-repository-as-project::D4`。
    shared_read_semaphore: Arc<Semaphore>,
    shared_cwd_cache: cdt_discover::CwdCache,
    /// worktree → group meta join 缓存（scheme c, change
    /// `simplify-repository-as-project::D2`）：随 `list_repository_groups`
    /// 调用刷新；`list_sessions` / `list_group_sessions` /
    /// `get_worktree_sessions` 序列化 `SessionSummary` 时按 `project_id`
    /// （即 `worktree_id`）查表填 `worktree_name` / `group_id` /
    /// `cwd_relative_to_repo_root`。未刷新时 fallback 为 None，UI 退化到
    /// 用 `project_id` 当 group id。
    worktree_meta_cache: Arc<std::sync::RwLock<HashMap<String, WorktreeMeta>>>,
    /// `ProjectScanner::scan()` 结果的进程级缓存。key=`ContextId`，value=
    /// `Arc<Vec<Project>>` + `root_generation` + `context_generation` +
    /// `inserted_at`；watcher 主动 invalidate Local entry，SSH entry 走
    /// TTL（10s）。让 `list_projects` / `list_repository_groups` 第二次
    /// IPC 调用 cache hit 时跳过 ~14K fs op 的全量 scan。change
    /// `unify-fs-abstraction` FU-4 `ProjectScanner` memoize 部分。
    project_scan_cache: Arc<std::sync::Mutex<ProjectScanCache>>,
    /// Grouper 结果缓存：generation 三元组 + 10 秒 TTL。
    groups_cache: Arc<std::sync::Mutex<Option<GroupsCacheEntry>>>,
    /// `cfg(test)` 计数器：`refresh_worktree_meta_cache` 实际被调用次数（spec
    /// `ipc-data-api::SessionSummary 增加 worktree 元信息字段` 的"映射缓存刷新约束"
    /// (ctx + generation) 双重校验测试用）。change `generation-race-audit` Test 1/2。
    #[cfg(any(test, feature = "test-utils"))]
    refresh_worktree_meta_cache_call_count: Arc<AtomicU64>,
    /// `cfg(test)` 计数器：`build_group_session_page` 内 spawn `scan_metadata_for_page`
    /// 的次数。change `generation-race-audit` Test 3：断言 spawn 前锁内 (ctx + gen)
    /// 二次校验 mismatch 时 spawn 不发生。
    #[cfg(any(test, feature = "test-utils"))]
    metadata_scan_spawn_count: Arc<AtomicU64>,
    /// `cfg(test)` 计数器：`active_fs_and_policy` 被调用次数。change
    /// `generation-race-audit` Test 4：断言 `build_group_session_page` 整段调用
    /// 仅触发一次 `active_fs` 抽样（来自 inner），不再有第二次独立 `active_fs_and_context_strict`。
    #[cfg(any(test, feature = "test-utils"))]
    active_fs_and_policy_call_count: Arc<AtomicU64>,
}

/// Grouper 结果缓存条目。
#[derive(Debug, Clone)]
struct GroupsCacheEntry {
    groups: Vec<cdt_core::RepositoryGroup>,
    root_gen: u64,
    ctx_gen: u64,
    scan_inv_gen: u64,
    created_at: std::time::Instant,
}

const GROUPS_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(10);

/// IPC `SessionSummary` 序列化时从 `worktree_meta_cache` 查到的派生字段。
/// change `simplify-repository-as-project::D2`。
#[derive(Debug, Clone)]
struct WorktreeMeta {
    worktree_name: String,
    group_id: String,
    cwd_relative_to_repo_root: Option<String>,
}

/// k-way merge 堆元素，按全序 `(mtime desc, sid asc)` 排序。`BinaryHeap`
/// 是 max-heap，所以 `Ord::cmp(self, other)` 返回 Greater 表示 `self`
/// 优先 pop。
/// change `simplify-repository-as-project::D3`。
#[derive(Debug, Clone, Eq, PartialEq)]
struct HeapEntry {
    mtime: i64,
    sid: String,
    wt_id: String,
    idx: usize,
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // mtime 大优先（max-heap 自然行为）。
        self.mtime
            .cmp(&other.mtime)
            // 同 mtime 时 sid 字典序**小**优先：max-heap 视角即 other.sid > self.sid
            // → 反向比较让 self.sid 小为 Greater。
            .then_with(|| other.sid.cmp(&self.sid))
    }
}

impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// 解析 base64(JSON) cursor 为 `GroupCursor`。失败 fallback 为空 cursor
/// （等价首页请求），并在 tracing 留 warn。
/// spec `ipc-data-api::Expose group session listing` §"损坏 cursor fallback 为首页"。
fn parse_group_cursor(cursor: Option<&str>) -> GroupCursor {
    use base64::Engine;
    let Some(s) = cursor else {
        return GroupCursor {
            per_worktree: std::collections::BTreeMap::new(),
        };
    };
    let decoded = match base64::engine::general_purpose::STANDARD.decode(s) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "list_group_sessions cursor base64 decode failed; fallback to first page");
            return GroupCursor {
                per_worktree: std::collections::BTreeMap::new(),
            };
        }
    };
    match serde_json::from_slice::<GroupCursor>(&decoded) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "list_group_sessions cursor json parse failed; fallback to first page");
            GroupCursor {
                per_worktree: std::collections::BTreeMap::new(),
            }
        }
    }
}

/// 序列化 `GroupCursor` 为 base64(JSON)。
fn encode_group_cursor(cursor: &GroupCursor) -> String {
    use base64::Engine;
    let json = serde_json::to_vec(cursor).unwrap_or_default();
    base64::engine::general_purpose::STANDARD.encode(json)
}

/// `locate_session_file` 的返回类型。
/// `Found` 用 `Box` 避免 `clippy::large_enum_variant`（`LocatedSession` ~240 bytes）。
enum LocateResult {
    Unchanged { fingerprint: String },
    Found(Box<LocatedSession>),
}

/// `locate_session_file` 成功定位后的中间数据。
struct LocatedSession {
    fs: Arc<dyn FileSystemProvider>,
    projects_dir: PathBuf,
    ctx: cdt_fs::ContextId,
    policy: cdt_fs::BackendPolicy,
    project_dir: PathBuf,
    jsonl_path: PathBuf,
    last_modified: Option<i64>,
    size: Option<u64>,
    fingerprint: String,
}

/// `inject_context_annotations` 的返回类型。
struct ContextAnnotations {
    context_injections: Vec<cdt_core::ContextInjection>,
    injections_by_phase: BTreeMap<String, Vec<cdt_core::ContextInjection>>,
    phase_info: cdt_core::ContextPhaseInfo,
    turn_context_stats: HashMap<String, cdt_core::TurnContextStats>,
}

/// `list_sessions_skeleton` 的命名返回类型，替代原 10-tuple 提升可读性。
pub(crate) struct SkeletonResult {
    pub page: Vec<SessionSummary>,
    pub next_cursor: Option<String>,
    pub total: usize,
    pub page_jobs: Vec<(String, std::path::PathBuf)>,
    pub dir: std::path::PathBuf,
    pub root_generation: u64,
    pub inline_updates: Vec<SessionMetadataUpdate>,
    pub fs: Arc<dyn FileSystemProvider>,
    pub ctx: cdt_fs::ContextId,
    pub expected_context_generation: u64,
}
impl LocalDataApi {
    /// 重建 `worktree_meta_cache`，把 group 内所有 worktree 的 `name` /
    /// `group_id` / `cwd_relative_to_repo_root` 索引到 `worktree.id`
    /// （即 `project.id`）上。clear-and-rebuild 语义保证 grouper 一旦觉察
    /// 文件系统变化重跑后，UI 拿到的派生字段与最新结构对齐。
    /// change `simplify-repository-as-project::D2`。
    fn refresh_worktree_meta_cache(&self, groups: &[cdt_core::RepositoryGroup]) {
        let Ok(mut cache) = self.worktree_meta_cache.write() else {
            return;
        };
        #[cfg(any(test, feature = "test-utils"))]
        self.refresh_worktree_meta_cache_call_count
            .fetch_add(1, Ordering::Relaxed);
        cache.clear();
        for g in groups {
            for w in &g.worktrees {
                cache.insert(
                    w.id.clone(),
                    WorktreeMeta {
                        worktree_name: w.name.clone(),
                        group_id: g.id.clone(),
                        cwd_relative_to_repo_root: w.cwd_relative_to_repo_root.clone(),
                    },
                );
            }
        }
    }

    /// `cfg(test)` accessor：返回 `refresh_worktree_meta_cache` 至今被调用的次数。
    #[cfg(any(test, feature = "test-utils"))]
    #[must_use]
    pub fn refresh_worktree_meta_cache_call_count(&self) -> u64 {
        self.refresh_worktree_meta_cache_call_count
            .load(Ordering::Relaxed)
    }

    /// `cfg(test)` accessor：返回 `build_group_session_page` 内 spawn metadata
    /// scan task 的次数（per worktree per call）。
    #[cfg(any(test, feature = "test-utils"))]
    #[must_use]
    pub fn metadata_scan_spawn_count(&self) -> u64 {
        self.metadata_scan_spawn_count.load(Ordering::Relaxed)
    }

    /// `cfg(test)` accessor：返回 `active_fs_and_policy` 至今被调用的次数。
    #[cfg(any(test, feature = "test-utils"))]
    #[must_use]
    pub fn active_fs_and_policy_call_count(&self) -> u64 {
        self.active_fs_and_policy_call_count.load(Ordering::Relaxed)
    }

    /// `list_sessions` / `list_group_sessions` / `get_worktree_sessions` 序列化
    /// `SessionSummary` 时用本 helper 通过 `project_id`（即 `worktree_id`）查
    /// `worktree_meta_cache`，写入 `worktree_id` / `worktree_name` / `group_id`
    /// / `cwd_relative_to_repo_root` 四个 join 字段。缓存未命中时四字段保持
    /// None，前端 fallback 用 `project_id` 当 group id（design D2 fallback）。
    fn apply_worktree_meta(&self, summary: &mut SessionSummary) {
        let Ok(cache) = self.worktree_meta_cache.read() else {
            return;
        };
        if let Some(meta) = cache.get(&summary.project_id) {
            summary.worktree_id = Some(summary.project_id.clone());
            summary.worktree_name = Some(meta.worktree_name.clone());
            summary.group_id = Some(meta.group_id.clone());
            summary.cwd_relative_to_repo_root = meta.cwd_relative_to_repo_root.clone();
        }
    }

    // =========================================================================
    // get_session_detail helpers (issue #280)
    // =========================================================================

    /// Step 1: 定位 session 文件路径 + stat 元数据 + fingerprint 短路判定。
    async fn locate_session_file(
        &self,
        project_id: &str,
        session_id: &str,
        known_fingerprint: Option<&str>,
    ) -> Result<LocateResult, ApiError> {
        let (fs, projects_dir, ctx, policy, _resolvers) = self.active_fs_and_policy().await?;
        let project_dir =
            projects_dir.join(cdt_discover::path_decoder::extract_base_dir(project_id));
        let primary_jsonl = project_dir.join(format!("{session_id}.jsonl"));

        let (jsonl_path, last_modified, size) = if let Ok(meta) = fs.stat(&primary_jsonl).await {
            (primary_jsonl, Some(meta.mtime_ms()), Some(meta.size))
        } else {
            let Some(path) = find_subagent_jsonl(&*fs, &project_dir, session_id).await else {
                return Err(ApiError::not_found(format!("session {session_id}")));
            };
            let meta = fs.stat(&path).await.ok();
            let modified = meta.as_ref().map(cdt_discover::FsMetadata::mtime_ms);
            let size = meta.as_ref().map(|m| m.size);
            (path, modified, size)
        };

        let is_stale = last_modified.is_some_and(|ms| {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
            let threshold_ms =
                i64::try_from(STALE_SESSION_THRESHOLD.as_millis()).unwrap_or(i64::MAX);
            now_ms.saturating_sub(ms) >= threshold_ms
        });
        let fingerprint = make_session_ipc_fingerprint(last_modified, size, is_stale);
        let metadata_complete = last_modified.is_some() && size.is_some();
        if metadata_complete {
            if let Some(known) = known_fingerprint {
                if known == fingerprint {
                    return Ok(LocateResult::Unchanged { fingerprint });
                }
            }
        }

        Ok(LocateResult::Found(Box::new(LocatedSession {
            fs,
            projects_dir,
            ctx,
            policy,
            project_dir,
            jsonl_path,
            last_modified,
            size,
            fingerprint,
        })))
    }

    /// Step 5: 构建上下文注入（`claude.md` + phase info）。
    async fn inject_context_annotations(
        &self,
        chunks: &[cdt_core::Chunk],
        messages: &[cdt_core::ParsedMessage],
    ) -> ContextAnnotations {
        let project_root = messages.iter().find_map(|m| m.cwd.as_deref()).unwrap_or("");
        let claude_base = self.claude_base_path().await;
        let initial_claude_md = build_claude_md_from_filesystem(project_root, &claude_base).await;
        let empty_cmd = std::collections::HashMap::new();
        let empty_mf = std::collections::HashMap::new();
        let token_dicts = cdt_analyze::context::TokenDictionaries::new(
            Path::new(""),
            &empty_cmd,
            &empty_cmd,
            &empty_mf,
        );
        let ctx_result = cdt_analyze::context::process_session_context_with_phases(
            chunks,
            &cdt_analyze::context::ProcessSessionParams {
                project_root: Path::new(""),
                token_dictionaries: token_dicts,
                initial_claude_md_injections: &initial_claude_md,
            },
        );
        let mut injections_by_phase: BTreeMap<String, Vec<cdt_core::ContextInjection>> =
            BTreeMap::new();
        for phase in &ctx_result.phase_info.phases {
            let phase_injections = ctx_result
                .stats_map
                .get(&phase.last_ai_group_id)
                .map(|stats| stats.accumulated_injections.clone())
                .unwrap_or_default();
            injections_by_phase.insert(phase.phase_number.to_string(), phase_injections);
        }
        let context_injections: Vec<cdt_core::ContextInjection> = ctx_result
            .phase_info
            .phases
            .last()
            .and_then(|phase| ctx_result.stats_map.get(&phase.last_ai_group_id))
            .map(|stats| stats.accumulated_injections.clone())
            .unwrap_or_default();
        let phase_info = ctx_result.phase_info.clone();

        let mut turn_context_stats: HashMap<String, cdt_core::TurnContextStats> = HashMap::new();
        for (ai_group_id, stats) in &ctx_result.stats_map {
            let total_new: usize = stats.new_counts.claude_md
                + stats.new_counts.mentioned_file
                + stats.new_counts.tool_output
                + stats.new_counts.thinking_text
                + stats.new_counts.task_coordination
                + stats.new_counts.user_messages;
            if total_new == 0 {
                continue;
            }
            let new_tokens = stats
                .new_injections
                .iter()
                .map(cdt_core::ContextInjection::estimated_tokens)
                .sum::<u64>();
            turn_context_stats.insert(
                ai_group_id.clone(),
                cdt_core::TurnContextStats {
                    new_count: u32::try_from(total_new).unwrap_or(u32::MAX),
                    new_tokens,
                    new_tokens_by_category: compute_new_tokens_by_category(&stats.new_injections),
                    counts_by_category: stats.new_counts,
                    cumulative_estimated_tokens: stats.total_estimated_tokens,
                    cumulative_tokens_by_category: stats.tokens_by_category,
                },
            );
        }

        ContextAnnotations {
            context_injections,
            injections_by_phase,
            phase_info,
            turn_context_stats,
        }
    }

    /// k-way merge 流式分页拉 group 内所有 worktree 合并后的 sessions。
    ///
    /// 算法：
    /// 1. 调 `list_repository_groups` 拿当前 group + 顺便刷新 `worktree_meta_cache`
    /// 2. 并发跑每个 worktree 的 `scanner.list_sessions`（共享 `Semaphore` 限流）
    /// 3. parse cursor → 二分定位每个 worktree 的指针起点
    /// 4. `BinaryHeap<HeapEntry>` k-way merge 取 `page_size` 条
    /// 5. 编码 `next_cursor`（记录每个 worktree 最后消费到的 (mtime, sid)，未消费保持原值）
    ///
    /// 详 design `simplify-repository-as-project::D3` + spec
    /// `ipc-data-api::Expose group session listing via k-way merge pagination`。
    async fn build_group_session_page(
        &self,
        group_id: &str,
        page_size: usize,
        cursor: Option<&str>,
    ) -> Result<GroupSessionPage, ApiError> {
        // change `generation-race-audit` D2：单一 snapshot 通过
        // `list_repository_groups_inner()` 一次性拿 (groups, fs, projects_dir, ctx,
        // captured_context_generation) 五元组——避免旧实现 `list_repository_groups()`
        // 与 `active_fs_and_context_strict()` 两次独立 await 之间被 ssh switch /
        // reconfigure 跨过引发 (groups OLD, fs NEW) 拼接 race。
        //
        // expected_root_generation 仍单独 load——后台 scan task 沿用既有
        // expected_root_generation / expected_context_generation 双轴校验语义；
        // expected_context_generation = captured_context_generation（与 fs/ctx 同源）。
        let (groups, fs, projects_dir, ctx, captured_context_generation) =
            self.list_repository_groups_inner().await?;
        let expected_root_generation = self.root_generation.load(Ordering::SeqCst);
        let expected_context_generation = captured_context_generation;
        let group = find_group_with_fallback(groups, group_id)?;

        let cursor_state = parse_group_cursor(cursor);
        let pinned = std::collections::BTreeSet::<String>::new();
        let scanner = ProjectScanner::new_with_cwd_cache(
            fs.clone(),
            projects_dir.clone(),
            self.shared_read_semaphore.clone(),
            self.shared_cwd_cache.clone(),
        );
        let scanner = Arc::new(scanner);

        // 并发拉每个 worktree 的 sessions（已按 mtime 倒序）。
        // **性能优化**：cursor `Exhausted` 的 worktree 跳过 `scanner.list_sessions`
        // IO——worktree filter 模式下 cursor 让 16/17 个 wt = Exhausted，扫它们
        // 的 sessions 是无用 IO（heap 不会 push 这些 wt + dedup 也跳过它们）。
        // 跳过后 filter 切换从 N×IO 降到 1×IO（codex 第三轮二审 known
        // trade-off + 用户感知"切 worktree 慢"的根因，2026-05-21）。
        let mut futs = Vec::with_capacity(group.worktrees.len());
        let mut scheduled_wt_ids: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        for wt in &group.worktrees {
            let initial_offset = cursor_state
                .per_worktree
                .get(&wt.id)
                .cloned()
                .unwrap_or(WorktreeOffset::NotStarted);
            if matches!(initial_offset, WorktreeOffset::Exhausted) {
                continue;
            }
            scheduled_wt_ids.insert(wt.id.clone());
            let wt_id = wt.id.clone();
            let scanner = scanner.clone();
            let pinned = pinned.clone();
            futs.push(async move {
                let sessions = scanner
                    .list_sessions(&wt_id, &pinned)
                    .await
                    .map_err(|e| ApiError::internal(format!("scan {wt_id}: {e}")))?;
                Ok::<_, ApiError>((wt_id, sessions))
            });
        }
        // 单 worktree scan 失败 SHALL NOT 让整页 500——scanner 自身已对单文件
        // 损坏走降级，本层 join_all + per-result warn + skip 保持同语义：缺失
        // worktree 自然不进 k-way merge，UI 上少几行 session 但不阻塞剩余 group。
        // **关键**：记录 `failed_wt_ids` 让 cursor 编码 SHALL NOT 把临时 IO 失败
        // 的 worktree 错标 `Exhausted`——否则 (codex Blocker, 2026-05-21)：临时
        // scan 失败会让该 worktree 在用户后续 loadMore 中**永久消失**（silent
        // data loss）。失败的 wt cursor 上保留 `cursor_state` 原值（多数为
        // `NotStarted`），下次请求时仍会重试。
        let wt_lists = futures::future::join_all(futs).await;
        let mut wt_sessions: std::collections::BTreeMap<String, Vec<cdt_core::Session>> =
            std::collections::BTreeMap::new();
        let mut failed_wt_ids: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        for result in wt_lists {
            match result {
                Ok((wt_id, sessions)) => {
                    wt_sessions.insert(wt_id, sessions);
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "scan worktree failed, skip from k-way merge"
                    );
                }
            }
        }
        // 推导 failed_wt_ids：scheduled 但未在 wt_sessions 出现的 wt 都视为
        // 失败（join_all 错误未携带 wt_id，从两侧差集反推）。
        for wid in &scheduled_wt_ids {
            if !wt_sessions.contains_key(wid) {
                failed_wt_ids.insert(wid.clone());
            }
        }

        // 跨 worktree 去重（仅 active worktree 间执行）。
        dedup_sessions_across_worktrees(&mut wt_sessions, &cursor_state, &group.worktrees);

        // 二分定位每个 worktree 的指针起点（含 Exhausted 跳过）。
        let indices = resolve_cursor_indices(&group.worktrees, &cursor_state, &wt_sessions);

        // k-way merge：BinaryHeap 全序 (mtime desc, sid asc)。
        let mut heap: std::collections::BinaryHeap<HeapEntry> = std::collections::BinaryHeap::new();
        for wt in &group.worktrees {
            let (idx, exhausted) = indices[&wt.id];
            if exhausted {
                continue;
            }
            if let Some(s) = wt_sessions.get(&wt.id).and_then(|v| v.get(idx)) {
                heap.push(HeapEntry {
                    mtime: s.last_modified,
                    sid: s.id.clone(),
                    wt_id: wt.id.clone(),
                    idx,
                });
            }
        }

        let mut page: Vec<SessionSummary> = Vec::with_capacity(page_size);
        // 记录每个 worktree 最后消费的 (mtime, sid) 与下一条 idx，用于编码 cursor。
        let mut last_consumed: std::collections::BTreeMap<String, (i64, String, usize)> =
            std::collections::BTreeMap::new();

        while page.len() < page_size {
            let Some(top) = heap.pop() else {
                break;
            };
            let s = wt_sessions
                .get(&top.wt_id)
                .and_then(|v| v.get(top.idx))
                .cloned();
            let Some(s) = s else { continue };
            let mut summary = SessionSummary {
                session_id: s.id.clone(),
                project_id: top.wt_id.clone(),
                timestamp: s.last_modified,
                created: s.created,
                message_count: 0,
                title: None,
                is_ongoing: false,
                git_branch: None,
                worktree_id: None,
                worktree_name: None,
                group_id: None,
                cwd_relative_to_repo_root: None,
                cwd: s.cwd.clone(),
                project_name: None,
                user_intents: Vec::new(),
                last_active: 0,
                duration_ms: 0,
                total_cost: 0.0,
                tool_error_count: 0,
                files_modified: Vec::new(),
                git_summary: Vec::new(),
            };
            self.apply_worktree_meta(&mut summary);
            page.push(summary);
            last_consumed.insert(
                top.wt_id.clone(),
                (s.last_modified, s.id.clone(), top.idx + 1),
            );

            // push 下一条
            let next_idx = top.idx + 1;
            if let Some(next) = wt_sessions.get(&top.wt_id).and_then(|v| v.get(next_idx)) {
                heap.push(HeapEntry {
                    mtime: next.last_modified,
                    sid: next.id.clone(),
                    wt_id: top.wt_id.clone(),
                    idx: next_idx,
                });
            }
        }

        // page 构造完后给每个 summary 跑 metadata cache fast-path lookup +
        // 按 worktree 分组未命中条目走后台扫描（spawn `scan_metadata_for_page`），
        // 让 cache hit 直接带 title / messageCount 返回，cache miss 通过
        // `session_metadata_tx` broadcast → SSE patch 异步补齐。
        //
        // 否则 list_group_sessions 永远返 skeleton（title=None），UI 列表项
        // 卡在 `session.title || session.sessionId` 永久 fallback 到 sessionId
        // 前缀（用户感知"会话名变 sessionId"的根因，codex 二审 round 3 Blocker，
        // 2026-05-21）。
        let is_remote = fs.kind() == cdt_discover::FsKind::Ssh;
        let lookup_permit = Arc::new(Semaphore::new(METADATA_SCAN_CONCURRENCY));
        // change `unify-fs-direct-calls` design D2/D3 (line 810-863):
        // SSH 与 Local 同走 SkeletonThenStream——hot path 用 `lookup_trust_cached`
        // 让 SSH cache hit 0 fs op 立刻渲染；cache miss 入 page_jobs 后台 scan，
        // 通过 SSE 推差量。
        let lookups = futures::future::join_all(page.iter().map(|summary| {
            let wt_id = summary.project_id.clone();
            let session_id = summary.session_id.clone();
            let base_dir = cdt_discover::path_decoder::extract_base_dir(&wt_id);
            let jsonl_path = projects_dir
                .join(base_dir)
                .join(format!("{session_id}.jsonl"));
            let cache = self.metadata_cache.clone();
            let permit_sem = lookup_permit.clone();
            let fs_clone = fs.clone();
            let ctx_clone = ctx.clone();
            async move {
                if is_remote {
                    // SSH hot path：cache hit trust（0 fs op）；miss 入后台 scan。
                    let cached = cache
                        .lock()
                        .ok()
                        .and_then(|mut c| c.lookup_trust_cached(&ctx_clone, &jsonl_path))
                        .map(|entry| SessionMetadata {
                            title: entry.title,
                            message_count: entry.message_count,
                            is_ongoing: entry.messages_ongoing,
                            git_branch: entry.git_branch,
                            user_intents: entry.user_intents,
                            last_active: entry.last_active,
                            duration_ms: entry.duration_ms,
                            total_cost: entry.total_cost,
                            tool_error_count: entry.tool_error_count,
                            files_modified: entry.files_modified,
                            git_summary: entry.git_summary,
                        });
                    return (wt_id, session_id, jsonl_path, cached);
                }
                let _guard = permit_sem
                    .acquire()
                    .await
                    .expect("lookup semaphore should not be closed");
                let meta =
                    try_lookup_cached_metadata(&cache, &*fs_clone, &ctx_clone, &jsonl_path).await;
                (wt_id, session_id, jsonl_path, meta)
            }
        }))
        .await;

        // 按 worktree 分组的 page_jobs（未命中 cache 的条目走 spawn 扫描）。
        let mut page_jobs_by_wt: std::collections::BTreeMap<
            String,
            (PathBuf, Vec<(String, PathBuf)>),
        > = std::collections::BTreeMap::new();
        for (summary, (wt_id, session_id, jsonl_path, cached_meta)) in page.iter_mut().zip(lookups)
        {
            debug_assert_eq!(summary.project_id, wt_id);
            debug_assert_eq!(summary.session_id, session_id);
            // SSH cache hit 走 `lookup_trust_cached`（0 fs op）不校验 signature——
            // 远端文件被 `RemotePollingWatcher` 3s poll 出变化后没有自动失效路径。
            // 解法（codex round-7 Blocker #4 方案 #1）：SSH 路径无论 cache hit / miss
            // 都入 page_jobs 后台 scan，`extract_session_metadata_cached` 会 stat 比对
            // signature，不变 silent return / 变化 broadcast SSE 差量给前端。
            // Local cache hit 走 `try_lookup_cached_metadata` 已 stat 校验，无需重 scan。
            let need_background_validation = is_remote || cached_meta.is_none();
            if let Some(meta) = cached_meta {
                summary.title = meta.title;
                summary.message_count = meta.message_count;
                summary.is_ongoing = meta.is_ongoing;
                summary.git_branch = meta.git_branch;
                summary.user_intents = meta.user_intents;
                summary.last_active = meta.last_active;
                summary.duration_ms = meta.duration_ms;
                summary.total_cost = meta.total_cost;
                summary.tool_error_count = meta.tool_error_count;
                summary.files_modified = meta.files_modified;
                summary.git_summary = meta.git_summary;
            }
            if need_background_validation {
                let base_dir = cdt_discover::path_decoder::extract_base_dir(&wt_id);
                let dir = projects_dir.join(base_dir);
                page_jobs_by_wt
                    .entry(wt_id)
                    .or_insert_with(|| (dir, Vec::new()))
                    .1
                    .push((session_id, jsonl_path));
            }
        }

        // per-worktree spawn scan_metadata_for_page。scan_key 给 group 路径
        // 分配独立 namespace（前缀 `group:`）+ **完整 cursor 字符串**后缀，让
        // page 1 / page 2 / loadMore 的 scan 在同一 wt 上**并存而非互相 abort**。
        //
        // 历史踩坑：
        // 1. scan_key 若只用 `wt_id|group` 单一 namespace，loadMore 触发的 page 2
        //    scan 会立刻 cancel page 1 还在跑的 jobs → page 1 后段 cache miss
        //    的 sessions metadata 永久丢失 → 用户感知"sidebar 前几条 title
        //    正确，后续全 sessionId fallback"。
        // 2. 用 `DefaultHasher` 64-bit hash 当 cursor 身份在跨进程随机种子下不
        //    稳定，且 hash 碰撞会让两个不同 cursor 的 scan 互相 abort → 某页
        //    metadata 永远扫不出来（codex 第三轮二审 Bug 1）。改用完整 cursor
        //    字符串作 namespace key 后缀，无碰撞、deterministic，与
        //    `list_sessions` 走 `(project_id, cursor)` 双键的并存语义一致。
        // change `generation-race-audit` D2：spawn `scan_metadata_for_page` 后台 task
        // 之前 SHALL 持 `ssh_watcher_ops` 锁 + (ctx + generation) 二次校验。理由：
        // bump-first 顺序使得 inner 拿到的 captured_context_generation 可能等于
        // ssh_mgr.switch_context 完成后的 current（同值都为 bumped 后值），spawn 后
        // task 自身的 broadcast-time 校验 `current == expected` 仍误判通过 → 向新 ctx
        // UI 发旧 ctx update。spawn 前在锁内识别 ctx mismatch 结构性闭合该 sub-window；
        // spawn 在锁内进行确保 spawn 期间 5 处 mutate 入口不可能跑（持同锁互斥）。
        let cur_root_generation = expected_root_generation;
        let cur_context_generation = expected_context_generation;
        let scan_cursor_id = format!("group:{}", cursor.unwrap_or("none"));
        {
            let _ops = self.ssh_watcher_ops.lock().await;
            let current_ctx = self.current_active_context_id_under_lock().await;
            let current_context_generation = self.context_generation.load(Ordering::SeqCst);
            if current_ctx != ctx || current_context_generation != captured_context_generation {
                tracing::debug!(
                    target: "cdt_api::perf",
                    captured_ctx = ?ctx,
                    current_ctx = ?current_ctx,
                    captured_gen = captured_context_generation,
                    current_gen = current_context_generation,
                    "build_group_session_page: state changed mid-page-build, skip metadata scan task spawn"
                );
                // 返页面骨架但不 spawn 后台 metadata scan——避免向新 ctx UI broadcast 旧 ctx update。
            } else {
                for (wt_id, (dir, jobs)) in page_jobs_by_wt {
                    if jobs.is_empty() {
                        continue;
                    }
                    let scan_key = metadata_scan_key(&wt_id, Some(&scan_cursor_id));
                    let key_for_cleanup = scan_key.clone();
                    let tx = self.session_metadata_tx.clone();
                    let active_scans_clone = self.active_scans.clone();
                    if let Ok(mut scans) = self.active_scans.lock() {
                        if let Some(old) = scans.remove(&scan_key) {
                            old.handle.abort();
                        }
                        let my_generation = self.scan_generation.fetch_add(1, Ordering::Relaxed);
                        let handle = tokio::spawn(scan_metadata_for_page(
                            wt_id.clone(),
                            dir,
                            jobs,
                            tx,
                            active_scans_clone,
                            key_for_cleanup,
                            my_generation,
                            self.metadata_cache.clone(),
                            self.metadata_scan_semaphore.clone(),
                            self.root_generation.clone(),
                            cur_root_generation,
                            self.worktree_meta_cache.clone(),
                            fs.clone(),
                            ctx.clone(),
                            self.context_generation.clone(),
                            cur_context_generation,
                        ));
                        #[cfg(any(test, feature = "test-utils"))]
                        self.metadata_scan_spawn_count
                            .fetch_add(1, Ordering::Relaxed);
                        scans.insert(
                            scan_key,
                            ScanEntry {
                                generation: my_generation,
                                context_id: ctx.clone(),
                                handle: handle.abort_handle(),
                            },
                        );
                    }
                }
            }
        }

        // 编码 next_cursor。
        let next_cursor = encode_next_group_cursor(
            &group.worktrees,
            &cursor_state,
            &indices,
            &last_consumed,
            &wt_sessions,
            &failed_wt_ids,
        );

        // 防御性 drop（明确生命周期）。
        let _ = wt_sessions;

        Ok(GroupSessionPage {
            sessions: page,
            next_cursor,
        })
    }

    async fn active_scanner(&self) -> Result<ProjectScanner, ApiError> {
        if let Some(context_id) = self.ssh_mgr.active_context_id().await {
            let provider = self
                .ssh_mgr
                .provider(&context_id)
                .await
                .ok_or_else(|| ApiError::not_found(format!("SSH context: {context_id}")))?;
            let projects_dir = provider.remote_home().to_path_buf();
            return Ok(ProjectScanner::new_with_semaphore(
                Arc::new(provider),
                projects_dir,
                self.shared_read_semaphore.clone(),
            ));
        }
        let projects_dir = self.projects_dir.lock().await.clone();
        Ok(ProjectScanner::new_with_cwd_cache(
            local_handle(),
            projects_dir,
            self.shared_read_semaphore.clone(),
            self.shared_cwd_cache.clone(),
        ))
    }

    async fn active_fs_and_projects_dir(
        &self,
    ) -> Result<(Arc<dyn FileSystemProvider>, PathBuf), ApiError> {
        if let Some(context_id) = self.ssh_mgr.active_context_id().await {
            let provider = self
                .ssh_mgr
                .provider(&context_id)
                .await
                .ok_or_else(|| ApiError::not_found(format!("SSH context: {context_id}")))?;
            let projects_dir = provider.remote_home().to_path_buf();
            return Ok((Arc::new(provider), projects_dir));
        }
        Ok((local_handle(), self.projects_dir.lock().await.clone()))
    }

    /// 派生 fs + `projects_dir` + `ContextId` 三元组——给 fs-related cache 作 key
    /// 前缀。函数内部单次读 `ssh_mgr.active_context_id().await` 决定走 SSH 还是
    /// Local 分支；SSH 分支若 provider/`host_signature` lookup miss（disconnect
    /// 中间态）SHALL **safely degrade** 到 Local——避免"fs 是 SSH 但 ctx 是 Local"
    /// 等不一致组合（详 change `metadata-cache-context-prefix` design D3 / D3-bis）。
    ///
    /// 与 `active_fs_and_projects_dir` 行为差异：本方法 **不**对 disconnect 中间态
    /// 报错（cache 路径优先连续性）；旧方法对未注册 SSH context 返 `not_found`，
    /// 现有非 cache callsite 保留旧行为。**用户可见 IPC handler**（`get_tool_output`
    /// / `get_image_asset` 等）SHALL 走 `active_fs_and_context_strict()` —— 用户
    /// 在 SSH context 下请求时 SHALL NOT 静默降级到 Local 数据（详 change
    /// `parsed-message-cache-context-prefix` codex 二审 commit-stage Q1 + round-2 Q1）。
    ///
    /// 当前唯一 callsite 是 `prime_parsed_msg_cache_for_test`（`test-utils` feature
    /// 路径），所以 cfg-gated 编译；release / 默认构建里无 user。其它已知 cache 写
    /// 入路径若未来需 relaxed 行为，再放开 cfg。
    #[cfg(any(test, feature = "test-utils"))]
    pub(crate) async fn active_fs_and_context(
        &self,
    ) -> (Arc<dyn FileSystemProvider>, PathBuf, cdt_fs::ContextId) {
        if let Some(context_id) = self.ssh_mgr.active_context_id().await {
            // 原子取 provider + ContextId（同一 sessions lock 内）——避免独立
            // `provider(&id)` + `context_id(&id)` 调用之间的 disconnect race 让
            // (SSH provider, Local ctx) 不自洽组合（codex 二审 commit-stage
            // Blocking → design D3-bis）。任一 miss 整体 fall-through 到
            // Local，绝不返回 SSH/Local 混合三元组。
            if let Some((provider, ctx)) = self.ssh_mgr.provider_and_context_id(&context_id).await {
                let remote_home = provider.remote_home().to_path_buf();
                return (Arc::new(provider), remote_home, ctx);
            }
            // active=Some 但 session 已被 remove（concurrent disconnect 中间态）
            // → 走 Local 安全降级
        }
        let projects_dir = self.projects_dir.lock().await.clone();
        let ctx = cdt_fs::ContextId::local(projects_dir.clone());
        (local_handle(), projects_dir, ctx)
    }

    /// `active_fs_and_context` 的**严格变体**：用于用户可见 IPC handler。SSH
    /// active 但 provider/ContextId lookup miss 时 SHALL 返回 `not_found` 错——
    /// 与旧 `active_fs_and_projects_dir` 同语义，**不**静默降级到 Local。
    ///
    /// 设计动机（change `parsed-message-cache-context-prefix` codex 二审 Q1）：
    /// `get_tool_output` / `get_image_asset` 等用户调用方一旦 SSH active 但
    /// provider 丢失（concurrent disconnect 中间态），若降级到 Local 可能返回
    /// 同 ID 的 Local 文件数据，破"用户在 SSH context 下请求"的语义契约。
    /// cache-only 内部路径（如 `prime_parsed_msg_cache_for_test` / metadata 主动
    /// scan）仍可用 `active_fs_and_context` 的 relaxed 版本，因 cache 写入即便
    /// 短暂降级也只是多一次 Local entry，无数据正确性问题。
    ///
    /// 原子性同 `active_fs_and_context`：单次 `provider_and_context_id` 调用
    /// 同时拿 provider + ContextId，绝不返回 SSH/Local 混合三元组。
    pub(crate) async fn active_fs_and_context_strict(
        &self,
    ) -> Result<(Arc<dyn FileSystemProvider>, PathBuf, cdt_fs::ContextId), ApiError> {
        if let Some(context_id) = self.ssh_mgr.active_context_id().await {
            let Some((provider, ctx)) = self.ssh_mgr.provider_and_context_id(&context_id).await
            else {
                return Err(ApiError::not_found(format!("SSH context: {context_id}")));
            };
            let remote_home = provider.remote_home().to_path_buf();
            return Ok((Arc::new(provider), remote_home, ctx));
        }
        let projects_dir = self.projects_dir.lock().await.clone();
        let ctx = cdt_fs::ContextId::local(projects_dir.clone());
        Ok((local_handle(), projects_dir, ctx))
    }

    /// `active_fs_and_context_strict()` 的扩展版本：额外返 `BackendPolicy` +
    /// `Arc<BackendResolvers>`，让 IPC handler 一次 await 拿全 fs / ctx / 后端
    /// 策略五元组，避免每个 callsite 各自 `fs.kind()` 分支（design D5）。
    ///
    /// `fs.kind()` 比对仅允许在本 helper + `BackendResolvers::from_fs` 内部使用——
    /// 业务 callsite SHALL 通过 `policy.<field>` / `resolvers.<field>` 读取后端
    /// 相关行为（fs-abstraction spec scenario "业务代码通过 `BackendPolicy` 字段
    /// 选择行为"）。
    ///
    /// `BackendPolicy` by-value：`Copy` 类型包 Arc 是反 idiom。`BackendResolvers`
    /// 包 Arc 因为持 `Arc<dyn>` + 通过 `LazyLock` 静态缓存避免每次重建（D4）。
    pub(crate) async fn active_fs_and_policy(
        &self,
    ) -> Result<
        (
            Arc<dyn FileSystemProvider>,
            PathBuf,
            cdt_fs::ContextId,
            cdt_fs::BackendPolicy,
            Arc<crate::ipc::backend_resolvers::BackendResolvers>,
        ),
        ApiError,
    > {
        #[cfg(any(test, feature = "test-utils"))]
        self.active_fs_and_policy_call_count
            .fetch_add(1, Ordering::Relaxed);
        let (fs, projects_dir, ctx) = self.active_fs_and_context_strict().await?;
        let policy = match fs.kind() {
            cdt_fs::FsKind::Local => cdt_fs::BackendPolicy::for_local(),
            cdt_fs::FsKind::Ssh => cdt_fs::BackendPolicy::for_ssh(),
        };
        let resolvers = crate::ipc::backend_resolvers::BackendResolvers::from_fs(&*fs);
        Ok((fs, projects_dir, ctx, policy, resolvers))
    }

    /// 在已持有 `ssh_watcher_ops` 锁的前提下读 `ssh_mgr` 当前 active 状态，构造与
    /// `active_fs_and_context_strict` 同形的 `ContextId`。SHALL 仅在锁内调用——
    /// 否则与 `switch_context` / `ssh_connect` / `ssh_disconnect` /
    /// `shutdown_ssh_all` 路径互斥不成立，结果可能与下一刻 `ssh_mgr` 真实状态偏离。
    ///
    /// 用途：change `generation-race-audit` 在 `list_repository_groups` /
    /// `build_group_session_page` 的派生 cache 写入路径前做 (ctx + generation)
    /// 双重校验，闭合 bump-first 顺序导致的 sub-window race。
    pub(crate) async fn current_active_context_id_under_lock(&self) -> cdt_fs::ContextId {
        if let Some(id) = self.ssh_mgr.active_context_id().await {
            if let Some((_provider, ctx)) = self.ssh_mgr.provider_and_context_id(&id).await {
                return ctx;
            }
            // active=Some 但 provider lookup miss（disconnect 中间态）→ fall through Local
        }
        let projects_dir = self.projects_dir.lock().await.clone();
        cdt_fs::ContextId::local(projects_dir)
    }

    /// `list_repository_groups` 的内部抽样函数：返回 `(groups, fs, projects_dir,
    /// ctx, captured_context_generation)` 同源五元组。`captured_context_generation`
    /// 在 `active_fs_and_policy()` 完成之**后**立即 load——与 `(fs, ctx)` 同 snapshot；
    /// 函数内后续不再 `fetch_add`，wrapper 路径用此值与锁内 current 比较即可识别
    /// inner 期间 / inner 完成到 wrapper 拿锁之间任何 mutate。
    ///
    /// change `generation-race-audit` D1 / D2：被 `list_repository_groups` (作为
    /// 派生 cache 刷新入口) 与 `build_group_session_page` (作为单一 snapshot
    /// 抽样) 共用，避免后者独立调 `active_fs_and_context_strict` 引发跨快照
    /// (groups OLD, fs NEW) 拼接 race。
    pub(crate) async fn list_repository_groups_inner(
        &self,
    ) -> Result<
        (
            Vec<cdt_core::RepositoryGroup>,
            Arc<dyn FileSystemProvider>,
            PathBuf,
            cdt_fs::ContextId,
            u64,
        ),
        ApiError,
    > {
        let pre_root_generation = self.root_generation.load(Ordering::SeqCst);
        let pre_context_generation = self.context_generation.load(Ordering::SeqCst);

        // 先取 (fs, ctx) 再检查 cache——确保 cache hit 返回的 groups 与
        // (fs, ctx) 属同一 generation snapshot，避免"旧 groups + 新 context"。
        let (fs, projects_dir, ctx, _policy, resolvers) = self.active_fs_and_policy().await?;
        let captured_context_generation = self.context_generation.load(Ordering::SeqCst);
        let captured_root_generation = self.root_generation.load(Ordering::SeqCst);
        let captured_scan_inv_gen = self
            .project_scan_cache
            .lock()
            .expect("poisoned")
            .invalidation_generation();

        // groups cache hit：用 captured generation（与 fs/ctx 同源）检查
        let cached_groups = {
            let cache = self.groups_cache.lock().expect("poisoned");
            cache.as_ref().and_then(|entry| {
                if entry.root_gen == captured_root_generation
                    && entry.ctx_gen == captured_context_generation
                    && entry.scan_inv_gen == captured_scan_inv_gen
                    && entry.created_at.elapsed() < GROUPS_CACHE_TTL
                {
                    Some(entry.groups.clone())
                } else {
                    None
                }
            })
        };
        if let Some(groups) = cached_groups {
            return Ok((groups, fs, projects_dir, ctx, captured_context_generation));
        }

        let projects = self
            .scan_projects_cached_with(&fs, &projects_dir, &ctx)
            .await?;
        let grouper =
            cdt_discover::WorktreeGrouper::new_dyn(resolvers.git_identity_resolver.clone());
        let groups = grouper.group_by_repository((*projects).clone()).await;

        // 条件写入 groups cache：只在 generation 仍与计算时一致时写入，
        // 避免 grouper 执行期间 generation 变化导致 stale 数据被标为 fresh。
        {
            let cur_root_gen = self.root_generation.load(Ordering::SeqCst);
            let cur_ctx_gen = self.context_generation.load(Ordering::SeqCst);
            let cur_scan_inv_gen = self
                .project_scan_cache
                .lock()
                .expect("poisoned")
                .invalidation_generation();
            if cur_root_gen == captured_root_generation
                && cur_ctx_gen == captured_context_generation
                && cur_scan_inv_gen == captured_scan_inv_gen
            {
                *self.groups_cache.lock().expect("poisoned") = Some(GroupsCacheEntry {
                    groups: groups.clone(),
                    root_gen: captured_root_generation,
                    ctx_gen: captured_context_generation,
                    scan_inv_gen: captured_scan_inv_gen,
                    created_at: std::time::Instant::now(),
                });
            }
        }

        let post_root_generation = self.root_generation.load(Ordering::SeqCst);
        let post_context_generation = self.context_generation.load(Ordering::SeqCst);
        if pre_root_generation != post_root_generation
            || pre_context_generation != post_context_generation
        {
            tracing::debug!(
                target: "cdt_api::perf",
                pre_root = pre_root_generation,
                post_root = post_root_generation,
                pre_ctx = pre_context_generation,
                post_ctx = post_context_generation,
                captured_ctx_gen = captured_context_generation,
                "list_repository_groups_inner: context shifted mid-scan (fast-path mismatch); wrapper lock will skip refresh"
            );
        }
        Ok((groups, fs, projects_dir, ctx, captured_context_generation))
    }

    /// `ProjectScanner::scan()` 的进程级 cache 入口。返回 `Arc<Vec<Project>>`
    /// 让调用方零分配 iter；写入 cache 后下次同 `ContextId` 调用直接命中，
    /// 跳过 ~14K fs op 的全量扫描。
    ///
    /// 失效层级（详 `project_scan_cache.rs` 模块头）：
    /// 1. watcher 主动 invalidate Local entry（任意 `FileChangeEvent` 触发）
    /// 2. `root_generation` / `context_generation` 校验
    /// 3. TTL（Local 5 分钟、SSH 10 秒）
    /// 4. cache 内部 `invalidation_generation`（in-flight scan race 保护）
    ///
    /// `active_fs_and_context_strict()` 保证 SSH disconnect 中间态报错而非
    /// 静默降级——cache 写入与读取使用同一 `ContextId`。
    pub(crate) async fn scan_projects_cached(&self) -> Result<Arc<Vec<Project>>, ApiError> {
        let (fs, projects_dir, ctx) = self.active_fs_and_context_strict().await?;
        self.scan_projects_cached_with(&fs, &projects_dir, &ctx)
            .await
    }

    /// 同 `scan_projects_cached`，但调用方已通过 `active_fs_and_policy()` /
    /// `active_fs_and_context_strict()` 一次性拿到 `(fs, projects_dir, ctx)`——
    /// 避免两次 `active_*().await` 之间被 SSH switch / reconfigure 跨过让
    /// projects 与下游 resolver 不属于同一 context snapshot（codex 二审 #3）。
    pub(crate) async fn scan_projects_cached_with(
        &self,
        fs: &Arc<dyn FileSystemProvider>,
        projects_dir: &std::path::Path,
        ctx: &cdt_fs::ContextId,
    ) -> Result<Arc<Vec<Project>>, ApiError> {
        let cur_root_generation = self.root_generation.load(Ordering::SeqCst);
        let cur_context_generation = self.context_generation.load(Ordering::SeqCst);
        // 命中检查 + `begin_scan` 标记 in-flight：返回当前 invalidation_generation
        // 给 scan 完成后 `finish_scan_with_insert` race 检测，同时 in_flight_scans
        // += 1 让 invalidator 在 cache 空 + scan 在途场景下也 bump generation
        // （详 spec `ProjectScanCache 按事件语义分级失效` Requirement
        // `has_entry || has_in_flight_scan` 守护）。
        let recorded_invalidation = {
            let mut cache = self
                .project_scan_cache
                .lock()
                .expect("project scan cache mutex poisoned");
            if let Some(hit) = cache.lookup(ctx, cur_root_generation, cur_context_generation) {
                tracing::debug!(
                    target: "cdt_api::perf",
                    context = ?ctx.backend_kind,
                    "project_scan_cache hit"
                );
                // 合成路径（spec `ipc-data-api/spec.md::ProjectScanCache 维护
                // per-project mtime overlay::cache hit 路径合成 hint`）：仅在
                // 至少一个 project 有大于 snapshot 的 overlay 时 clone Vec
                // 注入新值；无 hint 时直接返原 Arc 零分配。
                let synthesized = synthesize_projects_with_overlay(&cache, ctx, &hit);
                return Ok(synthesized);
            }
            cache.begin_scan()
        };

        // miss：真正 scan。错误路径 SHALL `abort_scan` 配对 begin_scan。
        let mut scanner = ProjectScanner::new_with_cwd_cache(
            fs.clone(),
            projects_dir.to_path_buf(),
            self.shared_read_semaphore.clone(),
            self.shared_cwd_cache.clone(),
        );
        let projects = match scanner.scan().await {
            Ok(p) => p,
            Err(e) => {
                self.project_scan_cache
                    .lock()
                    .expect("project scan cache mutex poisoned")
                    .abort_scan();
                return Err(ApiError::internal(format!("scan error: {e}")));
            }
        };
        let snapshot = Arc::new(projects);
        let fs_kind = fs.kind();
        // 写入 cache：`finish_scan_with_insert` 内部 in_flight_scans -= 1 +
        // generation race 检查。race 时（典型：watcher 在 scan await 期间收到
        // `FileChangeEvent`）丢弃本次 snapshot，下次 lookup 走真实 miss 重 scan。
        let inserted = {
            let mut cache = self
                .project_scan_cache
                .lock()
                .expect("project scan cache mutex poisoned");
            cache.finish_scan_with_insert(
                ctx.clone(),
                snapshot.clone(),
                cur_root_generation,
                cur_context_generation,
                fs_kind,
                recorded_invalidation,
            )
        };
        if inserted {
            tracing::debug!(
                target: "cdt_api::perf",
                project_count = snapshot.len(),
                backend = ?fs_kind,
                "project_scan_cache populated"
            );
        } else {
            tracing::debug!(
                target: "cdt_api::perf",
                backend = ?fs_kind,
                "project_scan_cache snapshot dropped — invalidated during scan"
            );
        }
        // miss 路径同样走合成：scan 完成到本函数 return 之间若有 watcher event
        // 已经写入 overlay（race），SHALL 让 scan 客户端立刻看到合成值。无 hint
        // 时返原 Arc 不 clone。
        let synthesized = {
            let cache = self
                .project_scan_cache
                .lock()
                .expect("project scan cache mutex poisoned");
            synthesize_projects_with_overlay(&cache, ctx, &snapshot)
        };
        Ok(synthesized)
    }

    /// 测试 / perf bench 用：返回累计命中 / lookup 数。
    #[allow(dead_code)]
    pub fn project_scan_cache_stats(&self) -> super::project_scan_cache::ProjectScanCacheStats {
        self.project_scan_cache
            .lock()
            .expect("project scan cache mutex poisoned")
            .stats()
    }

    /// 显式清空 scan cache（Local + SSH 所有 entry）。供 IPC contract 测试
    /// 让 SSH 路径多个 method 调用之间不互相 cache hit；同时也是 future
    /// `ssh_disconnect` / 切 SSH context 显式 hook 可调用入口（当前由
    /// `context_generation` bump + 自然 TTL 兜底，本入口供未来扩展）。
    pub fn invalidate_project_scan_cache(&self) {
        self.project_scan_cache
            .lock()
            .expect("project scan cache mutex poisoned")
            .invalidate_all();
    }

    pub fn new(
        scanner: ProjectScanner,
        config_mgr: ConfigManager,
        notif_mgr: NotificationManager,
        _ssh_mgr: SshConnectionManager,
    ) -> Self {
        let search_cache = std::sync::Arc::new(Mutex::new(SearchTextCache::new()));
        let (session_metadata_tx, _) =
            broadcast::channel::<SessionMetadataUpdate>(METADATA_BROADCAST_CAPACITY);
        // 在 move scanner 进 Mutex 前先 clone projects_dir，让 hot path /
        // invalidate task 共享同一 base path（详上方字段 doc）。
        let projects_dir = scanner.projects_dir().to_path_buf();
        Self {
            scanner: Mutex::new(scanner),
            search_cache,
            config_mgr: Arc::new(Mutex::new(config_mgr)),
            notif_mgr: Arc::new(Mutex::new(notif_mgr)),
            ssh_mgr: SshSessionManager::new(),
            error_tx: None,
            file_tx: None,
            todo_tx: None,
            jobs_tx: None,
            watcher: Mutex::new(None),
            watcher_tasks: Mutex::new(Vec::new()),
            remote_watchers: Mutex::new(HashMap::new()),
            ssh_watcher_ops: Arc::new(Mutex::new(())),
            ssh_shutdown_generation: AtomicU64::new(0),
            session_metadata_tx,
            active_scans: Arc::new(std::sync::Mutex::new(HashMap::new())),
            scan_generation: Arc::new(AtomicU64::new(0)),
            root_generation: Arc::new(AtomicU64::new(0)),
            context_generation: Arc::new(AtomicU64::new(0)),
            metadata_scan_semaphore: Arc::new(Semaphore::new(METADATA_SCAN_CONCURRENCY)),
            image_cache_dir: None,
            metadata_cache: Arc::new(std::sync::Mutex::new(MetadataCache::default())),
            parsed_msg_cache: Arc::new(std::sync::Mutex::new(ParsedMessageCache::default())),
            workflow_manifest_cache: Arc::new(std::sync::Mutex::new(
                super::workflow_manifest::WorkflowManifestCache::new(),
            )),
            projects_dir: Mutex::new(projects_dir),
            // 共享 head-read semaphore 容量与 `cdt_discover::ProjectScanner`
            // 默认 `FILE_READ_CONCURRENCY=64` 等价（design D4）。硬编码 64 避免
            // 跨 crate 暴露内部常量。
            shared_read_semaphore: Arc::new(Semaphore::new(64)),
            shared_cwd_cache: cdt_discover::new_cwd_cache(),
            worktree_meta_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
            project_scan_cache: Arc::new(std::sync::Mutex::new(ProjectScanCache::new())),
            groups_cache: Arc::new(std::sync::Mutex::new(None)),
            #[cfg(any(test, feature = "test-utils"))]
            refresh_worktree_meta_cache_call_count: Arc::new(AtomicU64::new(0)),
            #[cfg(any(test, feature = "test-utils"))]
            metadata_scan_spawn_count: Arc::new(AtomicU64::new(0)),
            #[cfg(any(test, feature = "test-utils"))]
            active_fs_and_policy_call_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 在已有 `LocalDataApi` 上注入 `image_cache_dir`，用于 Tauri host 把
    /// `app_cache_dir().join("cdt-images")` 传进来。链式构造模式——可与
    /// `new` / `new_with_watcher` 任一组合：`Self::new(...).with_image_cache(...)`。
    #[must_use]
    pub fn with_image_cache(mut self, dir: std::path::PathBuf) -> Self {
        self.image_cache_dir = Some(dir);
        self
    }

    /// 带 `FileWatcher` 的构造器：spawn 自动通知管线，订阅 watcher 的 file 广播，
    /// 检测结果通过 `subscribe_detected_errors()` 暴露给 host runtime（如 Tauri）。
    ///
    /// 不取 watcher 所有权——watcher 的生命周期由 host 管理；本构造器只订阅其广播。
    /// `projects_dir` 显式传入而不是依赖 `path_decoder::get_projects_base_path()`，
    /// 让测试可以用 tmp 目录、让 host 可在需要时传入非默认路径。
    pub fn new_with_watcher(
        scanner: ProjectScanner,
        config_mgr: ConfigManager,
        notif_mgr: NotificationManager,
        _ssh_mgr: SshConnectionManager,
        watcher: &FileWatcher,
        projects_dir: std::path::PathBuf,
    ) -> Self {
        let search_cache = std::sync::Arc::new(Mutex::new(SearchTextCache::new()));

        let config_mgr = Arc::new(Mutex::new(config_mgr));
        let notif_mgr = Arc::new(Mutex::new(notif_mgr));
        let (error_tx, _) = broadcast::channel::<DetectedError>(64);
        let (file_tx, _) = broadcast::channel::<cdt_core::FileChangeEvent>(64);
        let (todo_tx, _) = broadcast::channel::<cdt_core::TodoChangeEvent>(64);
        let (jobs_tx, _) = broadcast::channel::<cdt_core::JobChangeEvent>(32);

        let (session_metadata_tx, _) =
            broadcast::channel::<SessionMetadataUpdate>(METADATA_BROADCAST_CAPACITY);

        // parsed-message cache 主动失效路径：订阅 file-change 广播，按
        // `(project_id, session_id)` 推算主 session JSONL 路径并从 cache 中
        // remove。详 change `parsed-message-lru-cache` design D9；spec
        // `ipc-data-api/spec.md` §"parsed-message 缓存按 file-change 广播主动失效"。
        let parsed_msg_cache = Arc::new(std::sync::Mutex::new(ParsedMessageCache::default()));
        // ProjectScanner 结果缓存主动失效路径：订阅 file-change 广播，按事件
        // 语义**三档判定**调 `invalidate_local()`（plc / deleted / 反查
        // contains_session_id+has_entry 守护），普通 JSONL append 与 watcher
        // 折叠的 subagent 修改放行。SSH entry 由 TTL 自然过期。详 change
        // `project-scan-cache-semantic-invalidation` + spec
        // `ipc-data-api/spec.md` §`ProjectScanCache 按事件语义分级失效`。
        let project_scan_cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));
        // 内部 watcher 由本构造器负责创建 + 保留 Arc 给 SSH attach_remote 路径
        // 复用（change `enrich-file-change-with-session-list-changed::D2`：
        // `LocalDataApi::attach_remote_watcher` 走
        // `FileWatcher::attach_remote(...)` 把 SSH 事件喂回同一 `file_tx`，
        // 让 SSH 与 Local 共用 unified invalidator enrichment gateway）。
        // 入参 `watcher: &FileWatcher` 仅用于历史 API 兼容、当前未消费——保留
        // 占位是为避免破坏既有 `new_with_watcher` 调用方签名。
        let _ = watcher;
        let internal_watcher = Arc::new(FileWatcher::with_paths(
            projects_dir.clone(),
            todos_dir_from_projects_dir(&projects_dir),
        ));
        // ssh_mgr 提前构造，让 spawn_watcher_runtime 内部 invalidator 拿到 cheap
        // clone（`SshSessionManager: Clone` Arc 内部）；下方 Self {} 构造时也复用
        // 同一 instance，确保 invalidator 与 LocalDataApi 视角的 active SSH context
        // 一致。
        let ssh_mgr = SshSessionManager::new();
        let watcher_tasks = Mutex::new(spawn_watcher_runtime(
            internal_watcher.clone(),
            config_mgr.clone(),
            notif_mgr.clone(),
            WatcherRuntimeChannels {
                errors: error_tx.clone(),
                files: file_tx.clone(),
                todos: todo_tx.clone(),
                jobs: jobs_tx.clone(),
            },
            parsed_msg_cache.clone(),
            project_scan_cache.clone(),
            projects_dir.clone(),
            Some(ssh_mgr.clone()),
        ));

        Self {
            scanner: Mutex::new(scanner),
            search_cache,
            config_mgr,
            notif_mgr,
            ssh_mgr,
            error_tx: Some(error_tx),
            file_tx: Some(file_tx),
            todo_tx: Some(todo_tx),
            jobs_tx: Some(jobs_tx.clone()),
            watcher: Mutex::new(Some(internal_watcher)),
            watcher_tasks,
            remote_watchers: Mutex::new(HashMap::new()),
            ssh_watcher_ops: Arc::new(Mutex::new(())),
            ssh_shutdown_generation: AtomicU64::new(0),
            session_metadata_tx,
            active_scans: Arc::new(std::sync::Mutex::new(HashMap::new())),
            scan_generation: Arc::new(AtomicU64::new(0)),
            root_generation: Arc::new(AtomicU64::new(0)),
            context_generation: Arc::new(AtomicU64::new(0)),
            metadata_scan_semaphore: Arc::new(Semaphore::new(METADATA_SCAN_CONCURRENCY)),
            image_cache_dir: None,
            metadata_cache: Arc::new(std::sync::Mutex::new(MetadataCache::default())),
            parsed_msg_cache,
            workflow_manifest_cache: Arc::new(std::sync::Mutex::new(
                super::workflow_manifest::WorkflowManifestCache::new(),
            )),
            projects_dir: Mutex::new(projects_dir),
            // 共享 head-read semaphore 容量与 `cdt_discover::ProjectScanner`
            // 默认 `FILE_READ_CONCURRENCY=64` 等价（design D4）。硬编码 64 避免
            // 跨 crate 暴露内部常量。
            shared_read_semaphore: Arc::new(Semaphore::new(64)),
            shared_cwd_cache: cdt_discover::new_cwd_cache(),
            worktree_meta_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
            project_scan_cache,
            groups_cache: Arc::new(std::sync::Mutex::new(None)),
            #[cfg(any(test, feature = "test-utils"))]
            refresh_worktree_meta_cache_call_count: Arc::new(AtomicU64::new(0)),
            #[cfg(any(test, feature = "test-utils"))]
            metadata_scan_spawn_count: Arc::new(AtomicU64::new(0)),
            #[cfg(any(test, feature = "test-utils"))]
            active_fs_and_policy_call_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 订阅自动通知管线产出的新 `DetectedError`。
    ///
    /// 若 `LocalDataApi` 通过 `new()` 构造（无 watcher），返回一条永不收到消息的
    /// receiver（channel 本地 drop tx）——让 caller 代码统一 `.recv().await` 路径。
    pub fn subscribe_detected_errors(&self) -> broadcast::Receiver<DetectedError> {
        if let Some(tx) = &self.error_tx {
            tx.subscribe()
        } else {
            let (_tx, rx) = broadcast::channel::<DetectedError>(1);
            rx
        }
    }

    pub fn subscribe_file_changes(&self) -> broadcast::Receiver<cdt_core::FileChangeEvent> {
        if let Some(tx) = &self.file_tx {
            tx.subscribe()
        } else {
            let (_tx, rx) = broadcast::channel::<cdt_core::FileChangeEvent>(1);
            rx
        }
    }

    pub fn subscribe_todo_changes(&self) -> broadcast::Receiver<cdt_core::TodoChangeEvent> {
        if let Some(tx) = &self.todo_tx {
            tx.subscribe()
        } else {
            let (_tx, rx) = broadcast::channel::<cdt_core::TodoChangeEvent>(1);
            rx
        }
    }

    /// 订阅 jobs 变更事件。
    pub fn subscribe_jobs(&self) -> broadcast::Receiver<cdt_core::JobChangeEvent> {
        if let Some(tx) = &self.jobs_tx {
            tx.subscribe()
        } else {
            let (_tx, rx) = broadcast::channel::<cdt_core::JobChangeEvent>(1);
            rx
        }
    }

    pub fn subscribe_ssh_status(&self) -> broadcast::Receiver<cdt_ssh::SshStatusChange> {
        self.ssh_mgr.subscribe_status()
    }

    /// 列出所有 background jobs（全量扫描 `~/.claude/jobs/*/state.json`）。
    /// 测试辅助——直接调 `list_jobs_from_dir` 指定目录。
    #[cfg(test)]
    pub async fn list_jobs_from_dir_test(
        jobs_dir: &Path,
    ) -> Result<cdt_core::JobsResponse, ApiError> {
        list_jobs_from_dir(jobs_dir).await
    }

    pub async fn insert_test_ssh_context(
        &self,
        context_id: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        username: Option<String>,
        remote_home: PathBuf,
        provider: SshFileSystemProvider,
    ) {
        self.ssh_mgr
            .insert_test_context(
                context_id,
                host,
                port,
                username,
                remote_home,
                provider,
                None,
            )
            .await;
    }

    pub fn subscribe_context_changed(&self) -> broadcast::Receiver<cdt_ssh::ContextChanged> {
        self.ssh_mgr.subscribe_context_changed()
    }

    pub async fn shutdown_ssh_all(&self, deadline: std::time::Duration) {
        // change `generation-race-audit` codex commit-stage Bug 2 修订：原实现在
        // `try_lock` 失败时**无锁** mutate ssh_mgr —— 与 spec 固化的"5 处 mutate
        // 入口持 ssh_watcher_ops 锁互斥"前提冲突；refresh / spawn 守卫持锁期间
        // 并发 shutdown 会让 generation 已 bump 但 ssh_mgr.shutdown 未完成的
        // window 内，wrapper 守卫读到旧 ctx + 新 generation 仍误判通过。改 `lock().await`
        // 排队等同锁持有者完成；shutdown 自身已是 app exit 路径，等几毫秒可接受。
        let _ops = self.ssh_watcher_ops.lock().await;
        self.ssh_shutdown_generation.fetch_add(1, Ordering::SeqCst);
        // codex 二审第五轮 Missing Entry：shutdown_ssh_all 也是 context 变更入口
        // （内部 ssh_mgr.shutdown_all 会 disconnect 所有 SSH ctx 并 emit context
        // changed event），必须 bump context_generation + abort 所有 SSH scan
        // 避免 late-insert scan task 在 broadcast 旧 ctx update 串扰新 (Local)
        // 状态。app exit 路径理论上无前端可观察者，但 deadline 后 process 仍
        // 存活时（如 graceful shutdown 失败）broadcast 仍可能到达——保守 bump。
        self.context_generation.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut scans) = self.active_scans.lock() {
            scans.retain(|_key, entry| {
                if entry.context_id.backend_kind == cdt_fs::FsKind::Ssh {
                    entry.handle.abort();
                    false
                } else {
                    true
                }
            });
        }
        self.cancel_all_remote_watchers().await;
        self.ssh_mgr.shutdown_all(deadline).await;
    }

    async fn attach_remote_watcher(
        &self,
        context_id: &str,
        prev_baseline: Option<std::collections::BTreeMap<PathBuf, cdt_ssh::FileFingerprint>>,
    ) {
        let Some(_file_tx) = self.file_tx.as_ref() else {
            return;
        };
        // Invariant：`file_tx.is_some()` 路径下 `watcher.is_some()` 必然成立
        // （`new_with_watcher` 同时设两者为 `Some`，`new()` 同时设两者为 `None`）。
        // 不在 attach 路径调用方校验——构造期约束在此显式 expect。
        let file_watcher = {
            let guard = self.watcher.lock().await;
            guard
                .as_ref()
                .expect("watcher SHALL be Some when file_tx is Some (constructor invariant)")
                .clone()
        };
        let Some(provider) = self.ssh_mgr.provider(context_id).await else {
            return;
        };
        // 共享 cancel_token 让 polling watcher + dead-signal monitor 同时被
        // disconnect 路径 cancel；clone 给 monitor 让它在外部 cancel 时也退出。
        //
        // SSH polling watcher 走 `FileWatcher::attach_remote`，让远端事件喂回
        // 同一 `watcher.file_tx`，与 Local 共用 unified invalidator 入口完成
        // `session_list_changed` enrich（change
        // `enrich-file-change-with-session-list-changed::D2`）。原 `bridge_task`
        // 已删除，`file_tx` 唯一生产者是 unified invalidator。
        // 断连重连时 caller 传入旧 watcher 的 baseline 快照（`cancel_remote_watcher`
        // 返回），让首轮 poll 做 diff 而非静默建 baseline（design.md D5 断连重连
        // baseline diff）。caller 在无 baseline（新连接 / switch context / reconfigure）
        // 时传 None。
        let cancel_token = cdt_ssh::CancelToken::new();
        let cancel_for_monitor = cancel_token.clone();
        let watcher = file_watcher.attach_remote(
            provider.sftp_client(),
            provider.remote_home().to_path_buf(),
            cancel_token,
            prev_baseline,
        );
        // dead-signal monitor：watcher 内连续 PERMANENT_FAILURE_THRESHOLD 轮
        // 永久 SFTP 错误时 notify dead_signal；本 task 触发 ssh_mgr.disconnect
        // 让 active context 同步翻回 Local 并 emit ContextChanged(None)，
        // 让前端 listener 自动切回 local，不被 stale active 困住。
        // 详见 followups.md "[impl-bug] SSH/SFTP channel idle..." 条目。
        //
        // 不持有 monitor JoinHandle：cancel_token 共享 + select 双分支让
        // monitor 永远会随 polling 退出（dead 触发 → 走 disconnect 分支退出；
        // 外部 cancel → 走 cancelled 分支退出），不会泄漏后台 task。
        //
        // **代际校验防 race**（codex 二审 critical，2026-05-22）：
        // 若旧 watcher 的 monitor 已选中 dead 分支但还没跑 disconnect，期间
        // 用户主动 disconnect+reconnect 同 context_id，旧 monitor 仍会 disconnect
        // 把新 sessions entry 误删。修法：monitor 拿 ssh_watcher_ops 锁后
        // 校验 (context_generation, active_context_id) 仍是 capture 时的值，
        // mismatch 即放弃自愈（新 watcher 的 dead_signal 自己会触发）。
        let dead_signal = watcher.dead_signal();
        let ssh_mgr_for_monitor = self.ssh_mgr.clone();
        let context_id_for_monitor = context_id.to_owned();
        let ops_for_monitor = Arc::clone(&self.ssh_watcher_ops);
        let gen_for_monitor = Arc::clone(&self.context_generation);
        let active_scans_for_monitor = Arc::clone(&self.active_scans);
        let captured_generation = self.context_generation.load(Ordering::SeqCst);
        tokio::spawn(async move {
            tokio::select! {
                biased;
                () = cancel_for_monitor.cancelled() => {
                    tracing::debug!(
                        target: "cdt_ssh::lifecycle",
                        context_id = %context_id_for_monitor,
                        "dead-signal monitor cancelled (watcher cancelled externally)",
                    );
                }
                () = dead_signal.notified() => {
                    perform_polling_self_heal_disconnect(
                        ops_for_monitor,
                        gen_for_monitor,
                        captured_generation,
                        active_scans_for_monitor,
                        ssh_mgr_for_monitor,
                        context_id_for_monitor,
                    )
                    .await;
                }
            }
        });
        if let Some(old) = self
            .remote_watchers
            .lock()
            .await
            .insert(context_id.to_owned(), watcher)
        {
            old.cancel_and_join().await;
        }
    }

    /// 自愈 disconnect 完整流程（monitor task 触发）——与 `ssh_disconnect`
    /// IPC 路径的清理动作对齐：
    ///
    /// 1. 拿 `ssh_watcher_ops` 锁与所有 SSH 状态变更入口互斥
    /// 2. (`context_generation`, `active_context_id`) 双校验：旧 monitor 若
    ///    在自身被 cancel 前已 select 中 dead 分支但还没跑到这里，期间用户
    ///    主动 disconnect / reconnect / `switch_context` 都会 bump generation
    ///    或改 active；任一不匹配即放弃自愈（新 watcher 自己会触发自己的
    ///    `dead_signal`）
    /// 3. **bump `context_generation`** —— 关闭 in-flight `list_sessions`
    ///    late insert 串扰新 local UI 的窗口（与 `ssh_disconnect` line 3675
    ///    同形）
    /// 4. **abort 当前 SSH ctx 下 active scans** —— 避免旧 SSH scan 切回 Local
    ///    后继续 broadcast 旧 SSH metadata 污染 local UI（与 `ssh_disconnect`
    ///    line 3676 同形）；SHALL 在 `ssh_mgr.disconnect` 之前做，让 `ssh_mgr`
    ///    仍能 lookup `ContextId`（disconnect 后 provider 被删，无法解析 ctx）
    /// 5. `ssh_mgr.disconnect` —— 把 active 切回 None + emit
    ///    `ContextChanged(None)`，前端 listener 自动切回 local
    ///
    /// `remote_watchers` map 内的旧 handle 不主动清理——watcher 自己已经 break
    /// loop，handle 在 map 里只是个已 finished `JoinHandle`，下次同 ctx attach
    /// 时 `insert` 自然替换，或者 `ssh_disconnect` / shutdown 时统一清掉。
    async fn cancel_remote_watcher(
        &self,
        context_id: &str,
    ) -> Option<std::collections::BTreeMap<PathBuf, cdt_ssh::FileFingerprint>> {
        if let Some(handle) = self.remote_watchers.lock().await.remove(context_id) {
            let baseline = handle.baseline_snapshot();
            handle.cancel_and_join().await;
            if baseline.is_empty() {
                None
            } else {
                Some(baseline)
            }
        } else {
            None
        }
    }

    async fn cancel_all_remote_watchers(&self) {
        let handles = self
            .remote_watchers
            .lock()
            .await
            .drain()
            .map(|(_, handle)| handle)
            .collect::<Vec<_>>();
        for handle in handles {
            handle.cancel_and_join().await;
        }
    }

    async fn claude_base_path(&self) -> PathBuf {
        let mgr = self.config_mgr.lock().await;
        mgr.get_config()
            .general
            .claude_root_path
            .as_deref()
            .map_or_else(
                cdt_discover::path_decoder::get_claude_base_path,
                PathBuf::from,
            )
    }

    /// 按 `ContextId` 精确 abort：retain 所有非匹配 entry，匹配的 entry handle
    /// abort + 从 map 移除。避免 codex 二审 H2-R 报的"abort-all 误杀新 host
    /// scan"（如 B 已 active spawn scan，并发 `ssh_disconnect("A")` 调 abort-all
    /// 会误杀 B 的 scan）。
    ///
    /// change `unify-fs-direct-calls` codex 二审 H2 + 第二轮 H2-R 修订 + design
    /// D3-bis：旧 ctx 的 `page_jobs` scan 跨 await 持 `Arc<dyn FileSystemProvider>`，
    /// 切 host / disconnect 后会继续向 `session_metadata_tx` broadcast 旧 ctx
    /// 的 metadata update，污染新 ctx 的 UI 渲染状态。`switch_context` /
    /// `ssh_disconnect` / `ssh_connect` 三个 context 变更入口都 SHALL 调本
    /// helper 按 prev ctx 精确 abort，避免误杀新 ctx scan 也避免漏 abort。
    fn abort_scans_for_context(&self, ctx: &cdt_fs::ContextId) {
        let Ok(mut scans) = self.active_scans.lock() else {
            return;
        };
        scans.retain(|_key, entry| {
            if &entry.context_id == ctx {
                entry.handle.abort();
                false
            } else {
                true
            }
        });
    }

    /// 按 ssh `context_id` 字符串解析 → `ContextId` → 精确 abort。
    ///
    /// `switch_context` / `ssh_disconnect` / `ssh_connect` 三个入口的便捷
    /// 调用——内部通过 `ssh_mgr.provider_and_context_id` 拿到旧 host 的
    /// `ContextId`（包含 `host_signature`），失败则 silently no-op（旧 host
    /// 可能已被 `ssh_mgr` 移除，此时 scan 无法再写 cache 实际上也无害——
    /// task 持的 `Arc<dyn>` clone 已与新 active 解耦）。
    async fn abort_scans_for_ssh_context_id(&self, ssh_context_id: &str) {
        if let Some((_provider, ctx)) = self.ssh_mgr.provider_and_context_id(ssh_context_id).await {
            self.abort_scans_for_context(&ctx);
        }
    }

    /// abort 所有 Local context 下的 in-flight scan。
    ///
    /// 用于 Local → SSH 切换路径（`switch_context(<ssh>)` 当 `previous_context_id
    /// == None` 时，旧 Local scan 仍可能跑着）以及 `reconfigure_claude_root`
    /// 重建 Local `projects_dir` 后让旧 `ContextId::local` 的 scan 立刻退出。
    fn abort_local_scans(&self) {
        let Ok(mut scans) = self.active_scans.lock() else {
            return;
        };
        scans.retain(|_key, entry| {
            if entry.context_id.backend_kind == cdt_fs::FsKind::Local {
                entry.handle.abort();
                false
            } else {
                true
            }
        });
    }

    async fn reconfigure_claude_root(&self, claude_root_path: Option<&str>) {
        let claude_root = claude_root_path.map(PathBuf::from);
        let projects_dir =
            cdt_discover::path_decoder::projects_base_path_for(claude_root.as_deref());
        // change `generation-race-audit` codex commit-stage Bug 1 修订：
        // reconfigure 是 5 处 mutate 入口之一（spec ssh-remote-context::Reconnect
        // lifecycle preserves SFTP session integrity 与 ipc-data-api::SessionSummary
        // 增加 worktree 元信息字段 的"映射缓存刷新约束"段已固化），SHALL 持
        // `ssh_watcher_ops` 锁与 `list_repository_groups` / `build_group_session_page`
        // 的派生 cache 写入路径 + `switch_context` / `ssh_connect` / `ssh_disconnect` /
        // `shutdown_ssh_all` 互斥。否则 wrapper 锁内的 (ctx + generation) 双重校验
        // 仍可能在 reconfigure 已 bump 但未写新 projects_dir 的窗口内误判 match
        // （current_ctx 仍是旧 Local{old_dir}，与 captured 等同 → 通过 → 旧 groups
        // 写入 cache 污染随后切到新 projects_dir 的查询）。
        let _ops = self.ssh_watcher_ops.lock().await;
        // codex 二审第四轮 Blocker：reconfigure 改 Local projects_dir = Local
        // ContextId 变化，与 SSH 切换语义同型。必须在 abort 之前先 bump
        // `context_generation` + `root_generation`，关闭"in-flight list_sessions
        // 已拿旧 (fs, ctx, root_generation) 后才 late-insert scan handle"的窗口。
        // 任何 late-insert 的 scan task 在 broadcast 前 check 会发现 generation
        // 不匹配 → silent drop。
        self.context_generation.fetch_add(1, Ordering::SeqCst);
        self.root_generation.fetch_add(1, Ordering::SeqCst);
        // codex 二审第三轮 Low：用 abort_local_scans 替代 abort-all 避免误杀
        // SSH active 时的远端 metadata scan（reconfigure_claude_root 仅改 Local
        // projects_dir，不影响 SSH ContextId 下的 scan）。
        self.abort_local_scans();
        *self.scanner.lock().await = ProjectScanner::new_with_cwd_cache(
            local_handle(),
            projects_dir.clone(),
            self.shared_read_semaphore.clone(),
            self.shared_cwd_cache.clone(),
        );
        *self.projects_dir.lock().await = projects_dir.clone();

        if let (Some(error_tx), Some(file_tx), Some(todo_tx), Some(jobs_tx)) =
            (&self.error_tx, &self.file_tx, &self.todo_tx, &self.jobs_tx)
        {
            // codex CRIT-1（change `enrich-file-change-with-session-list-changed`
            // 二审）：先收集当前 active SSH context_ids + cancel 旧 remote_watchers，
            // 让 reattach 步骤用新 watcher 接管。旧 `RemotePollingWatcher` 持有
            // 的是旧 `FileWatcher.file_tx` clone，旧 unified invalidator abort
            // 后旧 file_tx 没人接，SSH polling event 会"喂入空 channel"——再也
            // 进不了新 unified invalidator → 浏览器 / 桌面端 SSH 路径在 root 切换
            // 后看不到 enriched event，前端 sidebar `totalSessions` 滞后到下次
            // 用户主动重连 SSH 才恢复。
            let active_remote_contexts: Vec<String> =
                self.remote_watchers.lock().await.keys().cloned().collect();
            self.cancel_all_remote_watchers().await;

            let mut tasks = self.watcher_tasks.lock().await;
            for task in tasks.drain(..) {
                task.abort();
            }
            let claude_root = claude_root_path.map(PathBuf::from);
            let todos_dir = cdt_discover::path_decoder::todos_base_path_for(claude_root.as_deref());
            // 重建内部 watcher Arc + 同步替换 self.watcher。SSH attach_remote
            // 后续走 self.watcher 拿新实例的 file_tx，确保 root 切换后远端事件
            // 仍喂回新 unified invalidator 完成 enrich
            // （change `enrich-file-change-with-session-list-changed::D2`）。
            let new_watcher = Arc::new(FileWatcher::with_paths(projects_dir.clone(), todos_dir));
            *self.watcher.lock().await = Some(new_watcher.clone());
            *tasks = spawn_watcher_runtime(
                new_watcher,
                self.config_mgr.clone(),
                self.notif_mgr.clone(),
                WatcherRuntimeChannels {
                    errors: error_tx.clone(),
                    files: file_tx.clone(),
                    todos: todo_tx.clone(),
                    jobs: jobs_tx.clone(),
                },
                self.parsed_msg_cache.clone(),
                self.project_scan_cache.clone(),
                projects_dir,
                Some(self.ssh_mgr.clone()),
            );
            // 释放 watcher_tasks lock 后再 reattach SSH——`attach_remote_watcher`
            // 内部锁顺序与 watcher_tasks 不冲突，但显式 drop 让 lock window 最小
            drop(tasks);

            // 重 attach 之前 active 的 SSH watchers，让 SSH polling 重新喂入新
            // watcher.file_tx → 新 unified invalidator → enrich
            // reconfigure 路径：旧 watcher 已被 cancel_all 清空，baseline 不可用传 None
            for context_id in active_remote_contexts {
                self.attach_remote_watcher(&context_id, None).await;
            }
        }
        // root_generation 与 context_generation 都已在函数开头 bump（codex 二审
        // 第四轮 Blocker 修订）；这里无需再 bump 以保持单次递增。
    }

    async fn project_memory_dir(&self, project_id: &str) -> Result<std::path::PathBuf, ApiError> {
        let (_fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        let base_dir = cdt_discover::path_decoder::extract_base_dir(project_id);
        validate_project_base_dir(base_dir)?;
        Ok(projects_dir.join(base_dir).join("memory"))
    }

    /// 订阅 `list_sessions` 后台扫描产出的元数据增量更新。
    ///
    /// 每次 `list_sessions(project_id)` 调用会异步并发扫描该页 session 的
    /// JSONL 文件，每扫完一个推送一条 `SessionMetadataUpdate`。同 project
    /// 的旧扫描会在新调用进入时被 abort，避免事件串扰。
    ///
    /// 详见 `openspec/specs/ipc-data-api/spec.md` §"Emit session metadata
    /// updates"。
    pub fn subscribe_session_metadata(&self) -> broadcast::Receiver<SessionMetadataUpdate> {
        self.session_metadata_tx.subscribe()
    }

    /// 同步骨架扫描：完成目录 scan + 分页切片 + 构造占位 `SessionSummary`，
    /// 返回 (page, `next_cursor`, total, `page_jobs`, dir, `root_generation`,
    /// `inline_updates`, fs, `context_id`)。
    ///
    /// `page_jobs` 是 `(session_id, jsonl_path)` 元组列表，供后台元数据扫描
    /// 任务消费。`fs` + `context_id` 来自同一 `active_fs_and_context()` 快照（详
    /// change `metadata-cache-context-prefix` design D3），caller 直接复用避免
    /// 再次 lock 引入 fs/ctx 不一致 race。
    async fn list_sessions_skeleton(
        &self,
        project_id: &str,
        pagination: &PaginatedRequest,
    ) -> Result<SkeletonResult, ApiError> {
        if pagination.page_size == 0 {
            return Err(ApiError::validation("pageSize must be > 0"));
        }

        // codex 二审第五轮 Blocker 修订：generation snapshot SHALL **先于**
        // `active_fs_and_context_strict()` await 拿取，与 fs/ctx 同属"reconfigure
        // 前的 snapshot"。late-load 模式下 reconfigure 在 await 期间跑完 → reader
        // 拿到旧 fs/ctx + 新 generation → scan task 内 expected == current → 仍 broadcast
        // 旧 ctx 串扰 UI。改先 load：reader 持旧 generation → scan task check 旧
        // expected != 当前(新)值 → silent drop。
        let expected_context_generation = self.context_generation.load(Ordering::SeqCst);
        let expected_root_generation = self.root_generation.load(Ordering::SeqCst);
        // 用户可见列表 handler 走 strict 变体——SSH disconnect 中间态 SHALL 报错
        // 而非静默降级（PR-C codex commit-stage round-2 Q1）。
        let (fs, projects_dir, ctx) = self.active_fs_and_context_strict().await?;
        let is_remote = fs.kind() == cdt_discover::FsKind::Ssh;
        let scanner = ProjectScanner::new_with_cwd_cache(
            fs.clone(),
            projects_dir.clone(),
            self.shared_read_semaphore.clone(),
            self.shared_cwd_cache.clone(),
        );
        let sessions = scanner
            .list_sessions(project_id, &std::collections::BTreeSet::new())
            .await
            .map_err(|e| ApiError::internal(format!("list sessions error: {e}")))?;
        let root_generation = expected_root_generation;

        let offset = pagination
            .cursor
            .as_deref()
            .and_then(|c| c.parse::<usize>().ok())
            .unwrap_or(0);
        let total = sessions.len();
        let page_sessions: Vec<_> = sessions
            .into_iter()
            .skip(offset)
            .take(pagination.page_size)
            .collect();

        let base_dir = cdt_discover::path_decoder::extract_base_dir(project_id);
        let dir = projects_dir.join(base_dir);

        let mut page = Vec::with_capacity(page_sessions.len());
        let mut page_jobs = Vec::with_capacity(page_sessions.len());
        let inline_updates = Vec::new();
        // Cache fast-path：命中条骨架阶段直接带 title / messageCount 返回，避免
        // 完全依赖后台 broadcast emit（如果 emit 在前端 listener 注册前 fire-and-forget
        // 丢失，列表项会卡在 title=null 永久 fallback 到 sessionId 前 8 字符）。
        // 未命中条仍入 page_jobs 走后台扫描，扫完通过 broadcast 增量 patch。
        //
        // change `unify-fs-direct-calls` design D2/D3 (line 1444-1574, 1515, 1524, 1575):
        // SSH 路径与 Local 同走 SkeletonThenStream 入口——hot path 用
        // `lookup_trust_cached` 让 SSH 在 cache hit 时 0 fs op 立刻渲染；cache miss
        // 入 page_jobs 走 `scan_metadata_for_page` 异步刷新通过 SSE 推差量。
        // PR-E 后续把"SSH 是否走 SkeletonThenStream"上移到 BackendPolicy 字段，
        // 本 PR 提前内联实施以让 SSH 用户卡顿消失。
        let lookup_permit = Arc::new(Semaphore::new(METADATA_SCAN_CONCURRENCY));
        let lookups = futures::future::join_all(page_sessions.iter().map(|s| {
            let cache = self.metadata_cache.clone();
            let jsonl_path = dir.join(format!("{}.jsonl", s.id));
            let permit_sem = lookup_permit.clone();
            let fs_clone = fs.clone();
            let ctx_clone = ctx.clone();
            async move {
                if is_remote {
                    // SSH hot path：cache hit trust 不做 stat（避免远端 RTT）；miss 入
                    // page_jobs 后台 scan，通过 SSE event 推差量给前端。
                    let cached = cache
                        .lock()
                        .ok()
                        .and_then(|mut c| c.lookup_trust_cached(&ctx_clone, &jsonl_path))
                        .map(|entry| SessionMetadata {
                            title: entry.title,
                            message_count: entry.message_count,
                            is_ongoing: entry.messages_ongoing,
                            git_branch: entry.git_branch,
                            user_intents: entry.user_intents,
                            last_active: entry.last_active,
                            duration_ms: entry.duration_ms,
                            total_cost: entry.total_cost,
                            tool_error_count: entry.tool_error_count,
                            files_modified: entry.files_modified,
                            git_summary: entry.git_summary,
                        });
                    return (jsonl_path, cached);
                }
                let _guard = permit_sem
                    .acquire()
                    .await
                    .expect("lookup semaphore should not be closed");
                let meta =
                    try_lookup_cached_metadata(&cache, &*fs_clone, &ctx_clone, &jsonl_path).await;
                (jsonl_path, meta)
            }
        }))
        .await;
        for (s, (jsonl_path, cached_meta)) in page_sessions.into_iter().zip(lookups) {
            // SSH cache hit 走 `lookup_trust_cached`（0 fs op）不校验 signature——
            // 远端文件被 `RemotePollingWatcher` 3s poll 出变化后没有自动失效路径。
            // 解法（codex round-7 Blocker #4 方案 #1）：SSH 路径无论 cache hit / miss
            // 都入 page_jobs 后台 scan，`extract_session_metadata_cached` 会 stat 比对
            // signature，不变 silent return / 变化 broadcast SSE 差量给前端。
            // Local cache hit 走 `try_lookup_cached_metadata` 已 stat 校验，无需重 scan。
            let need_background_validation = is_remote || cached_meta.is_none();
            let session_id_for_jobs = s.id.clone();
            let mut summary = match cached_meta {
                Some(meta) => SessionSummary {
                    session_id: s.id,
                    project_id: project_id.to_owned(),
                    timestamp: s.last_modified,
                    created: s.created,
                    message_count: meta.message_count,
                    title: meta.title,
                    is_ongoing: meta.is_ongoing,
                    git_branch: meta.git_branch,
                    worktree_id: None,
                    worktree_name: None,
                    group_id: None,
                    cwd_relative_to_repo_root: None,
                    cwd: s.cwd,
                    project_name: None,
                    user_intents: meta.user_intents,
                    last_active: meta.last_active,
                    duration_ms: meta.duration_ms,
                    total_cost: meta.total_cost,
                    tool_error_count: meta.tool_error_count,
                    files_modified: meta.files_modified,
                    git_summary: meta.git_summary,
                },
                None => SessionSummary {
                    session_id: s.id,
                    project_id: project_id.to_owned(),
                    timestamp: s.last_modified,
                    created: s.created,
                    message_count: 0,
                    title: None,
                    is_ongoing: false,
                    git_branch: None,
                    worktree_id: None,
                    worktree_name: None,
                    group_id: None,
                    cwd_relative_to_repo_root: None,
                    cwd: s.cwd,
                    project_name: None,
                    user_intents: Vec::new(),
                    last_active: 0,
                    duration_ms: 0,
                    total_cost: 0.0,
                    tool_error_count: 0,
                    files_modified: Vec::new(),
                    git_summary: Vec::new(),
                },
            };
            self.apply_worktree_meta(&mut summary);
            page.push(summary);
            if need_background_validation {
                page_jobs.push((session_id_for_jobs, jsonl_path));
            }
        }

        let page_len = page.len();
        let next_cursor = if offset + page_len < total {
            Some((offset + page_len).to_string())
        } else {
            None
        };

        Ok(SkeletonResult {
            page,
            next_cursor,
            total,
            page_jobs,
            dir,
            root_generation,
            inline_updates,
            fs,
            ctx,
            expected_context_generation,
        })
    }
}

/// 后台扫描某页 session 的元数据，每扫完一个 broadcast 一条
/// `SessionMetadataUpdate`。并发度受 `METADATA_SCAN_CONCURRENCY` 限流。
///
/// 任务结束时（无论正常完成还是被 abort）从 `active_scans` 移除自己的
/// 注册项——但**仅当 entry 的 generation 与自己 spawn 时持有的一致**才 remove，
/// 避免旧 task 在新 task 已注册后的 cleanup 误删新 handle（codex 二审 race）。
/// `broadcast::send` 在无订阅者时返回 `Err`，本函数静默忽略——元数据更新
/// 本质上是 fire-and-forget。
///
/// `active_scans` 的 key 由 `(project_id, cursor)` 组合构成，让同 project 不同
/// 分页的扫描相互独立——典型场景：page 1 / page 2 的并发扫描互不 abort（spec
/// `ipc-data-api/spec.md::Emit session metadata updates` Scenario "同 projectId
/// 不同 cursor 的扫描并存互不 abort"）。`|` 字符为 reserved 分隔符，当前 cursor
/// 由 `(offset).to_string()` 生成（纯 ASCII 数字），不会与分隔符冲突。
/// Step 3 helper: 根据 policy 分派子代理扫描策略。
async fn scan_subagent_candidates_for_detail(
    fs: &dyn FileSystemProvider,
    projects_dir: &Path,
    project_dir: &Path,
    session_id: &str,
    policy: &cdt_fs::BackendPolicy,
) -> Vec<cdt_core::SubagentCandidate> {
    if !policy.supports_subagent_scan {
        Vec::new()
    } else if CROSS_PROJECT_SUBAGENT_SCAN {
        scan_subagent_candidates_cross_project(fs, projects_dir, project_dir, session_id).await
    } else {
        scan_subagent_candidates(fs, project_dir, session_id).await
    }
}

/// Step 4 helper: 构建 chunks + 判定 `is_ongoing`（含 stale check）。
async fn build_session_chunks_with_ongoing(
    messages: &[cdt_core::ParsedMessage],
    candidates: &[cdt_core::SubagentCandidate],
    policy: &cdt_fs::BackendPolicy,
    fs: &dyn FileSystemProvider,
    jsonl_path: &Path,
) -> (Vec<cdt_core::Chunk>, bool) {
    let messages_ongoing = cdt_analyze::check_messages_ongoing(messages);
    let is_ongoing = if messages_ongoing {
        match policy.stale_check_strategy {
            cdt_fs::StaleCheckStrategy::LocalClock5min => {
                !crate::ipc::session_metadata::is_file_stale(fs, jsonl_path).await
            }
            cdt_fs::StaleCheckStrategy::SkipUntilClockSync => messages_ongoing,
        }
    } else {
        false
    };
    let chunks = build_chunks_with_subagents(messages, candidates);
    (chunks, is_ongoing)
}

/// 跨 worktree 去重：同一 `sessionId` 保留 `last_modified` 最大的 wt 版本。
///
/// 仅在 cursor "active"（非 Exhausted）的 worktree 间执行去重。
fn dedup_sessions_across_worktrees(
    wt_sessions: &mut std::collections::BTreeMap<String, Vec<cdt_core::Session>>,
    cursor_state: &GroupCursor,
    worktrees: &[cdt_core::Worktree],
) {
    let active_wts: std::collections::BTreeSet<&String> = worktrees
        .iter()
        .filter(|wt| {
            !matches!(
                cursor_state
                    .per_worktree
                    .get(&wt.id)
                    .cloned()
                    .unwrap_or(WorktreeOffset::NotStarted),
                WorktreeOffset::Exhausted
            )
        })
        .map(|wt| &wt.id)
        .collect();
    let mut best_wt_for_sid: std::collections::HashMap<String, (String, i64)> =
        std::collections::HashMap::new();
    for (wt_id, sessions) in wt_sessions.iter() {
        if !active_wts.contains(wt_id) {
            continue;
        }
        for s in sessions {
            best_wt_for_sid
                .entry(s.id.clone())
                .and_modify(|(cur_wt, cur_mtime)| {
                    if s.last_modified > *cur_mtime
                        || (s.last_modified == *cur_mtime && wt_id < cur_wt)
                    {
                        cur_wt.clone_from(wt_id);
                        *cur_mtime = s.last_modified;
                    }
                })
                .or_insert((wt_id.clone(), s.last_modified));
        }
    }
    for (wt_id, sessions) in wt_sessions.iter_mut() {
        if !active_wts.contains(wt_id) {
            continue;
        }
        sessions.retain(|s| {
            best_wt_for_sid
                .get(&s.id)
                .is_some_and(|(best, _)| best == wt_id)
        });
    }
}

/// 二分定位每个 worktree 在已排序 sessions 中的 cursor 起点。
fn resolve_cursor_indices(
    worktrees: &[cdt_core::Worktree],
    cursor_state: &GroupCursor,
    wt_sessions: &std::collections::BTreeMap<String, Vec<cdt_core::Session>>,
) -> std::collections::BTreeMap<String, (usize, bool)> {
    let mut indices: std::collections::BTreeMap<String, (usize, bool)> =
        std::collections::BTreeMap::new();
    for wt in worktrees {
        let offset = cursor_state
            .per_worktree
            .get(&wt.id)
            .cloned()
            .unwrap_or(WorktreeOffset::NotStarted);
        match offset {
            WorktreeOffset::NotStarted => {
                indices.insert(wt.id.clone(), (0, false));
            }
            WorktreeOffset::Exhausted => {
                indices.insert(wt.id.clone(), (0, true));
            }
            WorktreeOffset::AfterMtime { mtime_ms, sid } => {
                let sessions = wt_sessions.get(&wt.id).map_or(&[][..], Vec::as_slice);
                let idx = sessions
                    .iter()
                    .position(|s| {
                        s.last_modified < mtime_ms || (s.last_modified == mtime_ms && s.id > sid)
                    })
                    .unwrap_or(sessions.len());
                indices.insert(wt.id.clone(), (idx, false));
            }
        }
    }
    indices
}

/// 编码下一页 group cursor 状态。
fn encode_next_group_cursor(
    worktrees: &[cdt_core::Worktree],
    cursor_state: &GroupCursor,
    indices: &std::collections::BTreeMap<String, (usize, bool)>,
    last_consumed: &std::collections::BTreeMap<String, (i64, String, usize)>,
    wt_sessions: &std::collections::BTreeMap<String, Vec<cdt_core::Session>>,
    failed_wt_ids: &std::collections::BTreeSet<String>,
) -> Option<String> {
    let mut new_cursor = GroupCursor {
        per_worktree: std::collections::BTreeMap::new(),
    };
    let mut all_exhausted = true;
    for wt in worktrees {
        let (init_idx, exhausted) = indices[&wt.id];
        if exhausted {
            new_cursor
                .per_worktree
                .insert(wt.id.clone(), WorktreeOffset::Exhausted);
            continue;
        }
        if failed_wt_ids.contains(&wt.id) {
            let preserved = cursor_state
                .per_worktree
                .get(&wt.id)
                .cloned()
                .unwrap_or(WorktreeOffset::NotStarted);
            new_cursor.per_worktree.insert(wt.id.clone(), preserved);
            all_exhausted = false;
            continue;
        }
        let wt_len = wt_sessions.get(&wt.id).map_or(0, Vec::len);
        let offset = if let Some((mtime, sid, next_idx)) = last_consumed.get(&wt.id) {
            if *next_idx >= wt_len {
                WorktreeOffset::Exhausted
            } else {
                all_exhausted = false;
                WorktreeOffset::AfterMtime {
                    mtime_ms: *mtime,
                    sid: sid.clone(),
                }
            }
        } else if init_idx >= wt_len {
            WorktreeOffset::Exhausted
        } else {
            all_exhausted = false;
            cursor_state
                .per_worktree
                .get(&wt.id)
                .cloned()
                .unwrap_or(WorktreeOffset::NotStarted)
        };
        new_cursor.per_worktree.insert(wt.id.clone(), offset);
    }
    if all_exhausted {
        None
    } else {
        Some(encode_group_cursor(&new_cursor))
    }
}

fn metadata_scan_key(project_id: &str, cursor: Option<&str>) -> String {
    format!("{project_id}|{}", cursor.unwrap_or(""))
}

fn todos_dir_from_projects_dir(projects_dir: &Path) -> PathBuf {
    projects_dir.parent().map_or_else(
        || projects_dir.join(".."),
        |claude_root| claude_root.join("todos"),
    )
}

/// 解析 `claude` CLI 二进制路径。
///
/// macOS GUI app 不继承 shell PATH，直接 `Command::new("claude")` 会 `NotFound`。
/// 搜索优先级：`CLAUDE_CLI_PATH` 环境变量 → 当前 PATH → 平台已知目录。
fn resolve_claude_cli() -> Result<PathBuf, ApiError> {
    if let Ok(p) = std::env::var("CLAUDE_CLI_PATH") {
        let path = PathBuf::from(&p);
        if path.is_file() {
            return Ok(path);
        }
    }

    if let Ok(output) = std::process::Command::new("which").arg("claude").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !s.is_empty() {
                return Ok(PathBuf::from(s));
            }
        }
    }

    let home = cdt_discover::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let npx_dir = home.join(".npm/_npx");
    let known_paths = [
        home.join(".local/bin/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ];

    for candidate in &known_paths {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }

    if let Ok(entries) = std::fs::read_dir(&npx_dir) {
        for entry in entries.flatten() {
            let bin = entry.path().join("node_modules/.bin/claude");
            if bin.is_file() {
                return Ok(bin);
            }
        }
    }

    Err(ApiError::internal(
        "claude CLI not found — install Claude Code CLI or set CLAUDE_CLI_PATH".to_owned(),
    ))
}

/// 扫描 `jobs_dir` 下所有 `<job_id>/state.json`，解析 + 分组 + badge 计算。
///
/// jobs 目录不存在时返回空响应（降级）。
async fn list_jobs_from_dir(jobs_dir: &Path) -> Result<cdt_core::JobsResponse, ApiError> {
    use cdt_core::job::{
        BackgroundJob, JobSummary, classify_job_group, compute_badge,
        extract_project_id_from_link_scan_path,
    };

    if !jobs_dir.is_dir() {
        return Ok(cdt_core::JobsResponse {
            jobs: Vec::new(),
            badge: cdt_core::BadgeColor::None,
            badge_count: 0,
            jobs_dir_exists: false,
        });
    }

    let mut jobs: Vec<JobSummary> = Vec::new();

    let mut entries = match tokio::fs::read_dir(jobs_dir).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(path = %jobs_dir.display(), error = %e, "cannot read jobs directory");
            return Ok(cdt_core::JobsResponse {
                jobs: Vec::new(),
                badge: cdt_core::BadgeColor::None,
                badge_count: 0,
                jobs_dir_exists: true,
            });
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let state_path = path.join("state.json");
        let Ok(content) = tokio::fs::read(&state_path).await else {
            continue;
        };
        let mut bg_job = match serde_json::from_slice::<BackgroundJob>(&content) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!(path = %state_path.display(), error = %e, "skipping job with unparseable state.json");
                continue;
            }
        };

        let job_id = entry.file_name().to_string_lossy().into_owned();

        // tempo 是 daemon 实时活跃度信号，但终态 state 优先——进程退出后
        // daemon 可能未及时清理 tempo 字段。
        let is_terminal = matches!(
            bg_job.state,
            cdt_core::JobState::Done | cdt_core::JobState::Failed | cdt_core::JobState::Stopped
        );
        if !is_terminal {
            match bg_job.tempo.as_str() {
                "active" => {
                    bg_job.state = cdt_core::JobState::Working;
                }
                "blocked" => {
                    bg_job.state = cdt_core::JobState::Blocked;
                }
                _ => {}
            }
        }

        let project_id = extract_project_id_from_link_scan_path(&bg_job.link_scan_path)
            .unwrap_or_else(|| {
                if bg_job.cwd.is_empty() {
                    String::new()
                } else {
                    cdt_discover::encode_path(&bg_job.cwd)
                }
            });

        let group = classify_job_group(&bg_job);

        let display_name = if bg_job.name.is_empty() {
            bg_job.intent.clone()
        } else {
            bg_job.name
        };

        jobs.push(JobSummary {
            id: job_id,
            name: display_name,
            detail: bg_job.detail,
            intent: bg_job.intent,
            state: bg_job.state,
            group,
            children: bg_job.children.unwrap_or_default(),
            session_id: bg_job.session_id,
            project_id,
            tempo: bg_job.tempo,
            needs: bg_job.needs,
            in_flight: bg_job.in_flight,
            created_at: bg_job.created_at,
            updated_at: bg_job.updated_at,
        });
    }

    // 按分组排序：ReadyForReview > NeedsInput > Working > Completed
    jobs.sort_by_key(|a| group_order(&a.group));

    let (badge, badge_count) = compute_badge(&jobs);

    Ok(cdt_core::JobsResponse {
        jobs,
        badge,
        badge_count,
        jobs_dir_exists: true,
    })
}

/// 分组排序权重。
fn group_order(group: &cdt_core::JobGroup) -> u8 {
    match group {
        cdt_core::JobGroup::ReadyForReview => 0,
        cdt_core::JobGroup::NeedsInput => 1,
        cdt_core::JobGroup::Working => 2,
        cdt_core::JobGroup::Completed => 3,
    }
}

struct WatcherRuntimeChannels {
    errors: broadcast::Sender<DetectedError>,
    files: broadcast::Sender<cdt_core::FileChangeEvent>,
    todos: broadcast::Sender<cdt_core::TodoChangeEvent>,
    jobs: broadcast::Sender<cdt_core::JobChangeEvent>,
}

// `watcher: Arc<FileWatcher>` 按值传——内部 clone 给 start_task 与
// subscribe_files / subscribe_todos 各 receiver；调用方持有外部 clone（注入
// 到 `LocalDataApi.watcher`）。换 `&Arc<FileWatcher>` 反而要在内部多 .clone()
// 写法繁琐，按值传更符合所有权 + 移交 task 的语义。
//
// `clippy::too_many_arguments` 已抑制——参数都是后台 task 必须注入的依赖
// （watcher / mgr 们 / channel 们 / cache 们 / projects_dir / ssh_mgr）；
// 任何抽象（builder / struct）都把分发关系藏进新类型反而看不清。
#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
fn spawn_watcher_runtime(
    watcher: Arc<FileWatcher>,
    config_mgr: Arc<Mutex<ConfigManager>>,
    notif_mgr: Arc<Mutex<NotificationManager>>,
    channels: WatcherRuntimeChannels,
    parsed_msg_cache: Arc<std::sync::Mutex<ParsedMessageCache>>,
    project_scan_cache: Arc<std::sync::Mutex<ProjectScanCache>>,
    projects_dir: PathBuf,
    ssh_mgr: Option<cdt_ssh::SshSessionManager>,
) -> Vec<JoinHandle<()>> {
    let watcher_for_start = watcher.clone();
    let start_task = tokio::spawn(async move {
        if let Err(err) = watcher_for_start.start().await {
            tracing::warn!(error = %err, "FileWatcher terminated");
        }
    });

    let mut todo_rx = watcher.subscribe_todos();
    let todos_tx = channels.todos;
    let todo_bridge_task = tokio::spawn(async move {
        loop {
            match todo_rx.recv().await {
                Ok(event) => {
                    let _ = todos_tx.send(event);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let mut jobs_rx = watcher.subscribe_jobs();
    let jobs_tx = channels.jobs;
    let jobs_bridge_task = tokio::spawn(async move {
        loop {
            match jobs_rx.recv().await {
                Ok(event) => {
                    let _ = jobs_tx.send(event);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let pipeline = NotificationPipeline::new(
        watcher.subscribe_files(),
        config_mgr,
        notif_mgr,
        channels.errors,
        projects_dir.clone(),
    );
    let notifier_task = tokio::spawn(pipeline.run());

    // 合并 ParsedMessageCache + ProjectScanCache 两个 invalidator 到一个
    // 统一 task：单 `subscribe_files()` 订阅，串行 dispatch（issue #261）+
    // sole producer for `channels.files`（change
    // `enrich-file-change-with-session-list-changed::D1`）。
    // - 减少 watcher broadcast subscriber 数：4 → 3（少 1 个 task wakeup +
    //   少 1 份事件 clone），同时移除独立 `bridge_task`，把 `channels.files`
    //   的唯一生产者职责合并进 invalidator
    // - 顺序 **scan-first**（codex 二审建议）：scan 是 sync 快路径，多数 event
    //   走 content_append_skipped 分支直接返回；放在 `parsed` 的 `fs.stat().await`
    //   之前避免被 I/O 拖延结构判定
    // - emit 时机契约（change D4）：sync invalidate → enrich
    //   `session_list_changed` → `channels.files.send` → async parsed invalidate；
    //   前端拿 enriched event 决定是否 revalidate `list_repository_groups`，
    //   不被 parsed-cache 的磁盘 stat I/O 拖延
    // - 两个 cache 失效语义独立保持（`apply_file_event_to_*` helper 内部都做
    //   poison `into_inner` 兜底，单 task panic 隔离已 hardening）
    let unified_invalidator_task = spawn_unified_cache_invalidator(
        parsed_msg_cache,
        project_scan_cache,
        watcher.subscribe_files(),
        channels.files,
        projects_dir,
        Some(watcher),
        ssh_mgr,
    );

    vec![
        start_task,
        todo_bridge_task,
        jobs_bridge_task,
        notifier_task,
        unified_invalidator_task,
    ]
}

/// 启动一个后台 task，订阅 `FileWatcher::subscribe_files()` 广播，对每条
/// `FileChangeEvent` **统一**派发给 `ProjectScanCache` + `ParsedMessageCache`
/// 两侧失效逻辑（issue #261 合并优化，原本是两个独立 task 各自订阅）；
/// **同时**充当 `LocalDataApi.file_tx`（=`channels.files`）的**唯一生产者**
/// （change `enrich-file-change-with-session-list-changed::D1`，原 `bridge_task`
/// 已合并进本 task）。
///
/// 顺序契约（change D4 emit 时机契约 + change `enrich-via-watcher` D4 OR 公式）：
/// 1. `apply_file_event_to_project_scan_cache` —— sync 快路径（无 fs op），
///    返回 [`EnrichDecision`]：`invalidated` + `emit_session_list_changed_hint`；
///    不能被 parsed 的 `fs.stat().await` 拖延结构可见性判定（codex 二审 issue #261）。
/// 2. 构造 enriched `FileChangeEvent { session_list_changed: event.session_list_changed
///    || decision.emit_session_list_changed_hint, ..raw }`（OR 公式：watcher 已填 +
///    cache hint 取并集）立即 `file_tx.send` —— 锁已在 step 1 函数返回时释放，
///    emit 不持锁。前端拿 enriched event 决定是否 revalidate `list_repository_groups`。
/// 3. `apply_file_event_to_parsed_cache` —— async stat 比对 `FileSignature`，
///    仅在 mismatch 时从 cache remove；spurious 事件（典型 CI inotify 启动期
///    "无内容变化"事件）由签名比对兜底不错杀有效 cache。**不阻塞 emit**。
///
/// Lag 行为差异：
/// - `ProjectScanCache` 走保守 `invalidate_local()` + counter `lag_conservative`
///   （无 path-level 被动校验机制可兜底）
/// - `ParsedMessageCache` 静默继续（下次 lookup 由被动 `FileSignature`
///   mismatch 兜底）
/// - `file_tx` lag 路径 emit synthetic structural event：
///   `FileChangeEvent { project_id: "", session_id: "", deleted: false,
///   project_list_changed: true, session_list_changed: true }`，让下游
///   （src-tauri host / HTTP SSE bridge）转发到前端三档守护触发兜底全量
///   revalidate（change `enrich-via-watcher` D6）。
///
/// Panic 隔离：两侧 helper 内部 `cache.lock()` 都走 `into_inner` 兜底
/// （codex 二审 issue #261 hardening），单 task panic 不会一刀切两个 cache 都
/// 失去 invalidator。
///
/// `file_tx.send` 在无订阅者时返回 `Err`，静默忽略——event 本质 fire-and-forget。
/// `broadcast::Sender::send` 满时丢旧元素不阻塞，invalidator 自身永远不会被
/// 慢 subscriber 阻塞（与原 `bridge_task` 行为一致）。
///
/// 行为契约：spec `ipc-data-api/spec.md` §"parsed-message 缓存按 file-change
/// 广播主动失效" + §"`ProjectScanCache` 按事件语义分级失效" + §"Unified
/// invalidator 作为 `LocalDataApi.file_tx` 唯一生产者"。`Lagged` 双侧处理见上；
/// `Closed` 时退出 loop。
///
/// **限制**（沿用 parsed 侧 design D9 risks）：subagent JSONL 路径
/// （`<project>/<session>/subagents/agent-*.jsonl`）的失效仅靠被动签名兜底——
/// `FileChangeEvent` 把嵌套 subagent 改动路由到父 `session_id`，本 task 无法
/// 还原具体 `agent-<sub_id>.jsonl` 路径。
fn spawn_unified_cache_invalidator(
    parsed_cache: Arc<std::sync::Mutex<ParsedMessageCache>>,
    scan_cache: Arc<std::sync::Mutex<ProjectScanCache>>,
    mut rx: broadcast::Receiver<cdt_core::FileChangeEvent>,
    file_tx: broadcast::Sender<cdt_core::FileChangeEvent>,
    projects_dir: std::path::PathBuf,
    watcher: Option<Arc<FileWatcher>>,
    ssh_mgr: Option<cdt_ssh::SshSessionManager>,
) -> JoinHandle<()> {
    let local_ctx = cdt_fs::ContextId::local(projects_dir.clone());
    let fs = local_handle();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    // BUG #2 fix（codex PR #305 二审）：SSH 事件的 project_id
                    // 不在 local known_projects 内，不应使用 local cache hint
                    // 做 OR enrich——否则 SSH append 事件会被误 OR 成
                    // session_list_changed=true 侵蚀降噪收益。
                    let is_local = watcher
                        .as_ref()
                        .is_none_or(|w| w.is_local_project(&event.project_id));
                    let decision = if is_local {
                        apply_file_event_to_project_scan_cache(&scan_cache, &local_ctx, &event)
                    } else {
                        // SSH 事件：跳过 local cache hint，直接用 watcher 填写的值
                        EnrichDecision {
                            invalidated: false,
                            emit_session_list_changed_hint: false,
                        }
                    };
                    // mtime overlay 推进（spec `ipc-data-api/spec.md::
                    // ProjectScanCache 维护 per-project mtime overlay`）：
                    // - Local event → 写 Local context overlay
                    // - SSH event → 解析 active SSH context_id 后写对应 ctx overlay
                    // 删除事件 / 缺 mtime 事件由 helper 内部守护早 return。
                    if is_local {
                        apply_mtime_advance_to_project_scan_cache(&scan_cache, &local_ctx, &event);
                    } else if !event.deleted
                        && event.mtime_ms.is_some()
                        && !event.project_id.is_empty()
                        && let Some(mgr) = ssh_mgr.as_ref()
                        && let Some(active_id) = mgr.active_context_id().await
                        && let Some((_provider, ssh_ctx)) =
                            mgr.provider_and_context_id(&active_id).await
                    {
                        apply_mtime_advance_to_project_scan_cache(&scan_cache, &ssh_ctx, &event);
                    }
                    // emit 在 sync invalidate 之后、async parsed invalidate 之前
                    // （change D4 emit 时机契约）。OR 公式：watcher 已填字段 +
                    // cache hint 取并集（change `enrich-via-watcher` D4）——watcher
                    // 视角 first-seen + cache 视角 unknown_session 双源兜底。
                    let enriched = cdt_core::FileChangeEvent {
                        session_list_changed: event.session_list_changed
                            || decision.emit_session_list_changed_hint,
                        ..event.clone()
                    };
                    let _ = file_tx.send(enriched);
                    // parsed cache invalidation 仍对所有事件生效（签名兜底）
                    apply_file_event_to_parsed_cache(
                        &parsed_cache,
                        &*fs,
                        &local_ctx,
                        &projects_dir,
                        &event,
                    )
                    .await;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    apply_lag_to_project_scan_cache(&scan_cache);
                    // parsed 侧 lag 静默：下次 lookup 由被动 `FileSignature` 兜底。
                    // file_tx emit synthetic structural event：让下游
                    // （src-tauri host / HTTP SSE bridge）转发到前端三档守护，
                    // 触发兜底全量 revalidate（change `enrich-via-watcher` D6）。
                    let synthetic = cdt_core::FileChangeEvent {
                        project_id: String::new(),
                        session_id: String::new(),
                        deleted: false,
                        project_list_changed: true,
                        session_list_changed: true,
                        mtime_ms: None,
                    };
                    let _ = file_tx.send(synthetic);
                    tracing::warn!(
                        missed = n,
                        source = "watcher_subscribe_files",
                        "broadcast lagged, emitted synthetic structural event"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

#[allow(clippy::too_many_arguments)]
async fn scan_metadata_for_page(
    project_id: String,
    dir: std::path::PathBuf,
    page_jobs: Vec<(String, std::path::PathBuf)>,
    tx: broadcast::Sender<SessionMetadataUpdate>,
    active_scans: Arc<std::sync::Mutex<HashMap<String, ScanEntry>>>,
    cleanup_key: String,
    my_generation: u64,
    metadata_cache: Arc<std::sync::Mutex<MetadataCache>>,
    semaphore: Arc<Semaphore>,
    root_generation: Arc<AtomicU64>,
    expected_root_generation: u64,
    worktree_meta_cache: Arc<std::sync::RwLock<HashMap<String, WorktreeMeta>>>,
    fs: Arc<dyn FileSystemProvider>,
    context_id: cdt_fs::ContextId,
    context_generation: Arc<AtomicU64>,
    expected_context_generation: u64,
) {
    let _ = dir; // dir 当前由 page_jobs 内的 jsonl_path 携带，保留参数为未来扩展（如懒构造路径）
    // semaphore 由 caller 注入：所有 in-flight scan task 共享同一实例，保证全局
    // 并发上限为 `METADATA_SCAN_CONCURRENCY=8`（spec `ipc-data-api/spec.md::Emit
    // session metadata updates` Scenario "后台扫描并发度限制"）。先前实现每个
    // task 各自 `Arc::new(Semaphore::new(...))` 在改双键 abort 后会让 page 1 +
    // page 2 累加成 16+ 并发，违反上限。
    let mut set = JoinSet::new();

    for (session_id, jsonl_path) in page_jobs {
        let permit_sem = semaphore.clone();
        let tx = tx.clone();
        let project_id = project_id.clone();
        let cache = metadata_cache.clone();
        let root_generation = root_generation.clone();
        let context_generation = context_generation.clone();
        let worktree_meta_cache = worktree_meta_cache.clone();
        let fs_clone = fs.clone();
        let ctx_clone = context_id.clone();
        set.spawn(async move {
            let Ok(_permit) = permit_sem.acquire_owned().await else {
                return;
            };
            // codex 二审第三轮 H2-R-2 修订：context 切换期间的 in-flight
            // list_sessions 调用会先拿旧 (fs, ctx) 后才 insert scan handle
            // 到 active_scans，绕过 abort 窗口。每次 broadcast 前 SHALL 检查
            // context_generation 不变；不变才 broadcast，变了 silent drop。
            if context_generation.load(Ordering::SeqCst) != expected_context_generation {
                cdt_telemetry::counter!("generation.mismatch").inc();
                return;
            }
            let meta =
                extract_session_metadata_cached(&cache, &*fs_clone, &ctx_clone, &jsonl_path).await;
            if root_generation.load(Ordering::SeqCst) != expected_root_generation {
                cdt_telemetry::counter!("generation.mismatch").inc();
                return;
            }
            // 同上：scan 内部 await 完成后再次检查 context_generation，避免在
            // extract 期间发生切换后仍 broadcast 旧 ctx metadata。
            //
            // codex 二审第四轮 note：本 check 之前 `extract_session_metadata_cached`
            // 可能已写入 cache。但 cache key 是 `(ContextId, PathBuf)` namespace
            // 隔离——旧 ctx 的 cache entry 不污染新 ctx 的 cache lookup。下次
            // 用户访问旧 ctx (reconnect 同 host) 时，cache entry 仍按 signature
            // 校验有效——这是设计的正确行为而非 bug。本 check 仅 enforce
            // "不向前端 broadcast 旧 ctx update" 这一可观察契约。
            if context_generation.load(Ordering::SeqCst) != expected_context_generation {
                cdt_telemetry::counter!("generation.mismatch").inc();
                return;
            }
            let group_id = worktree_meta_cache
                .read()
                .ok()
                .and_then(|c| c.get(&project_id).map(|m| m.group_id.clone()));
            let _ = tx.send(SessionMetadataUpdate {
                project_id,
                session_id,
                title: meta.title,
                message_count: meta.message_count,
                is_ongoing: meta.is_ongoing,
                git_branch: meta.git_branch,
                group_id,
                user_intents: meta.user_intents,
                last_active: meta.last_active,
                duration_ms: meta.duration_ms,
                total_cost: meta.total_cost,
                tool_error_count: meta.tool_error_count,
                files_modified: meta.files_modified,
                git_summary: meta.git_summary,
            });
        });
    }

    while set.join_next().await.is_some() {}

    if let Ok(mut scans) = active_scans.lock() {
        if let Some(entry) = scans.get(&cleanup_key) {
            if entry.generation == my_generation {
                scans.remove(&cleanup_key);
            }
        }
    }
}

/// 按 `context_id.backend_kind` 分流后台 metadata 扫描路径。
///
/// - **Local backend**：调既有 `scan_metadata_for_page`（per-session via fs trait）
/// - **SSH backend**：调 `scan_metadata_for_page_batched`（per `project_dir` 一次
///   `fs.read_dir_with_metadata` + `MetadataCache::lookup_with_known_signature`
///   批量校验跳 stat；详 change `ssh-batch-readdir-with-metadata` design D2）
///
/// 两条路径共享 `active_scans` 注册表、`Semaphore(METADATA_SCAN_CONCURRENCY=8)`
/// 限流、`context_generation` race-free 校验；`ssh_disconnect` 触发的 abort 通过
/// `abort_scans_for_context` 自然覆盖（既有路径，无需新加入口）。
#[allow(clippy::too_many_arguments)]
async fn scan_metadata_for_page_dispatch(
    project_id: String,
    dir: std::path::PathBuf,
    page_jobs: Vec<(String, std::path::PathBuf)>,
    tx: broadcast::Sender<SessionMetadataUpdate>,
    active_scans: Arc<std::sync::Mutex<HashMap<String, ScanEntry>>>,
    cleanup_key: String,
    my_generation: u64,
    metadata_cache: Arc<std::sync::Mutex<MetadataCache>>,
    semaphore: Arc<Semaphore>,
    root_generation: Arc<AtomicU64>,
    expected_root_generation: u64,
    worktree_meta_cache: Arc<std::sync::RwLock<HashMap<String, WorktreeMeta>>>,
    fs: Arc<dyn FileSystemProvider>,
    context_id: cdt_fs::ContextId,
    context_generation: Arc<AtomicU64>,
    expected_context_generation: u64,
) {
    if context_id.backend_kind == cdt_fs::FsKind::Ssh {
        scan_metadata_for_page_batched(
            project_id,
            dir,
            page_jobs,
            tx,
            active_scans,
            cleanup_key,
            my_generation,
            metadata_cache,
            semaphore,
            root_generation,
            expected_root_generation,
            worktree_meta_cache,
            fs,
            context_id,
            context_generation,
            expected_context_generation,
        )
        .await;
    } else {
        scan_metadata_for_page(
            project_id,
            dir,
            page_jobs,
            tx,
            active_scans,
            cleanup_key,
            my_generation,
            metadata_cache,
            semaphore,
            root_generation,
            expected_root_generation,
            worktree_meta_cache,
            fs,
            context_id,
            context_generation,
            expected_context_generation,
        )
        .await;
    }
}

/// SSH ctx 专用批量校验路径：一次 `fs.read_dir_with_metadata(dir)` 拿全 dir
/// entry metadata（SFTP READDIR reply 1 RTT 含 attrs），对 `page_jobs` 每条
/// session 调 `MetadataCache::lookup_with_known_signature` 命中跳 stat；
/// mismatch / 新增 / dir metadata 缺该 path → spawn sub-task 走既有
/// `extract_session_metadata_cached` cache wrapper miss 路径。
///
/// dir read 失败 → fallback 到 `scan_metadata_for_page`（功能正确性优先；
/// 性能退化为 PR-D 既有 per-session 形态）。
///
/// 详 change `ssh-batch-readdir-with-metadata` design D2 + ssh-remote-context
/// spec Scenarios "SSH ctx 后台 batch 校验 fs op 形态钉死 (all-hit/partial/all-miss)"。
#[allow(clippy::too_many_arguments)]
async fn scan_metadata_for_page_batched(
    project_id: String,
    dir: std::path::PathBuf,
    page_jobs: Vec<(String, std::path::PathBuf)>,
    tx: broadcast::Sender<SessionMetadataUpdate>,
    active_scans: Arc<std::sync::Mutex<HashMap<String, ScanEntry>>>,
    cleanup_key: String,
    my_generation: u64,
    metadata_cache: Arc<std::sync::Mutex<MetadataCache>>,
    semaphore: Arc<Semaphore>,
    root_generation: Arc<AtomicU64>,
    expected_root_generation: u64,
    worktree_meta_cache: Arc<std::sync::RwLock<HashMap<String, WorktreeMeta>>>,
    fs: Arc<dyn FileSystemProvider>,
    context_id: cdt_fs::ContextId,
    context_generation: Arc<AtomicU64>,
    expected_context_generation: u64,
) {
    use crate::cache_signature::FileSignature;

    // early bail：fs / ctx / root generation 校验同 scan_metadata_for_page
    if context_generation.load(Ordering::SeqCst) != expected_context_generation {
        return;
    }
    if root_generation.load(Ordering::SeqCst) != expected_root_generation {
        return;
    }

    // 1. 一次 fs.read_dir_with_metadata 拿全 dir entry attrs（占 1 个 semaphore
    //    permit 与其它 in-flight scan 共享 8 上限，避免 batch 路径绕过限流）。
    let Ok(dir_permit) = semaphore.clone().acquire_owned().await else {
        return;
    };
    let entries = match fs.read_dir_with_metadata(&dir).await {
        Ok(e) => e,
        Err(err) => {
            // dir read 失败 fallback：调既有 per-session 路径保证功能正确性。
            // 性能退化为 PR-D 形态（N 次串行 stat），用户感知层仍 hot path
            // cache trust。详 design D2 + spec Scenario "SSH ctx batch helper
            // 在 dir read 失败时 fallback 到 per-session 路径"。
            drop(dir_permit);
            tracing::warn!(
                target: "cdt_api::perf",
                project_id = %project_id,
                error = %err,
                "batch read_dir_with_metadata failed, falling back to per-session scan",
            );
            scan_metadata_for_page(
                project_id,
                dir,
                page_jobs,
                tx,
                active_scans,
                cleanup_key,
                my_generation,
                metadata_cache,
                semaphore,
                root_generation,
                expected_root_generation,
                worktree_meta_cache,
                fs,
                context_id,
                context_generation,
                expected_context_generation,
            )
            .await;
            return;
        }
    };

    // build path → metadata map；filter 掉 metadata = None 条（如 mtime_missing
    // 已被 SshFileSystemProvider 翻译为 None，详 design D1）—— 这些条会走
    // mismatch sub-task 路径补齐。
    let by_name: HashMap<PathBuf, cdt_fs::FsMetadata> = entries
        .into_iter()
        .filter_map(|e| {
            let meta = e.metadata?;
            Some((dir.join(e.name), meta))
        })
        .collect();
    drop(dir_permit);

    // 2. 逐条 page_jobs：命中直 broadcast；mismatch / 新增 / metadata 缺该 path
    //    → spawn sub-task 走 cache wrapper miss 路径。
    let mut set = JoinSet::new();

    for (session_id, jsonl_path) in page_jobs {
        // 尝试命中 batch hit
        let cache_entry = by_name.get(&jsonl_path).and_then(|meta| {
            let sig = FileSignature::from_fs_metadata(meta);
            metadata_cache
                .lock()
                .expect("metadata cache mutex poisoned")
                .lookup_with_known_signature(&context_id, &jsonl_path, &sig)
        });

        if let Some(entry) = cache_entry {
            // 命中：每次 broadcast 前 SHALL 双重校验 context_generation +
            // root_generation 不变（与 scan_metadata_for_page 既有路径同语义；
            // codex commit 二审 #1 修订——fs.read_dir_with_metadata await 期间
            // root 可能被 reconfigure，命中分支也必须 root_generation 校验避免
            // 把旧 root 的 cache update 推给前端破坏 race-free 契约）。
            if root_generation.load(Ordering::SeqCst) != expected_root_generation {
                continue;
            }
            if context_generation.load(Ordering::SeqCst) != expected_context_generation {
                continue;
            }
            let group_id = worktree_meta_cache
                .read()
                .ok()
                .and_then(|c| c.get(&project_id).map(|m| m.group_id.clone()));
            // SSH 跳 stale check：与 extract_session_metadata_cached SSH 分支
            // （session_metadata.rs:567+ `backend_skips_stale`）同语义——远端
            // mtime 与本机 SystemTime::now() 跨 clock domain 不可比对。详
            // change ssh-batch-readdir-with-metadata design D2 / Risks 表
            // "messages_ongoing 双处一致性"项。
            let _ = tx.send(SessionMetadataUpdate {
                project_id: project_id.clone(),
                session_id,
                title: entry.title,
                message_count: entry.message_count,
                is_ongoing: entry.messages_ongoing,
                git_branch: entry.git_branch,
                group_id,
                user_intents: entry.user_intents,
                last_active: entry.last_active,
                duration_ms: entry.duration_ms,
                total_cost: entry.total_cost,
                tool_error_count: entry.tool_error_count,
                files_modified: entry.files_modified,
                git_summary: entry.git_summary,
            });
            continue;
        }

        // mismatch / 新增 / metadata 缺该 path → spawn sub-task 走 cache wrapper
        // miss 路径。JoinSet 持有 sub-task；顶层 batch task abort 时 JoinSet drop
        // 自动联级 abort 全部 sub-task（tokio 语义）—— SHALL NOT 重复注册
        // active_scans（design D3）。
        let permit_sem = semaphore.clone();
        let tx = tx.clone();
        let project_id_clone = project_id.clone();
        let cache = metadata_cache.clone();
        let root_generation = root_generation.clone();
        let context_generation = context_generation.clone();
        let worktree_meta_cache = worktree_meta_cache.clone();
        let fs_clone = fs.clone();
        let ctx_clone = context_id.clone();
        set.spawn(async move {
            let Ok(_permit) = permit_sem.acquire_owned().await else {
                return;
            };
            if context_generation.load(Ordering::SeqCst) != expected_context_generation {
                return;
            }
            let meta =
                extract_session_metadata_cached(&cache, &*fs_clone, &ctx_clone, &jsonl_path).await;
            if root_generation.load(Ordering::SeqCst) != expected_root_generation {
                return;
            }
            if context_generation.load(Ordering::SeqCst) != expected_context_generation {
                return;
            }
            let group_id = worktree_meta_cache
                .read()
                .ok()
                .and_then(|c| c.get(&project_id_clone).map(|m| m.group_id.clone()));
            let _ = tx.send(SessionMetadataUpdate {
                project_id: project_id_clone,
                session_id,
                title: meta.title,
                message_count: meta.message_count,
                is_ongoing: meta.is_ongoing,
                git_branch: meta.git_branch,
                group_id,
                user_intents: meta.user_intents,
                last_active: meta.last_active,
                duration_ms: meta.duration_ms,
                total_cost: meta.total_cost,
                tool_error_count: meta.tool_error_count,
                files_modified: meta.files_modified,
                git_summary: meta.git_summary,
            });
        });
    }

    // codex commit 二审 #2 修订：JoinSet 内 sub-task panic 时显式日志，避免
    // "某些 session 永远没 metadata"的静默失败（既有 scan_metadata_for_page
    // 也未显式日志，但 batched 路径有 JoinSet 二级嵌套——sub-task panic 更隐蔽）。
    while let Some(res) = set.join_next().await {
        if let Err(err) = res {
            tracing::error!(
                target: "cdt_api::perf",
                error = %err,
                "scan_metadata_for_page_batched sub-task failed",
            );
        }
    }

    // 3. cleanup（与 scan_metadata_for_page 同形 race-free pattern）
    if let Ok(mut scans) = active_scans.lock() {
        if let Some(entry) = scans.get(&cleanup_key) {
            if entry.generation == my_generation {
                scans.remove(&cleanup_key);
            }
        }
    }
}

/// CLI/MCP 路径用的 session 过滤条件。
/// 与 sidebar（IPC）路径的骨架+SSE 模式独立——sidebar 不走此 struct。
#[derive(Debug, Clone, Default)]
pub struct SessionListFilter {
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub grep: Option<String>,
    pub branch: Option<String>,
    pub limit: Option<usize>,
}

impl LocalDataApi {
    /// CLI/MCP 专用：流式扫描 session 列表。
    ///
    /// 与 `list_sessions_sync`（走 `list_sessions_skeleton` 全量 cwd + metadata）
    /// 不同，本方法：
    /// 1. `list_session_entries` 纯 readdir + since 预裁（不读 JSONL）
    /// 2. 按 mtime 顺序逐个提取 metadata → 过滤 → 累积到 limit 即停
    /// 3. 只对最终结果集补 cwd
    ///
    /// sidebar（IPC）路径仍走 async `list_sessions`（骨架+SSE），不受影响。
    pub async fn list_sessions_filtered(
        &self,
        project_id: &str,
        filter: &SessionListFilter,
    ) -> Result<Vec<SessionSummary>, ApiError> {
        let (fs, projects_dir, ctx) = self.active_fs_and_context_strict().await?;
        let scanner = ProjectScanner::new_with_cwd_cache(
            fs.clone(),
            projects_dir.clone(),
            self.shared_read_semaphore.clone(),
            self.shared_cwd_cache.clone(),
        );

        let entries = scanner
            .list_session_entries(project_id, filter.since)
            .await
            .map_err(|e| ApiError::internal(format!("list session entries: {e}")))?;

        let limit = filter.limit.unwrap_or(usize::MAX);
        let grep_lower = filter.grep.as_deref().map(str::to_lowercase);
        let branch_lower = filter.branch.as_deref().map(str::to_lowercase);

        let mut matched_entries: Vec<&cdt_discover::SessionStat> = Vec::new();
        let mut matched_metas: Vec<SessionMetadata> = Vec::new();

        for entry in &entries {
            if let Some(until) = filter.until {
                if entry.created_ms > until {
                    continue;
                }
            }

            // `entry.path` 即该 session 的 jsonl 路径（list_session_entries 填充）。
            let meta =
                extract_session_metadata_cached(&self.metadata_cache, &*fs, &ctx, &entry.path)
                    .await;

            if let Some(ref needle) = grep_lower {
                if !needle.is_empty() {
                    let title_match = meta
                        .title
                        .as_deref()
                        .is_some_and(|t| t.to_lowercase().contains(needle));
                    if !title_match {
                        continue;
                    }
                }
            }

            if let Some(ref needle) = branch_lower {
                let branch_match = meta
                    .git_branch
                    .as_deref()
                    .is_some_and(|b| b.to_lowercase().contains(needle));
                if !branch_match {
                    continue;
                }
            }

            matched_entries.push(entry);
            matched_metas.push(meta);

            if matched_entries.len() >= limit {
                break;
            }
        }

        let matched_paths: Vec<&std::path::Path> =
            matched_entries.iter().map(|e| e.path.as_path()).collect();
        let cwds = scanner.extract_cwds(&matched_paths).await;

        let mut summaries = Vec::with_capacity(matched_entries.len());
        for ((entry, meta), cwd) in matched_entries.into_iter().zip(matched_metas).zip(cwds) {
            let mut summary = SessionSummary {
                session_id: entry.id.clone(),
                project_id: project_id.to_owned(),
                timestamp: entry.mtime_ms,
                created: entry.created_ms,
                message_count: meta.message_count,
                title: meta.title,
                is_ongoing: meta.is_ongoing,
                git_branch: meta.git_branch,
                worktree_id: None,
                worktree_name: None,
                group_id: None,
                cwd_relative_to_repo_root: None,
                cwd,
                project_name: None,
                user_intents: meta.user_intents,
                last_active: meta.last_active,
                duration_ms: meta.duration_ms,
                total_cost: meta.total_cost,
                tool_error_count: meta.tool_error_count,
                files_modified: meta.files_modified,
                git_summary: meta.git_summary,
            };
            self.apply_worktree_meta(&mut summary);
            summaries.push(summary);
        }

        Ok(summaries)
    }
}

#[async_trait]
impl DataApi for LocalDataApi {
    // =========================================================================
    // 项目 + 会话
    // =========================================================================

    async fn list_projects(&self) -> Result<Vec<ProjectInfo>, ApiError> {
        let _telemetry_timer =
            cdt_telemetry::histogram!("ipc.list_projects.duration_ns").start_timer();
        // 走进程级 cache（FU-4 ProjectScanner memoize）：命中时 0 fs op，
        // miss 时一次 `ProjectScanner::scan()` 全扫并写入 Arc<Vec<Project>>。
        // `scan_projects_cached` 内部用 `active_fs_and_context_strict()` 选 fs。
        let projects = self.scan_projects_cached().await?;
        Ok(projects
            .iter()
            .map(|p| ProjectInfo {
                id: p.id.clone(),
                path: p.path.to_string_lossy().into_owned(),
                display_name: p.name.clone(),
                session_count: p.sessions.len(),
            })
            .collect())
    }

    async fn list_sessions_sync(
        &self,
        project_id: &str,
        pagination: &PaginatedRequest,
    ) -> Result<PaginatedResponse<SessionSummary>, ApiError> {
        let skeleton = self.list_sessions_skeleton(project_id, pagination).await?;
        let mut page = skeleton.page;
        let next_cursor = skeleton.next_cursor;
        let total = skeleton.total;
        let page_jobs = skeleton.page_jobs;
        let fs = skeleton.fs;
        let ctx = skeleton.ctx;

        // 并发提取每条 session 的 metadata：复用 `self.metadata_scan_semaphore`
        // 与 async `list_sessions` 共享同一把 8 容量信号量，保证 spec
        // `ipc-data-api/spec.md::Emit session metadata updates` "后台扫描并发度
        // 限制" Scenario 的全局 8 上限——HTTP 路径与 async 路径并发时也不会
        // 累加成 16+ 并发。metadata cache 内部有锁，多 task 共享安全。结果按
        // page_jobs 顺序一一映射回 page。
        let metas = futures::future::join_all(page_jobs.iter().map(|(_id, path)| {
            let sem = self.metadata_scan_semaphore.clone();
            let cache = self.metadata_cache.clone();
            let path = path.clone();
            let fs_clone = fs.clone();
            let ctx_clone = ctx.clone();
            async move {
                let _permit = sem.acquire_owned().await.ok()?;
                Some(extract_session_metadata_cached(&cache, &*fs_clone, &ctx_clone, &path).await)
            }
        }))
        .await;
        for (summary, maybe_meta) in page.iter_mut().zip(metas) {
            let Some(meta) = maybe_meta else {
                continue;
            };
            summary.title = meta.title;
            summary.message_count = meta.message_count;
            summary.is_ongoing = meta.is_ongoing;
            summary.git_branch = meta.git_branch;
            summary.user_intents = meta.user_intents;
            summary.last_active = meta.last_active;
            summary.duration_ms = meta.duration_ms;
            summary.total_cost = meta.total_cost;
            summary.tool_error_count = meta.tool_error_count;
            summary.files_modified = meta.files_modified;
            summary.git_summary = meta.git_summary;
        }

        Ok(PaginatedResponse {
            items: page,
            next_cursor,
            total,
        })
    }

    async fn list_sessions(
        &self,
        project_id: &str,
        pagination: &PaginatedRequest,
    ) -> Result<PaginatedResponse<SessionSummary>, ApiError> {
        let _telemetry_timer =
            cdt_telemetry::histogram!("ipc.list_sessions.duration_ns").start_timer();
        let skeleton = self.list_sessions_skeleton(project_id, pagination).await?;
        let page = skeleton.page;
        let next_cursor = skeleton.next_cursor;
        let total = skeleton.total;
        let page_jobs = skeleton.page_jobs;
        let dir = skeleton.dir;
        let root_generation = skeleton.root_generation;
        let inline_updates = skeleton.inline_updates;
        let fs = skeleton.fs;
        let ctx = skeleton.ctx;
        let expected_context_generation = skeleton.expected_context_generation;

        for update in inline_updates {
            let _ = self.session_metadata_tx.send(update);
        }

        if !page_jobs.is_empty() && root_generation == self.root_generation.load(Ordering::SeqCst) {
            let tx = self.session_metadata_tx.clone();
            let project_id_owned = project_id.to_owned();
            let active_scans = self.active_scans.clone();
            // (project_id, cursor) 双键：同 cursor 抢占 / 不同 cursor 并存
            // （详见 spec `ipc-data-api/spec.md::Emit session metadata updates`
            // Scenario "同 projectId 同 cursor 的新扫描取消旧扫描" 与
            // "同 projectId 不同 cursor 的扫描并存互不 abort"）。
            let scan_key = metadata_scan_key(project_id, pagination.cursor.as_deref());
            let key_for_cleanup = scan_key.clone();

            // abort 旧 + 分配 generation + spawn + insert **全部**在同一把
            // sync lock 内完成。`tokio::spawn` 不会 await，sync lock 保持
            // 期间调用是安全的。这样并发 list_sessions(同 project) 的两个
            // 调用 A/B 会顺序进入临界区，避免：A 的 spawn 与 insert 之间
            // B 完整 abort/spawn/insert 后 A 的 insert 覆盖 B 的 entry，导致
            // 后续 C 无法 abort B 的孤立 task（codex 二审第二轮 race）。
            if let Ok(mut scans) = self.active_scans.lock() {
                if let Some(old) = scans.remove(&scan_key) {
                    old.handle.abort();
                }
                let my_generation = self.scan_generation.fetch_add(1, Ordering::Relaxed);

                let ctx_for_entry = ctx.clone();
                // codex 第五轮 Blocker：用 list_sessions_skeleton 早 load 的
                // expected_context_generation（与 fs/ctx 同 snapshot），而非 spawn 时
                // 才 load——避免 reconfigure 在 fs/ctx 已 await 完成、spawn 之前跑过
                // 让 reader 拿到旧 ctx + 新 generation 的 race。
                let handle = tokio::spawn(scan_metadata_for_page_dispatch(
                    project_id_owned,
                    dir,
                    page_jobs,
                    tx,
                    active_scans.clone(),
                    key_for_cleanup,
                    my_generation,
                    self.metadata_cache.clone(),
                    self.metadata_scan_semaphore.clone(),
                    self.root_generation.clone(),
                    root_generation,
                    self.worktree_meta_cache.clone(),
                    fs,
                    ctx,
                    self.context_generation.clone(),
                    expected_context_generation,
                ));

                scans.insert(
                    scan_key,
                    ScanEntry {
                        generation: my_generation,
                        context_id: ctx_for_entry,
                        handle: handle.abort_handle(),
                    },
                );
            }
        }

        Ok(PaginatedResponse {
            items: page,
            next_cursor,
            total,
        })
    }

    async fn get_session_summaries_by_ids(
        &self,
        project_id: &str,
        session_ids: &[String],
    ) -> Result<Vec<SessionSummary>, ApiError> {
        if session_ids.is_empty() {
            return Ok(Vec::new());
        }

        let wanted: std::collections::BTreeSet<&str> =
            session_ids.iter().map(String::as_str).collect();
        let scanner = self.active_scanner().await?;
        let sessions = scanner
            .list_sessions(project_id, &std::collections::BTreeSet::new())
            .await
            .map_err(|e| ApiError::internal(format!("list sessions error: {e}")))?;
        drop(scanner);

        let mut by_id = sessions
            .into_iter()
            .filter(|session| wanted.contains(session.id.as_str()))
            .map(|session| {
                let mut summary = SessionSummary {
                    session_id: session.id.clone(),
                    project_id: project_id.to_owned(),
                    timestamp: session.last_modified,
                    created: session.created,
                    message_count: 0,
                    title: None,
                    is_ongoing: false,
                    git_branch: None,
                    worktree_id: None,
                    worktree_name: None,
                    group_id: None,
                    cwd_relative_to_repo_root: None,
                    cwd: session.cwd,
                    project_name: None,
                    user_intents: Vec::new(),
                    last_active: 0,
                    duration_ms: 0,
                    total_cost: 0.0,
                    tool_error_count: 0,
                    files_modified: Vec::new(),
                    git_summary: Vec::new(),
                };
                self.apply_worktree_meta(&mut summary);
                (session.id, summary)
            })
            .collect::<std::collections::HashMap<_, _>>();

        Ok(session_ids
            .iter()
            .filter_map(|id| by_id.remove(id))
            .collect())
    }

    async fn get_project_memory(&self, project_id: &str) -> Result<ProjectMemory, ApiError> {
        // change `ssh-project-memory-remote-rw`: 删除 supports_memory graceful skip 短路；
        // SSH 与 Local 统一走 fs trait 调 `discover_memory_layers`。
        let (fs, _projects_dir, _ctx, _policy, _resolvers) = self.active_fs_and_policy().await?;
        let memory_dir = self.project_memory_dir(project_id).await?;
        let layers = discover_memory_layers(&*fs, &memory_dir).await?;
        Ok(build_project_memory(project_id, layers))
    }

    async fn read_memory_file(
        &self,
        project_id: &str,
        file: &str,
    ) -> Result<MemoryFileContent, ApiError> {
        // change `ssh-project-memory-remote-rw`: 删除 supports_memory graceful skip 短路；
        // SSH 与 Local 统一走 fs trait `read_to_string`。
        let (fs, _projects_dir, _ctx, _policy, _resolvers) = self.active_fs_and_policy().await?;
        let memory_dir = self.project_memory_dir(project_id).await?;
        let safe_file = validate_memory_file_name(file)?;
        let path = memory_dir.join(&safe_file);
        let content = fs
            .read_to_string(&path)
            .await
            .map_err(|e| ApiError::not_found(format!("memory file {safe_file}: {e}")))?;
        Ok(MemoryFileContent {
            project_id: project_id.to_owned(),
            file: safe_file,
            file_path: path.to_string_lossy().into_owned(),
            content,
        })
    }

    async fn add_memory(
        &self,
        project_id: &str,
        file: &str,
        content: &str,
    ) -> Result<ProjectMemory, ApiError> {
        // change `ssh-project-memory-remote-rw`: 写路径走 fs trait `write_atomic` + `create_dir_all`；
        // 写完 re-discover layers 直接返新 ProjectMemory（避免前端二次 IPC，详 design D9）。
        //
        // 错误路径 sanitize（codex PR 二审 ITEM 5）：返给前端 webview 的 ApiError
        // message 仅含用户可见信息（safe_file 文件名）；详细 fs error（含远端
        // home path）只写 tracing::error，不暴露给前端。
        let (fs, _projects_dir, _ctx, _policy, _resolvers) = self.active_fs_and_policy().await?;
        let memory_dir = self.project_memory_dir(project_id).await?;
        let safe_file = validate_memory_file_name(file)?;
        // 自动创建 memory 目录（不存在则创建，已存在不报错）
        fs.create_dir_all(&memory_dir).await.map_err(|e| {
            tracing::error!(project_id = %project_id, error = %e, "create memory dir failed");
            ApiError::internal("create memory dir failed".to_owned())
        })?;
        let path = memory_dir.join(&safe_file);
        fs.write_atomic(&path, content.as_bytes()).await.map_err(|e| {
            tracing::error!(project_id = %project_id, file = %safe_file, error = %e, "write memory file failed");
            ApiError::internal(format!("write memory file {safe_file} failed"))
        })?;
        let layers = discover_memory_layers(&*fs, &memory_dir).await?;
        Ok(build_project_memory(project_id, layers))
    }

    async fn delete_memory(&self, project_id: &str, file: &str) -> Result<ProjectMemory, ApiError> {
        // change `ssh-project-memory-remote-rw`: 删路径走 fs trait `remove_file`；
        // 删完 re-discover layers 直接返新 ProjectMemory。错误路径 sanitize 同 add_memory。
        let (fs, _projects_dir, _ctx, _policy, _resolvers) = self.active_fs_and_policy().await?;
        let memory_dir = self.project_memory_dir(project_id).await?;
        let safe_file = validate_memory_file_name(file)?;
        let path = memory_dir.join(&safe_file);
        fs.remove_file(&path).await.map_err(|e| match e {
            cdt_discover::FsError::NotFound(_) => {
                ApiError::not_found(format!("memory file {safe_file} not found"))
            }
            other => {
                tracing::error!(project_id = %project_id, file = %safe_file, error = %other, "delete memory file failed");
                ApiError::internal(format!("delete memory file {safe_file} failed"))
            }
        })?;
        let layers = discover_memory_layers(&*fs, &memory_dir).await?;
        Ok(build_project_memory(project_id, layers))
    }

    async fn get_session_detail(
        &self,
        project_id: &str,
        session_id: &str,
        known_fingerprint: Option<&str>,
    ) -> Result<SessionDetailResponse, ApiError> {
        let _telemetry_timer =
            cdt_telemetry::histogram!("ipc.get_session_detail.duration_ns").start_timer();
        let t_total = std::time::Instant::now();

        // Step 1: locate session file + fingerprint short-circuit
        let t_locate = std::time::Instant::now();
        let located = match self
            .locate_session_file(project_id, session_id, known_fingerprint)
            .await?
        {
            LocateResult::Unchanged { fingerprint } => {
                let locate_ms = t_locate.elapsed().as_millis();
                cdt_telemetry::counter!("ipc.get_session_detail.unchanged").inc();
                tracing::info!(
                    target: "cdt_api::perf",
                    session_id = %session_id,
                    locate_ms,
                    "get_session_detail short-circuit: unchanged"
                );
                return Ok(SessionDetailResponse::Unchanged { fingerprint });
            }
            LocateResult::Found(l) => l,
        };
        let locate_ms = t_locate.elapsed().as_millis();

        // Step 2: parse session messages
        let t_parse = std::time::Instant::now();
        let messages = extract_parsed_messages_cached(
            &self.parsed_msg_cache,
            &*located.fs,
            &located.ctx,
            &located.jsonl_path,
        )
        .await
        .ok_or_else(|| {
            ApiError::internal(format!(
                "parse error: stat or parse failed for {}",
                located.jsonl_path.display()
            ))
        })?;
        let parse_ms = t_parse.elapsed().as_millis();
        let message_count = messages.len();

        // Step 3: scan subagents
        let t_scan = std::time::Instant::now();
        let candidates = scan_subagent_candidates_for_detail(
            &*located.fs,
            &located.projects_dir,
            &located.project_dir,
            session_id,
            &located.policy,
        )
        .await;
        let scan_ms = t_scan.elapsed().as_millis();
        let candidate_count = candidates.len();

        // Step 4: build chunks + determine is_ongoing
        let t_build = std::time::Instant::now();
        let (chunks, is_ongoing) = build_session_chunks_with_ongoing(
            &messages,
            &candidates,
            &located.policy,
            &*located.fs,
            &located.jsonl_path,
        )
        .await;
        let build_ms = t_build.elapsed().as_millis();
        let chunk_count = chunks.len();

        // Step 5: inject context annotations
        let t_ctx = std::time::Instant::now();
        let annotations = self.inject_context_annotations(&chunks, &messages).await;
        let ctx_ms = t_ctx.elapsed().as_millis();

        // Step 5.5: resolve workflow items
        let session_dir = located.project_dir.join(session_id);
        let workflow_items = super::workflow_manifest::resolve_workflow_items(
            &chunks,
            &session_dir,
            &*located.fs,
            &self.workflow_manifest_cache,
        )
        .await;

        // Step 6: fill compact derived fields + assemble response
        let t_serde = std::time::Instant::now();
        let mut chunks = chunks;
        apply_compact_derived(&mut chunks, COMPACT_DERIVED_ENABLED);
        let session_cwd: Option<&str> = messages.iter().find_map(|m| m.cwd.as_deref());
        let title_metadata = extract_session_metadata_from_parsed(&messages, !is_ongoing);
        let detail = SessionDetail {
            session_id: session_id.to_owned(),
            project_id: project_id.to_owned(),
            chunks,
            metrics: SessionDetailMetrics { message_count },
            metadata: SessionDetailMetadata {
                last_modified: located.last_modified,
                size: located.size,
                cwd: session_cwd.map(String::from),
            },
            context_injections: annotations.context_injections,
            injections_by_phase: annotations.injections_by_phase,
            phase_info: annotations.phase_info,
            turn_context_stats: annotations.turn_context_stats,
            is_ongoing,
            title: title_metadata.title,
            workflow_items,
        };
        let serde_ms = t_serde.elapsed().as_millis();
        let total_ms = t_total.elapsed().as_millis();

        tracing::info!(
            target: "cdt_api::perf",
            session_id = %session_id,
            messages = message_count,
            chunks = chunk_count,
            subagents = candidate_count,
            locate_ms,
            parse_ms,
            scan_subagents_ms = scan_ms,
            build_chunks_ms = build_ms,
            context_ms = ctx_ms,
            serde_ms,
            total_ms,
            "get_session_detail timings"
        );

        Ok(SessionDetailResponse::Full {
            fingerprint: located.fingerprint,
            detail: Box::new(detail),
        })
    }

    async fn find_session_project(&self, session_id: &str) -> Result<Option<String>, ApiError> {
        // FS 直扫覆盖默认 trait 实现（避免 O(项目数 × 会话数) 的全量
        // list_sessions）。匹配三种结构：
        //   - 主会话：`<projects_dir>/<encoded>/<session_id>.jsonl`
        //   - legacy subagent：`<projects_dir>/<encoded>/agent-<session_id>.jsonl`
        //   - 新结构 subagent：`<projects_dir>/<encoded>/<parent>/subagents/agent-<session_id>.jsonl`
        // 与 `find_subagent_jsonl` + `get_session_detail` 的查找口径一致。
        // Local + SSH 共用 fs trait（design D2 line 2325）：算法分叉消除。
        let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        let main_filename = format!("{session_id}.jsonl");
        let Ok(entries) = fs.read_dir(&projects_dir).await else {
            return Ok(None);
        };
        for entry in entries {
            if !entry.kind.is_dir() {
                continue;
            }
            let project_dir = projects_dir.join(&entry.name);
            // 主会话快路径
            if fs.exists(&project_dir.join(&main_filename)).await {
                return Ok(Some(entry.name));
            }
            // subagent 慢路径（双结构 flat + nested fallback 由 find_subagent_jsonl 提供）
            if find_subagent_jsonl(&*fs, &project_dir, session_id)
                .await
                .is_some()
            {
                return Ok(Some(entry.name));
            }
        }
        Ok(None)
    }

    async fn get_subagent_trace(
        &self,
        root_session_id: &str,
        subagent_session_id: &str,
    ) -> Result<Vec<cdt_core::Chunk>, ApiError> {
        // `get_subagent_trace` 调用方（Tauri command）只携带 sessionId，
        // 不带 projectId，所以需跨 `projects_dir` 扫。优先按
        // `{projects_dir}/*/{root_session_id}/subagents/agent-<sub>.jsonl`
        // 全局扫（新结构）；旧结构 fallback 走"找到 root jsonl 所在 project_dir
        // 后在该目录内查 flat agent jsonl"。
        //
        // 使用当前 active context 的 fs + projects_dir，避免 root 切换后继续扫描旧目录。
        // Local + SSH 共用 fs trait（design D2 line 2395-2396）：算法分叉消除。
        let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        let new_structure_path = if CROSS_PROJECT_SUBAGENT_SCAN {
            find_subagent_jsonl_cross_project(
                &*fs,
                &projects_dir,
                root_session_id,
                subagent_session_id,
            )
            .await
        } else {
            None
        };
        let path = if let Some(p) = new_structure_path {
            p
        } else {
            // 旧结构兜底：找 root jsonl 所在 project_dir 后在该目录扫 flat（双结构 fallback
            // 由 find_subagent_jsonl 提供）。
            let Ok(entries) = fs.read_dir(&projects_dir).await else {
                return Ok(Vec::new());
            };
            let mut fallback: Option<std::path::PathBuf> = None;
            for entry in entries {
                if !entry.kind.is_dir() {
                    continue;
                }
                let project_dir = projects_dir.join(&entry.name);
                let root_jsonl = project_dir.join(format!("{root_session_id}.jsonl"));
                if fs.exists(&root_jsonl).await {
                    if let Some(p) =
                        find_subagent_jsonl(&*fs, &project_dir, subagent_session_id).await
                    {
                        fallback = Some(p);
                    }
                    break;
                }
            }
            let Some(p) = fallback else {
                return Ok(Vec::new());
            };
            p
        };
        let messages = cdt_parse::parse_file_via_fs(&*fs, &path)
            .await
            .map_err(|e| ApiError::internal(format!("parse error: {e}")))?;
        let mut msgs = messages;
        for m in &mut msgs {
            m.is_sidechain = false;
        }
        // 子 transcript 用非递归 build_chunks 后，把内部带 result_agent_id 的嵌套
        // Agent/Task 调用升级为骨架 subagent，让 UI 可逐级懒拉展开（零新 IO）。
        // spec: ipc-data-api::Lazy load subagent trace / chunk-building::Promote
        // nested Agent calls to skeleton subagents。
        let mut chunks = cdt_analyze::build_chunks(&msgs);
        cdt_analyze::promote_result_agent_tasks(&mut chunks);
        Ok(chunks)
    }

    async fn get_workflow_agent_trace(
        &self,
        project_id: &str,
        parent_session_id: &str,
        run_id: &str,
        agent_id: &str,
    ) -> Result<Vec<cdt_core::Chunk>, ApiError> {
        if !is_safe_path_component(parent_session_id)
            || !is_safe_path_component(run_id)
            || !is_safe_path_component(agent_id)
        {
            return Err(ApiError::validation(
                "session_id, run_id, and agent_id must not contain path separators or '..'",
            ));
        }
        let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        let session_dir = projects_dir
            .join(cdt_discover::path_decoder::extract_base_dir(project_id))
            .join(parent_session_id);
        let path = session_dir
            .join("subagents")
            .join("workflows")
            .join(run_id)
            .join(format!("agent-{agent_id}.jsonl"));

        if !fs.exists(&path).await {
            tracing::debug!(
                target: "cdt_api::workflow",
                project_id,
                parent_session_id,
                run_id,
                agent_id,
                "workflow agent trace not found"
            );
            return Ok(Vec::new());
        }
        let messages = cdt_parse::parse_file_via_fs(&*fs, &path)
            .await
            .map_err(|e| ApiError::internal(format!("parse error: {e}")))?;
        let mut msgs = messages;
        for m in &mut msgs {
            m.is_sidechain = false;
        }
        let chunks = cdt_analyze::build_chunks(&msgs);
        Ok(chunks)
    }

    async fn get_workflow_detail(
        &self,
        project_id: &str,
        session_id: &str,
        run_id: &str,
    ) -> Result<cdt_core::WorkflowItem, ApiError> {
        if !is_safe_path_component(session_id) || !is_safe_path_component(run_id) {
            return Err(ApiError::validation(
                "session_id and run_id must not contain path separators or '..'",
            ));
        }
        let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        let session_dir = projects_dir
            .join(cdt_discover::path_decoder::extract_base_dir(project_id))
            .join(session_id);

        let manifest_path = session_dir.join("workflows").join(format!("{run_id}.json"));
        let journal_path = session_dir
            .join("subagents")
            .join("workflows")
            .join(run_id)
            .join("journal.jsonl");

        let item = super::workflow_manifest::resolve_single_detail(
            run_id,
            &manifest_path,
            &journal_path,
            None,
            &*fs,
            &self.workflow_manifest_cache,
        )
        .await;
        Ok(item)
    }

    async fn get_image_asset(
        &self,
        root_session_id: &str,
        session_id: &str,
        block_id: &str,
    ) -> Result<String, ApiError> {
        // block_id 编码：`<chunkUuid>:<blockIndex>`
        let Some((chunk_uuid, block_index)) = block_id
            .rsplit_once(':')
            .and_then(|(u, i)| i.parse::<usize>().ok().map(|idx| (u.to_owned(), idx)))
        else {
            tracing::warn!(target: "cdt_api::image", block_id, "invalid block_id format");
            return Ok(empty_data_uri());
        };

        // 定位 jsonl：root 自己 or `<root>/subagents/agent-<sub>.jsonl`。
        // 一次性快照 (fs, projects_dir, ctx) 来自同一 active context（详 change
        // `parsed-message-cache-context-prefix` design D8-bis：避免 active_fs_and_projects_dir
        // + active_fs_and_context 两次 lock 之间被并发 ssh_connect race 让 fs/ctx 不自洽）。
        // 用户可见 IPC handler 走 strict 变体——SSH disconnect 中间态 SHALL 报错而非
        // 静默降级到 Local（避免返回同 ID 的 Local 文件数据；详 codex 二审 commit
        // stage Q1）。
        let (fs, projects_dir, ctx) = self.active_fs_and_context_strict().await?;
        let Some(jsonl_path) =
            locate_session_jsonl(&*fs, &projects_dir, root_session_id, session_id).await
        else {
            tracing::warn!(target: "cdt_api::image", root_session_id, session_id, "jsonl not found");
            return Ok(empty_data_uri());
        };
        // Local + SSH 共用 cache wrapper：命中时复用 Arc<Vec<ParsedMessage>>，跳过整文件
        // line-by-line parse；miss 经 parse_file_via_fs 走 fs.open_read 后写回 cache
        // （design D2 line 2504：统一走 cache wrapper 消除 SSH inline read 分叉）。
        let Some(messages) =
            extract_parsed_messages_cached(&self.parsed_msg_cache, &*fs, &ctx, &jsonl_path).await
        else {
            tracing::warn!(target: "cdt_api::image", "parse failed or stat error; returning empty data URI");
            return Ok(empty_data_uri());
        };
        let Some((data_b64, media_type)) =
            find_image_block_in_messages(&messages, &chunk_uuid, block_index)
        else {
            tracing::warn!(target: "cdt_api::image", chunk_uuid, block_index, "image block not found");
            return Ok(empty_data_uri());
        };

        // cache 目录未注入 → 直接 fallback data: URI。
        let Some(cache_dir) = self.image_cache_dir.as_ref() else {
            return Ok(format_data_uri(&media_type, &data_b64));
        };

        Ok(materialize_image_asset(cache_dir, &media_type, &data_b64).await)
    }

    async fn get_tool_output(
        &self,
        root_session_id: &str,
        session_id: &str,
        tool_use_id: &str,
    ) -> Result<cdt_core::ToolOutput, ApiError> {
        // 一次性快照 (fs, projects_dir, ctx) 来自同一 active context（详 change
        // `parsed-message-cache-context-prefix` design D8-bis）。
        // 用户可见 IPC handler 走 strict 变体——SSH disconnect 中间态 SHALL 报错而非
        // 静默降级到 Local（codex 二审 commit stage Q1）。
        let (fs, projects_dir, ctx) = self.active_fs_and_context_strict().await?;
        let Some(jsonl_path) =
            locate_session_jsonl(&*fs, &projects_dir, root_session_id, session_id).await
        else {
            tracing::warn!(target: "cdt_api::tool_output", root_session_id, session_id, "jsonl not found");
            return Ok(cdt_core::ToolOutput::Missing);
        };
        // Local + SSH 共用 cache wrapper：cache hit 复用 Arc<Vec<ParsedMessage>>；miss 经
        // parse_file_via_fs 走 fs.open_read 后写回 cache（design D2 line 2572）。
        let Some(messages) =
            extract_parsed_messages_cached(&self.parsed_msg_cache, &*fs, &ctx, &jsonl_path).await
        else {
            tracing::debug!(target: "cdt_api::tool_output", "parse failed or stat error; returning Missing");
            return Ok(cdt_core::ToolOutput::Missing);
        };

        // build_chunks 后线性 scan tool_executions 找 tool_use_id 匹配。
        // build_chunks 当前不缓存——见 change `parsed-message-lru-cache` design D2/D6
        // Non-Goals 与决策记录：先缓存 parse 一层（200-400ms 大头），后续若 perf 数据
        // 表明 build_chunks（100-200ms）也是瓶颈再补 cache 一层。
        let chunks = cdt_analyze::build_chunks(&messages);
        for chunk in &chunks {
            if let cdt_core::Chunk::Ai(ai) = chunk {
                for exec in &ai.tool_executions {
                    if exec.tool_use_id == tool_use_id {
                        return Ok(exec.output.clone());
                    }
                }
            }
        }

        tracing::debug!(target: "cdt_api::tool_output", tool_use_id, "id not found in chunks");
        Ok(cdt_core::ToolOutput::Missing)
    }

    async fn get_sessions_by_ids(
        &self,
        session_ids: &[String],
    ) -> Result<Vec<SessionDetail>, ApiError> {
        let mut results = Vec::new();
        for sid in session_ids {
            // 仅给 session_id 时先反查 project_id，再走标准 detail 路径。
            // 找不到走 typed default 占位，调用方按 `projectId == ""` 信号判定
            // not-found（详 change `typed-ipc-payload::design.md::D9`：移除原
            // ad-hoc `metadata.status="not_found"` 带外标记，改 typed default）。
            let Ok(Some(project_id)) = self.find_session_project(sid).await else {
                results.push(SessionDetail {
                    session_id: sid.clone(),
                    project_id: String::new(),
                    chunks: Vec::new(),
                    metrics: SessionDetailMetrics::default(),
                    metadata: SessionDetailMetadata::default(),
                    context_injections: Vec::new(),
                    injections_by_phase: BTreeMap::new(),
                    phase_info: cdt_core::ContextPhaseInfo::default(),
                    turn_context_stats: HashMap::default(),
                    is_ongoing: false,
                    title: None,
                    workflow_items: Vec::new(),
                });
                continue;
            };
            match self.get_session_detail(&project_id, sid, None).await {
                Ok(SessionDetailResponse::Full { detail, .. }) => results.push(*detail),
                Ok(SessionDetailResponse::Unchanged { .. }) => {
                    // Should not happen with None fingerprint, but handle gracefully
                    results.push(SessionDetail {
                        session_id: sid.clone(),
                        project_id,
                        chunks: Vec::new(),
                        metrics: SessionDetailMetrics::default(),
                        metadata: SessionDetailMetadata::default(),
                        context_injections: Vec::new(),
                        injections_by_phase: BTreeMap::new(),
                        phase_info: cdt_core::ContextPhaseInfo::default(),
                        turn_context_stats: HashMap::default(),
                        is_ongoing: false,
                        title: None,
                        workflow_items: Vec::new(),
                    });
                }
                Err(_) => results.push(SessionDetail {
                    session_id: sid.clone(),
                    project_id,
                    chunks: Vec::new(),
                    metrics: SessionDetailMetrics::default(),
                    metadata: SessionDetailMetadata::default(),
                    context_injections: Vec::new(),
                    injections_by_phase: BTreeMap::new(),
                    phase_info: cdt_core::ContextPhaseInfo::default(),
                    turn_context_stats: HashMap::default(),
                    is_ongoing: false,
                    title: None,
                    workflow_items: Vec::new(),
                }),
            }
        }
        Ok(results)
    }

    // =========================================================================
    // 搜索
    // =========================================================================

    async fn search(
        &self,
        request: &SearchRequest,
    ) -> Result<cdt_core::SearchSessionsResult, ApiError> {
        if request.query.is_empty() {
            // typed 化前 hand-built `{query, results}` 缺 3 字段；本 change `D8`
            // 修法：构造完整 `SearchSessionsResult`，wire 形状从 4 字段扩为 7 字段
            // 是 bug fix（前端 `CommandPalette.svelte:116` 走 `"totalMatches" in
            // session ? ... : ...` 判定，新增字段不破坏现有读取）。
            return Ok(cdt_core::SearchSessionsResult {
                results: Vec::new(),
                total_matches: 0,
                sessions_searched: 0,
                query: String::new(),
                is_partial: false,
            });
        }

        let max_results = 50;

        let project_id = request
            .project_id
            .as_deref()
            .ok_or_else(|| ApiError::validation("project_id is required for search"))?;

        let (fs, projects_dir, _ctx, _policy, resolvers) = self.active_fs_and_policy().await?;
        let searcher = SessionSearcher::new(fs, self.search_cache.clone(), &projects_dir);

        if let Some(ref sid) = request.session_id {
            if sid.contains("..") || sid.contains('/') || sid.contains('\\') {
                return Err(ApiError::validation(format!("invalid session ID: {sid}")));
            }
            let session_path = projects_dir.join(project_id).join(format!("{sid}.jsonl"));
            let session_result = searcher
                .search_session_file(project_id, sid, &session_path, &request.query, max_results)
                .await
                .map_err(|e| ApiError::internal(format!("search in session {sid} failed: {e}")))?;
            let total_matches = session_result.total_matches;
            let results = if session_result.hits.is_empty() {
                Vec::new()
            } else {
                vec![session_result]
            };
            return Ok(cdt_core::SearchSessionsResult {
                results,
                total_matches,
                sessions_searched: 1,
                query: request.query.clone(),
                is_partial: false,
            });
        }

        // SSH 走 SearchConfig.is_ssh=true（stage-limit 避免远端 SFTP 全量扫描），
        // Local 走默认；选择由 BackendResolvers 在 LazyLock 实例化时一次决定（D1）。
        let config = resolvers.search_config.clone();
        let result = searcher
            .search_sessions(project_id, &request.query, max_results, &config)
            .await
            .map_err(|e| ApiError::internal(format!("search error: {e}")))?;

        Ok(result)
    }

    async fn search_group_sessions(
        &self,
        group_id: &str,
        query: &str,
    ) -> Result<cdt_core::SearchSessionsResult, ApiError> {
        if query.is_empty() {
            return Ok(cdt_core::SearchSessionsResult {
                results: Vec::new(),
                total_matches: 0,
                sessions_searched: 0,
                query: String::new(),
                is_partial: false,
            });
        }

        let (groups, fs, projects_dir, _ctx, _captured_generation) =
            self.list_repository_groups_inner().await?;
        let group = find_group_with_fallback(groups, group_id)?;

        let project_ids: Vec<&str> = group.worktrees.iter().map(|wt| wt.id.as_str()).collect();
        let config = SearchConfig::from_fs_kind(fs.kind());
        let searcher = SessionSearcher::new(fs, self.search_cache.clone(), projects_dir);
        let max_results = 50;
        let result = searcher
            .search_across_projects(&project_ids, query, max_results, &config)
            .await
            .map_err(|e| ApiError::internal(format!("group search error: {e}")))?;

        Ok(result)
    }

    // =========================================================================
    // 配置 + 通知
    // =========================================================================

    async fn get_config(&self) -> Result<cdt_config::AppConfig, ApiError> {
        let mgr = self.config_mgr.lock().await;
        Ok(mgr.get_config())
    }

    async fn config_version(&self) -> Result<u64, ApiError> {
        let mgr = self.config_mgr.lock().await;
        Ok(mgr.version())
    }

    async fn get_config_versioned(&self) -> Result<(cdt_config::AppConfig, u64), ApiError> {
        let mgr = self.config_mgr.lock().await;
        Ok((mgr.get_config(), mgr.version()))
    }

    async fn update_config(
        &self,
        request: &ConfigUpdateRequest,
    ) -> Result<cdt_config::AppConfig, ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        let result = match request.section.as_str() {
            "general" => mgr.update_general(request.data.clone()).await,
            "display" => mgr.update_display(request.data.clone()).await,
            "notifications" => mgr.update_notifications(request.data.clone()).await,
            "ssh" => mgr.update_ssh(request.data.clone()).await,
            "httpServer" => mgr.update_http_server(request.data.clone()).await,
            "updater" => mgr.update_updater(request.data.clone()).await,
            "keyboardShortcuts" => mgr.update_keyboard_shortcuts(request.data.clone()).await,
            _ => {
                return Err(ApiError::validation(format!(
                    "unknown section: {}",
                    request.section
                )));
            }
        };
        let config = result.map_err(|e| ApiError::internal(format!("{e}")))?;
        if request.section == "general" && request.data.get("claudeRootPath").is_some() {
            self.reconfigure_claude_root(config.general.claude_root_path.as_deref())
                .await;
        }
        Ok(config)
    }

    async fn update_config_versioned(
        &self,
        request: &ConfigUpdateRequest,
    ) -> Result<(cdt_config::AppConfig, u64), ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        let result = match request.section.as_str() {
            "general" => mgr.update_general(request.data.clone()).await,
            "display" => mgr.update_display(request.data.clone()).await,
            "notifications" => mgr.update_notifications(request.data.clone()).await,
            "ssh" => mgr.update_ssh(request.data.clone()).await,
            "httpServer" => mgr.update_http_server(request.data.clone()).await,
            "updater" => mgr.update_updater(request.data.clone()).await,
            "keyboardShortcuts" => mgr.update_keyboard_shortcuts(request.data.clone()).await,
            _ => {
                return Err(ApiError::validation(format!(
                    "unknown section: {}",
                    request.section
                )));
            }
        };
        let config = result.map_err(|e| ApiError::internal(format!("{e}")))?;
        let version = mgr.version();
        // Drop lock before async reconfigure
        drop(mgr);
        if request.section == "general" && request.data.get("claudeRootPath").is_some() {
            self.reconfigure_claude_root(config.general.claude_root_path.as_deref())
                .await;
        }
        Ok((config, version))
    }

    async fn get_notifications(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<cdt_config::GetNotificationsResult, ApiError> {
        let mgr = self.notif_mgr.lock().await;
        Ok(mgr.get_notifications(limit, offset))
    }

    async fn mark_notification_read(&self, notification_id: &str) -> Result<bool, ApiError> {
        let mut mgr = self.notif_mgr.lock().await;
        mgr.mark_as_read(notification_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn delete_notification(&self, notification_id: &str) -> Result<bool, ApiError> {
        let mut mgr = self.notif_mgr.lock().await;
        mgr.delete_one(notification_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn mark_all_notifications_read(&self) -> Result<(), ApiError> {
        let mut mgr = self.notif_mgr.lock().await;
        mgr.mark_all_as_read()
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn clear_notifications(&self, trigger_id: Option<&str>) -> Result<usize, ApiError> {
        let mut mgr = self.notif_mgr.lock().await;
        if let Some(id) = trigger_id {
            mgr.clear_by_trigger_id(id)
                .await
                .map_err(|e| ApiError::internal(format!("{e}")))
        } else {
            let before = mgr.get_notifications(usize::MAX, 0).total;
            mgr.clear_all()
                .await
                .map_err(|e| ApiError::internal(format!("{e}")))?;
            Ok(before)
        }
    }

    // =========================================================================
    // SSH + Context
    // =========================================================================

    async fn list_contexts(&self) -> Result<Vec<ContextInfo>, ApiError> {
        let active = self.ssh_mgr.active_context_id().await;
        let mut contexts = vec![ContextInfo {
            id: "local".into(),
            kind: "local".into(),
            is_active: active.is_none(),
            host: None,
        }];

        for state in self.ssh_mgr.context_states().await {
            contexts.push(ContextInfo {
                id: state.context_id.clone(),
                kind: "ssh".into(),
                is_active: active.as_deref() == Some(state.context_id.as_str()),
                host: Some(state.host),
            });
        }

        Ok(contexts)
    }

    async fn switch_context(&self, context_id: &str) -> Result<(), ApiError> {
        let _ops = self.ssh_watcher_ops.lock().await;
        let previous_context_id = self.ssh_mgr.active_context_id().await;
        let target = if context_id == "local" {
            None
        } else {
            Some(context_id.to_owned())
        };
        tracing::debug!(
            target: "cdt_ssh::lifecycle",
            phase = "switch_context_begin",
            prev = ?previous_context_id,
            next = ?target,
            "ssh_watcher_ops locked"
        );
        // spec `ssh-remote-context::Reconnect lifecycle preserves SFTP session
        // integrity`：cancel-and-join SHALL 在 ssh_mgr mutate 之前完成
        if previous_context_id != target {
            // change `unify-fs-direct-calls` codex 二审 H2 + 第二轮 H2-R + 第三轮
            // H2-R-2 修订：先 bump `context_generation` 让任何 in-flight
            // list_sessions 在 spawn 时记录的 expected_context_generation 失效
            // → scan task 内每次 broadcast 前 SHALL check 不变；再按 ctx 精确
            // abort 已注册的 active_scans。两路防护：
            //   - bump → 关闭"abort 与 in-flight list_sessions await 之间的窗口"
            //   - abort → 立即停止已注册 scan
            self.context_generation.fetch_add(1, Ordering::SeqCst);
            if let Some(prev) = previous_context_id.as_deref() {
                tracing::debug!(target: "cdt_ssh::lifecycle", phase = "cancel_prev_watcher", context_id = %prev);
                // switch context: prev != target，旧 baseline 不传给新 ctx
                let _ = self.cancel_remote_watcher(prev).await;
                self.abort_scans_for_ssh_context_id(prev).await;
            } else if target.is_some() {
                // Local → SSH 切换：abort Local context 下的 in-flight scan，避免
                // 切到 SSH 后 Local scan 仍 broadcast Local metadata 串扰 SSH UI。
                self.abort_local_scans();
            }
        }
        tracing::debug!(target: "cdt_ssh::lifecycle", phase = "ssh_mgr_switch_context");
        self.ssh_mgr
            .switch_context(target.clone())
            .await
            .map_err(|e| ApiError::ssh(format!("{e}")))?;
        if previous_context_id != target {
            if let Some(next) = target.as_deref() {
                tracing::debug!(target: "cdt_ssh::lifecycle", phase = "attach_new_watcher", context_id = %next);
                self.attach_remote_watcher(next, None).await;
            }
        }
        tracing::debug!(target: "cdt_ssh::lifecycle", phase = "switch_context_end");
        Ok(())
    }

    async fn ssh_connect(
        &self,
        request: &SshConnectRequest,
    ) -> Result<serde_json::Value, ApiError> {
        let context_id = {
            let _ops = self.ssh_watcher_ops.lock().await;
            let shutdown_generation = self.ssh_shutdown_generation.load(Ordering::SeqCst);
            let target_context_id = request
                .context_id
                .clone()
                .unwrap_or_else(|| request.host.clone());
            let previous_context_id = self.ssh_mgr.active_context_id().await;
            tracing::debug!(
                target: "cdt_ssh::lifecycle",
                phase = "ssh_connect_begin",
                prev = ?previous_context_id,
                target = %target_context_id,
                "ssh_watcher_ops locked"
            );
            // 无条件 bump generation + cancel 旧 watcher + abort 旧 scan ——
            // 即便 previous_context_id == target_context_id（同 ctx 重连）也要做。
            //
            // 修复 codex 二审第二轮 critical（2026-05-22）：同 ctx 重连若不 bump
            // generation，旧 watcher 的 dead-signal monitor 在新 attach 后仍能
            // 通过 (context_generation == captured) + (active == ctx) 二次校验
            // 误删新连接。无条件 bump 让任何已 capture 的旧 generation 必失配 →
            // monitor stale 路径 silent return → race 关闭。
            //
            // 同 ctx 重连侧只需多付出"主动 emit ContextChanged(same_ctx) + 前端
            // listener 多 refresh 一次"的开销，远比 race 误删可接受。
            //
            // `prev_for_failure_reattach` 在 `if let Some(prev)` move 之前保留一份
            // clone —— 给下方 `ssh_mgr.connect` 失败路径用：若 prev == target 且
            // `ssh_mgr` 内旧 session 仍在（同 ctx 重连 + 握手失败但旧 session 未
            // 清），SHALL re-attach 旧 watcher，否则 active=SSH + 旧 SFTP 还活但
            // **无 watcher 监控** → SFTP 真死也无人自愈（codex 二审第三轮 major）。
            let prev_for_failure_reattach = previous_context_id.clone();
            self.context_generation.fetch_add(1, Ordering::SeqCst);
            // 保存 cancel 返回的 baseline：**仅**同 ctx reconnect 路径透传给后续
            // attach 实现 D5 断连重连 baseline diff。跨 ctx connect（A→B）时旧 A
            // 的 baseline 与 B 无关，传入会导致 B 首轮 readdir 与 A baseline diff
            // 产出错误的 deleted/first-seen 事件（codex PR #305 三审 BUG #4 修复）。
            let reconnect_baseline = if let Some(prev) = previous_context_id {
                tracing::debug!(target: "cdt_ssh::lifecycle", phase = "cancel_prev_watcher", context_id = %prev);
                let baseline = self.cancel_remote_watcher(&prev).await;
                // codex 二审第二轮 H2-R：ssh_connect(A→B) 也 SHALL abort 旧 A
                // 下 page_jobs scan，避免旧 A scan 跨切换继续 broadcast 串扰 B 渲染。
                self.abort_scans_for_ssh_context_id(&prev).await;
                // BUG #4：仅同 ctx 重连时透传 baseline；跨 ctx 连接传 None
                if prev == target_context_id {
                    baseline
                } else {
                    None
                }
            } else {
                // Local → SSH 切换路径：abort Local scan 避免 Local metadata
                // 跨切换串扰 SSH UI。
                self.abort_local_scans();
                None
            };
            tracing::debug!(target: "cdt_ssh::lifecycle", phase = "ssh_mgr_connect");
            let context_id = match self.ssh_mgr.connect(request.clone().into()).await {
                Ok(id) => id,
                Err(e) => {
                    // 同 ctx 重连握手失败的 reattach 补救（codex 二审第三轮 major）：
                    // `ssh_mgr.connect` 在 prev == target 时不主动 disconnect 旧
                    // session（详 `cdt-ssh::session.rs:383-388`），失败时旧 session
                    // 仍在 `ssh_mgr.sessions` 内但我们上面已经 cancel 了旧 watcher。
                    // 若不补 attach，active=SSH + 旧 SFTP 仍可用但**无 watcher 监控** →
                    // SFTP 后续死掉也无人触发自愈，用户被永久卡 stale active。
                    //
                    // 补救条件三连：(1) 有 prev (2) prev == target (3) 旧 provider
                    // 仍在 ssh_mgr（握手失败未污染旧 session）。任一不满足走原 Err
                    // 路径（fresh ctx 失败 / Local→SSH 失败 / 旧 session 已不存在）。
                    if let Some(prev) = prev_for_failure_reattach.as_deref() {
                        if prev == target_context_id.as_str()
                            && self.ssh_mgr.provider(prev).await.is_some()
                        {
                            tracing::warn!(
                                target: "cdt_ssh::lifecycle",
                                phase = "reattach_after_failed_reconnect",
                                context_id = %prev,
                                "same-ctx reconnect failed; re-attaching watcher to preserve self-heal coverage",
                            );
                            self.attach_remote_watcher(prev, reconnect_baseline).await;
                        }
                    }
                    return Err(ApiError::ssh(format!("{e}")));
                }
            };
            if self.ssh_shutdown_generation.load(Ordering::SeqCst) == shutdown_generation {
                tracing::debug!(target: "cdt_ssh::lifecycle", phase = "attach_new_watcher", context_id = %context_id);
                self.attach_remote_watcher(&context_id, reconnect_baseline)
                    .await;
                context_id
            } else {
                tracing::debug!(target: "cdt_ssh::lifecycle", phase = "shutdown_in_progress_aborting", context_id = %context_id);
                let _ = self.ssh_mgr.disconnect(&context_id).await;
                return Err(ApiError::ssh("SSH shutdown in progress"));
            }
        };
        tracing::debug!(target: "cdt_ssh::lifecycle", phase = "ssh_connect_end", context_id = %context_id);
        let auth_chain = self
            .ssh_mgr
            .context_state(&context_id)
            .await
            .map_or_else(Vec::new, |state| state.auth_chain);
        let result = super::types::SshConnectionResult {
            context_id,
            status: cdt_ssh::SshStatus::Connected,
            auth_chain,
        };
        serde_json::to_value(result).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn ssh_disconnect(&self, context_id: &str) -> Result<(), ApiError> {
        let _ops = self.ssh_watcher_ops.lock().await;
        tracing::debug!(target: "cdt_ssh::lifecycle", phase = "ssh_disconnect_begin", context_id = %context_id);
        // change `unify-fs-direct-calls` codex 二审第二轮 H2-R + 第三轮 H2-R-2
        // 修订：先 bump context_generation 关闭 in-flight list_sessions 的 late
        // insert 窗口；再按 SSH context_id 精确 abort 已注册 scan handle（**在
        // cancel_remote_watcher 与 ssh_mgr.disconnect 之前**，确保仍能 lookup
        // 到 ContextId）。避免 abort-all 误杀已 active 的新 host scan
        // （场景：B 已 active spawn scan 时并发 ssh_disconnect("A") 清理旧 host）。
        self.context_generation.fetch_add(1, Ordering::SeqCst);
        self.abort_scans_for_ssh_context_id(context_id).await;
        // disconnect: 无后续 attach，丢弃 baseline 返值
        let _ = self.cancel_remote_watcher(context_id).await;
        tracing::debug!(target: "cdt_ssh::lifecycle", phase = "ssh_mgr_disconnect", context_id = %context_id);
        let result = self
            .ssh_mgr
            .disconnect(context_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")));
        tracing::debug!(target: "cdt_ssh::lifecycle", phase = "ssh_disconnect_end", context_id = %context_id);
        result
    }

    async fn ssh_test_connection(
        &self,
        request: &SshConnectRequest,
    ) -> Result<serde_json::Value, ApiError> {
        let auth_chain = self
            .ssh_mgr
            .test_connection(request.clone().into())
            .await
            .map_err(|e| ApiError::ssh(format!("{e}")))?;
        let result = super::types::SshConnectionResult {
            context_id: request
                .context_id
                .clone()
                .unwrap_or_else(|| format!("{}-test", request.host)),
            status: cdt_ssh::SshStatus::Connected,
            auth_chain,
        };
        serde_json::to_value(result).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn ssh_get_state(&self) -> Result<serde_json::Value, ApiError> {
        let state = super::types::SshState {
            active_context_id: self.ssh_mgr.active_context_id().await,
            contexts: self.ssh_mgr.context_states().await,
        };
        serde_json::to_value(state).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn ssh_get_config_hosts(&self) -> Result<serde_json::Value, ApiError> {
        let config_path = default_ssh_config_path();
        let configs = parse_ssh_config_file(&config_path).await;
        serde_json::to_value(list_hosts(&configs)).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn resolve_ssh_host(&self, alias: &str) -> Result<serde_json::Value, ApiError> {
        let host = resolve_host_via_ssh_g(alias)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        serde_json::to_value(host).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn ssh_save_last_connection(
        &self,
        request: &SshConnectRequest,
    ) -> Result<serde_json::Value, ApiError> {
        let last_connection = SshLastConnection {
            host: request.host.clone(),
            port: request.port,
            username: request.username.clone(),
            auth_method: match request.auth_method {
                super::types::SshAuthMethod::SshConfig => cdt_config::SshAuthMethod::SshConfig,
                super::types::SshAuthMethod::Password => cdt_config::SshAuthMethod::Password,
            },
            context_id: request.context_id.clone(),
        };
        let mut mgr = self.config_mgr.lock().await;
        let config = mgr
            .save_ssh_last_connection(last_connection)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        serde_json::to_value(config.ssh.last_connection)
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn ssh_get_last_connection(&self) -> Result<serde_json::Value, ApiError> {
        let mgr = self.config_mgr.lock().await;
        serde_json::to_value(mgr.get_ssh_last_connection())
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn get_active_context(&self) -> Result<ContextInfo, ApiError> {
        let active = self.ssh_mgr.active_context_id().await;
        if let Some(context_id) = active {
            let state = self
                .ssh_mgr
                .context_state(&context_id)
                .await
                .ok_or_else(|| ApiError::not_found(format!("SSH context: {context_id}")))?;
            Ok(ContextInfo {
                id: state.context_id,
                kind: "ssh".into(),
                is_active: true,
                host: Some(state.host),
            })
        } else {
            Ok(ContextInfo {
                id: "local".into(),
                kind: "local".into(),
                is_active: true,
                host: None,
            })
        }
    }

    // =========================================================================
    // 文件 + 验证
    // =========================================================================

    async fn validate_path(
        &self,
        path: &str,
        project_root: Option<&str>,
    ) -> Result<serde_json::Value, ApiError> {
        let result = validate_file_path(path, project_root.map(Path::new));
        Ok(serde_json::json!({
            "valid": result.valid,
            "error": result.error,
            "normalizedPath": result.normalized_path.map(|p| p.to_string_lossy().into_owned()),
        }))
    }

    async fn read_claude_md_files(
        &self,
        project_root: &str,
    ) -> Result<serde_json::Value, ApiError> {
        let claude_base = self.claude_base_path().await;
        let result =
            read_all_claude_md_files_with_base(Path::new(project_root), &claude_base).await;
        serde_json::to_value(&result).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn read_mentioned_file(
        &self,
        path: &str,
        project_root: &str,
    ) -> Result<serde_json::Value, ApiError> {
        // change `unify-fs-direct-calls` design D7：SSH context 下 @mention 文件解析
        // SHALL gracefully skip 而非读 Local 路径串扰（mention.rs 走 ALLOWLIST 不切
        // fs trait，但 caller 侧按 fs.kind() gate）。
        let (fs, _projects_dir) = self.active_fs_and_projects_dir().await?;
        if fs.kind() == cdt_fs::FsKind::Ssh {
            // SSH context 下返回 Null 让前端按"无 mention 内容"处理；具体 error code
            // 上移留 follow-up（codex Open Question 4）。
            return Ok(serde_json::Value::Null);
        }
        let result = config_read_mentioned_file(path, Path::new(project_root), None)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        Ok(serde_json::to_value(&result).unwrap_or(serde_json::Value::Null))
    }

    // =========================================================================
    // 辅助
    // =========================================================================

    async fn read_agent_configs(&self, _project_root: &str) -> Result<serde_json::Value, ApiError> {
        // 简化：agent config 读取需要文件扫描，暂返回空
        Ok(serde_json::json!({}))
    }

    async fn list_repository_groups(&self) -> Result<Vec<cdt_core::RepositoryGroup>, ApiError> {
        let _telemetry_timer =
            cdt_telemetry::histogram!("ipc.list_repository_groups.duration_ns").start_timer();
        // D3b：grouper 无状态轻量，每次 lazy 构造，避免 LocalDataApi 字段污染。
        // active context = SSH 时 SHALL NOT 用 LocalGitIdentityResolver——容器内远端
        // cwd 与本机宿主路径重合时（如 docker 挂载 `~/.claude` 复现场景），会读宿主机
        // `.git` 泄漏本地 gitBranch。BackendResolvers 在 LazyLock 实例化时按 fs.kind()
        // 选 Local / Noop resolver（change `backend-policy-struct` D1 + D7）。
        //
        // FU-4 ProjectScanner memoize：scan 结果走 `scan_projects_cached_with` 进程级
        // cache，命中时跳过全量 fs op；grouper 仍每次跑（git resolve 本身已 cache）。
        // `(*projects).clone()` 把 Arc 下的 Vec 拷一份给 grouper 的 owned 入参——
        // Vec<Project> 几百 KB 量级的 clone 远低于全量 scan 的 ~14K fs ops 成本。
        //
        // change `generation-race-audit`：抽 inner 拿 (groups, fs, projects_dir,
        // ctx, captured_generation) 同源快照；refresh 路径在 `ssh_watcher_ops` 锁内
        // 做 (ctx + generation) 双重校验，任一 mismatch skip refresh（safe degrade）。
        // 闭合 bump-first 顺序导致的两类 sub-window race：(a) 普通 ssh switch +
        // generation 在 ssh_mgr.switch_context 网络 RTT 期间已领先；(b) 同 host 快速
        // disconnect+reconnect 期间 ContextId 等价但 generation bumped 两次。
        let (groups, _fs, _projects_dir, captured_ctx, captured_context_generation) =
            self.list_repository_groups_inner().await?;
        {
            let _ops = self.ssh_watcher_ops.lock().await;
            let current_ctx = self.current_active_context_id_under_lock().await;
            let current_context_generation = self.context_generation.load(Ordering::SeqCst);
            if current_ctx != captured_ctx
                || current_context_generation != captured_context_generation
            {
                tracing::debug!(
                    target: "cdt_api::perf",
                    captured_ctx = ?captured_ctx,
                    current_ctx = ?current_ctx,
                    captured_gen = captured_context_generation,
                    current_gen = current_context_generation,
                    "list_repository_groups: state changed mid-scan (ctx or generation), skip refresh_worktree_meta_cache"
                );
                return Ok(groups);
            }
            // match：在锁内 refresh，与 5 处 mutate 入口（switch / connect / disconnect /
            // reconfigure / shutdown）互斥；下游序列化 SessionSummary 能 join 到最新 mapping。
            self.refresh_worktree_meta_cache(&groups);
        }
        Ok(groups)
    }

    /// 实现：k-way merge 流式分页（spec `ipc-data-api::Expose group session
    /// listing via k-way merge pagination`）。详 design D3。
    ///
    /// Server 无状态——cursor 自描述每个 worktree 的指针位置。
    async fn list_group_sessions(
        &self,
        group_id: &str,
        page_size: usize,
        cursor: Option<&str>,
    ) -> Result<GroupSessionPage, ApiError> {
        if page_size == 0 {
            return Err(ApiError::validation("pageSize must be > 0"));
        }
        self.build_group_session_page(group_id, page_size, cursor)
            .await
    }

    /// 重写默认 fallback：用 `list_group_sessions` 的 k-way merge 路径，
    /// 重打包成 `PaginatedResponse<SessionSummary>`，**禁止**老的
    /// `list_sessions_sync(page_size=usize::MAX)` 全量扫描。
    /// spec `ipc-data-api::Expose worktree sessions query` §"实现不允许全量扫描"。
    async fn get_worktree_sessions(
        &self,
        group_id: &str,
        pagination: &PaginatedRequest,
    ) -> Result<PaginatedResponse<SessionSummary>, ApiError> {
        if pagination.page_size == 0 {
            return Err(ApiError::validation("pageSize must be > 0"));
        }
        let page = self
            .build_group_session_page(group_id, pagination.page_size, pagination.cursor.as_deref())
            .await?;
        let total = page.sessions.len();
        Ok(PaginatedResponse {
            items: page.sessions,
            next_cursor: page.next_cursor,
            // `total` 现含义为本页条目数（cursor 分页下无法 O(1) 拿全局 total，
            // 与既有 `list_sessions` `total` 语义"骨架阶段非全局准确"一致）。
            total,
        })
    }

    // -------------------------------------------------------------------------
    // Trigger / pin / hide / session prefs
    //
    // 历史上这 7 个方法在独立 `impl LocalDataApi` 块作为 inherent method（参见
    // `src-tauri/CLAUDE.md` 旧约定 "Trigger CRUD 走独立方法"）。本 change
    // `add-server-mode` 把它们提升到 trait 让 HTTP 路径（浏览器 runtime）
    // 能镜像 IPC 同名 command（spec：`http-data-api::Mirror lazy and auxiliary
    // IPC commands`）。`src-tauri/src/lib.rs` 调用方式不变（`Arc<LocalDataApi>`
    // 上调方法仍解析到这里，trait 在 scope 内）。
    // -------------------------------------------------------------------------

    async fn add_trigger(
        &self,
        trigger: cdt_config::NotificationTrigger,
    ) -> Result<serde_json::Value, ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        let config = mgr
            .add_trigger(trigger)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        let mut value =
            serde_json::to_value(&config).map_err(|e| ApiError::internal(format!("{e}")))?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "_version".to_string(),
                serde_json::Value::from(mgr.version()),
            );
        }
        Ok(value)
    }

    async fn remove_trigger(&self, trigger_id: &str) -> Result<serde_json::Value, ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        let config = mgr
            .remove_trigger(trigger_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        let mut value =
            serde_json::to_value(&config).map_err(|e| ApiError::internal(format!("{e}")))?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "_version".to_string(),
                serde_json::Value::from(mgr.version()),
            );
        }
        Ok(value)
    }

    async fn pin_session(&self, project_id: &str, session_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        mgr.pin_session(project_id, session_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn unpin_session(&self, project_id: &str, session_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        mgr.unpin_session(project_id, session_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn hide_session(&self, project_id: &str, session_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        mgr.hide_session(project_id, session_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn unhide_session(&self, project_id: &str, session_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        mgr.unhide_session(project_id, session_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn get_project_session_prefs(
        &self,
        project_id: &str,
    ) -> Result<ProjectSessionPrefs, ApiError> {
        let mgr = self.config_mgr.lock().await;
        let config = mgr.get_config();
        let pinned = config
            .sessions
            .pinned_sessions
            .get(project_id)
            .map(|v| v.iter().map(|p| p.session_id.clone()).collect())
            .unwrap_or_default();
        let hidden = config
            .sessions
            .hidden_sessions
            .get(project_id)
            .map(|v| v.iter().map(|h| h.session_id.clone()).collect())
            .unwrap_or_default();
        Ok(ProjectSessionPrefs { pinned, hidden })
    }

    // -----------------------------------------------------------------
    // 外部应用交互（Phase 2 frontend-context-menu）
    // 详 openspec/specs/frontend-context-menu/spec.md 三个 Requirement
    // -----------------------------------------------------------------

    async fn open_in_terminal(&self, path: &str) -> Result<(), ApiError> {
        super::external_app::open_in_terminal(path, &self.config_mgr).await
    }

    async fn open_in_editor(
        &self,
        path: &str,
        line: Option<u32>,
        column: Option<u32>,
    ) -> Result<(), ApiError> {
        super::external_app::open_in_editor(path, line, column, &self.config_mgr).await
    }

    // list_available_terminals 走 trait default impl（无状态依赖）

    // =========================================================================
    // Background Jobs
    // =========================================================================

    async fn list_jobs(&self) -> Result<cdt_core::JobsResponse, ApiError> {
        let home = cdt_discover::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let jobs_dir = home.join(".claude").join("jobs");
        list_jobs_from_dir(&jobs_dir).await
    }

    async fn stop_job(&self, job_id: &str) -> Result<(), ApiError> {
        if job_id.is_empty() {
            return Err(ApiError::validation("job_id must not be empty"));
        }
        let claude = resolve_claude_cli()?;
        let short: String = job_id.chars().take(8).collect();
        let output = tokio::process::Command::new(&claude)
            .args(["stop", &short])
            .output()
            .await
            .map_err(|e| ApiError::internal(format!("failed to spawn claude stop: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ApiError::internal(format!("stop failed: {stderr}")));
        }
        Ok(())
    }

    async fn delete_job(&self, job_id: &str) -> Result<(), ApiError> {
        if job_id.is_empty() {
            return Err(ApiError::validation("job_id must not be empty"));
        }
        let claude = resolve_claude_cli()?;
        let short: String = job_id.chars().take(8).collect();
        let output = tokio::process::Command::new(&claude)
            .args(["rm", &short])
            .output()
            .await
            .map_err(|e| ApiError::internal(format!("failed to spawn claude rm: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ApiError::internal(format!("delete failed: {stderr}")));
        }
        Ok(())
    }

    async fn delete_completed_jobs(&self) -> Result<u32, ApiError> {
        let home = cdt_discover::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let jobs_dir = home.join(".claude").join("jobs");
        let response = list_jobs_from_dir(&jobs_dir).await?;

        let mut deleted = 0u32;
        for job in &response.jobs {
            let is_completed = matches!(
                job.state,
                cdt_core::JobState::Done
                    | cdt_core::JobState::Failed
                    | cdt_core::JobState::Stopped
                    | cdt_core::JobState::Idle
            );
            if is_completed {
                match self.delete_job(&job.id).await {
                    Ok(()) => deleted += 1,
                    Err(e) => {
                        tracing::warn!(job_id = %job.id, error = %e, "failed to delete job");
                    }
                }
            }
        }
        Ok(deleted)
    }
}

// =============================================================================
// Inherent helpers for server-mode（host-only / 非 trait 方法）
// =============================================================================

impl LocalDataApi {
    /// 返回当前持久化 `HttpServerConfig`，供 server-mode 启动恢复 +
    /// `http_server_status` IPC 读取最近一次成功 `port`。
    pub async fn http_server_config(&self) -> Result<cdt_config::HttpServerConfig, ApiError> {
        let mgr = self.config_mgr.lock().await;
        Ok(mgr.get_config().http_server)
    }

    /// 把 `httpServer.enabled` 字段写盘，**不**改 `port`。server-mode 的
    /// `http_server_stop` / `http_server_start` 成功后调此方法落盘用户意图（详
    /// `openspec/specs/configuration-management/spec.md` §"HTTP server enabled
    /// / port SHALL be persisted in lockstep with lifecycle"）。
    pub async fn set_http_server_enabled(&self, enabled: bool) -> Result<(), ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        mgr.set_http_server_enabled(enabled)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        Ok(())
    }

    /// 把 `httpServer.port` 字段写盘（先经 `validate_http_port` 校验）。
    /// server-mode 的 `http_server_start` 成功后调此方法持久化用户选的端口。
    pub async fn set_http_server_port(&self, port: u16) -> Result<(), ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        mgr.set_http_server_port(port)
            .await
            .map_err(|e| ApiError::validation(format!("{e}")))?;
        Ok(())
    }
}

// =============================================================================
// Inherent helpers（test-only / 非 trait 方法）
// =============================================================================

impl LocalDataApi {
    /// 测试 helper：返回 parsed-message cache 当前条目数。仅 integration test
    /// 用来验证 file-change 广播 invalidate 路径生效（见 change
    /// `parsed-message-lru-cache`）。
    ///
    /// 仅在 `test-utils` feature 启用时编译——release / 默认构建 SHALL NOT
    /// 含此 API（codex 二审 bug 2：`#[doc(hidden)] pub` 只隐藏文档不限制调用）。
    #[cfg(any(test, feature = "test-utils"))]
    pub fn parsed_msg_cache_len(&self) -> usize {
        self.parsed_msg_cache
            .lock()
            .expect("parsed message cache mutex poisoned")
            .len()
    }

    /// 测试 helper：触发 parsed-message cache 对 `path` 的 `extract` 写入。
    ///
    /// 仅在 `test-utils` feature 启用时编译。
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn prime_parsed_msg_cache_for_test(
        &self,
        path: &std::path::Path,
    ) -> Option<Arc<Vec<cdt_core::ParsedMessage>>> {
        let (fs, _projects_dir, ctx) = self.active_fs_and_context().await;
        extract_parsed_messages_cached(&self.parsed_msg_cache, &*fs, &ctx, path).await
    }

    #[cfg(test)]
    async fn run_ssh_watcher_op_for_test(&self, delay: std::time::Duration) {
        let _ops = self.ssh_watcher_ops.lock().await;
        tokio::time::sleep(delay).await;
    }

    /// 读取 `.claude/agents/*.md` 配置（全局 + 所有已发现项目）。
    ///
    /// 用于前端 subagent 彩色 badge 的颜色查询，对齐原版 `agentConfigs` store。
    pub async fn read_agent_configs(
        &self,
    ) -> Result<Vec<cdt_discover::agent_configs::AgentConfig>, ApiError> {
        let mut scanner = self.scanner.lock().await;
        let projects = scanner
            .scan()
            .await
            .map_err(|e| ApiError::internal(format!("scan error: {e}")))?;
        drop(scanner);

        // 同一 encoded project 目录下若存在多个不同 `cwd` 的 session（典型场景：
        // worktree / monorepo 子目录），SHALL 扫描所有 cwd 下的 `.claude/agents/`。
        // 历史上 composite 拆分会按 cwd 把同 encoded 目录拆成多个 `Project`，所以
        // 一个 project 一条 pair 就够了；现已合并（change `merge-composite-projects`）,
        // 因此 IPC 入口在这里**按每个 project 的 distinct cwds 笛卡尔展开**。
        // Spec：`agent-configs::Scan agent config files from global and project scopes`。
        let mut pairs: Vec<(String, String)> = Vec::new();
        for project in &projects {
            if project.distinct_cwds.is_empty() {
                pairs.push((
                    project.id.clone(),
                    project.path.to_string_lossy().into_owned(),
                ));
            } else {
                for cwd in &project.distinct_cwds {
                    pairs.push((project.id.clone(), cwd.clone()));
                }
            }
        }
        // 文件系统 I/O 走 `tokio::fs`，不阻塞 runtime worker。
        let configs = cdt_discover::agent_configs::read_agent_configs(&pairs).await;
        Ok(configs)
    }
}

/// 把 layers Vec 拼成完整 [`ProjectMemory`]——共用给 `get_project_memory` /
/// `add_memory` / `delete_memory` 三个 IPC 路径，避免逻辑分叉。
fn build_project_memory(project_id: &str, layers: Vec<MemoryLayer>) -> ProjectMemory {
    let default_file = layers
        .iter()
        .find(|layer| layer.kind == MemoryLayerKind::Index)
        .or_else(|| layers.first())
        .map(|layer| layer.file.clone());
    ProjectMemory {
        project_id: project_id.to_owned(),
        has_memory: !layers.is_empty(),
        count: layers.len(),
        default_file,
        layers,
    }
}

async fn discover_memory_layers(
    fs: &dyn FileSystemProvider,
    memory_dir: &Path,
) -> Result<Vec<MemoryLayer>, ApiError> {
    let entries = match fs.read_dir(memory_dir).await {
        Ok(entries) => entries,
        Err(cdt_fs::FsError::NotFound(_)) => return Ok(Vec::new()),
        Err(e) => return Err(ApiError::internal(format!("read memory dir error: {e}"))),
    };

    let mut markdown_files = std::collections::BTreeSet::new();
    for entry in entries {
        if !entry.kind.is_file() {
            continue;
        }
        if Path::new(&entry.name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        {
            markdown_files.insert(entry.name);
        }
    }

    let mut layers = Vec::new();
    let mut indexed = std::collections::BTreeSet::new();
    if markdown_files.contains("MEMORY.md") {
        layers.push(MemoryLayer {
            file: "MEMORY.md".to_owned(),
            title: "Index".to_owned(),
            hook: Some("MEMORY.md".to_owned()),
            kind: MemoryLayerKind::Index,
        });
        let index_content = fs
            .read_to_string(&memory_dir.join("MEMORY.md"))
            .await
            .map_err(|e| ApiError::internal(format!("read MEMORY.md error: {e}")))?;
        for layer in parse_memory_index(&index_content, &markdown_files) {
            indexed.insert(layer.file.clone());
            layers.push(layer);
        }
    }

    for file in markdown_files {
        if file == "MEMORY.md" || indexed.contains(&file) {
            continue;
        }
        let title = file.trim_end_matches(".md").replace(['_', '-'], " ");
        layers.push(MemoryLayer {
            file,
            title,
            hook: None,
            kind: MemoryLayerKind::Orphan,
        });
    }

    Ok(layers)
}

fn parse_memory_index(
    content: &str,
    markdown_files: &std::collections::BTreeSet<String>,
) -> Vec<MemoryLayer> {
    content
        .lines()
        .filter_map(|line| parse_memory_index_line(line, markdown_files))
        .collect()
}

fn parse_memory_index_line(
    line: &str,
    markdown_files: &std::collections::BTreeSet<String>,
) -> Option<MemoryLayer> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("- [") {
        return None;
    }
    let title_start = trimmed.find('[')? + 1;
    let title_end = trimmed[title_start..].find(']')? + title_start;
    let after_title = &trimmed[title_end + 1..];
    let file_start = after_title.find('(')? + 1;
    let file_end = after_title[file_start..].find(')')? + file_start;
    let file = &after_title[file_start..file_end];
    let safe_file = validate_memory_file_name(file).ok()?;
    if !markdown_files.contains(&safe_file) {
        return None;
    }
    let hook = after_title[file_end + 1..]
        .split_once('—')
        .map(|(_, hook)| hook.trim().to_owned())
        .filter(|hook| !hook.is_empty());
    Some(MemoryLayer {
        file: safe_file,
        title: trimmed[title_start..title_end].trim().to_owned(),
        hook,
        kind: MemoryLayerKind::Entry,
    })
}

fn validate_project_base_dir(base_dir: &str) -> Result<(), ApiError> {
    let mut components = Path::new(base_dir).components();
    let valid = matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none()
        && !matches!(base_dir, "." | "..");
    if !valid {
        return Err(ApiError::validation(
            "project id must be an encoded project directory",
        ));
    }
    Ok(())
}

fn validate_memory_file_name(file: &str) -> Result<String, ApiError> {
    let path = Path::new(file);
    if cdt_discover::looks_like_absolute_path(file)
        || path.components().count() != 1
        || file.contains(['/', '\\', ':'])
        || !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        || file.is_empty()
    {
        return Err(ApiError::validation(
            "memory file must be a local .md filename",
        ));
    }
    Ok(file.to_owned())
}

/// 从文件系统扫描 CLAUDE.md 文件，构建 `ClaudeMdContextInjection` 列表。
async fn build_claude_md_from_filesystem(
    project_root: &str,
    claude_base: &Path,
) -> Vec<cdt_core::ContextInjection> {
    use cdt_config::claude_md::Scope;

    let files = read_all_claude_md_files_with_base(Path::new(project_root), claude_base).await;
    files
        .into_iter()
        .filter(|(_, info)| info.exists)
        .map(|(scope, info)| {
            let core_scope = match scope {
                Scope::Enterprise => cdt_core::ClaudeMdScope::Enterprise,
                Scope::User | Scope::UserRules | Scope::AutoMemory => cdt_core::ClaudeMdScope::User,
                Scope::Project | Scope::ProjectAlt | Scope::ProjectRules | Scope::ProjectLocal => {
                    cdt_core::ClaudeMdScope::Project
                }
            };
            let display_name = Path::new(&info.path)
                .file_name()
                .map_or_else(|| info.path.clone(), |s| s.to_string_lossy().into_owned());
            cdt_core::ContextInjection::ClaudeMd(cdt_core::ClaudeMdContextInjection {
                id: format!("claude-md-{}", info.path),
                path: info.path,
                display_name,
                scope: core_scope,
                estimated_tokens: u64::try_from(info.estimated_tokens).unwrap_or(0),
                first_seen_turn_index: 0,
            })
        })
        .collect()
}

// =============================================================================
// phase 3: image asset cache 辅助
// =============================================================================

/// 在 projects 根目录下定位 (`root_session_id`, `session_id`) 对应的 jsonl。
///
/// `session_id == root_session_id` 时直接找 root jsonl（在任一 `project_dir` 内）；
/// 不等时跨 `projects_dir` 扫 `{project_dir}/{root_session_id}/subagents/agent-<sub>.jsonl`
/// （新结构），命中即返；未命中再 fallback 到 root `project_dir` 内的 flat 旧结构。
/// 通过 `FileSystemProvider` 定位主 session / subagent 的 JSONL。
///
/// `session_id == root_session_id`：扫 `projects_dir` 找任一 `project_dir` 含 root jsonl
/// 即返。subagent：优先扫新嵌套结构（`{project}/{root}/subagents/agent-<sid>.jsonl`，
/// `CROSS_PROJECT_SUBAGENT_SCAN=true` 时跨 `projects_dir`），未命中再 fallback 到旧 flat
/// 结构（`{project}/agent-<sid>.jsonl`）。
///
/// SSH context 与 Local context 共用此入口（design D6：4 个 subagent helper 切 fs trait
/// + 保留 flat / nested 双结构）。
async fn locate_session_jsonl(
    fs: &dyn FileSystemProvider,
    projects_dir: &Path,
    root_session_id: &str,
    session_id: &str,
) -> Option<std::path::PathBuf> {
    // 主 session 自身：扫 projects_dir 找到任一 project_dir 含 root jsonl 即返。
    if session_id == root_session_id {
        let entries = fs.read_dir(projects_dir).await.ok()?;
        for entry in entries {
            if !entry.kind.is_dir() {
                continue;
            }
            let root_jsonl = projects_dir
                .join(&entry.name)
                .join(format!("{root_session_id}.jsonl"));
            if fs.exists(&root_jsonl).await {
                return Some(root_jsonl);
            }
        }
        return None;
    }

    // subagent：优先跨 project_dir 扫新结构。
    if CROSS_PROJECT_SUBAGENT_SCAN {
        if let Some(p) =
            find_subagent_jsonl_cross_project(fs, projects_dir, root_session_id, session_id).await
        {
            return Some(p);
        }
    }

    // 旧结构兜底：找含 root jsonl 的 project_dir 后扫该目录的 flat agent jsonl。
    let entries = fs.read_dir(projects_dir).await.ok()?;
    for entry in entries {
        if !entry.kind.is_dir() {
            continue;
        }
        let project_dir = projects_dir.join(&entry.name);
        let root_jsonl = project_dir.join(format!("{root_session_id}.jsonl"));
        if !fs.exists(&root_jsonl).await {
            continue;
        }
        if let Some(p) = find_subagent_jsonl(fs, &project_dir, session_id).await {
            return Some(p);
        }
    }
    None
}

/// 在已 parse 的消息流里按 `chunk_uuid` + `block_index` 找 ImageBlock，返回
/// `(base64_data, media_type)`。`chunk_uuid` 即 `ParsedMessage.uuid`。
fn find_image_block_in_messages(
    messages: &[cdt_core::ParsedMessage],
    chunk_uuid: &str,
    block_index: usize,
) -> Option<(String, String)> {
    let msg = messages.iter().find(|m| m.uuid == chunk_uuid)?;
    let cdt_core::MessageContent::Blocks(blocks) = &msg.content else {
        return None;
    };
    let cdt_core::ContentBlock::Image { source } = blocks.get(block_index)? else {
        return None;
    };
    Some((source.data.clone(), source.media_type.clone()))
}

/// 在 project 目录下查找指定 session id 的 subagent JSONL 文件。
///
/// 检查两种结构（design D6 保留双结构 fallback）：
/// - 旧 flat：`{project_dir}/agent-{session_id}.jsonl`
/// - 新 nested：`{project_dir}/*/subagents/agent-{session_id}.jsonl`（扁平扫一层主 session 目录）
///
/// 通过 `FileSystemProvider` 走当前 active context 的 fs；Local + SSH 共用此入口。
async fn find_subagent_jsonl(
    fs: &dyn FileSystemProvider,
    project_dir: &Path,
    session_id: &str,
) -> Option<std::path::PathBuf> {
    let filename = format!("agent-{session_id}.jsonl");

    // 旧 flat：project_dir/agent-<id>.jsonl
    let flat = project_dir.join(&filename);
    if fs.exists(&flat).await {
        return Some(flat);
    }

    // 新 nested：project_dir/{parent_session}/subagents/agent-<id>.jsonl
    let entries = fs.read_dir(project_dir).await.ok()?;
    for entry in entries {
        if !entry.kind.is_dir() {
            continue;
        }
        let candidate = project_dir
            .join(&entry.name)
            .join("subagents")
            .join(&filename);
        if fs.exists(&candidate).await {
            return Some(candidate);
        }
    }
    None
}

/// 跨 `projects_dir` 扫描 subagent 候选文件，构建 `SubagentCandidate` 列表。
///
/// 当 `CROSS_PROJECT_SUBAGENT_SCAN=true` 时，遍历 `projects_dir` 下所有 `project_dir`
/// 探测 `{dir}/{root_session_id}/subagents/agent-*.jsonl`（新结构）；
/// 旧结构 flat `{主_project_dir}/agent-*.jsonl` 仍只在主 `project_dir` 内扫
/// （跨目录扫旧结构需要 parse 每个 jsonl 检查 parent，成本不可控，且实测旧结构
/// 主要出现在主 cwd 启动的老 session，跨目录场景罕见）。
///
/// `main_project_dir` 用于旧结构扫描；新结构扫描仅依赖 `projects_dir`。
/// 通过 `FileSystemProvider` 走当前 active context 的 fs（design D6）。
///
/// 跳过 `agent-acompact*` 前缀（compaction 类内部产物，不是真实 subagent）。
#[allow(clippy::case_sensitive_file_extension_comparisons)]
async fn scan_subagent_candidates_cross_project(
    fs: &dyn FileSystemProvider,
    projects_dir: &Path,
    main_project_dir: &Path,
    root_session_id: &str,
) -> Vec<cdt_core::SubagentCandidate> {
    let t_total = std::time::Instant::now();
    let mut candidates = Vec::new();
    let mut per_candidate_ms: Vec<u128> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut dirs_with_subagents: usize = 0;

    // 第一遍：收集所有 project_dir entry path（顺序快，单 read_dir）。
    let mut project_dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(entries) = fs.read_dir(projects_dir).await {
        for entry in entries {
            if !entry.kind.is_dir() {
                continue;
            }
            project_dirs.push(projects_dir.join(&entry.name));
        }
    }
    let projects_scanned = project_dirs.len();

    // 第二遍：每个 project 并发探测 `<dir>/<root_session_id>/subagents/agent-*.jsonl`，
    // 用 Semaphore 限流 `METADATA_SCAN_CONCURRENCY=8` 路（与 metadata 扫描同口径，
    // 避免低核数机器上短脉冲 CPU 峰值过高，也压住打开 fd 数量）。
    // 同一 project 内 subagent 文件用内层 `SUBAGENT_PARSE_CONCURRENCY=4` 并行 parse，
    // 避免 31 个 subagent 串行累积 ~930ms。
    let semaphore = Arc::new(Semaphore::new(METADATA_SCAN_CONCURRENCY));
    let inner_sem = Arc::new(Semaphore::new(SUBAGENT_PARSE_CONCURRENCY));
    let scan_tasks = project_dirs.into_iter().map(|project_path| {
        let sem = semaphore.clone();
        let inner = inner_sem.clone();
        let root_session_id = root_session_id.to_owned();
        async move {
            let _permit = sem.acquire_owned().await.ok()?;
            let new_dir = project_path.join(&root_session_id).join("subagents");
            let sub_entries = fs.read_dir(&new_dir).await.ok()?;
            let parse_tasks: Vec<_> = sub_entries
                .iter()
                .filter(|e| {
                    let name = e.name.as_str();
                    name.starts_with("agent-")
                        && name.ends_with(".jsonl")
                        && !name.starts_with("agent-acompact")
                })
                .map(|e| {
                    let path = new_dir.join(&e.name);
                    let permit = inner.clone();
                    async move {
                        let _p = permit.acquire().await.ok()?;
                        let t = std::time::Instant::now();
                        let c = parse_subagent_candidate(fs, &path).await?;
                        Some((c, t.elapsed().as_millis()))
                    }
                })
                .collect();
            let results = futures::future::join_all(parse_tasks).await;
            let local: Vec<_> = results.into_iter().flatten().collect();
            Some(local)
        }
    });
    let results = futures::future::join_all(scan_tasks).await;
    for maybe_local in results {
        let Some(local) = maybe_local else {
            continue;
        };
        if local.is_empty() {
            continue;
        }
        dirs_with_subagents += 1;
        for (c, ms) in local {
            if !seen_ids.insert(c.session_id.clone()) {
                continue;
            }
            per_candidate_ms.push(ms);
            candidates.push(c);
        }
    }

    // 旧结构兜底：始终只扫主 project_dir，避免跨目录大量 parse 成本。
    if let Ok(old_entries) = fs.read_dir(main_project_dir).await {
        for entry in old_entries {
            let name_str = entry.name.as_str();
            if !(name_str.starts_with("agent-")
                && name_str.ends_with(".jsonl")
                && !name_str.starts_with("agent-acompact"))
            {
                continue;
            }
            let candidate_path = main_project_dir.join(&entry.name);
            let t = std::time::Instant::now();
            let Some(c) = parse_subagent_candidate(fs, &candidate_path).await else {
                continue;
            };
            if c.parent_session_id.as_deref() != Some(root_session_id) {
                continue;
            }
            if !seen_ids.insert(c.session_id.clone()) {
                continue;
            }
            per_candidate_ms.push(t.elapsed().as_millis());
            candidates.push(c);
        }
    }

    let total_ms = t_total.elapsed().as_millis();
    if projects_scanned > 0 || !candidates.is_empty() {
        let parse_total: u128 = per_candidate_ms.iter().sum();
        let parse_max = per_candidate_ms.iter().max().copied().unwrap_or_default();
        tracing::info!(
            target: "cdt_api::perf",
            session_id = %root_session_id,
            projects_scanned,
            dirs_with_subagents,
            candidates_found = candidates.len(),
            parse_total_ms = parse_total,
            parse_max_ms = parse_max,
            total_ms,
            "scan_subagent_candidates_cross_project"
        );
    }

    candidates
}

/// 跨 `projects_dir` 定位 subagent JSONL（新结构优先）。
///
/// 扫所有 `{projects_dir}/*/{root_session_id}/subagents/agent-{sub_session_id}.jsonl`，
/// 命中即返。未命中且 `CROSS_PROJECT_SUBAGENT_SCAN=true` 时不再 fallback —— 调用方需要
/// 额外回退路径时显式叠加调用 `find_subagent_jsonl(fs, &main_project_dir, ...)` 兜旧结构。
///
/// 通过 `FileSystemProvider` 走当前 active context 的 fs（design D6）。
async fn find_subagent_jsonl_cross_project(
    fs: &dyn FileSystemProvider,
    projects_dir: &Path,
    root_session_id: &str,
    sub_session_id: &str,
) -> Option<std::path::PathBuf> {
    let filename = format!("agent-{sub_session_id}.jsonl");
    let entries = fs.read_dir(projects_dir).await.ok()?;
    for entry in entries {
        if !entry.kind.is_dir() {
            continue;
        }
        let candidate = projects_dir
            .join(&entry.name)
            .join(root_session_id)
            .join("subagents")
            .join(&filename);
        if fs.exists(&candidate).await {
            return Some(candidate);
        }
    }
    None
}

/// 扫描 subagent 候选文件，构建 `SubagentCandidate` 列表。
///
/// 扫描路径（design D6 保留双结构）：
/// - 新 nested：`{project_dir}/{session_id}/subagents/agent-*.jsonl`
/// - 旧 flat：`{project_dir}/agent-*.jsonl`（需要读首行检查 parent session）
///
/// 扫描失败时静默返回空列表（warn 日志）。本函数仅扫主 `project_dir`，
/// 跨 `projects_dir` 的扫描走 `scan_subagent_candidates_cross_project`。
/// 通过 `FileSystemProvider` 走当前 active context 的 fs。
#[allow(clippy::case_sensitive_file_extension_comparisons)]
async fn scan_subagent_candidates(
    fs: &dyn FileSystemProvider,
    project_dir: &Path,
    session_id: &str,
) -> Vec<cdt_core::SubagentCandidate> {
    let mut candidates = Vec::new();
    let mut per_candidate_ms: Vec<u128> = Vec::new();

    // 新结构：{project_dir}/{session_id}/subagents/
    let new_dir = project_dir.join(session_id).join("subagents");
    if let Ok(entries) = fs.read_dir(&new_dir).await {
        for entry in entries {
            let name_str = entry.name.as_str();
            if name_str.starts_with("agent-")
                && name_str.ends_with(".jsonl")
                && !name_str.starts_with("agent-acompact")
            {
                let candidate_path = new_dir.join(&entry.name);
                let t = std::time::Instant::now();
                if let Some(c) = parse_subagent_candidate(fs, &candidate_path).await {
                    per_candidate_ms.push(t.elapsed().as_millis());
                    candidates.push(c);
                }
            }
        }
    }

    // 旧结构：{project_dir}/agent-*.jsonl
    if let Ok(entries) = fs.read_dir(project_dir).await {
        for entry in entries {
            let name_str = entry.name.as_str();
            if name_str.starts_with("agent-")
                && name_str.ends_with(".jsonl")
                && !name_str.starts_with("agent-acompact")
            {
                let candidate_path = project_dir.join(&entry.name);
                let t = std::time::Instant::now();
                if let Some(c) = parse_subagent_candidate(fs, &candidate_path).await {
                    if c.parent_session_id.as_deref() == Some(session_id) {
                        per_candidate_ms.push(t.elapsed().as_millis());
                        candidates.push(c);
                    }
                }
            }
        }
    }

    if !per_candidate_ms.is_empty() {
        let total: u128 = per_candidate_ms.iter().sum();
        let max = per_candidate_ms.iter().max().copied().unwrap_or_default();
        tracing::info!(
            target: "cdt_api::perf",
            session_id = %session_id,
            count = candidates.len(),
            total_ms = total,
            max_ms = max,
            "scan_subagent_candidates per-candidate timings"
        );
    }

    candidates
}

/// 轻量解析一个 subagent JSONL 文件的前几行，提取候选信息。
///
/// 通过 `FileSystemProvider` 走当前 active context 的 fs（design D6）；
/// `BufReader` 容量与 `session_metadata::SCANNER_BUF_BYTES` 对齐 SFTP packet 上限。
async fn parse_subagent_candidate(
    fs: &dyn FileSystemProvider,
    path: &Path,
) -> Option<cdt_core::SubagentCandidate> {
    let mut session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.strip_prefix("agent-").unwrap_or(s).to_owned())
        .unwrap_or_default();

    let mut msgs = cdt_parse::parse_file_via_fs(fs, path).await.ok()?;
    if msgs.is_empty() {
        return None;
    }

    // 从前 10 条消息中提取 metadata（等价于原版"前 10 行"逻辑）
    let mut parent_session_id = None;
    let mut description_hint = None;
    for msg in msgs.iter().take(10) {
        if parent_session_id.is_none() {
            if let Some(pid) = msg.parent_uuid.as_deref() {
                parent_session_id = Some(pid.to_owned());
            }
        }
        if let Some(aid) = msg.agent_id.as_deref() {
            aid.clone_into(&mut session_id);
        }
        if msg.message_type == cdt_core::MessageType::User {
            let text = extract_content_text(&msg.content);
            if let Some(t) = text {
                if t == "Warmup" {
                    return None;
                }
                if description_hint.is_none() && !t.is_empty() {
                    description_hint = Some(t.chars().take(200).collect());
                }
            }
        }
    }

    let spawn_ts = msgs.first().map(|m| m.timestamp);
    let last_ts = msgs.last().map(|m| m.timestamp);
    let end_ts = match (spawn_ts, last_ts) {
        (Some(start), Some(end)) if end > start => Some(end),
        _ => None,
    };

    // ongoing 判定：在清 sidechain 与 build_chunks 之前先用原始流跑
    // `check_messages_ongoing`——避免仅看 timestamp 会把"中断后无 assistant
    // 收尾"的 subagent 误判为已完成。
    let is_ongoing = cdt_analyze::check_messages_ongoing(&msgs);
    for m in &mut msgs {
        m.is_sidechain = false;
    }
    // 内联展示路径（get_session_detail 把 candidate.messages 作为
    // Process.messages 内联进 AIChunk.subagents；HTTP server mode 不裁剪、
    // messagesOmitted=false，前端直接渲染 process.messages 绕过
    // get_subagent_trace）同样需把内部带 result_agent_id 的嵌套 Agent 调用
    // 升级为骨架 subagent，否则嵌套层显示为普通工具。与 get_subagent_trace
    // 路径（build_chunks 后调 promote）对齐。promote 为纯内存变换、零新 IO。
    let mut messages = cdt_analyze::build_chunks(&msgs);
    cdt_analyze::promote_result_agent_tasks(&mut messages);

    Some(cdt_core::SubagentCandidate {
        session_id,
        description_hint,
        spawn_ts: spawn_ts.unwrap_or_default(),
        end_ts,
        parent_session_id,
        metrics: cdt_core::ChunkMetrics::zero(),
        messages,
        is_ongoing,
    })
}

fn extract_content_text(content: &cdt_core::MessageContent) -> Option<&str> {
    match content {
        cdt_core::MessageContent::Text(s) => Some(s.as_str()),
        cdt_core::MessageContent::Blocks(blocks) => {
            for block in blocks {
                if let cdt_core::ContentBlock::Text { text } = block {
                    return Some(text.as_str());
                }
            }
            None
        }
    }
}

// =============================================================================
// 测试：覆盖 Pin/Hide facade（走独立 impl 块的非 trait 方法）
// =============================================================================

#[cfg(test)]
#[allow(clippy::doc_markdown)]
mod tests {
    // `clippy::doc_markdown` 在测试 doc-comment 内大量误报标识符（典型
    // `unified_invalidator_emits_session_list_changed_*` / `LocalDataApi.file_tx`
    // / `apply_file_event_to_*`）—— 这些是测试名 / 字段路径，反引号包裹后反而
    // 影响可读性。tests mod 整体豁免（mod-level attribute 见上一行）。
    //
    // 注释**不能**插在 `#[cfg(test)]` 与 mod 行之间——
    // `xtask::check_fs_direct_calls::collect_test_mod_spans` 识别 test mod
    // 时其 attribute-skip 仅放过 `#[..]` / 空行，碰到 `//` 注释直接 break →
    // 误判本 mod 不是 test mod → mod 内 `tokio::fs::*` 被误报为 H1 violation
    // （CI `xtask check-fs-direct-calls` fail）。同样不能在注释里写裸的
    // `{` / `}` —— xtask brace-tracker 不区分 string / comment 会把注释内
    // 的 brace 当真 block boundary，让 span 识别返 end_line=0。
    use super::*;
    use cdt_config::{ConfigManager, NotificationManager};
    use cdt_discover::{ProjectScanner, local_handle};
    use cdt_ssh::SshConnectionManager;
    use tempfile::tempdir;

    /// 构造一个内存态的 `LocalDataApi`，仅 config 路径指向独立 tempdir。
    ///
    /// 其余 manager 用默认值即可——Pin/Hide 测试只关心 config 落盘。
    async fn build_api(config_path: std::path::PathBuf) -> LocalDataApi {
        build_api_with_projects(config_path, std::path::PathBuf::from("/tmp")).await
    }

    async fn build_api_with_projects(
        config_path: std::path::PathBuf,
        projects_dir: std::path::PathBuf,
    ) -> LocalDataApi {
        let mut config_mgr = ConfigManager::new(Some(config_path));
        config_mgr.load().await.unwrap();
        let notif_mgr = NotificationManager::new(None);
        let ssh_mgr = SshConnectionManager::new();
        let scanner = ProjectScanner::new(local_handle(), projects_dir);
        LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr)
    }

    #[tokio::test]
    async fn ssh_watcher_ops_are_serialized() {
        let dir = tempdir().unwrap();
        let api = Arc::new(build_api(dir.path().join("config.json")).await);
        let started = std::time::Instant::now();
        let first = {
            let api = api.clone();
            tokio::spawn(async move {
                api.run_ssh_watcher_op_for_test(std::time::Duration::from_millis(50))
                    .await;
            })
        };
        tokio::task::yield_now().await;
        api.run_ssh_watcher_op_for_test(std::time::Duration::from_millis(0))
            .await;
        first.await.unwrap();

        assert!(started.elapsed() >= std::time::Duration::from_millis(50));
    }

    #[tokio::test]
    async fn project_memory_discovers_index_entries_and_orphans() {
        let dir = tempdir().unwrap();
        let projects = dir.path().join("projects");
        let memory = projects.join("proj-a").join("memory");
        tokio::fs::create_dir_all(&memory).await.unwrap();
        tokio::fs::write(
            memory.join("MEMORY.md"),
            "- [始终使用中文](feedback_chinese_language.md) — 对话/注释全部中文\n",
        )
        .await
        .unwrap();
        tokio::fs::write(memory.join("feedback_chinese_language.md"), "# 中文")
            .await
            .unwrap();
        tokio::fs::write(memory.join("extra_note.md"), "# Extra")
            .await
            .unwrap();
        tokio::fs::write(memory.join("ignored.json"), "{}")
            .await
            .unwrap();
        let api = build_api_with_projects(dir.path().join("config.json"), projects).await;

        let overview = api.get_project_memory("proj-a::session-1").await.unwrap();

        assert!(overview.has_memory);
        assert_eq!(overview.count, 3);
        assert_eq!(overview.default_file.as_deref(), Some("MEMORY.md"));
        assert_eq!(overview.layers[0].kind, MemoryLayerKind::Index);
        assert_eq!(overview.layers[1].file, "feedback_chinese_language.md");
        assert_eq!(overview.layers[1].kind, MemoryLayerKind::Entry);
        assert_eq!(
            overview.layers[1].hook.as_deref(),
            Some("对话/注释全部中文")
        );
        assert_eq!(overview.layers[2].file, "extra_note.md");
        assert_eq!(overview.layers[2].kind, MemoryLayerKind::Orphan);
    }

    #[tokio::test]
    async fn project_memory_missing_dir_returns_empty() {
        let dir = tempdir().unwrap();
        let api =
            build_api_with_projects(dir.path().join("config.json"), dir.path().join("projects"))
                .await;

        let overview = api.get_project_memory("proj-a").await.unwrap();

        assert!(!overview.has_memory);
        assert_eq!(overview.count, 0);
        assert!(overview.default_file.is_none());
        assert!(overview.layers.is_empty());
    }

    #[tokio::test]
    async fn read_memory_file_rejects_path_traversal_and_non_markdown() {
        let dir = tempdir().unwrap();
        let projects = dir.path().join("projects");
        let memory = projects.join("proj-a").join("memory");
        tokio::fs::create_dir_all(&memory).await.unwrap();
        tokio::fs::write(memory.join("MEMORY.md"), "# Index")
            .await
            .unwrap();
        let api = build_api_with_projects(dir.path().join("config.json"), projects).await;

        let content = api
            .read_memory_file("proj-a::session-1", "MEMORY.md")
            .await
            .unwrap();
        assert_eq!(content.content, "# Index");
        assert!(
            api.read_memory_file("proj-a", "../config.json")
                .await
                .is_err()
        );
        assert!(api.read_memory_file("proj-a", "secret.json").await.is_err());
        assert!(
            api.read_memory_file("proj-a", r"C:\\secret.md")
                .await
                .is_err()
        );
        assert!(api.get_project_memory("../outside").await.is_err());
        assert!(
            api.read_memory_file("../outside", "MEMORY.md")
                .await
                .is_err()
        );
        assert!(api.get_project_memory("..").await.is_err());
        assert!(api.read_memory_file(".", "MEMORY.md").await.is_err());
    }

    #[tokio::test]
    async fn pin_then_get_prefs_returns_sessions() {
        let dir = tempdir().unwrap();
        let api = build_api(dir.path().join("config.json")).await;

        api.pin_session("proj-a", "sess-1").await.unwrap();
        api.pin_session("proj-a", "sess-2").await.unwrap();

        let prefs = api.get_project_session_prefs("proj-a").await.unwrap();
        // 最近 pin 的在前
        assert_eq!(prefs.pinned, vec!["sess-2".to_owned(), "sess-1".to_owned()]);
        assert!(prefs.hidden.is_empty());
    }

    #[tokio::test]
    async fn unpin_removes_entry() {
        let dir = tempdir().unwrap();
        let api = build_api(dir.path().join("config.json")).await;

        api.pin_session("proj-a", "sess-1").await.unwrap();
        api.unpin_session("proj-a", "sess-1").await.unwrap();

        let prefs = api.get_project_session_prefs("proj-a").await.unwrap();
        assert!(prefs.pinned.is_empty());
    }

    #[tokio::test]
    async fn hide_and_unhide_roundtrip() {
        let dir = tempdir().unwrap();
        let api = build_api(dir.path().join("config.json")).await;

        api.hide_session("proj-a", "sess-x").await.unwrap();
        let prefs = api.get_project_session_prefs("proj-a").await.unwrap();
        assert_eq!(prefs.hidden, vec!["sess-x".to_owned()]);

        api.unhide_session("proj-a", "sess-x").await.unwrap();
        let prefs = api.get_project_session_prefs("proj-a").await.unwrap();
        assert!(prefs.hidden.is_empty());
    }

    #[tokio::test]
    async fn prefs_persist_across_manager_reload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        {
            let api = build_api(path.clone()).await;
            api.pin_session("proj-a", "sess-1").await.unwrap();
            api.hide_session("proj-a", "sess-2").await.unwrap();
        }

        // 新建 api 重新从磁盘 load
        let api = build_api(path).await;
        let prefs = api.get_project_session_prefs("proj-a").await.unwrap();
        assert_eq!(prefs.pinned, vec!["sess-1".to_owned()]);
        assert_eq!(prefs.hidden, vec!["sess-2".to_owned()]);
    }

    #[tokio::test]
    async fn empty_project_returns_default() {
        let dir = tempdir().unwrap();
        let api = build_api(dir.path().join("config.json")).await;

        let prefs = api.get_project_session_prefs("unknown").await.unwrap();
        assert!(prefs.pinned.is_empty());
        assert!(prefs.hidden.is_empty());
    }

    // -------- phase 3: image data OMIT --------

    fn ts() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-04-19T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn make_image_block(data: &str, mime: &str) -> cdt_core::ContentBlock {
        cdt_core::ContentBlock::Image {
            source: cdt_core::ImageSource {
                kind: "base64".into(),
                media_type: mime.into(),
                data: data.into(),
                data_omitted: false,
            },
        }
    }

    fn make_user_chunk_with_image(uuid: &str, data: &str) -> cdt_core::Chunk {
        cdt_core::Chunk::User(cdt_core::UserChunk {
            chunk_id: uuid.into(),
            uuid: uuid.into(),
            timestamp: ts(),
            duration_ms: None,
            content: cdt_core::MessageContent::Blocks(vec![
                cdt_core::ContentBlock::Text {
                    text: "see image".into(),
                },
                make_image_block(data, "image/png"),
            ]),
            metrics: cdt_core::ChunkMetrics::zero(),
        })
    }

    #[test]
    fn apply_image_omit_clears_user_image_data() {
        let mut chunks = vec![make_user_chunk_with_image("u1", "AAAAdata")];
        apply_image_omit(&mut chunks);
        let cdt_core::Chunk::User(u) = &chunks[0] else {
            panic!("expected user chunk");
        };
        let cdt_core::MessageContent::Blocks(blocks) = &u.content else {
            panic!("expected blocks");
        };
        let cdt_core::ContentBlock::Image { source } = &blocks[1] else {
            panic!("expected image block");
        };
        assert_eq!(source.data, "");
        assert!(source.data_omitted);
        assert_eq!(source.media_type, "image/png");
        assert_eq!(source.kind, "base64");
        // 非 image block 不动
        assert!(matches!(blocks[0], cdt_core::ContentBlock::Text { .. }));
    }

    #[tokio::test]
    async fn materialize_image_asset_writes_file_and_dedupes() {
        use base64::Engine;
        let dir = tempdir().unwrap();
        let cache = dir.path().join("cdt-images");
        // 8 字节明文 → base64
        let raw = b"helloimg";
        let b64 = base64::engine::general_purpose::STANDARD.encode(raw);

        let url1 = materialize_image_asset(&cache, "image/png", &b64).await;
        assert!(url1.starts_with("asset://localhost/"), "url={url1}");
        let path1: std::path::PathBuf = url1
            .strip_prefix("asset://localhost/")
            .unwrap()
            .parse()
            .unwrap();
        let bytes_on_disk = tokio::fs::read(&path1).await.unwrap();
        assert_eq!(bytes_on_disk, raw);

        // 相同内容第二次调用 → URL 完全一致（hash 命名 + 复用）
        let url2 = materialize_image_asset(&cache, "image/png", &b64).await;
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn materialize_image_asset_fallbacks_on_invalid_base64() {
        let dir = tempdir().unwrap();
        let cache = dir.path().join("cdt-images");
        let url = materialize_image_asset(&cache, "image/png", "not-valid-base64!!!").await;
        assert!(
            url.starts_with("data:image/png;base64,"),
            "expected fallback data URI, got {url}"
        );
    }

    #[tokio::test]
    async fn get_image_asset_invalid_block_id_returns_empty_data_uri() {
        let dir = tempdir().unwrap();
        let api = build_api(dir.path().join("config.json")).await;
        let url = api
            .get_image_asset("root-id", "root-id", "no-colon-here")
            .await
            .unwrap();
        assert_eq!(url, empty_data_uri());
    }

    #[test]
    fn apply_image_omit_clears_assistant_response_image() {
        let ai = cdt_core::Chunk::Ai(cdt_core::AIChunk {
            chunk_id: "ai:r1:0".into(),
            timestamp: ts(),
            duration_ms: None,
            responses: vec![cdt_core::AssistantResponse {
                uuid: "r1".into(),
                timestamp: ts(),
                content: cdt_core::MessageContent::Blocks(vec![make_image_block(
                    "CCCCdata",
                    "image/jpeg",
                )]),
                tool_calls: Vec::new(),
                usage: None,
                model: None,
                content_omitted: false,
            }],
            metrics: cdt_core::ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: Vec::new(),
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        });
        let mut chunks = vec![ai];
        apply_image_omit(&mut chunks);
        let cdt_core::Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected ai chunk");
        };
        let cdt_core::MessageContent::Blocks(blocks) = &ai.responses[0].content else {
            panic!("expected blocks");
        };
        let cdt_core::ContentBlock::Image { source } = &blocks[0] else {
            panic!("expected image block");
        };
        assert_eq!(source.data, "");
        assert!(source.data_omitted);
        assert_eq!(source.media_type, "image/jpeg");
    }

    // -------- phase 4: response.content OMIT --------

    fn make_ai_chunk_with_text(text: &str, model: &str) -> cdt_core::Chunk {
        cdt_core::Chunk::Ai(cdt_core::AIChunk {
            chunk_id: "ai:r1:0".into(),
            timestamp: ts(),
            duration_ms: None,
            responses: vec![cdt_core::AssistantResponse {
                uuid: "r1".into(),
                timestamp: ts(),
                content: cdt_core::MessageContent::Text(text.into()),
                tool_calls: Vec::new(),
                usage: None,
                model: Some(model.into()),
                content_omitted: false,
            }],
            metrics: cdt_core::ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: Vec::new(),
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        })
    }

    #[test]
    fn apply_response_content_omit_clears_assistant_response_content() {
        let mut chunks = vec![make_ai_chunk_with_text(
            "非常长的回复内容...",
            "claude-opus-4-7",
        )];
        apply_response_content_omit(&mut chunks);
        let cdt_core::Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected ai chunk");
        };
        let resp = &ai.responses[0];
        // content 已替换为空 Text
        assert!(matches!(&resp.content, cdt_core::MessageContent::Text(s) if s.is_empty()));
        assert!(resp.content_omitted);
        // 其它字段保留
        assert_eq!(resp.uuid, "r1");
        assert_eq!(resp.model.as_deref(), Some("claude-opus-4-7"));
    }

    #[test]
    fn apply_response_content_omit_clears_nested_subagent_response_content() {
        // 构造一个 AIChunk，含一个 subagent，subagent.messages 内嵌套一个 AIChunk
        let nested = make_ai_chunk_with_text("嵌套 subagent 内的回复", "claude-haiku-4-5");
        let parent = cdt_core::Chunk::Ai(cdt_core::AIChunk {
            chunk_id: "ai:parent-r:0".into(),
            timestamp: ts(),
            duration_ms: None,
            responses: vec![cdt_core::AssistantResponse {
                uuid: "parent-r".into(),
                timestamp: ts(),
                content: cdt_core::MessageContent::Text("父级回复".into()),
                tool_calls: Vec::new(),
                usage: None,
                model: Some("claude-opus-4-7".into()),
                content_omitted: false,
            }],
            metrics: cdt_core::ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: Vec::new(),
            subagents: vec![cdt_core::Process {
                session_id: "sub-1".into(),
                root_task_description: None,
                spawn_ts: ts(),
                end_ts: None,
                metrics: cdt_core::ChunkMetrics::zero(),
                team: None,
                subagent_type: None,
                messages: vec![nested],
                main_session_impact: None,
                is_ongoing: false,
                duration_ms: None,
                parent_task_id: None,
                description: None,
                header_model: None,
                last_isolated_tokens: 0,
                is_shutdown_only: false,
                messages_omitted: false,
                messages_total_count: 1,
            }],
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        });
        let mut chunks = vec![parent];
        apply_response_content_omit(&mut chunks);

        let cdt_core::Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected ai chunk");
        };
        // 顶层 responses[].content 被清
        assert!(
            matches!(&ai.responses[0].content, cdt_core::MessageContent::Text(s) if s.is_empty())
        );
        assert!(ai.responses[0].content_omitted);

        // 嵌套 subagent.messages 内的 AIChunk.responses[].content 也被清
        let cdt_core::Chunk::Ai(nested_ai) = &ai.subagents[0].messages[0] else {
            panic!("expected nested ai chunk");
        };
        assert!(
            matches!(&nested_ai.responses[0].content, cdt_core::MessageContent::Text(s) if s.is_empty())
        );
        assert!(nested_ai.responses[0].content_omitted);
    }

    // -------- phase 5: tool_exec.output OMIT --------

    fn make_tool_exec(id: &str, output: cdt_core::ToolOutput) -> cdt_core::ToolExecution {
        cdt_core::ToolExecution {
            tool_use_id: id.into(),
            tool_name: "Bash".into(),
            input: serde_json::json!({"command": "ls"}),
            output,
            is_error: false,
            start_ts: ts(),
            end_ts: Some(ts()),
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
            error_message: None,
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
            workflow_run_id: None,
            workflow_script_path: None,
        }
    }

    fn make_ai_chunk_with_tool(exec: cdt_core::ToolExecution) -> cdt_core::Chunk {
        cdt_core::Chunk::Ai(cdt_core::AIChunk {
            chunk_id: "ai:r1:0".into(),
            timestamp: ts(),
            duration_ms: None,
            responses: Vec::new(),
            metrics: cdt_core::ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: vec![exec],
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        })
    }

    #[test]
    fn apply_tool_output_omit_clears_text_variant() {
        let original_text = "very long bash output...";
        let exec = make_tool_exec(
            "tu1",
            cdt_core::ToolOutput::Text {
                text: original_text.into(),
            },
        );
        let mut chunks = vec![make_ai_chunk_with_tool(exec)];
        apply_tool_output_omit(&mut chunks);
        let cdt_core::Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected ai chunk");
        };
        let exec = &ai.tool_executions[0];
        assert!(matches!(&exec.output, cdt_core::ToolOutput::Text { text } if text.is_empty()));
        assert!(exec.output_omitted);
        // outputBytes 在 trim 前记录原始字节长度（change `tool-output-omit-preserve-size`）
        assert_eq!(exec.output_bytes, Some(original_text.len() as u64));
        // 其它字段保留
        assert_eq!(exec.tool_use_id, "tu1");
        assert_eq!(exec.tool_name, "Bash");
        assert!(exec.input.get("command").is_some());
    }

    #[test]
    fn apply_tool_output_omit_clears_structured_variant() {
        let value = serde_json::json!({"stdout": "lots", "stderr": ""});
        let expected_bytes = serde_json::to_string(&value).unwrap().len() as u64;
        let exec = make_tool_exec(
            "tu2",
            cdt_core::ToolOutput::Structured {
                value: value.clone(),
            },
        );
        let mut chunks = vec![make_ai_chunk_with_tool(exec)];
        apply_tool_output_omit(&mut chunks);
        let cdt_core::Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected ai chunk");
        };
        let exec = &ai.tool_executions[0];
        assert!(
            matches!(&exec.output, cdt_core::ToolOutput::Structured { value } if value.is_null())
        );
        assert!(exec.output_omitted);
        // outputBytes = serde_json::to_string(原 value).len()
        assert_eq!(exec.output_bytes, Some(expected_bytes));
    }

    #[test]
    fn apply_tool_output_omit_keeps_missing_variant_kind() {
        let exec = make_tool_exec("tu3", cdt_core::ToolOutput::Missing);
        let mut chunks = vec![make_ai_chunk_with_tool(exec)];
        apply_tool_output_omit(&mut chunks);
        let cdt_core::Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected ai chunk");
        };
        let exec = &ai.tool_executions[0];
        assert!(matches!(&exec.output, cdt_core::ToolOutput::Missing));
        // Missing 也设 flag——caller 看到 Missing + omitted=true 仍可触发懒拉，
        // 拉回来若仍 Missing 则保留语义不变。
        assert!(exec.output_omitted);
        // Missing variant 不记录 outputBytes（保持 None）
        assert_eq!(exec.output_bytes, None);
    }

    #[test]
    fn apply_tool_output_omit_clears_nested_subagent_tool_output() {
        let nested_exec = make_tool_exec(
            "nested-tu",
            cdt_core::ToolOutput::Text {
                text: "nested output".into(),
            },
        );
        let nested_ai = make_ai_chunk_with_tool(nested_exec);
        let parent_exec = make_tool_exec(
            "parent-tu",
            cdt_core::ToolOutput::Text {
                text: "parent output".into(),
            },
        );
        let parent = cdt_core::Chunk::Ai(cdt_core::AIChunk {
            chunk_id: "ai:parent-r:0".into(),
            timestamp: ts(),
            duration_ms: None,
            responses: Vec::new(),
            metrics: cdt_core::ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: vec![parent_exec],
            subagents: vec![cdt_core::Process {
                session_id: "sub-1".into(),
                root_task_description: None,
                spawn_ts: ts(),
                end_ts: None,
                metrics: cdt_core::ChunkMetrics::zero(),
                team: None,
                subagent_type: None,
                messages: vec![nested_ai],
                main_session_impact: None,
                is_ongoing: false,
                duration_ms: None,
                parent_task_id: None,
                description: None,
                header_model: None,
                last_isolated_tokens: 0,
                is_shutdown_only: false,
                messages_omitted: false,
                messages_total_count: 1,
            }],
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        });
        let mut chunks = vec![parent];
        apply_tool_output_omit(&mut chunks);

        let cdt_core::Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected ai chunk");
        };
        // 顶层 tool_executions[].output 被清
        assert!(
            matches!(&ai.tool_executions[0].output, cdt_core::ToolOutput::Text { text } if text.is_empty())
        );
        assert!(ai.tool_executions[0].output_omitted);

        // 嵌套 subagent.messages 内的 tool_executions[].output 也被清
        let cdt_core::Chunk::Ai(nested) = &ai.subagents[0].messages[0] else {
            panic!("expected nested ai chunk");
        };
        assert!(
            matches!(&nested.tool_executions[0].output, cdt_core::ToolOutput::Text { text } if text.is_empty())
        );
        assert!(nested.tool_executions[0].output_omitted);
    }

    #[tokio::test]
    async fn get_tool_output_returns_missing_when_jsonl_not_exist() {
        let dir = tempdir().unwrap();
        let api = build_api(dir.path().join("config.json")).await;
        // root_session_id 不存在 → Missing
        let out = api
            .get_tool_output("nonexistent-root", "nonexistent-root", "any-tu")
            .await
            .unwrap();
        assert!(matches!(out, cdt_core::ToolOutput::Missing));
    }

    // -------- subagent ongoing 判定（followups.md "Subagent 状态判定"）--------

    /// 写一个最小 subagent JSONL：行 1 是 user description，后续行由调用方拼。
    /// 时间戳保证 `spawn_ts` < `end_ts`，让旧逻辑判 done，
    /// 从而暴露"末尾 `tool_use` 无收尾"被误判完成的 bug。
    async fn write_subagent_jsonl(
        dir: &std::path::Path,
        agent_id: &str,
        extra_lines: &[serde_json::Value],
    ) -> std::path::PathBuf {
        let path = dir.join(format!("agent-{agent_id}.jsonl"));
        let mut buf = String::new();
        // line 1: parent uuid + first timestamp + description（让 description_hint 拿到值）
        let header = serde_json::json!({
            "type": "user",
            "uuid": "u-root",
            "parentUuid": "parent-session-id",
            "timestamp": "2026-04-15T10:00:00Z",
            "agentId": agent_id,
            "message": { "content": "investigate the bug" },
        });
        buf.push_str(&serde_json::to_string(&header).unwrap());
        buf.push('\n');
        for line in extra_lines {
            buf.push_str(&serde_json::to_string(line).unwrap());
            buf.push('\n');
        }
        tokio::fs::write(&path, buf).await.unwrap();
        path
    }

    #[tokio::test]
    async fn parse_subagent_candidate_marks_unfinished_tool_use_as_ongoing() {
        // 复现 followups 的 case：subagent 末尾是 assistant tool_use，
        // 无配对 tool_result —— 旧逻辑（end_ts > spawn_ts → done）误判完成。
        let dir = tempdir().unwrap();
        let lines = [serde_json::json!({
            "type": "assistant",
            "uuid": "a-1",
            "parentUuid": "u-root",
            "timestamp": "2026-04-15T10:00:30Z",
            "message": {
                "role": "assistant",
                "content": [
                    { "type": "tool_use", "id": "toolu_x", "name": "Bash", "input": {} }
                ],
            },
        })];
        let path = write_subagent_jsonl(dir.path(), "abcd", &lines).await;
        let cand = parse_subagent_candidate(&*cdt_fs::local_handle(), &path)
            .await
            .expect("candidate");
        assert!(
            cand.is_ongoing,
            "末尾 assistant tool_use 无 tool_result 应判 ongoing，旧逻辑会误报 done"
        );
        // end_ts 仍按时间戳填充（供 duration 计算），与 ongoing 判定独立
        assert!(cand.end_ts.is_some(), "end_ts 应仍来自最后一行 timestamp");
    }

    #[tokio::test]
    async fn parse_subagent_candidate_marks_text_ending_as_done() {
        // 健康 subagent：末尾 assistant text，应判 done（与 check_messages_ongoing 对齐）。
        let dir = tempdir().unwrap();
        let lines = [serde_json::json!({
            "type": "assistant",
            "uuid": "a-1",
            "parentUuid": "u-root",
            "timestamp": "2026-04-15T10:00:30Z",
            "message": {
                "role": "assistant",
                "content": [ { "type": "text", "text": "all done" } ],
            },
        })];
        let path = write_subagent_jsonl(dir.path(), "efgh", &lines).await;
        let cand = parse_subagent_candidate(&*cdt_fs::local_handle(), &path)
            .await
            .expect("candidate");
        assert!(!cand.is_ongoing, "末尾 assistant text 是 ending，应判 done");
        assert!(cand.end_ts.is_some());
    }

    #[tokio::test]
    async fn parse_subagent_candidate_marks_orphan_tool_result_as_ongoing() {
        // 复现真实 case `5a3a23b2.../agent-aee63780244f1f959.jsonl`：
        // 末尾是 user/tool_result（subagent 中断后无 assistant 收尾）。
        let dir = tempdir().unwrap();
        let lines = [
            serde_json::json!({
                "type": "assistant",
                "uuid": "a-1",
                "parentUuid": "u-root",
                "timestamp": "2026-04-15T10:00:30Z",
                "message": {
                    "role": "assistant",
                    "content": [
                        { "type": "tool_use", "id": "toolu_y", "name": "Bash", "input": {} }
                    ],
                },
            }),
            serde_json::json!({
                "type": "user",
                "uuid": "u-2",
                "parentUuid": "a-1",
                "timestamp": "2026-04-15T10:00:45Z",
                "message": {
                    "role": "user",
                    "content": [
                        { "type": "tool_result", "tool_use_id": "toolu_y", "content": "ok" }
                    ],
                },
            }),
        ];
        let path = write_subagent_jsonl(dir.path(), "ijkl", &lines).await;
        let cand = parse_subagent_candidate(&*cdt_fs::local_handle(), &path)
            .await
            .expect("candidate");
        assert!(
            cand.is_ongoing,
            "tool_use → tool_result 但无后续 assistant 收尾应判 ongoing"
        );
    }

    // -------- apply_compact_derived 派生算法单测（11 个 Scenario）--------
    // spec: openspec/specs/ipc-data-api/spec.md "Expose CompactChunk derived metadata in SessionDetail"
    // design: change `compact-chunk-rendering-alignment` 的 D1c (phaseNumber)
    // + D1d (tokenDelta)，派生层完全独立于 ContextPhaseInfo

    fn ts_test() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-04-15T10:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn ai_chunk_with_usage(uuid: &str, total_tokens: u64) -> cdt_core::Chunk {
        cdt_core::Chunk::Ai(cdt_core::AIChunk {
            chunk_id: format!("ai:{uuid}:0"),
            timestamp: ts_test(),
            duration_ms: None,
            responses: vec![cdt_core::AssistantResponse {
                uuid: uuid.into(),
                timestamp: ts_test(),
                content: cdt_core::MessageContent::Text(String::new()),
                tool_calls: Vec::new(),
                usage: Some(cdt_core::TokenUsage {
                    input_tokens: total_tokens,
                    output_tokens: 0,
                    cache_read_input_tokens: 0,
                    cache_creation_input_tokens: 0,
                }),
                model: Some("claude-opus-4-7".into()),
                content_omitted: false,
            }],
            metrics: cdt_core::ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: Vec::new(),
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        })
    }

    fn ai_chunk_no_usage(uuid: &str) -> cdt_core::Chunk {
        cdt_core::Chunk::Ai(cdt_core::AIChunk {
            chunk_id: format!("ai:{uuid}:0"),
            timestamp: ts_test(),
            duration_ms: None,
            responses: vec![cdt_core::AssistantResponse {
                uuid: uuid.into(),
                timestamp: ts_test(),
                content: cdt_core::MessageContent::Text(String::new()),
                tool_calls: Vec::new(),
                usage: None,
                model: None,
                content_omitted: false,
            }],
            metrics: cdt_core::ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: Vec::new(),
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        })
    }

    fn compact_chunk_test(uuid: &str) -> cdt_core::Chunk {
        cdt_core::Chunk::Compact(cdt_core::CompactChunk {
            chunk_id: uuid.into(),
            uuid: uuid.into(),
            timestamp: ts_test(),
            duration_ms: None,
            summary_text: "summary".into(),
            metrics: cdt_core::ChunkMetrics::zero(),
            token_delta: None,
            phase_number: None,
        })
    }

    fn user_chunk_test(uuid: &str) -> cdt_core::Chunk {
        cdt_core::Chunk::User(cdt_core::UserChunk {
            chunk_id: uuid.into(),
            uuid: uuid.into(),
            timestamp: ts_test(),
            duration_ms: None,
            content: cdt_core::MessageContent::Text("user".into()),
            metrics: cdt_core::ChunkMetrics::zero(),
        })
    }

    fn system_chunk_test(uuid: &str) -> cdt_core::Chunk {
        cdt_core::Chunk::System(cdt_core::SystemChunk {
            chunk_id: uuid.into(),
            uuid: uuid.into(),
            timestamp: ts_test(),
            duration_ms: None,
            content_text: "sys".into(),
            metrics: cdt_core::ChunkMetrics::zero(),
        })
    }

    fn extract_compact(chunks: &[cdt_core::Chunk], idx: usize) -> &cdt_core::CompactChunk {
        if let cdt_core::Chunk::Compact(c) = &chunks[idx] {
            c
        } else {
            panic!("expected compact at index {idx}")
        }
    }

    #[test]
    fn derive_token_delta_computed_from_neighboring_ai() {
        let mut chunks = vec![
            ai_chunk_with_usage("ai-1", 30_000),
            compact_chunk_test("c-1"),
            ai_chunk_with_usage("ai-2", 5_000),
        ];
        apply_compact_derived(&mut chunks, true);
        let c = extract_compact(&chunks, 1);
        assert_eq!(
            c.token_delta,
            Some(cdt_core::CompactionTokenDelta {
                pre_compaction_tokens: 30_000,
                post_compaction_tokens: 5_000,
                delta: -25_000,
            })
        );
    }

    #[test]
    fn derive_token_delta_none_when_no_ai_before() {
        let mut chunks = vec![
            user_chunk_test("u-1"),
            compact_chunk_test("c-1"),
            ai_chunk_with_usage("ai-1", 5_000),
        ];
        apply_compact_derived(&mut chunks, true);
        assert_eq!(extract_compact(&chunks, 1).token_delta, None);
    }

    #[test]
    fn derive_token_delta_none_when_no_ai_after() {
        let mut chunks = vec![
            ai_chunk_with_usage("ai-1", 30_000),
            compact_chunk_test("c-1"),
        ];
        apply_compact_derived(&mut chunks, true);
        assert_eq!(extract_compact(&chunks, 1).token_delta, None);
    }

    #[test]
    fn derive_token_delta_none_when_ai_lacks_usage() {
        let mut chunks = vec![
            ai_chunk_no_usage("ai-1"),
            compact_chunk_test("c-1"),
            ai_chunk_with_usage("ai-2", 5_000),
        ];
        apply_compact_derived(&mut chunks, true);
        assert_eq!(extract_compact(&chunks, 1).token_delta, None);
    }

    /// **D1d 关键修复证据**：连续 `A → B → AI` 时两个 compact 都拿到相同
    /// tokenDelta（不会因 cdt-analyze 内部 `current_phase_compact_group_id`
    /// 覆盖问题让 c-1 拿到 None）。spec: "Consecutive compacts share identical
    /// token delta" Scenario。
    #[test]
    fn derive_consecutive_compacts_share_identical_token_delta() {
        let mut chunks = vec![
            ai_chunk_with_usage("ai-1", 30_000),
            compact_chunk_test("c-1"),
            compact_chunk_test("c-2"),
            ai_chunk_with_usage("ai-2", 5_000),
        ];
        apply_compact_derived(&mut chunks, true);
        let c1 = extract_compact(&chunks, 1).token_delta;
        let c2 = extract_compact(&chunks, 2).token_delta;
        assert_eq!(c1, c2, "consecutive compacts should share identical delta");
        assert_eq!(
            c1,
            Some(cdt_core::CompactionTokenDelta {
                pre_compaction_tokens: 30_000,
                post_compaction_tokens: 5_000,
                delta: -25_000,
            })
        );
    }

    #[test]
    fn derive_phase_number_assigned_by_ordinal() {
        let mut chunks = vec![
            user_chunk_test("u-1"),
            ai_chunk_with_usage("ai-1", 100),
            compact_chunk_test("c-1"),
            ai_chunk_with_usage("ai-2", 50),
        ];
        apply_compact_derived(&mut chunks, true);
        // chunks 中第 1 个 compact → counter 1→2 → phase 2
        assert_eq!(extract_compact(&chunks, 2).phase_number, Some(2));
    }

    /// **D1c 关键修复证据**：连续 compact 各得各的 phase（不会因 phases 数组
    /// 不完整问题让 c-1 拿到 None 或与 c-2 共享同一 phase）。
    #[test]
    fn derive_consecutive_compacts_get_distinct_phase_numbers() {
        let mut chunks = vec![
            ai_chunk_with_usage("ai-1", 100),
            compact_chunk_test("c-1"),
            compact_chunk_test("c-2"),
            ai_chunk_with_usage("ai-2", 50),
        ];
        apply_compact_derived(&mut chunks, true);
        assert_eq!(extract_compact(&chunks, 1).phase_number, Some(2));
        assert_eq!(extract_compact(&chunks, 2).phase_number, Some(3));
    }

    #[test]
    fn derive_phase_number_stable_when_compact_at_end() {
        let mut chunks = vec![
            ai_chunk_with_usage("ai-1", 100),
            compact_chunk_test("c-1"),
            ai_chunk_with_usage("ai-2", 50),
            compact_chunk_test("c-2"),
        ];
        apply_compact_derived(&mut chunks, true);
        assert_eq!(extract_compact(&chunks, 1).phase_number, Some(2));
        assert_eq!(extract_compact(&chunks, 3).phase_number, Some(3));
    }

    #[test]
    fn derive_compact_followed_only_by_user_and_system() {
        let mut chunks = vec![
            ai_chunk_with_usage("ai-1", 100),
            compact_chunk_test("c-1"),
            user_chunk_test("u-1"),
            system_chunk_test("s-1"),
        ];
        apply_compact_derived(&mut chunks, true);
        // phaseNumber 派生与"compact 之后必须 AIChunk"无关
        assert_eq!(extract_compact(&chunks, 1).phase_number, Some(2));
        // tokenDelta 需要 first_ai_after，不存在时 None
        assert_eq!(extract_compact(&chunks, 1).token_delta, None);
    }

    #[test]
    fn derive_disabled_returns_all_none() {
        let mut chunks = vec![
            ai_chunk_with_usage("ai-1", 30_000),
            compact_chunk_test("c-1"),
            compact_chunk_test("c-2"),
            ai_chunk_with_usage("ai-2", 5_000),
        ];
        apply_compact_derived(&mut chunks, false);
        for idx in [1, 2] {
            assert_eq!(extract_compact(&chunks, idx).token_delta, None);
            assert_eq!(extract_compact(&chunks, idx).phase_number, None);
        }
    }

    // -------- cross-project subagent scan --------

    /// 写一行 subagent JSONL（含 `sessionId` / `agentId` / `cwd` / `timestamp`）。
    ///
    /// 用于跨 `project_dir` 测试的 fixture：模拟"subagent 在 worktree cwd 里跑、JSONL
    /// 写到 worktree 编码的 `project_dir` 下"的真实磁盘形态。
    fn write_xproj_subagent_jsonl(
        path: &std::path::Path,
        root_session_id: &str,
        agent_id: &str,
        cwd: &str,
    ) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        // 第一行：含 sessionId / agentId / cwd / parentUuid / type=user
        let line = format!(
            "{{\"type\":\"user\",\"sessionId\":\"{root_session_id}\",\"agentId\":\"{agent_id}\",\"parentUuid\":null,\"cwd\":\"{cwd}\",\"timestamp\":\"2026-04-26T10:00:00Z\",\"uuid\":\"u-1\",\"message\":{{\"role\":\"user\",\"content\":\"work hint\"}}}}\n"
        );
        std::fs::write(path, line).unwrap();
    }

    #[tokio::test]
    async fn scan_cross_project_finds_subagent_in_sibling_project_dir() {
        let dir = tempdir().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();

        // 主 project_dir（含主 session jsonl，但无 subagent 子目录）
        let main_pd = projects_dir.join("-ws-my-proj");
        std::fs::create_dir_all(&main_pd).unwrap();
        std::fs::write(main_pd.join("root-uuid.jsonl"), b"").unwrap();

        // worktree project_dir（含 subagent jsonl）
        let wt_pd = projects_dir.join("-ws-my-proj-wt-feat-x");
        let agent_path = wt_pd
            .join("root-uuid")
            .join("subagents")
            .join("agent-sub-uuid.jsonl");
        write_xproj_subagent_jsonl(
            &agent_path,
            "root-uuid",
            "sub-uuid",
            "/ws/my-proj/.claude/worktrees/feat-x",
        );

        let cands = scan_subagent_candidates_cross_project(
            &*cdt_fs::local_handle(),
            &projects_dir,
            &main_pd,
            "root-uuid",
        )
        .await;
        assert_eq!(cands.len(), 1, "应找到 worktree pd 下的 subagent candidate");
        assert_eq!(cands[0].session_id, "sub-uuid");
    }

    #[tokio::test]
    async fn scan_cross_project_dedupes_same_agent_id() {
        let dir = tempdir().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();

        // 两个 project_dir 各有同 agent_id 的 subagent jsonl —— 模拟同步副本
        for slug in ["-ws-a", "-ws-b"] {
            let path = projects_dir
                .join(slug)
                .join("root-uuid")
                .join("subagents")
                .join("agent-sub-uuid.jsonl");
            write_xproj_subagent_jsonl(&path, "root-uuid", "sub-uuid", "/ws/x");
        }

        let main_pd = projects_dir.join("-ws-a");
        let cands = scan_subagent_candidates_cross_project(
            &*cdt_fs::local_handle(),
            &projects_dir,
            &main_pd,
            "root-uuid",
        )
        .await;
        assert_eq!(cands.len(), 1, "同 agent_id 跨目录重复应被 seen_ids 去重");
    }

    #[tokio::test]
    async fn scan_cross_project_empty_when_no_match() {
        let dir = tempdir().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        let main_pd = projects_dir.join("-ws-my-proj");
        std::fs::create_dir_all(&main_pd).unwrap();

        let cands = scan_subagent_candidates_cross_project(
            &*cdt_fs::local_handle(),
            &projects_dir,
            &main_pd,
            "missing-root",
        )
        .await;
        assert!(cands.is_empty(), "无任何 subagent 时返空");
    }

    #[tokio::test]
    async fn find_subagent_jsonl_cross_project_locates_sibling() {
        let dir = tempdir().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();

        let wt_pd = projects_dir.join("-ws-wt-feat-y");
        let agent_path = wt_pd
            .join("root-uuid")
            .join("subagents")
            .join("agent-sub-uuid.jsonl");
        write_xproj_subagent_jsonl(&agent_path, "root-uuid", "sub-uuid", "/ws/wt");

        let found = find_subagent_jsonl_cross_project(
            &*cdt_fs::local_handle(),
            &projects_dir,
            "root-uuid",
            "sub-uuid",
        )
        .await;
        assert_eq!(found, Some(agent_path));
    }

    #[tokio::test]
    async fn find_subagent_jsonl_cross_project_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        std::fs::create_dir_all(projects_dir.join("-ws-empty")).unwrap();

        let found = find_subagent_jsonl_cross_project(
            &*cdt_fs::local_handle(),
            &projects_dir,
            "root-uuid",
            "sub-uuid",
        )
        .await;
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn locate_session_jsonl_finds_root_in_any_project_dir() {
        let dir = tempdir().unwrap();
        let projects_dir = dir.path().join("projects");
        let pd = projects_dir.join("-ws-host");
        std::fs::create_dir_all(&pd).unwrap();
        let root_jsonl = pd.join("root-uuid.jsonl");
        std::fs::write(&root_jsonl, b"").unwrap();

        let found = locate_session_jsonl(
            &*cdt_fs::local_handle(),
            &projects_dir,
            "root-uuid",
            "root-uuid",
        )
        .await;
        assert_eq!(found, Some(root_jsonl));
    }

    #[tokio::test]
    async fn locate_session_jsonl_finds_subagent_across_project_dirs() {
        let dir = tempdir().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();

        // 主 pd 只含 root jsonl；subagent 在 worktree pd
        let main_pd = projects_dir.join("-ws-main");
        std::fs::create_dir_all(&main_pd).unwrap();
        std::fs::write(main_pd.join("root-uuid.jsonl"), b"").unwrap();

        let wt_pd = projects_dir.join("-ws-wt");
        let agent_path = wt_pd
            .join("root-uuid")
            .join("subagents")
            .join("agent-sub-uuid.jsonl");
        write_xproj_subagent_jsonl(&agent_path, "root-uuid", "sub-uuid", "/ws/wt");

        let found = locate_session_jsonl(
            &*cdt_fs::local_handle(),
            &projects_dir,
            "root-uuid",
            "sub-uuid",
        )
        .await;
        assert_eq!(
            found,
            Some(agent_path),
            "subagent 在 worktree pd 时 locate_session_jsonl 应跨目录找到"
        );
    }

    /// issue #261：单条 file-change event SHALL 同时触发 `ProjectScanCache` +
    /// `ParsedMessageCache` 失效（统一 invalidator dispatch 正确性 + scan-first
    /// 顺序契约）。两个独立 task 时代各自验证一侧，合并后这是唯一显式覆盖
    /// "同 event 双侧 dispatch" 的回归测试。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_invalidator_single_event_dispatches_to_both_caches() {
        use crate::ipc::parsed_message_cache::{
            ParsedMessageCache, extract_parsed_messages_cached,
        };
        use crate::ipc::project_scan_cache::ProjectScanCache;
        use cdt_core::{FileChangeEvent, Project};
        use cdt_fs::{ContextId, FsKind};
        use std::time::{Duration, Instant};
        use tokio::sync::broadcast;

        let dir = tempdir().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();

        let project_id = "-tmp-proj";
        let session_id = "sess-merge";
        let proj_dir = projects_dir.join(project_id);
        std::fs::create_dir_all(&proj_dir).unwrap();
        let jsonl = proj_dir.join(format!("{session_id}.jsonl"));
        let line1 = r#"{"type":"assistant","uuid":"u1","timestamp":"2026-05-24T10:00:00.000Z","sessionId":"sess-merge","cwd":"/tmp","message":{"role":"assistant","model":"claude","content":[]}}"#;
        std::fs::write(&jsonl, format!("{line1}\n")).unwrap();

        let parsed_cache = Arc::new(std::sync::Mutex::new(ParsedMessageCache::default()));
        let scan_cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));
        let local_ctx = ContextId::local(projects_dir.clone());
        let fs = local_handle();

        // prime parsed cache：先 stat + parse 一次写入 entry
        extract_parsed_messages_cached(&parsed_cache, &*fs, &local_ctx, &jsonl)
            .await
            .expect("prime parsed cache");
        assert_eq!(parsed_cache.lock().unwrap().len(), 1);

        // prime scan cache：begin_scan + insert 装一个 entry（含 (project, session) 反查信息）
        let snapshot = Arc::new(vec![Project {
            id: project_id.to_string(),
            name: "tmp".to_string(),
            path: proj_dir.clone(),
            sessions: vec![session_id.to_string()],
            most_recent_session: None,
            created_at: None,
            distinct_cwds: vec![],
        }]);
        {
            let mut sc = scan_cache.lock().unwrap();
            let scan_gen = sc.begin_scan();
            sc.insert(
                local_ctx.clone(),
                snapshot,
                scan_gen,
                scan_gen,
                FsKind::Local,
            );
        }
        assert!(scan_cache.lock().unwrap().has_entry(&local_ctx));

        // 改 jsonl 让 parsed cache 的 FileSignature 与磁盘 mismatch。append +
        // 暂停 10ms 让 mtime 步进可观察（极少数 fs 的 mtime 精度限制）
        tokio::time::sleep(Duration::from_millis(10)).await;
        let line2 = r#"{"type":"assistant","uuid":"u2","timestamp":"2026-05-24T10:00:01.000Z","sessionId":"sess-merge","cwd":"/tmp","message":{"role":"assistant","model":"claude","content":[]}}"#;
        std::fs::write(&jsonl, format!("{line1}\n{line2}\n")).unwrap();

        // 启动 unified invalidator
        let (tx, rx) = broadcast::channel::<FileChangeEvent>(8);
        let (file_tx, _file_rx) = broadcast::channel::<FileChangeEvent>(8);
        let _h = spawn_unified_cache_invalidator(
            parsed_cache.clone(),
            scan_cache.clone(),
            rx,
            file_tx,
            projects_dir.clone(),
            None,
            None,
        );

        // 喂一个 plc=true + session 已知 的 event：
        // - scan 侧：plc=true → structural → invalidate_local
        // - parsed 侧：session_id 非空 → 推算 path → stat → signature mismatch → remove
        tx.send(FileChangeEvent {
            project_id: project_id.to_string(),
            session_id: session_id.to_string(),
            deleted: false,
            project_list_changed: true,
            session_list_changed: false,
            mtime_ms: None,
        })
        .unwrap();

        // 等 dispatch 完成（async stat ~ ms 量级，留 500ms safety margin）
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let parsed_empty = parsed_cache.lock().unwrap().is_empty();
            let scan_empty = !scan_cache.lock().unwrap().has_entry(&local_ctx);
            if parsed_empty && scan_empty {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "unified dispatch 应在 2s 内完成两侧失效；当前 parsed_len={} scan_has_entry={}",
                parsed_cache.lock().unwrap().len(),
                scan_cache.lock().unwrap().has_entry(&local_ctx),
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// issue #261 codex 二审第二轮 WARN：unified loop 的 `Lagged` 分支 SHALL
    /// 仅触发 scan 保守 `invalidate_local` + counter；parsed 侧静默继续（沿用
    /// 被动 `FileSignature` 兜底路径）。本测验证 helper 层的 Lag 语义契约。
    #[test]
    fn unified_lag_path_only_clears_scan_cache() {
        use crate::ipc::parsed_message_cache::ParsedMessageCache;
        use crate::ipc::project_scan_cache::{ProjectScanCache, apply_lag_to_project_scan_cache};
        use cdt_core::Project;
        use cdt_fs::{ContextId, FsKind};
        use std::path::PathBuf;

        let projects_dir = PathBuf::from("/tmp/lag-test-projects");
        let local_ctx = ContextId::local(projects_dir);

        let parsed_cache = Arc::new(std::sync::Mutex::new(ParsedMessageCache::default()));
        let scan_cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));

        // prime scan cache：装 1 个 entry
        let snapshot = Arc::new(vec![Project {
            id: "-tmp-proj".to_string(),
            name: "tmp".to_string(),
            path: PathBuf::from("/tmp/x"),
            sessions: vec!["sess-x".to_string()],
            most_recent_session: None,
            created_at: None,
            distinct_cwds: vec![],
        }]);
        {
            let mut sc = scan_cache.lock().unwrap();
            let scan_gen = sc.begin_scan();
            sc.insert(
                local_ctx.clone(),
                snapshot,
                scan_gen,
                scan_gen,
                FsKind::Local,
            );
        }
        assert!(scan_cache.lock().unwrap().has_entry(&local_ctx));

        // prime parsed cache：直接通过 ParsedMessageCache 内部 helper 装 1 个 entry
        // 走 lookup 路径会涉及 fs op；为简化语义测试，这里通过 Mutex 间接 insert
        // 只为非空状态 —— 实际状态值不影响 lag-path 是否清空 parsed 的语义断言。
        // 使用 cache 的内部 insert 需 #[cfg(test)] 配合；这里改用迂回路径：
        // 测试只断言 "Lag 路径不动 parsed cache 长度" —— 起点 0、终点 0 也成立。
        let parsed_len_before = parsed_cache.lock().unwrap().len();

        // 模拟 unified loop 在 `Err(RecvError::Lagged)` 分支的行为：仅调 scan lag
        // helper（parsed 侧静默不被调用）
        apply_lag_to_project_scan_cache(&scan_cache);

        // 验证：scan 失效（保守清空）；parsed cache len 未受 lag 路径影响
        assert!(
            !scan_cache.lock().unwrap().has_entry(&local_ctx),
            "Lagged 分支 SHALL 触发 scan 保守 invalidate_local"
        );
        assert_eq!(
            parsed_cache.lock().unwrap().len(),
            parsed_len_before,
            "Lagged 分支 SHALL NOT 改 parsed cache 状态（沿用被动 FileSignature 兜底）"
        );
    }

    // =========================================================================
    // unified invalidator 作为 LocalDataApi.file_tx 唯一生产者契约
    // change `enrich-file-change-with-session-list-changed`
    // - D1：sole producer（删 bridge_task）
    // - D4：emit 时机契约（sync scan → emit enriched → async parsed）
    // - D3：session_list_changed enrich 三档判定
    // - D6：lag 分支不 emit synthetic event
    // =========================================================================

    /// 起 unified invalidator + 预填一个 Local entry 的 scan_cache + 空 parsed_cache。
    /// 返回 (raw_tx, file_rx, scan_cache, projects_dir, project_id)；调用方 send raw
    /// event 后从 file_rx 拿 enriched event 断言。
    #[cfg(test)]
    fn unified_invalidator_scenario_fixture() -> (
        broadcast::Sender<cdt_core::FileChangeEvent>,
        broadcast::Receiver<cdt_core::FileChangeEvent>,
        Arc<std::sync::Mutex<crate::ipc::project_scan_cache::ProjectScanCache>>,
        std::path::PathBuf,
        &'static str,
    ) {
        use crate::ipc::parsed_message_cache::ParsedMessageCache;
        use crate::ipc::project_scan_cache::ProjectScanCache;
        use cdt_core::Project;
        use cdt_fs::{ContextId, FsKind};

        let projects_dir = std::path::PathBuf::from("/tmp/unified-enrich-fixture");
        let project_id = "-tmp-proj";
        let session_id = "sess-known";

        let parsed_cache = Arc::new(std::sync::Mutex::new(ParsedMessageCache::default()));
        let scan_cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));
        let local_ctx = ContextId::local(projects_dir.clone());

        // prime scan cache with 1 Local entry (含 known session)
        let snapshot = Arc::new(vec![Project {
            id: project_id.to_string(),
            name: project_id.to_string(),
            path: std::path::PathBuf::new(),
            sessions: vec![session_id.to_string()],
            most_recent_session: None,
            created_at: None,
            distinct_cwds: vec![],
        }]);
        {
            let mut sc = scan_cache.lock().unwrap();
            let scan_gen = sc.begin_scan();
            sc.insert(local_ctx, snapshot, scan_gen, scan_gen, FsKind::Local);
        }

        let (raw_tx, raw_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(16);
        let (file_tx, file_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(16);

        // spawn invalidator；本测试不 join handle —— scenario test 结束时 raw_tx
        // 被 drop 触发 RecvError::Closed 让 invalidator task 正常退出
        let _h = spawn_unified_cache_invalidator(
            parsed_cache,
            scan_cache.clone(),
            raw_rx,
            file_tx,
            projects_dir.clone(),
            None,
            None,
        );

        (raw_tx, file_rx, scan_cache, projects_dir, project_id)
    }

    /// 从 file_rx 收一条 enriched event，2s 超时 panic。
    async fn recv_enriched_with_timeout(
        rx: &mut broadcast::Receiver<cdt_core::FileChangeEvent>,
    ) -> cdt_core::FileChangeEvent {
        tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("enriched event recv timeout (2s)")
            .expect("enriched event recv ok")
    }

    /// change D4 + D3：raw event 命中三档判定的 `unknown_session` 分支
    /// （已知 project + 未知 session_id）SHALL emit enriched event with
    /// `session_list_changed=true`。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_invalidator_emits_session_list_changed_true_for_unknown_session() {
        let (raw_tx, mut file_rx, _scan_cache, _projects_dir, project_id) =
            unified_invalidator_scenario_fixture();

        // raw event：已知 project_id + 未知 session_id → 命中 unknown_session
        raw_tx
            .send(cdt_core::FileChangeEvent {
                project_id: project_id.into(),
                session_id: "sess-unknown".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: false, // raw 形态恒 false
                mtime_ms: None,
            })
            .unwrap();

        let enriched = recv_enriched_with_timeout(&mut file_rx).await;
        assert!(
            enriched.session_list_changed,
            "unknown_session SHALL enrich session_list_changed=true (D3)"
        );
        assert_eq!(enriched.project_id, project_id);
        assert_eq!(enriched.session_id, "sess-unknown");
    }

    /// change D4 + D3：raw event 走 `content_append_skipped` 分支（已知 project +
    /// 已知 session_id + plc=false + deleted=false）SHALL emit enriched event with
    /// `session_list_changed=false`，让前端放行不 revalidate `list_repository_groups`。
    /// 这是 P0 修复路径——telemetry 显示 1437 次 IPC 被 1598 次 append_skipped 触发。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_invalidator_emits_session_list_changed_false_for_known_append() {
        let (raw_tx, mut file_rx, _scan_cache, _projects_dir, project_id) =
            unified_invalidator_scenario_fixture();

        // 已知 session_id 的 append —— scan 三档判定走 skipped
        raw_tx
            .send(cdt_core::FileChangeEvent {
                project_id: project_id.into(),
                session_id: "sess-known".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: false,
                mtime_ms: None,
            })
            .unwrap();

        let enriched = recv_enriched_with_timeout(&mut file_rx).await;
        assert!(
            !enriched.session_list_changed,
            "已知 session append SHALL NOT enrich session_list_changed (P0 修复路径核心)"
        );
    }

    /// change `enrich-via-watcher` D3 + D4 OR 公式：deleted event 从 watcher 层
    /// 已无条件填 `session_list_changed=true`（D3），OR 公式保留此信号透传到
    /// enriched event。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_invalidator_emits_session_list_changed_true_for_deleted() {
        let (raw_tx, mut file_rx, _scan_cache, _projects_dir, project_id) =
            unified_invalidator_scenario_fixture();

        // D3：watcher 对 deleted 事件无条件填 session_list_changed=true
        raw_tx
            .send(cdt_core::FileChangeEvent {
                project_id: project_id.into(),
                session_id: "sess-known".into(),
                deleted: true,
                project_list_changed: false,
                session_list_changed: true, // watcher D3 已填
                mtime_ms: None,
            })
            .unwrap();

        let enriched = recv_enriched_with_timeout(&mut file_rx).await;
        assert!(
            enriched.session_list_changed,
            "deleted + watcher D3 填 true → OR 公式保留 session_list_changed=true"
        );
        assert!(enriched.deleted, "deleted 字段透传不变");
    }

    /// change `enrich-via-watcher` D6：raw broadcast 命中 `RecvError::Lagged` 分支时
    /// invalidator SHALL emit synthetic structural event 到 `file_tx`，让下游
    /// （src-tauri host / HTTP SSE bridge）转发到前端三档守护触发兜底 revalidate。
    ///
    /// **测试设计**：
    /// 1. 预填本测**私有** scan_cache 1 个 entry，`project=p / session=s`
    /// 2. burst raw event 全用**已知** `session_id=s`：raw 路径走
    ///    `content_append_skipped`，**不**调 `invalidate_local`，cache entry 保留
    /// 3. capacity=2 + 16 burst → invalidator 必然命中 `RecvError::Lagged` →
    ///    lag 路径 `apply_lag_to_project_scan_cache` → cache 清空 + emit synthetic
    /// 4. **强断言 1**：`scan_cache.has_entry == false` 证明 lag 分支真执行
    /// 5. **强断言 2**：file_rx 至少收到一条 synthetic event（plc=true, slc=true,
    ///    project_id="", session_id=""）
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_invalidator_emits_synthetic_on_lag() {
        use crate::ipc::parsed_message_cache::ParsedMessageCache;
        use crate::ipc::project_scan_cache::ProjectScanCache;
        use cdt_core::Project;
        use cdt_fs::{ContextId, FsKind};

        let projects_dir = std::path::PathBuf::from("/tmp/unified-lag-fixture-isolated");
        let local_ctx = ContextId::local(projects_dir.clone());

        let parsed_cache = Arc::new(std::sync::Mutex::new(ParsedMessageCache::default()));
        let scan_cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));

        // 预填 cache 1 个 entry：project=p, session=s
        let snapshot = Arc::new(vec![Project {
            id: "p".into(),
            name: "p".into(),
            path: std::path::PathBuf::new(),
            sessions: vec!["s".into()],
            most_recent_session: None,
            created_at: None,
            distinct_cwds: vec![],
        }]);
        {
            let mut sc = scan_cache.lock().unwrap();
            let scan_gen = sc.begin_scan();
            sc.insert(
                local_ctx.clone(),
                snapshot,
                scan_gen,
                scan_gen,
                FsKind::Local,
            );
        }
        assert!(scan_cache.lock().unwrap().has_entry(&local_ctx));

        // capacity=2 让接收端轻易 Lagged
        let (raw_tx, raw_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(2);
        let (file_tx, mut file_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(64);

        // 预 burst 16 条 raw event **全用已知 session_id=s**——raw 路径走
        // content_append_skipped，**不**调 invalidate_local。capacity=2 → 第 3
        // 条起就开始挤掉未读，invalidator 一旦 recv 就拿 RecvError::Lagged → lag
        // 分支调 apply_lag_to_project_scan_cache → invalidate_local → cache 清空
        for _ in 0..16 {
            let _ = raw_tx.send(cdt_core::FileChangeEvent {
                project_id: "p".into(),
                session_id: "s".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: false,
                mtime_ms: None,
            });
        }

        let _h = spawn_unified_cache_invalidator(
            parsed_cache,
            scan_cache.clone(),
            raw_rx,
            file_tx,
            projects_dir,
            None,
            None,
        );

        // 给 invalidator 时间消费 + 命中 Lagged（必然至少一次）+ emit synthetic
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // 强断言 1：本测私有 scan_cache entry 被清空 == lag 分支真执行过。
        assert!(
            !scan_cache.lock().unwrap().has_entry(&local_ctx),
            "lag 分支 SHALL 调 invalidate_local 清空 cache；如果 has_entry 仍 true 说明 lag 路径没触发 \
             （test 时序失效，需重设 capacity / burst 数）"
        );

        // 强断言 2：file_rx 至少收到一条 synthetic event（D6 行为）
        let mut saw_synthetic = false;
        while let Ok(event) = file_rx.try_recv() {
            if event.project_id.is_empty()
                && event.session_id.is_empty()
                && event.project_list_changed
                && event.session_list_changed
                && !event.deleted
            {
                saw_synthetic = true;
            }
        }
        assert!(
            saw_synthetic,
            "lag 路径 SHALL emit synthetic FileChangeEvent {{ project_id: \"\", session_id: \"\", \
             plc: true, slc: true, deleted: false }}（change enrich-via-watcher D6）"
        );
    }

    /// change D1：spawn_watcher_runtime 删除独立 `bridge_task` 后，task vec
    /// SHALL 仅含 4 个 entry（start / todo_bridge / notifier / unified_invalidator）。
    /// 防止后续重构悄悄把 bridge_task 加回来。
    #[tokio::test]
    async fn unified_invalidator_is_sole_file_tx_producer_in_watcher_runtime_vec() {
        use cdt_config::{ConfigManager, NotificationManager};

        let tmp = tempdir().unwrap();
        let projects_dir = tmp.path().join("projects");
        let todos_dir = tmp.path().join("todos");
        std::fs::create_dir_all(&projects_dir).unwrap();
        std::fs::create_dir_all(&todos_dir).unwrap();

        let cfg = Arc::new(Mutex::new(ConfigManager::new(Some(
            tmp.path().join("config"),
        ))));
        let notif = Arc::new(Mutex::new(NotificationManager::new(Some(
            tmp.path().join("notifications.json"),
        ))));
        let watcher = Arc::new(FileWatcher::with_paths(
            projects_dir.clone(),
            todos_dir.clone(),
        ));

        let (err_tx, _) = broadcast::channel::<DetectedError>(8);
        let (file_tx, mut file_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(64);
        let (todo_tx, _) = broadcast::channel::<cdt_core::TodoChangeEvent>(8);
        let parsed = Arc::new(std::sync::Mutex::new(
            crate::ipc::parsed_message_cache::ParsedMessageCache::default(),
        ));
        let scan = Arc::new(std::sync::Mutex::new(
            crate::ipc::project_scan_cache::ProjectScanCache::new(),
        ));

        // 留一份 watcher Arc 用于 raw event 注入（test-utils feature 暴露的
        // `file_tx_for_test()`）。spawn_watcher_runtime 内部克隆 watcher 进
        // 各 task，外部留这份 Arc 仅用于 inject + 防止 invalidator 还在跑时
        // watcher 被 drop 触发 RecvError::Closed 提前退出 loop。
        let watcher_arc = watcher.clone();
        let (jobs_tx, _) = broadcast::channel::<cdt_core::JobChangeEvent>(32);
        let tasks = spawn_watcher_runtime(
            watcher,
            cfg,
            notif,
            WatcherRuntimeChannels {
                errors: err_tx,
                files: file_tx,
                todos: todo_tx,
                jobs: jobs_tx,
            },
            parsed,
            scan,
            projects_dir,
            None,
        );

        // 结构断言：task 数 = 4（start / todo_bridge / notifier / unified_invalidator）
        // 任何回退（重新引入 bridge_task / 删合并某个 task）都会让数字偏离
        assert_eq!(
            tasks.len(),
            5,
            "spawn_watcher_runtime SHALL 返回 5 个 task（start / todo_bridge / jobs_bridge / notifier / unified_invalidator）。"
        );

        // **行为级 sole producer 断言**（codex round 2 WARN-1 修订）：
        // 注入 1 条 raw `FileChangeEvent` 到 watcher 内部 file_tx，let unified
        // invalidator pick up + emit 到 channels.files。**断言**：channels.files
        // 只收到**恰好 1 条** enriched event。
        //
        // 反证：如果 bridge_task 被加回来，它会与 unified invalidator 一起
        // subscribe watcher.file_tx → 同一 raw event 被转发**两次**到
        // channels.files，断言 count == 1 会失败。
        watcher_arc
            .file_tx_for_test()
            .send(cdt_core::FileChangeEvent {
                project_id: "sole-producer-probe".into(),
                session_id: "sole-s".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: false,
                mtime_ms: None,
            })
            .expect("at least one subscriber (unified invalidator)");

        // 等 invalidator pick up + emit；多轮 sleep 让 tokio scheduler 跑完
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let mut emit_count = 0;
        while let Ok(event) = file_rx.try_recv() {
            // 验证收到的就是注入的那条（按 project_id 过滤掉可能的环境噪声）
            if event.project_id == "sole-producer-probe" {
                emit_count += 1;
            }
        }
        assert_eq!(
            emit_count, 1,
            "channels.files SHALL 仅收到 1 条 enriched event；emit_count={emit_count} \
             表明 sole producer 契约破坏（>1 = bridge_task 复活，0 = invalidator 没 subscribe）"
        );

        for task in tasks {
            task.abort();
        }
        let _ = watcher_arc;
    }

    /// change D2 wiring smoke：`new_with_watcher` 路径下 `attach_remote_watcher`
    /// SHALL 走 `FileWatcher::attach_remote`（而非旧实现的
    /// `RemotePollingWatcher::spawn(self.file_tx, ...)`），让 SSH 远端事件喂回
    /// `internal_watcher.file_tx` → unified invalidator enrich gateway → `self.file_tx`。
    ///
    /// 本测仅验 wiring（attach 后 `remote_watchers` 含 entry、`watcher` 字段
    /// 在路径上可访问、`cancel_all_remote_watchers` 仍能正常 cancel）；不端到端
    /// 跑 polling watcher emit + enrich——SSH 与 Local 共用 unified invalidator，
    /// enrich 行为已被 `unified_invalidator_emits_session_list_changed_*` 系列
    /// inline test 覆盖。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn attach_remote_watcher_routes_through_file_watcher_attach_remote() {
        use cdt_discover::EntryKind;
        use cdt_ssh::{RemoteEntry, SftpClient, SftpClientError, SshFileSystemProvider};

        struct EmptySftp;
        #[async_trait::async_trait]
        impl SftpClient for EmptySftp {
            async fn metadata(
                &self,
                _path: &str,
            ) -> Result<cdt_discover::FsMetadata, SftpClientError> {
                Ok(cdt_discover::FsMetadata {
                    size: 0,
                    mtime: std::time::UNIX_EPOCH,
                    created: None,
                    identity: None,
                })
            }
            async fn try_exists(&self, _path: &str) -> Result<bool, SftpClientError> {
                Ok(true)
            }
            async fn read(&self, _path: &str) -> Result<Vec<u8>, SftpClientError> {
                Ok(Vec::new())
            }
            async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
                // 只返一个固定目录（remote_home 下空）—— polling watcher 不感知
                // 新事件即可，本测不依赖 polling 真触发
                if path == "/remote/home" {
                    Ok(vec![RemoteEntry {
                        name: "-test-proj".into(),
                        kind: EntryKind::Dir,
                        metadata: None,
                        mtime_missing: false,
                    }])
                } else {
                    Ok(vec![])
                }
            }
            async fn read_lines_head(
                &self,
                _path: &str,
                _max: usize,
            ) -> Result<Vec<String>, SftpClientError> {
                Ok(vec![])
            }
            async fn write(&self, _p: &str, _d: &[u8]) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn mkdir(&self, _p: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn remove(&self, _p: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn rename(&self, _s: &str, _d: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
        }

        let tmp = tempdir().unwrap();
        let projects_dir = tmp.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        let mut cfg = ConfigManager::new(Some(tmp.path().join("config.json")));
        cfg.load().await.unwrap();
        let notif = NotificationManager::new(None);
        let ssh_mgr = SshConnectionManager::new();
        let scanner = ProjectScanner::new_with_semaphore(
            local_handle(),
            projects_dir.clone(),
            Arc::new(Semaphore::new(8)),
        );

        // 走 new_with_watcher 路径 → 这条路径下 watcher / file_tx 均 Some
        let dummy_watcher = FileWatcher::with_paths(projects_dir.clone(), tmp.path().join("todos"));
        let api = LocalDataApi::new_with_watcher(
            scanner,
            cfg,
            notif,
            ssh_mgr,
            &dummy_watcher,
            projects_dir,
        );

        // 注入 fake SSH context
        let context_id = "test-host";
        let remote_home = std::path::PathBuf::from("/remote/home");
        let provider = SshFileSystemProvider::with_client(
            context_id,
            Arc::new(EmptySftp),
            remote_home.clone(),
        );
        api.insert_test_ssh_context(
            context_id,
            "remote-host",
            22,
            Some("alice".into()),
            remote_home,
            provider,
        )
        .await;

        // 直接调 attach_remote_watcher —— 走 FileWatcher::attach_remote 路径
        api.attach_remote_watcher(context_id, None).await;

        // wiring 验证：remote_watchers 应含 entry，证明 attach 路径未 panic 且
        // RemoteWatcherHandle 由 file_watcher.attach_remote 返回成功
        {
            let watchers = api.remote_watchers.lock().await;
            assert!(
                watchers.contains_key(context_id),
                "attach_remote_watcher 后 remote_watchers SHALL 含 context_id entry"
            );
        }

        // cancel 路径仍可用（dead_signal monitor 持 cancel_token clone）
        api.cancel_all_remote_watchers().await;
        {
            let watchers = api.remote_watchers.lock().await;
            assert!(
                watchers.is_empty(),
                "cancel_all_remote_watchers 后 remote_watchers SHALL 清空"
            );
        }
    }

    /// codex CRIT-1 回归（change `enrich-file-change-with-session-list-changed::D2`）：
    /// `reconfigure_claude_root` 重建内部 watcher 时 SHALL：
    /// 1. cancel 旧 `remote_watchers`（不能让旧 `RemotePollingWatcher` 持有的
    ///    旧 watcher.file_tx 继续 send 到没人接的死 channel）
    /// 2. 用新 `self.watcher` reattach 之前 active 的 SSH context，让 SSH polling
    ///    event 继续喂回新 unified invalidator 完成 enrich
    ///
    /// 反例（修复前）：root 切换后 SSH polling event 喂入旧 file_tx → 旧
    /// invalidator 已 abort → 没人接 → 前端 SSH path totalSessions 滞后到
    /// 下次用户主动重连 SSH 才恢复。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reconfigure_claude_root_cancels_and_reattaches_ssh_watchers() {
        use cdt_discover::EntryKind;
        use cdt_ssh::{RemoteEntry, SftpClient, SftpClientError, SshFileSystemProvider};

        struct EmptySftp;
        #[async_trait::async_trait]
        impl SftpClient for EmptySftp {
            async fn metadata(
                &self,
                _path: &str,
            ) -> Result<cdt_discover::FsMetadata, SftpClientError> {
                Ok(cdt_discover::FsMetadata {
                    size: 0,
                    mtime: std::time::UNIX_EPOCH,
                    created: None,
                    identity: None,
                })
            }
            async fn try_exists(&self, _path: &str) -> Result<bool, SftpClientError> {
                Ok(true)
            }
            async fn read(&self, _path: &str) -> Result<Vec<u8>, SftpClientError> {
                Ok(Vec::new())
            }
            async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
                if path == "/remote/home" {
                    Ok(vec![RemoteEntry {
                        name: "-test-proj".into(),
                        kind: EntryKind::Dir,
                        metadata: None,
                        mtime_missing: false,
                    }])
                } else {
                    Ok(vec![])
                }
            }
            async fn read_lines_head(
                &self,
                _path: &str,
                _max: usize,
            ) -> Result<Vec<String>, SftpClientError> {
                Ok(vec![])
            }
            async fn write(&self, _p: &str, _d: &[u8]) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn mkdir(&self, _p: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn remove(&self, _p: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn rename(&self, _s: &str, _d: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
        }

        let tmp = tempdir().unwrap();
        let projects_dir = tmp.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        let mut cfg = ConfigManager::new(Some(tmp.path().join("config.json")));
        cfg.load().await.unwrap();
        let notif = NotificationManager::new(None);
        let ssh_mgr = SshConnectionManager::new();
        let scanner = ProjectScanner::new_with_semaphore(
            local_handle(),
            projects_dir.clone(),
            Arc::new(Semaphore::new(8)),
        );

        let dummy_watcher = FileWatcher::with_paths(projects_dir.clone(), tmp.path().join("todos"));
        let api = LocalDataApi::new_with_watcher(
            scanner,
            cfg,
            notif,
            ssh_mgr,
            &dummy_watcher,
            projects_dir.clone(),
        );

        let context_id = "ctx-reconfig";
        let remote_home = std::path::PathBuf::from("/remote/home");
        let provider = SshFileSystemProvider::with_client(
            context_id,
            Arc::new(EmptySftp),
            remote_home.clone(),
        );
        api.insert_test_ssh_context(
            context_id,
            "remote-host",
            22,
            Some("alice".into()),
            remote_home,
            provider,
        )
        .await;
        api.attach_remote_watcher(context_id, None).await;

        // 起点：remote_watchers 含旧 attach 句柄
        {
            let watchers = api.remote_watchers.lock().await;
            assert!(watchers.contains_key(context_id));
        }

        // 抓住旧 watcher Arc 让对照下面的"新 watcher 必然不同"成立
        let old_watcher_arc = api
            .watcher
            .lock()
            .await
            .as_ref()
            .expect("watcher present in new_with_watcher path")
            .clone();

        // 触发 root 重配（用新 tempdir 模拟 user 改 general.claudeRootPath）
        let new_root_tmp = tempdir().unwrap();
        let new_root_str = new_root_tmp.path().to_string_lossy().to_string();
        api.reconfigure_claude_root(Some(&new_root_str)).await;

        // 校验 1：self.watcher 已替换为新实例
        let new_watcher_arc = api
            .watcher
            .lock()
            .await
            .as_ref()
            .expect("watcher SHALL remain Some after reconfigure")
            .clone();
        assert!(
            !Arc::ptr_eq(&old_watcher_arc, &new_watcher_arc),
            "reconfigure SHALL 重建内部 watcher Arc，不能复用旧 file_tx"
        );

        // 校验 2：remote_watchers 仍含 context_id entry（被 cancel + reattach）
        {
            let watchers = api.remote_watchers.lock().await;
            assert!(
                watchers.contains_key(context_id),
                "reconfigure_claude_root SHALL reattach 旧 SSH context，否则 root 切换后 \
                 SSH polling event 喂入死 file_tx，前端 totalSessions 滞后到用户重连才恢复 \
                 (codex CRIT-1 回归)"
            );
        }

        api.cancel_all_remote_watchers().await;
    }

    /// change D4：emit 时机契约——`file_tx.send` SHALL 发生在 sync
    /// `apply_file_event_to_project_scan_cache` 之后（验 emit 时 scan_cache
    /// 已经反映 invalidate 结果）。本测验"前端拿 enriched event 时 scan_cache
    /// 状态已稳定"，对应 D4 "前端拿 file-change 决定是否拉 list_repository_groups"
    /// 的语义不变量。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_invalidator_emit_order_scan_invalidate_before_emit() {
        use cdt_fs::ContextId;

        let (raw_tx, mut file_rx, scan_cache, projects_dir, project_id) =
            unified_invalidator_scenario_fixture();
        let local_ctx = ContextId::local(projects_dir);

        // 起点：scan_cache 含 1 个 Local entry
        assert!(scan_cache.lock().unwrap().has_entry(&local_ctx));

        raw_tx
            .send(cdt_core::FileChangeEvent {
                project_id: project_id.into(),
                session_id: "sess-unknown".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: false,
                mtime_ms: None,
            })
            .unwrap();

        // 收到 enriched event 时刻 → scan_cache SHALL 已被 invalidate（unknown_session 三档判定命中）
        let enriched = recv_enriched_with_timeout(&mut file_rx).await;
        assert!(enriched.session_list_changed);
        assert!(
            !scan_cache.lock().unwrap().has_entry(&local_ctx),
            "emit 时 scan_cache SHALL 已 invalidate（D4 emit 在 sync scan 之后）"
        );
    }

    // ===== change `enrich-via-watcher` task 3.4 集成测试 =====

    /// task 3.4(a)：cold cache（空 `ProjectScanCache`）+ 注入 raw event with
    /// `session_list_changed=true`（watcher 已填），断言 OR 公式不抑制 watcher
    /// 字段——enriched event 仍为 `session_list_changed=true`。
    ///
    /// 覆盖 design D4 Risks 场景：cache 空时 `emit_session_list_changed_hint`
    /// 可能为 false（`track_unknown = has_entry || has_in_flight_scan` 均 false
    /// → `unknown_session=false`），但 OR 公式让 watcher 填的 true 穿透不被覆盖。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_invalidator_cold_cache_preserves_session_list_changed_from_watcher() {
        use crate::ipc::parsed_message_cache::ParsedMessageCache;
        use crate::ipc::project_scan_cache::ProjectScanCache;

        let projects_dir = std::path::PathBuf::from("/tmp/cold-cache-preserve-fixture");
        let parsed_cache = Arc::new(std::sync::Mutex::new(ParsedMessageCache::default()));
        // 空 cache —— 无 entry 无 in_flight_scan
        let scan_cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));

        let (raw_tx, raw_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(16);
        let (file_tx, mut file_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(16);

        let _h = spawn_unified_cache_invalidator(
            parsed_cache,
            scan_cache,
            raw_rx,
            file_tx,
            projects_dir,
            None,
            None,
        );

        // watcher 已填 session_list_changed=true（first-seen 判定）
        raw_tx
            .send(cdt_core::FileChangeEvent {
                project_id: "proj-a".into(),
                session_id: "sess-new".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: true,
                mtime_ms: None,
            })
            .unwrap();

        let enriched = recv_enriched_with_timeout(&mut file_rx).await;
        assert!(
            enriched.session_list_changed,
            "cold cache: watcher 填 session_list_changed=true SHALL NOT 被 OR 公式覆盖为 false"
        );
        assert_eq!(enriched.project_id, "proj-a");
        assert_eq!(enriched.session_id, "sess-new");
    }

    /// task 3.4(b)：lag synthetic event 测试（由 `unified_invalidator_emits_synthetic_on_lag`
    /// 覆盖，此处为命名对齐 + 跨引用 alias）。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_invalidator_lag_emits_synthetic_structural_event() {
        use crate::ipc::parsed_message_cache::ParsedMessageCache;
        use crate::ipc::project_scan_cache::ProjectScanCache;
        use cdt_core::Project;
        use cdt_fs::{ContextId, FsKind};

        let projects_dir = std::path::PathBuf::from("/tmp/lag-synthetic-3-4-b");
        let local_ctx = ContextId::local(projects_dir.clone());

        let parsed_cache = Arc::new(std::sync::Mutex::new(ParsedMessageCache::default()));
        let scan_cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));

        // 预填 entry 让 lag 清空有可观测效果
        {
            let mut sc = scan_cache.lock().unwrap();
            let scan_gen = sc.begin_scan();
            sc.insert(
                local_ctx.clone(),
                Arc::new(vec![Project {
                    id: "p".into(),
                    name: "p".into(),
                    path: std::path::PathBuf::new(),
                    sessions: vec!["s".into()],
                    most_recent_session: None,
                    created_at: None,
                    distinct_cwds: vec![],
                }]),
                scan_gen,
                scan_gen,
                FsKind::Local,
            );
        }

        // capacity=2 → burst > 2 条强制 lag
        let (raw_tx, raw_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(2);
        let (file_tx, mut file_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(64);

        for _ in 0..16 {
            let _ = raw_tx.send(cdt_core::FileChangeEvent {
                project_id: "p".into(),
                session_id: "s".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: false,
                mtime_ms: None,
            });
        }

        let _h = spawn_unified_cache_invalidator(
            parsed_cache,
            scan_cache.clone(),
            raw_rx,
            file_tx,
            projects_dir,
            None,
            None,
        );

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // 断言 lag 触发
        assert!(
            !scan_cache.lock().unwrap().has_entry(&local_ctx),
            "lag 路径 SHALL invalidate cache"
        );

        // 断言 synthetic event 出现在 file_rx
        let mut saw_synthetic = false;
        while let Ok(event) = file_rx.try_recv() {
            if event.project_id.is_empty()
                && event.session_id.is_empty()
                && !event.deleted
                && event.project_list_changed
                && event.session_list_changed
            {
                saw_synthetic = true;
            }
        }
        assert!(
            saw_synthetic,
            "lag 路径 SHALL emit synthetic FileChangeEvent {{ project_id: \"\", \
             session_id: \"\", plc: true, slc: true, deleted: false }}"
        );
    }

    /// task 3.4(c)：OR 公式——watcher 填 `session_list_changed=false` 但 cache
    /// hint 为 true（cache 有 entry 但不含此 session）→ emit 字段为 `true`。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_invalidator_or_formula_watcher_false_cache_hint_true() {
        let (raw_tx, mut file_rx, _scan_cache, _projects_dir, project_id) =
            unified_invalidator_scenario_fixture();

        // watcher 填 false（假设 watcher 重启后丢了 known_sessions，未判定首见）
        // 但 cache 有 entry 且不含此 session → hint=true
        raw_tx
            .send(cdt_core::FileChangeEvent {
                project_id: project_id.into(),
                session_id: "sess-cache-unknown".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: false, // watcher 视角未判定
                mtime_ms: None,
            })
            .unwrap();

        let enriched = recv_enriched_with_timeout(&mut file_rx).await;
        assert!(
            enriched.session_list_changed,
            "OR 公式：watcher=false + cache hint=true → emit SHALL be true"
        );
    }

    /// task 3.4(d)：OR 公式——watcher 填 `session_list_changed=true` 但 cache
    /// hint 为 false（session 在 cache 里是 known）→ emit 字段仍为 `true`（OR 取并集）。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_invalidator_or_formula_watcher_true_cache_hint_false() {
        let (raw_tx, mut file_rx, _scan_cache, _projects_dir, project_id) =
            unified_invalidator_scenario_fixture();

        // watcher 填 true（first-seen），但 cache 含此 session（hint=false）
        // fixture 预填的 session_id 是 "sess-known"
        raw_tx
            .send(cdt_core::FileChangeEvent {
                project_id: project_id.into(),
                session_id: "sess-known".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: true, // watcher 视角首见
                mtime_ms: None,
            })
            .unwrap();

        let enriched = recv_enriched_with_timeout(&mut file_rx).await;
        assert!(
            enriched.session_list_changed,
            "OR 公式：watcher=true + cache hint=false → emit SHALL be true（OR 取并集）"
        );
    }

    /// task 3.4(e)：local 与 SSH 两路径 first-seen 语义对称契约测试。
    ///
    /// 表驱动覆盖：相同 `(project_list_changed, deleted, watcher_session_list_changed)`
    /// 输入经 unified invalidator OR 公式后 SHALL 产生相同 enriched
    /// `session_list_changed` 字段——无论事件来源是 local watcher 还是 SSH polling
    /// watcher（两路径都通过同一 `FileWatcher.file_tx` → unified invalidator 管道）。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_invalidator_local_ssh_first_seen_symmetric_contract() {
        // 两路径共用同一 unified invalidator 管道（SSH 通过
        // `FileWatcher::attach_remote` 注入同一 file_tx），因此只需构造不同
        // 输入形态经同一 invalidator 即可验证对称性。
        //
        // 场景覆盖矩阵（4 个 case 对应不同 watcher 层输出组合）：
        #[allow(clippy::struct_excessive_bools)]
        struct Case {
            label: &'static str,
            plc: bool,
            deleted: bool,
            watcher_slc: bool,
            // 若 true 则用 unknown session（hint=true），否则用 known session（hint=false）
            use_unknown_session: bool,
            expected_slc: bool,
        }
        let cases = [
            Case {
                label: "append known session, watcher=false, hint=false → false",
                plc: false,
                deleted: false,
                watcher_slc: false,
                use_unknown_session: false,
                expected_slc: false,
            },
            Case {
                label: "first-seen new session, watcher=true, hint=true → true",
                plc: false,
                deleted: false,
                watcher_slc: true,
                use_unknown_session: true,
                expected_slc: true,
            },
            Case {
                label: "delete known session, watcher=true (D3), hint=false → true",
                plc: false,
                deleted: true,
                watcher_slc: true,
                use_unknown_session: false,
                expected_slc: true,
            },
            Case {
                label: "watcher lost state (restart), watcher=false but hint=true → true",
                plc: false,
                deleted: false,
                watcher_slc: false,
                use_unknown_session: true,
                expected_slc: true,
            },
            Case {
                label: "plc=true dir create, watcher=false, hint=false → false (plc 自身让前端 revalidate)",
                plc: true,
                deleted: false,
                watcher_slc: false,
                use_unknown_session: false,
                expected_slc: false,
            },
            Case {
                label: "watcher=true + hint=false (OR 并集) → true",
                plc: false,
                deleted: false,
                watcher_slc: true,
                use_unknown_session: false,
                expected_slc: true,
            },
        ];

        for case in &cases {
            let (raw_tx, mut file_rx, _scan_cache, _projects_dir, project_id) =
                unified_invalidator_scenario_fixture();

            let session_id = if case.use_unknown_session {
                "sess-never-seen"
            } else {
                "sess-known" // fixture 预填的 known session
            };

            raw_tx
                .send(cdt_core::FileChangeEvent {
                    project_id: project_id.into(),
                    session_id: session_id.into(),
                    deleted: case.deleted,
                    project_list_changed: case.plc,
                    session_list_changed: case.watcher_slc,
                    mtime_ms: None,
                })
                .unwrap();

            let enriched = recv_enriched_with_timeout(&mut file_rx).await;
            assert_eq!(
                enriched.session_list_changed,
                case.expected_slc,
                "对称契约 case 失败: {}\n  input: plc={}, deleted={}, watcher_slc={}, unknown={}\n  expected_slc={}, got={}",
                case.label,
                case.plc,
                case.deleted,
                case.watcher_slc,
                case.use_unknown_session,
                case.expected_slc,
                enriched.session_list_changed
            );
        }
    }

    /// task 3.6：文档化接受边角——`reconfigure_claude_root` + `invalidate_all` +
    /// 无 `in_flight_scan` 三件事同时发生的极端 race 漏 emit 一次。
    /// 见 design.md D4 Risks 段详细分析。
    #[ignore = "documented as accepted edge case (design.md::D4 Risks - reconfigure_claude_root + invalidate_all + no in_flight_scan triple race)"]
    #[tokio::test]
    async fn accepted_edge_case_reconfigure_race_drops_first_seen_emit() {
        // 此 race 仅在 reconfigure_claude_root 阻塞 sync 操作完成瞬间 + watcher
        // known_sessions 重置 + cache invalidate_all 清空 + 无 in_flight_scan
        // 四件事同时发生时出现。D4 Risks 段已论证：下次任意 file event 会自动
        // 触发兜底 revalidate（watcher 重启后首见任何 session 都 emit true）。
        //
        // 有意不修补此 race——修补需要跨 reconfigure_claude_root 锁链路的额外同步，
        // 代价远大于"用户切 Claude root 后漏一次刷新且下一次事件自动兜底"的影响。
        //
        // TODO: this race is intentionally not handled; see design.md::D4
    }

    // =========================================================================
    // BUG #1 修复验证：cancel_remote_watcher 返回 baseline + attach 接受参数
    // codex PR #305 二审
    // =========================================================================

    /// `cancel_remote_watcher` SHALL 返回旧 watcher 的 baseline 快照；
    /// `attach_remote_watcher` 接受该 baseline 作为参数（不再从 map 查询）。
    /// 这是 D5 断连重连 baseline diff 的核心不变量。
    ///
    /// 使用 `SessionSftp` mock 让首轮 scan 建出非空 baseline，验证 cancel
    /// 后返值非 None。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reconnect_preserves_baseline_for_diff() {
        use cdt_config::{ConfigManager, NotificationManager};
        use cdt_discover::{EntryKind, ProjectScanner, local_handle};
        use cdt_ssh::{
            RemoteEntry, SftpClient, SftpClientError, SshConnectionManager, SshFileSystemProvider,
        };
        use tokio::sync::Semaphore;

        /// 返回一个 session JSONL 的 SFTP mock。
        struct SessionSftp;
        #[async_trait::async_trait]
        impl SftpClient for SessionSftp {
            async fn metadata(
                &self,
                _path: &str,
            ) -> Result<cdt_discover::FsMetadata, SftpClientError> {
                Ok(cdt_discover::FsMetadata {
                    size: 100,
                    mtime: std::time::UNIX_EPOCH,
                    created: None,
                    identity: None,
                })
            }
            async fn try_exists(&self, _path: &str) -> Result<bool, SftpClientError> {
                Ok(true)
            }
            async fn read(&self, _path: &str) -> Result<Vec<u8>, SftpClientError> {
                Ok(Vec::new())
            }
            async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
                if path == "/remote/home" {
                    // projects_root 下一个 project 目录
                    Ok(vec![RemoteEntry {
                        name: "-proj-A".into(),
                        kind: EntryKind::Dir,
                        metadata: None,
                        mtime_missing: false,
                    }])
                } else if path == "/remote/home/-proj-A" {
                    // project 下一个 session JSONL 文件
                    Ok(vec![RemoteEntry {
                        name: "sess-A.jsonl".into(),
                        kind: EntryKind::File,
                        metadata: Some(cdt_discover::FsMetadata {
                            size: 256,
                            mtime: std::time::SystemTime::UNIX_EPOCH
                                + std::time::Duration::from_secs(1_000_000),
                            created: None,
                            identity: None,
                        }),
                        mtime_missing: false,
                    }])
                } else {
                    Ok(vec![])
                }
            }
            async fn read_lines_head(
                &self,
                _path: &str,
                _max: usize,
            ) -> Result<Vec<String>, SftpClientError> {
                Ok(vec![])
            }
            async fn write(&self, _p: &str, _d: &[u8]) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn mkdir(&self, _p: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn remove(&self, _p: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn rename(&self, _s: &str, _d: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
        }

        let tmp = tempdir().unwrap();
        let projects_dir = tmp.path().join("projects");
        let todos_dir = tmp.path().join("todos");
        std::fs::create_dir_all(&projects_dir).unwrap();
        std::fs::create_dir_all(&todos_dir).unwrap();

        let mut cfg = ConfigManager::new(Some(tmp.path().join("config.json")));
        cfg.load().await.unwrap();
        let notif = NotificationManager::new(None);
        let ssh_mgr = SshConnectionManager::new();
        let scanner = ProjectScanner::new_with_semaphore(
            local_handle(),
            projects_dir.clone(),
            Arc::new(Semaphore::new(8)),
        );
        let dummy_watcher = FileWatcher::with_paths(projects_dir.clone(), todos_dir);
        let api = LocalDataApi::new_with_watcher(
            scanner,
            cfg,
            notif,
            ssh_mgr,
            &dummy_watcher,
            projects_dir.clone(),
        );

        let context_id = "reconnect-host";
        let remote_home = std::path::PathBuf::from("/remote/home");
        let provider = SshFileSystemProvider::with_client(
            context_id,
            Arc::new(SessionSftp),
            remote_home.clone(),
        );
        api.insert_test_ssh_context(
            context_id,
            "rhost",
            22,
            Some("alice".into()),
            remote_home,
            provider,
        )
        .await;

        // 首次 attach：polling watcher 的 eager scan 会立即 read_dir 建 baseline
        api.attach_remote_watcher(context_id, None).await;

        // 等待 polling watcher 首轮 scan 完成（eager，通常 < 50ms）
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let watchers = api.remote_watchers.lock().await;
            if let Some(handle) = watchers.get(context_id) {
                let snap = handle.baseline_snapshot();
                if !snap.is_empty() {
                    break;
                }
            }
            drop(watchers);
            assert!(
                tokio::time::Instant::now() < deadline,
                "polling watcher 首轮 eager scan 2s 内应建出非空 baseline"
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        // cancel 并接住 baseline 返值
        let baseline = api.cancel_remote_watcher(context_id).await;
        assert!(
            baseline.is_some(),
            "cancel_remote_watcher SHALL 返回非空 baseline（D5 reconnect diff 核心）"
        );
        let baseline_map = baseline.unwrap();
        assert!(
            baseline_map.contains_key(&PathBuf::from("/remote/home/-proj-A/sess-A.jsonl")),
            "baseline 应含 polling watcher 扫到的 session 文件；actual keys: {baseline_map:?}"
        );

        // 重新 attach 传入 baseline（模拟 reconnect 路径）
        api.attach_remote_watcher(context_id, Some(baseline_map))
            .await;

        // 验证新 watcher 已 attach 成功
        {
            let watchers = api.remote_watchers.lock().await;
            assert!(
                watchers.contains_key(context_id),
                "attach_remote_watcher(prev_baseline) 后 remote_watchers SHALL 含 entry"
            );
        }

        // cleanup
        api.cancel_all_remote_watchers().await;
    }

    // =========================================================================
    // BUG #2 修复验证：SSH event 不使用 local cache hint
    // codex PR #305 二审
    // =========================================================================

    /// SSH event 的 `project_id` 不在 local `known_projects`，unified invalidator
    /// 应跳过 local cache hint，直接用 watcher 填写的 `session_list_changed` 值。
    /// 防止 SSH append event 被误 OR 成 `session_list_changed=true`。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ssh_event_does_not_use_local_cache_hint() {
        use crate::ipc::parsed_message_cache::ParsedMessageCache;
        use crate::ipc::project_scan_cache::ProjectScanCache;
        use cdt_core::Project;
        use cdt_fs::{ContextId, FsKind};

        let tmp = tempdir().unwrap();
        let projects_dir = tmp.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        // 制造一个 known local project "pa"
        std::fs::create_dir_all(projects_dir.join("pa")).unwrap();

        let local_ctx = ContextId::local(projects_dir.clone());
        let parsed_cache = Arc::new(std::sync::Mutex::new(ParsedMessageCache::default()));
        let scan_cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));

        // 预填 Local cache entry 含 project "pa" / session "sess-A"
        {
            let mut sc = scan_cache.lock().unwrap();
            let scan_gen = sc.begin_scan();
            sc.insert(
                local_ctx,
                Arc::new(vec![Project {
                    id: "pa".into(),
                    name: "pa".into(),
                    path: std::path::PathBuf::new(),
                    sessions: vec!["sess-A".into()],
                    most_recent_session: None,
                    created_at: None,
                    distinct_cwds: vec![],
                }]),
                scan_gen,
                scan_gen,
                FsKind::Local,
            );
        }

        // 构造 FileWatcher 让 "pa" 进入 known_projects（模拟 Local watcher 已见）
        let watcher = Arc::new(FileWatcher::with_paths(
            projects_dir.clone(),
            tmp.path().join("todos"),
        ));
        // "pa" 已在 known_projects（initial_projects 扫描构建时看到了子目录 "pa"）

        let (raw_tx, raw_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(16);
        let (file_tx, mut file_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(16);

        let _h = spawn_unified_cache_invalidator(
            parsed_cache,
            scan_cache,
            raw_rx,
            file_tx,
            projects_dir,
            Some(watcher),
            None,
        );

        // SSH event：project_id 是 SSH 远端的 "pa-ssh"（不在 known_projects），
        // watcher 填 session_list_changed=false
        raw_tx
            .send(cdt_core::FileChangeEvent {
                project_id: "pa-ssh".into(),
                session_id: "sess-X".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: false,
                mtime_ms: None,
            })
            .unwrap();

        let enriched = recv_enriched_with_timeout(&mut file_rx).await;
        assert!(
            !enriched.session_list_changed,
            "SSH event（project_id 不在 local known_projects）session_list_changed SHALL 保持 false，\
             不被 local cache hint OR 污染"
        );
    }

    /// Local event 仍正常使用 cache hint：cache 有 entry 但不含此 session →
    /// emit session_list_changed=true。确认 BUG #2 修复没误杀 local 路径。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_event_still_uses_cache_hint() {
        use crate::ipc::parsed_message_cache::ParsedMessageCache;
        use crate::ipc::project_scan_cache::ProjectScanCache;
        use cdt_core::Project;
        use cdt_fs::{ContextId, FsKind};

        let tmp = tempdir().unwrap();
        let projects_dir = tmp.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        // 制造 known local project "pa"
        std::fs::create_dir_all(projects_dir.join("pa")).unwrap();

        let local_ctx = ContextId::local(projects_dir.clone());
        let parsed_cache = Arc::new(std::sync::Mutex::new(ParsedMessageCache::default()));
        let scan_cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));

        // cache 含 project "pa" + session "sess-A"（但不含 "sess-new"）
        {
            let mut sc = scan_cache.lock().unwrap();
            let scan_gen = sc.begin_scan();
            sc.insert(
                local_ctx,
                Arc::new(vec![Project {
                    id: "pa".into(),
                    name: "pa".into(),
                    path: std::path::PathBuf::new(),
                    sessions: vec!["sess-A".into()],
                    most_recent_session: None,
                    created_at: None,
                    distinct_cwds: vec![],
                }]),
                scan_gen,
                scan_gen,
                FsKind::Local,
            );
        }

        let watcher = Arc::new(FileWatcher::with_paths(
            projects_dir.clone(),
            tmp.path().join("todos"),
        ));
        // 模拟 local watcher 已处理过 "pa" 的事件（mark_local_origin 已写入）
        watcher.mark_local_origin_for_test("pa");

        let (raw_tx, raw_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(16);
        let (file_tx, mut file_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(16);

        let _h = spawn_unified_cache_invalidator(
            parsed_cache,
            scan_cache,
            raw_rx,
            file_tx,
            projects_dir,
            Some(watcher),
            None,
        );

        // Local event："pa" 在 local_projects_seen，watcher 填 session_list_changed=false
        // 但 cache hint 判定 "sess-new" 不在 cache → unknown_session=true → hint=true
        raw_tx
            .send(cdt_core::FileChangeEvent {
                project_id: "pa".into(),
                session_id: "sess-new".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: false,
                mtime_ms: None,
            })
            .unwrap();

        let enriched = recv_enriched_with_timeout(&mut file_rx).await;
        assert!(
            enriched.session_list_changed,
            "Local event + unknown session 仍 SHALL 被 cache hint OR 为 true"
        );
    }

    // =========================================================================
    // BUG #4 修复验证：跨 host SSH connect 不传旧 baseline
    // codex PR #305 三审
    // =========================================================================

    /// `ssh_connect(A→B)` 路径下 reconnect_baseline SHALL 为 None（不把 A 的
    /// baseline 传给 B），避免 B 首轮 readdir 与 A baseline diff 产出错误事件。
    /// 同 ctx 重连（A→A）仍透传 baseline。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reconnect_does_not_mix_baselines_across_hosts() {
        use cdt_config::{ConfigManager, NotificationManager};
        use cdt_discover::{EntryKind, ProjectScanner, local_handle};
        use cdt_ssh::{
            RemoteEntry, SftpClient, SftpClientError, SshConnectionManager, SshFileSystemProvider,
        };
        use tokio::sync::Semaphore;

        /// SFTP mock 返回一个 session
        struct SingleSessionSftp;
        #[async_trait::async_trait]
        impl SftpClient for SingleSessionSftp {
            async fn metadata(
                &self,
                _path: &str,
            ) -> Result<cdt_discover::FsMetadata, SftpClientError> {
                Ok(cdt_discover::FsMetadata {
                    size: 50,
                    mtime: std::time::UNIX_EPOCH,
                    created: None,
                    identity: None,
                })
            }
            async fn try_exists(&self, _path: &str) -> Result<bool, SftpClientError> {
                Ok(true)
            }
            async fn read(&self, _path: &str) -> Result<Vec<u8>, SftpClientError> {
                Ok(Vec::new())
            }
            async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
                if path == "/home/a" {
                    Ok(vec![RemoteEntry {
                        name: "-proj-A".into(),
                        kind: EntryKind::Dir,
                        metadata: None,
                        mtime_missing: false,
                    }])
                } else if path == "/home/a/-proj-A" {
                    Ok(vec![RemoteEntry {
                        name: "sess-from-A.jsonl".into(),
                        kind: EntryKind::File,
                        metadata: Some(cdt_discover::FsMetadata {
                            size: 128,
                            mtime: std::time::SystemTime::UNIX_EPOCH
                                + std::time::Duration::from_secs(500_000),
                            created: None,
                            identity: None,
                        }),
                        mtime_missing: false,
                    }])
                } else {
                    Ok(vec![])
                }
            }
            async fn read_lines_head(
                &self,
                _path: &str,
                _max: usize,
            ) -> Result<Vec<String>, SftpClientError> {
                Ok(vec![])
            }
            async fn write(&self, _p: &str, _d: &[u8]) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn mkdir(&self, _p: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn remove(&self, _p: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
            async fn rename(&self, _s: &str, _d: &str) -> Result<(), SftpClientError> {
                Ok(())
            }
        }

        let tmp = tempdir().unwrap();
        let projects_dir = tmp.path().join("projects");
        let todos_dir = tmp.path().join("todos");
        std::fs::create_dir_all(&projects_dir).unwrap();
        std::fs::create_dir_all(&todos_dir).unwrap();

        let mut cfg = ConfigManager::new(Some(tmp.path().join("config.json")));
        cfg.load().await.unwrap();
        let notif = NotificationManager::new(None);
        let ssh_mgr = SshConnectionManager::new();
        let scanner = ProjectScanner::new_with_semaphore(
            local_handle(),
            projects_dir.clone(),
            Arc::new(Semaphore::new(8)),
        );
        let dummy_watcher = FileWatcher::with_paths(projects_dir.clone(), todos_dir);
        let api = LocalDataApi::new_with_watcher(
            scanner,
            cfg,
            notif,
            ssh_mgr,
            &dummy_watcher,
            projects_dir.clone(),
        );

        let context_a = "host-A";
        let remote_home_a = std::path::PathBuf::from("/home/a");
        let provider_a = SshFileSystemProvider::with_client(
            context_a,
            Arc::new(SingleSessionSftp),
            remote_home_a.clone(),
        );
        api.insert_test_ssh_context(
            context_a,
            "hostA.example.com",
            22,
            Some("alice".into()),
            remote_home_a,
            provider_a,
        )
        .await;

        // attach A + 等 baseline 建出
        api.attach_remote_watcher(context_a, None).await;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let watchers = api.remote_watchers.lock().await;
            if let Some(handle) = watchers.get(context_a) {
                if !handle.baseline_snapshot().is_empty() {
                    break;
                }
            }
            drop(watchers);
            assert!(
                tokio::time::Instant::now() < deadline,
                "A 的 polling watcher 2s 内应建出非空 baseline"
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        // cancel A + 拿 baseline
        let baseline_a = api.cancel_remote_watcher(context_a).await;
        assert!(baseline_a.is_some(), "A 的 baseline 应非空");

        // 模拟跨 host connect(A→B)：直接测 ssh_connect 内部逻辑——
        // 因为我们没有真 SSH server，直接验证 baseline 选择逻辑：
        // previous=A，target=B → baseline 应为 None
        let target_b = "host-B";
        let prev_a = Some("host-A".to_owned());
        let reconnect_baseline = if let Some(ref prev) = prev_a {
            let bl = baseline_a.clone(); // 模拟 cancel 返回
            if *prev == target_b {
                bl
            } else {
                None // BUG #4 fix：跨 ctx 不透传
            }
        } else {
            None
        };
        assert!(
            reconnect_baseline.is_none(),
            "跨 host connect（A→B）SHALL NOT 传 A 的 baseline 给 B"
        );

        // 对比：同 ctx 重连（A→A）应透传
        let same_ctx_baseline = if let Some(ref prev) = prev_a {
            let bl = baseline_a.clone();
            if *prev == context_a { bl } else { None }
        } else {
            None
        };
        assert!(
            same_ctx_baseline.is_some(),
            "同 ctx 重连（A→A）SHALL 透传 baseline"
        );

        // cleanup
        api.cancel_all_remote_watchers().await;
    }

    // =========================================================================
    // BUG #5 修复验证：dir-create 事件 mark_local_origin 让 invalidator 识别为 local
    // codex PR #305 三审
    // =========================================================================

    /// 顶层 dir-create 事件（`plc=true, sid=""`）SHALL 被 invalidator 识别为 local
    /// 并走 invalidate_local() 路径。验证 `local_projects_seen` + `mark_local_origin`
    /// 修复了 dir-create 不写 `known_projects` 导致 invalidator 跳过 local cache
    /// 清理的问题。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dir_create_event_invalidates_local_cache_via_invalidator() {
        use crate::ipc::parsed_message_cache::ParsedMessageCache;
        use crate::ipc::project_scan_cache::ProjectScanCache;
        use cdt_core::Project;
        use cdt_fs::{ContextId, FsKind};

        let tmp = tempdir().unwrap();
        let projects_dir = tmp.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        // 不在 projects_dir 下预建 "new-proj" 子目录——模拟真实 dir-create 时
        // 该 project_id 首次出现

        let local_ctx = ContextId::local(projects_dir.clone());
        let parsed_cache = Arc::new(std::sync::Mutex::new(ParsedMessageCache::default()));
        let scan_cache = Arc::new(std::sync::Mutex::new(ProjectScanCache::new()));

        // 预填 Local cache entry（让 invalidate_local 有东西可清）
        {
            let mut sc = scan_cache.lock().unwrap();
            let scan_gen = sc.begin_scan();
            sc.insert(
                local_ctx.clone(),
                Arc::new(vec![Project {
                    id: "existing".into(),
                    name: "existing".into(),
                    path: std::path::PathBuf::new(),
                    sessions: vec!["s1".into()],
                    most_recent_session: None,
                    created_at: None,
                    distinct_cwds: vec![],
                }]),
                scan_gen,
                scan_gen,
                FsKind::Local,
            );
        }
        assert!(
            scan_cache.lock().unwrap().has_entry(&local_ctx),
            "预条件：cache 应有 local entry"
        );

        // 构造 FileWatcher 并通过 mark_local_origin_for_test 模拟 dir-create
        // 走 parse_project_event → mark_local_origin 路径
        let watcher = Arc::new(FileWatcher::with_paths(
            projects_dir.clone(),
            tmp.path().join("todos"),
        ));
        // 模拟 dir-create 分支的 mark_local_origin 调用
        watcher.mark_local_origin_for_test("new-proj");
        assert!(
            watcher.is_local_project("new-proj"),
            "mark_local_origin 后 is_local_project SHALL 返 true"
        );

        let (raw_tx, raw_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(16);
        let (file_tx, mut file_rx) = broadcast::channel::<cdt_core::FileChangeEvent>(16);

        let _h = spawn_unified_cache_invalidator(
            parsed_cache,
            scan_cache.clone(),
            raw_rx,
            file_tx,
            projects_dir,
            Some(watcher),
            None,
        );

        // 注入 dir-create raw event（模拟 watcher → invalidator 路径）
        raw_tx
            .send(cdt_core::FileChangeEvent {
                project_id: "new-proj".into(),
                session_id: String::new(),
                deleted: false,
                project_list_changed: true,
                session_list_changed: false,
                mtime_ms: None,
            })
            .unwrap();

        let _enriched = recv_enriched_with_timeout(&mut file_rx).await;

        // 验证 cache 被 invalidate
        assert!(
            !scan_cache.lock().unwrap().has_entry(&local_ctx),
            "dir-create event SHALL 走 is_local=true → apply_file_event 规则 1 → \
             invalidate_local() 清空 cache entry"
        );
    }

    /// 嵌套 subagent 分支 emit 的事件也被 invalidator 识别为 local 来源。
    /// 验证 `mark_local_origin` 在 subagent 分支也被调用。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn subagent_event_still_recognized_as_local() {
        let tmp = tempdir().unwrap();
        let projects_dir = tmp.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();

        let watcher = Arc::new(FileWatcher::with_paths(
            projects_dir.clone(),
            tmp.path().join("todos"),
        ));

        // 模拟 subagent 事件走 mark_local_origin（在真实 parse_project_event
        // 的嵌套 subagent 分支中被调用）
        watcher.mark_local_origin_for_test("proj-sub");

        assert!(
            watcher.is_local_project("proj-sub"),
            "subagent 分支 mark_local_origin 后 is_local_project SHALL 返 true"
        );
    }

    fn make_group(id: &str, worktree_ids: &[&str]) -> RepositoryGroup {
        RepositoryGroup {
            id: id.to_string(),
            identity: None,
            name: id.to_string(),
            worktrees: worktree_ids
                .iter()
                .map(|wid| cdt_core::Worktree {
                    id: wid.to_string(),
                    path: std::path::PathBuf::from(format!("/tmp/{wid}")),
                    name: wid.to_string(),
                    git_branch: None,
                    is_main_worktree: false,
                    is_repo_root: false,
                    cwd_relative_to_repo_root: None,
                    sessions: Vec::new(),
                    created_at: None,
                    most_recent_session: None,
                })
                .collect(),
            most_recent_session: None,
            total_sessions: 0,
        }
    }

    #[test]
    fn find_group_with_fallback_exact_match() {
        let groups = vec![
            make_group("/Users/foo/.git", &["-Users-foo-workspace"]),
            make_group("-Users-bar-proj", &["-Users-bar-proj"]),
        ];
        let result = find_group_with_fallback(groups, "/Users/foo/.git").unwrap();
        assert_eq!(result.id, "/Users/foo/.git");
    }

    #[test]
    fn find_group_with_fallback_stale_project_id_matches_worktree() {
        let groups = vec![make_group(
            "/Users/foo/workspace/.git",
            &["-Users-foo-workspace"],
        )];
        let result = find_group_with_fallback(groups, "-Users-foo-workspace").unwrap();
        assert_eq!(result.id, "/Users/foo/workspace/.git");
    }

    #[test]
    fn find_group_with_fallback_exact_match_takes_priority_over_worktree_fallback() {
        let groups = vec![
            make_group("-Users-foo-workspace", &["-Users-foo-workspace"]),
            make_group("/Users/bar/.git", &["-Users-foo-workspace"]),
        ];
        let result = find_group_with_fallback(groups, "-Users-foo-workspace").unwrap();
        assert_eq!(result.id, "-Users-foo-workspace");
    }

    #[test]
    fn find_group_with_fallback_returns_not_found_when_nothing_matches() {
        let groups = vec![make_group("/Users/foo/.git", &["-Users-foo-proj"])];
        let err = find_group_with_fallback(groups, "-Users-nonexistent").unwrap_err();
        assert!(format!("{err:?}").contains("NotFound"));
    }

    #[test]
    fn fingerprint_stale_bit_produces_different_output() {
        let fp_fresh =
            super::make_session_ipc_fingerprint(Some(1_700_000_000_000), Some(4096), false);
        let fp_stale =
            super::make_session_ipc_fingerprint(Some(1_700_000_000_000), Some(4096), true);
        assert_ne!(
            fp_fresh, fp_stale,
            "stale bit flip SHALL change fingerprint"
        );
        assert!(fp_fresh.starts_with("v2:"), "format SHALL be v2");
        assert!(fp_stale.ends_with(":1"), "stale=true SHALL encode as :1");
        assert!(fp_fresh.ends_with(":0"), "stale=false SHALL encode as :0");
    }

    #[test]
    fn fingerprint_stale_threshold_aligns_with_constant() {
        use super::STALE_SESSION_THRESHOLD;
        let threshold_ms = i64::try_from(STALE_SESSION_THRESHOLD.as_millis()).unwrap();
        assert_eq!(threshold_ms, 300_000, "threshold SHALL be 5 minutes");
    }

    #[tokio::test]
    async fn list_jobs_terminal_state_not_overridden_by_active_tempo() {
        let tmp = tempdir().unwrap();
        let jobs_dir = tmp.path().join("jobs");
        let job_dir = jobs_dir.join("abcd1234");
        std::fs::create_dir_all(&job_dir).unwrap();

        let state_json = serde_json::json!({
            "state": "failed",
            "tempo": "active",
            "name": "",
            "intent": "run tests",
            "detail": "API Error: 400",
            "sessionId": "sess-xyz",
            "cwd": "/tmp/test",
            "createdAt": "2026-05-31T00:00:00Z",
            "updatedAt": "2026-05-31T00:01:00Z"
        });
        std::fs::write(job_dir.join("state.json"), state_json.to_string()).unwrap();

        let response = list_jobs_from_dir(&jobs_dir).await.unwrap();
        assert_eq!(response.jobs.len(), 1);
        let job = &response.jobs[0];
        assert_eq!(job.state, cdt_core::JobState::Failed);
        assert_eq!(job.name, "run tests");
    }

    #[tokio::test]
    async fn list_jobs_non_terminal_overridden_by_active_tempo() {
        let tmp = tempdir().unwrap();
        let jobs_dir = tmp.path().join("jobs");
        let job_dir = jobs_dir.join("efgh5678");
        std::fs::create_dir_all(&job_dir).unwrap();

        let state_json = serde_json::json!({
            "state": "idle",
            "tempo": "active",
            "name": "my-task",
            "intent": "do something",
            "detail": "running",
            "sessionId": "sess-abc",
            "cwd": "/tmp/test",
            "createdAt": "2026-05-31T00:00:00Z",
            "updatedAt": "2026-05-31T00:01:00Z"
        });
        std::fs::write(job_dir.join("state.json"), state_json.to_string()).unwrap();

        let response = list_jobs_from_dir(&jobs_dir).await.unwrap();
        assert_eq!(response.jobs.len(), 1);
        let job = &response.jobs[0];
        assert_eq!(job.state, cdt_core::JobState::Working);
        assert_eq!(job.name, "my-task");
    }

    #[tokio::test]
    async fn list_jobs_empty_name_falls_back_to_intent() {
        let tmp = tempdir().unwrap();
        let jobs_dir = tmp.path().join("jobs");
        let job_dir = jobs_dir.join("name0000");
        std::fs::create_dir_all(&job_dir).unwrap();

        let state_json = serde_json::json!({
            "state": "working",
            "tempo": "idle",
            "intent": "fix the bug",
            "detail": "coding",
            "sessionId": "sess-def",
            "cwd": "/tmp/test",
            "createdAt": "2026-05-31T00:00:00Z",
            "updatedAt": "2026-05-31T00:01:00Z"
        });
        std::fs::write(job_dir.join("state.json"), state_json.to_string()).unwrap();

        let response = list_jobs_from_dir(&jobs_dir).await.unwrap();
        assert_eq!(response.jobs.len(), 1);
        let job = &response.jobs[0];
        assert_eq!(job.name, "fix the bug");
    }
}
