//! `LocalDataApi`：`DataApi` trait 的本地文件系统实现。
//!
//! 组装底层 crate 调用，作为默认的数据 API 实现。

use std::collections::HashMap;
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
use cdt_discover::{
    FileSystemProvider, ProjectScanner, SearchConfig, SearchTextCache, SessionSearcher,
    local_handle,
};
use cdt_parse::{ParseError, parse_entry_at, parse_file};
use cdt_ssh::{
    RemotePollingWatcher, RemoteWatcherHandle, SshConnectionManager, SshFileSystemProvider,
    SshSessionManager, default_ssh_config_path, list_hosts, parse_ssh_config_file,
    resolve_host_via_ssh_g,
};
use cdt_watch::FileWatcher;

use super::error::ApiError;
use super::events::SessionMetadataUpdate;
use super::parsed_message_cache::{ParsedMessageCache, extract_parsed_messages_cached};
use super::session_metadata::{
    LOCAL_CACHE_SCOPE, MetadataCache, SessionMetadata, extract_session_metadata_cached_via_fs,
    try_lookup_cached_metadata, try_lookup_cached_metadata_with_signature,
};
use super::traits::DataApi;
use super::types::{
    ConfigUpdateRequest, ContextInfo, MemoryFileContent, MemoryLayer, MemoryLayerKind,
    PaginatedRequest, PaginatedResponse, ProjectInfo, ProjectMemory, ProjectSessionPrefs,
    SearchRequest, SessionDetail, SessionSummary, SshConnectRequest,
};
use crate::cache_signature::{FileIdentity, FileSignature};
use crate::notifier::NotificationPipeline;

/// 元数据扫描的最大并发数。文件扫描是 I/O 密集，8 路并发足够打满 `NVMe`
/// 顺序读且不抢 tokio runtime（详见 design.md decision 2）。
pub const METADATA_SCAN_CONCURRENCY: usize = 8;

/// IPC payload 优化：`get_session_detail` 默认把每个 `Process.messages`
/// 裁剪为空 `Vec`、设 `messages_omitted=true`，砍掉 ~60% payload。前端
/// `SubagentCard` 展开时调 `get_subagent_trace` 懒拉取。
///
/// 紧急回滚：把本常量改为 `false` 即恢复完整 payload；前端 fallback 路径自动生效。
const OMIT_SUBAGENT_MESSAGES: bool = true;

/// IPC payload 优化（phase 3）：`get_session_detail` 默认把所有 `ContentBlock::Image`
/// 的 base64 `data` 替换为空 + 设 `data_omitted=true`，砍掉 image-heavy session 的
/// 大头 payload（实测 7826d1b8 case 4840 KB → ~620 KB）。前端 `ImageBlock`
/// `IntersectionObserver` 进视口时调 `get_image_asset` 懒拉文件 URL。
///
/// 紧急回滚：把本常量改为 `false` 即恢复完整 base64 payload；前端 fallback
/// 走 `data:` URI 路径。行为契约见 change `session-detail-image-asset-cache`。
const OMIT_IMAGE_DATA: bool = true;

/// IPC payload 优化（phase 4）：`get_session_detail` 默认把所有
/// `AIChunk.responses[].content` 替换为空 `MessageContent::Text("")` + 设
/// `content_omitted=true`，砍掉首屏 IPC 最大单一字段（实测 46a25772 case
/// 1257 KB / 41%）。前端无任何代码读 `responses[].content`（chunk 显示文本
/// 走 `semanticSteps`），故无需懒拉接口。
///
/// 紧急回滚：把本常量改为 `false` 即恢复完整 payload；前端零改动也无需 fallback。
/// 行为契约见 change `session-detail-response-content-omit`。
const OMIT_RESPONSE_CONTENT: bool = true;

/// IPC payload 优化（phase 5）：`get_session_detail` 默认把所有
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

/// 派生 `CompactChunk` 的 `token_delta` / `phase_number` 两个可选字段。
///
/// 算法（D1c phaseNumber + D1d tokenDelta，详见 design.md 修订链）：
///
/// - 派生层**完全独立**于 `cdt_core::ContextPhaseInfo`——避开 cdt-analyze 内部
///   `current_phase_compact_group_id` 在连续 compact 时被覆盖的问题
/// - phaseNumber：按 chunks 顺序遍历，每遇 `Compact` 就 `compact_counter += 1`
///   （从 1 起），赋 `Some(counter)`。chunks 中第 i 个 compact → phase i+1
/// - tokenDelta：对每个 compact 独立查 `find_last_ai_before` /
///   `find_first_ai_after`，分别取它们的 last/first response usage 总和算
///   `post - pre`；任一缺值 → `None`
///
/// 两趟扫描避免可变借用冲突：Pass 1 不可变借用算 (delta, phase)，Pass 2 可变借用写入。
/// SSH context 下取代 `LocalGitIdentityResolver` 用于 `list_repository_groups`——
/// 远端无 git 解析能力（不能 spawn 子进程，也不能 SFTP 读容器内 `.git` 因为
/// 大多数远端是非 git 项目），所有 git 字段返回 None / true 兜底。
/// 修复 codex R3 P1[1]：避免容器内 cwd 与本机宿主路径重合时泄漏本地 gitBranch。
struct NoopGitIdentityResolver;

#[async_trait::async_trait]
impl cdt_discover::GitIdentityResolver for NoopGitIdentityResolver {
    async fn resolve_identity(
        &self,
        _path: &std::path::Path,
    ) -> Option<cdt_core::RepositoryIdentity> {
        None
    }

    async fn get_branch(&self, _path: &std::path::Path) -> Option<String> {
        None
    }

    async fn is_main_worktree(&self, _path: &std::path::Path) -> bool {
        true
    }
}

pub(crate) fn parse_jsonl_content(
    content: &str,
) -> Result<Vec<cdt_core::ParsedMessage>, ParseError> {
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
/// `subagent.messages` 在 `OMIT_SUBAGENT_MESSAGES=true` 时已为空，本函数对其
/// 是安全 no-op；`OMIT_SUBAGENT_MESSAGES=false` 回滚时本函数仍能命中嵌套层。
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

/// 遍历 chunks 内所有 `AIChunk.responses[].content`，替换为空
/// `MessageContent::Text("")` 并设 `content_omitted = true`。覆盖顶层
/// `AIChunk` 与 `AIChunk.subagents[].messages[]` 嵌套层。`subagent.messages`
/// 在 `OMIT_SUBAGENT_MESSAGES=true` 时已为空，本函数对其是安全 no-op；
/// `OMIT_SUBAGENT_MESSAGES=false` 回滚时本函数仍能命中嵌套层。
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
/// 覆盖顶层 `AIChunk` 与 `AIChunk.subagents[].messages[]` 嵌套层。
/// `subagent.messages` 在 `OMIT_SUBAGENT_MESSAGES=true` 时已为空，本函数
/// 对其是安全 no-op；`OMIT_SUBAGENT_MESSAGES=false` 回滚时本函数仍能命中
/// 嵌套层。
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
const METADATA_BROADCAST_CAPACITY: usize = 256;

/// 单条 active scan 注册项：generation 作为版本号让 cleanup 时只在
/// 自己仍是当前注册的 scan 时才 remove，避免旧 task 误删新 handle
/// （codex 二审找到的 race，详见 `scan_metadata_for_page`）。
#[derive(Debug)]
struct ScanEntry {
    generation: u64,
    handle: AbortHandle,
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
    watcher_tasks: Mutex<Vec<JoinHandle<()>>>,
    remote_watchers: Mutex<HashMap<String, RemoteWatcherHandle>>,
    ssh_watcher_ops: Mutex<()>,
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
    /// 当前 projects root；随 `general.claudeRootPath` 运行时重配。
    projects_dir: Mutex<PathBuf>,
}

impl LocalDataApi {
    async fn active_scanner(&self) -> Result<ProjectScanner, ApiError> {
        if let Some(context_id) = self.ssh_mgr.active_context_id().await {
            let provider = self
                .ssh_mgr
                .provider(&context_id)
                .await
                .ok_or_else(|| ApiError::not_found(format!("SSH context: {context_id}")))?;
            let projects_dir = provider.remote_home().to_path_buf();
            return Ok(ProjectScanner::new(Arc::new(provider), projects_dir));
        }
        let projects_dir = self.projects_dir.lock().await.clone();
        Ok(ProjectScanner::new(local_handle(), projects_dir))
    }

    async fn active_fs_and_projects_dir(
        &self,
    ) -> Result<(Arc<dyn FileSystemProvider>, PathBuf), ApiError> {
        let (fs, dir, _scope) = self.active_fs_dir_and_scope().await?;
        Ok((fs, dir))
    }

    /// 一次性快照三元组 `(fs, projects_dir, cache_scope)`——避免分两次读
    /// `active_context_id()` 中间被 disconnect/switch 抢断造成 fs 与 scope 错配
    /// （codex 二审 PR #178 V2 必须修 1）。
    ///
    /// `cache_scope = "local"` 或 SSH `context_id`，用于 metadata cache 与
    /// `SessionMetadataUpdate` broadcast 路径的 context 隔离。
    async fn active_fs_dir_and_scope(
        &self,
    ) -> Result<(Arc<dyn FileSystemProvider>, PathBuf, String), ApiError> {
        if let Some(context_id) = self.ssh_mgr.active_context_id().await {
            let provider = self
                .ssh_mgr
                .provider(&context_id)
                .await
                .ok_or_else(|| ApiError::not_found(format!("SSH context: {context_id}")))?;
            let projects_dir = provider.remote_home().to_path_buf();
            return Ok((Arc::new(provider), projects_dir, context_id));
        }
        Ok((
            local_handle(),
            self.projects_dir.lock().await.clone(),
            LOCAL_CACHE_SCOPE.to_owned(),
        ))
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
            watcher_tasks: Mutex::new(Vec::new()),
            remote_watchers: Mutex::new(HashMap::new()),
            ssh_watcher_ops: Mutex::new(()),
            ssh_shutdown_generation: AtomicU64::new(0),
            session_metadata_tx,
            active_scans: Arc::new(std::sync::Mutex::new(HashMap::new())),
            scan_generation: Arc::new(AtomicU64::new(0)),
            root_generation: Arc::new(AtomicU64::new(0)),
            metadata_scan_semaphore: Arc::new(Semaphore::new(METADATA_SCAN_CONCURRENCY)),
            image_cache_dir: None,
            metadata_cache: Arc::new(std::sync::Mutex::new(MetadataCache::default())),
            parsed_msg_cache: Arc::new(std::sync::Mutex::new(ParsedMessageCache::default())),
            projects_dir: Mutex::new(projects_dir),
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
        let (error_tx, _) = broadcast::channel::<DetectedError>(256);
        let (file_tx, _) = broadcast::channel::<cdt_core::FileChangeEvent>(256);
        let (todo_tx, _) = broadcast::channel::<cdt_core::TodoChangeEvent>(256);

        let (session_metadata_tx, _) =
            broadcast::channel::<SessionMetadataUpdate>(METADATA_BROADCAST_CAPACITY);

        // parsed-message cache 主动失效路径：订阅 file-change 广播，按
        // `(project_id, session_id)` 推算主 session JSONL 路径并从 cache 中
        // remove。详 change `parsed-message-lru-cache` design D9；spec
        // `ipc-data-api/spec.md` §"parsed-message 缓存按 file-change 广播主动失效"。
        let parsed_msg_cache = Arc::new(std::sync::Mutex::new(ParsedMessageCache::default()));
        let watcher_tasks = Mutex::new(spawn_watcher_runtime(
            FileWatcher::with_paths(
                projects_dir.clone(),
                todos_dir_from_projects_dir(&projects_dir),
            ),
            config_mgr.clone(),
            notif_mgr.clone(),
            WatcherRuntimeChannels {
                errors: error_tx.clone(),
                files: file_tx.clone(),
                todos: todo_tx.clone(),
            },
            parsed_msg_cache.clone(),
            projects_dir.clone(),
        ));

        let _ = watcher;

        Self {
            scanner: Mutex::new(scanner),
            search_cache,
            config_mgr,
            notif_mgr,
            ssh_mgr: SshSessionManager::new(),
            error_tx: Some(error_tx),
            file_tx: Some(file_tx),
            todo_tx: Some(todo_tx),
            watcher_tasks,
            remote_watchers: Mutex::new(HashMap::new()),
            ssh_watcher_ops: Mutex::new(()),
            ssh_shutdown_generation: AtomicU64::new(0),
            session_metadata_tx,
            active_scans: Arc::new(std::sync::Mutex::new(HashMap::new())),
            scan_generation: Arc::new(AtomicU64::new(0)),
            root_generation: Arc::new(AtomicU64::new(0)),
            metadata_scan_semaphore: Arc::new(Semaphore::new(METADATA_SCAN_CONCURRENCY)),
            image_cache_dir: None,
            metadata_cache: Arc::new(std::sync::Mutex::new(MetadataCache::default())),
            parsed_msg_cache,
            projects_dir: Mutex::new(projects_dir),
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

    pub fn subscribe_ssh_status(&self) -> broadcast::Receiver<cdt_ssh::SshStatusChange> {
        self.ssh_mgr.subscribe_status()
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
            .insert_test_context(context_id, host, port, username, remote_home, provider)
            .await;
    }

    pub fn subscribe_context_changed(&self) -> broadcast::Receiver<cdt_ssh::ContextChanged> {
        self.ssh_mgr.subscribe_context_changed()
    }

    pub async fn shutdown_ssh_all(&self, deadline: std::time::Duration) {
        self.ssh_shutdown_generation.fetch_add(1, Ordering::SeqCst);
        let Ok(_ops) = self.ssh_watcher_ops.try_lock() else {
            self.ssh_mgr.shutdown_all(deadline).await;
            return;
        };
        self.cancel_all_remote_watchers().await;
        self.ssh_mgr.shutdown_all(deadline).await;
    }

    async fn attach_remote_watcher(&self, context_id: &str) {
        let Some(file_tx) = self.file_tx.as_ref() else {
            return;
        };
        let Some(provider) = self.ssh_mgr.provider(context_id).await else {
            return;
        };
        let watcher = RemotePollingWatcher::spawn(
            provider.sftp_client(),
            provider.remote_home().to_path_buf(),
            file_tx.clone(),
            cdt_ssh::CancelToken::new(),
        );
        if let Some(old) = self
            .remote_watchers
            .lock()
            .await
            .insert(context_id.to_owned(), watcher)
        {
            old.cancel_and_join().await;
        }
    }

    async fn cancel_remote_watcher(&self, context_id: &str) {
        if let Some(handle) = self.remote_watchers.lock().await.remove(context_id) {
            handle.cancel_and_join().await;
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

    async fn reconfigure_claude_root(&self, claude_root_path: Option<&str>) {
        let claude_root = claude_root_path.map(PathBuf::from);
        let projects_dir =
            cdt_discover::path_decoder::projects_base_path_for(claude_root.as_deref());
        {
            let mut active = self
                .active_scans
                .lock()
                .expect("active_scans lock poisoned");
            for entry in active.values() {
                entry.handle.abort();
            }
            active.clear();
        }
        *self.scanner.lock().await = ProjectScanner::new(local_handle(), projects_dir.clone());
        *self.projects_dir.lock().await = projects_dir.clone();

        if let (Some(error_tx), Some(file_tx), Some(todo_tx)) =
            (&self.error_tx, &self.file_tx, &self.todo_tx)
        {
            let mut tasks = self.watcher_tasks.lock().await;
            for task in tasks.drain(..) {
                task.abort();
            }
            let claude_root = claude_root_path.map(PathBuf::from);
            let todos_dir = cdt_discover::path_decoder::todos_base_path_for(claude_root.as_deref());
            *tasks = spawn_watcher_runtime(
                FileWatcher::with_paths(projects_dir.clone(), todos_dir),
                self.config_mgr.clone(),
                self.notif_mgr.clone(),
                WatcherRuntimeChannels {
                    errors: error_tx.clone(),
                    files: file_tx.clone(),
                    todos: todo_tx.clone(),
                },
                self.parsed_msg_cache.clone(),
                projects_dir,
            );
        }
        self.root_generation.fetch_add(1, Ordering::SeqCst);
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
    /// 返回 (page, `next_cursor`, total, `page_jobs`, dir)。
    ///
    /// `page_jobs` 是 `(session_id, jsonl_path)` 元组列表，供后台元数据扫描
    /// 任务消费。
    async fn list_sessions_skeleton(
        &self,
        project_id: &str,
        pagination: &PaginatedRequest,
    ) -> Result<
        (
            Vec<SessionSummary>,
            Option<String>,
            usize,
            Vec<(String, std::path::PathBuf)>,
            std::path::PathBuf,
            u64,
            Arc<dyn cdt_discover::FileSystemProvider>,
            String,
        ),
        ApiError,
    > {
        if pagination.page_size == 0 {
            return Err(ApiError::validation("pageSize must be > 0"));
        }

        // 一次性快照 (fs, projects_dir, cache_scope)，避免两次独立读 active context
        // 中间被 disconnect/switch 抢断让 fs 与 scope 错配（codex 二审 V2 必须修 1）。
        let (fs, projects_dir, cache_scope) = self.active_fs_dir_and_scope().await?;
        let is_remote = fs.kind() == cdt_discover::FsKind::Ssh;
        let mut scanner = ProjectScanner::new(fs.clone(), projects_dir.clone());
        let _ = scanner
            .scan()
            .await
            .map_err(|e| ApiError::internal(format!("scan error: {e}")))?;
        let sessions = scanner
            .list_sessions(project_id, &std::collections::BTreeSet::new())
            .await
            .map_err(|e| ApiError::internal(format!("list sessions error: {e}")))?;
        let projects_dir = scanner.projects_dir().to_path_buf();
        let root_generation = self.root_generation.load(Ordering::SeqCst);

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
        // Cache fast-path：命中条骨架阶段直接带 title / messageCount 返回，避免
        // 完全依赖后台 broadcast emit（如果 emit 在前端 listener 注册前 fire-and-forget
        // 丢失，列表项会卡在 title=null 永久 fallback 到 sessionId 前 8 字符）。
        // 未命中条仍入 page_jobs 走后台扫描，扫完通过 broadcast 增量 patch。
        //
        // 并发执行 stat + cache lookup：caller 可传任意大 page_size（如
        // `list_all_sessions` 路径 50 条/页），串行 stat 累计 ms 数随 page 线性
        // 放大；用 `join_all` 让 tokio runtime 并发调度 stat。lookup 内部仅做
        // sync mutex lock + map lookup，相互不会阻塞。并发上限用 `Semaphore`
        // 卡到 `METADATA_SCAN_CONCURRENCY`，避免 caller 传超大 page_size 时
        // 一次性把 tokio blocking pool 占满（codex 二审 Q3）。
        let lookup_permit = Arc::new(Semaphore::new(METADATA_SCAN_CONCURRENCY));
        // SSH 路径用 Session.last_modified+size 直接构造 signature，避免每条
        // 都发一次 SFTP stat（SFTP RTT ~50-200ms，page=20 串行就是几秒）。
        // 本地路径仍走 tokio::fs::metadata 的 fast-path（识别 inode 维度）。
        let lookups = futures::future::join_all(page_sessions.iter().map(|s| {
            let cache = self.metadata_cache.clone();
            let jsonl_path = dir.join(format!("{}.jsonl", s.id));
            let permit_sem = lookup_permit.clone();
            let scope = cache_scope.clone();
            let session_sig = if is_remote {
                Some(FileSignature {
                    mtime: epoch_ms_to_system_time(s.last_modified),
                    size: s.size,
                    identity: FileIdentity::None,
                })
            } else {
                None
            };
            async move {
                let _guard = permit_sem
                    .acquire()
                    .await
                    .expect("lookup semaphore should not be closed");
                let meta = if let Some(sig) = session_sig {
                    try_lookup_cached_metadata_with_signature(&cache, &scope, &jsonl_path, sig)
                        .await
                } else {
                    try_lookup_cached_metadata(&cache, &jsonl_path).await
                };
                (jsonl_path, meta)
            }
        }))
        .await;
        for (s, (jsonl_path, cached_meta)) in page_sessions.into_iter().zip(lookups) {
            if let Some(meta) = cached_meta {
                page.push(SessionSummary {
                    session_id: s.id,
                    project_id: project_id.to_owned(),
                    timestamp: s.last_modified,
                    message_count: meta.message_count,
                    title: meta.title,
                    is_ongoing: meta.is_ongoing,
                    git_branch: meta.git_branch,
                    worktree_id: None,
                    worktree_name: None,
                });
            } else {
                page.push(SessionSummary {
                    session_id: s.id.clone(),
                    project_id: project_id.to_owned(),
                    timestamp: s.last_modified,
                    message_count: 0,
                    title: None,
                    is_ongoing: false,
                    git_branch: None,
                    worktree_id: None,
                    worktree_name: None,
                });
                // 本地 + SSH 都进 page_jobs：让后台扫描走 fs trait 分流
                // （`scan_metadata_for_page` 内部按 fs.kind() 选 local/SSH 路径）。
                // 历史 PR #176 让 SSH 走骨架阶段同步 read_to_string，但读取
                // 失败时 fallback 不进 page_jobs → 永久卡骨架；改后 SSH 也走
                // 后台异步扫描 + broadcast SessionMetadataUpdate 增量 patch。
                page_jobs.push((s.id, jsonl_path));
            }
        }

        let page_len = page.len();
        let next_cursor = if offset + page_len < total {
            Some((offset + page_len).to_string())
        } else {
            None
        };

        Ok((
            page,
            next_cursor,
            total,
            page_jobs,
            dir,
            root_generation,
            fs,
            cache_scope,
        ))
    }
}

/// 把 epoch milliseconds 转成 `SystemTime`，便于从 `Session.last_modified` 构造
/// `FileSignature`。负数（理论上不应出现）退化为 `UNIX_EPOCH`。
fn epoch_ms_to_system_time(ms: i64) -> std::time::SystemTime {
    let Ok(ms_u64) = u64::try_from(ms) else {
        return std::time::SystemTime::UNIX_EPOCH;
    };
    std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(ms_u64)
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
fn metadata_scan_key(project_id: &str, cursor: Option<&str>) -> String {
    format!("{project_id}|{}", cursor.unwrap_or(""))
}

fn todos_dir_from_projects_dir(projects_dir: &Path) -> PathBuf {
    projects_dir.parent().map_or_else(
        || projects_dir.join(".."),
        |claude_root| claude_root.join("todos"),
    )
}

struct WatcherRuntimeChannels {
    errors: broadcast::Sender<DetectedError>,
    files: broadcast::Sender<cdt_core::FileChangeEvent>,
    todos: broadcast::Sender<cdt_core::TodoChangeEvent>,
}

fn spawn_watcher_runtime(
    watcher: FileWatcher,
    config_mgr: Arc<Mutex<ConfigManager>>,
    notif_mgr: Arc<Mutex<NotificationManager>>,
    channels: WatcherRuntimeChannels,
    parsed_msg_cache: Arc<std::sync::Mutex<ParsedMessageCache>>,
    projects_dir: PathBuf,
) -> Vec<JoinHandle<()>> {
    let watcher = Arc::new(watcher);
    let watcher_for_start = watcher.clone();
    let start_task = tokio::spawn(async move {
        if let Err(err) = watcher_for_start.start().await {
            tracing::warn!(error = %err, "FileWatcher terminated");
        }
    });

    let mut bridge_rx = watcher.subscribe_files();
    let bridge_task = tokio::spawn(async move {
        loop {
            match bridge_rx.recv().await {
                Ok(event) => {
                    let _ = channels.files.send(event);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let mut todo_rx = watcher.subscribe_todos();
    let todo_bridge_task = tokio::spawn(async move {
        loop {
            match todo_rx.recv().await {
                Ok(event) => {
                    let _ = channels.todos.send(event);
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

    let invalidator_task = spawn_parsed_msg_cache_invalidator(
        parsed_msg_cache,
        watcher.subscribe_files(),
        projects_dir,
    );

    vec![
        start_task,
        bridge_task,
        todo_bridge_task,
        notifier_task,
        invalidator_task,
    ]
}

/// 启动一个后台 task，订阅 `FileWatcher::subscribe_files()` 广播，对每条
/// `FileChangeEvent` 按 `projects_dir / project_id / "{session_id}.jsonl"`
/// 推算主 session JSONL 路径，**stat 比对当前 `FileSignature` 与 cache 中记录**，
/// 仅在 signature 不一致时才从 parsed-message cache 中 remove。
///
/// 这层 stat 比对避免 watcher spurious 事件（CI 上 inotify 启动期对刚创建的
/// watch dir 偶发"无内容变化"事件、metadata-only touch 等）错杀有效 cache。
/// stat 失败（文件被删 / 权限）走保守 `remove` —— 反正下次 lookup 也 stat fail，
/// 提前清掉 cache entry 不影响正确性。
///
/// 行为契约：spec `ipc-data-api/spec.md` §"parsed-message 缓存按 file-change
/// 广播主动失效"。`broadcast::Receiver::recv` 返回 `Lagged` 时静默 continue
/// （下次 lookup 由被动 `FileSignature` mismatch 兜底）；`Closed` 时退出 loop。
///
/// **限制**（详 design D9 risks）：subagent JSONL 路径
/// （`<project>/<session>/subagents/agent-*.jsonl`）的失效仅靠被动签名兜底——
/// `FileChangeEvent` 把嵌套 subagent 改动路由到父 `session_id`，本 task 无法
/// 还原具体 `agent-<sub_id>.jsonl` 路径。
fn spawn_parsed_msg_cache_invalidator(
    cache: Arc<std::sync::Mutex<ParsedMessageCache>>,
    mut rx: broadcast::Receiver<cdt_core::FileChangeEvent>,
    projects_dir: std::path::PathBuf,
) -> JoinHandle<()> {
    use crate::cache_signature::FileSignature;
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(evt) => {
                    // 顶层 dir-create 事件 session_id 为空，无主 JSONL 路径可推算，跳过。
                    if evt.session_id.is_empty() {
                        continue;
                    }
                    let path = projects_dir
                        .join(&evt.project_id)
                        .join(format!("{}.jsonl", evt.session_id));
                    match tokio::fs::metadata(&path).await {
                        Ok(meta) => {
                            let current_sig = FileSignature::from_metadata(&meta);
                            cache
                                .lock()
                                .expect("parsed message cache mutex poisoned")
                                .remove_if_signature_mismatch(&path, &current_sig);
                        }
                        Err(_) => {
                            // 文件已删或 stat 失败：保守 remove，反正下次 lookup
                            // 也会 stat fail，提前清掉不影响正确性。
                            cache
                                .lock()
                                .expect("parsed message cache mutex poisoned")
                                .remove(&path);
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // lag 仅代表事件激增；下次 lookup 由被动 FileSignature 兜底
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
    fs: Arc<dyn cdt_discover::FileSystemProvider>,
    expected_cache_scope: String,
    ssh_mgr: SshSessionManager,
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
        let fs = fs.clone();
        let scope = expected_cache_scope.clone();
        let ssh_mgr = ssh_mgr.clone();
        set.spawn(async move {
            let Ok(_permit) = permit_sem.acquire_owned().await else {
                return;
            };
            let meta =
                extract_session_metadata_cached_via_fs(&cache, &fs, &scope, &jsonl_path).await;
            if root_generation.load(Ordering::SeqCst) != expected_root_generation {
                return;
            }
            // codex 二审 PR #178 必须修 2：active context 已切走（用户 disconnect 或
            // switch 到别的 context）就不再 broadcast——避免旧 context 的
            // metadata patch 错误的 sidebar 数据。比对当前 `active_context_id`
            // 与启动 task 时记录的 expected_cache_scope。
            let current_scope = ssh_mgr
                .active_context_id()
                .await
                .unwrap_or_else(|| LOCAL_CACHE_SCOPE.to_owned());
            if current_scope != scope {
                tracing::debug!(
                    project_id,
                    session_id,
                    expected = %scope,
                    actual = %current_scope,
                    "skip session-metadata-update broadcast: active context changed"
                );
                return;
            }
            let _ = tx.send(SessionMetadataUpdate {
                project_id,
                session_id,
                title: meta.title,
                message_count: meta.message_count,
                is_ongoing: meta.is_ongoing,
                git_branch: meta.git_branch,
                // 携带 expected scope 让前端二次过滤——emit-time check 与 send
                // 之间仍有 TOCTOU 窗口（codex 二审 V2 必须修 2）。前端 listener 用
                // `contextStore.activeContextId` 比对，不匹配就丢弃。
                context_id: Some(scope.clone()),
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

#[async_trait]
impl DataApi for LocalDataApi {
    // =========================================================================
    // 项目 + 会话
    // =========================================================================

    async fn list_projects(&self) -> Result<Vec<ProjectInfo>, ApiError> {
        let mut scanner = self.active_scanner().await?;
        let projects = scanner
            .scan()
            .await
            .map_err(|e| ApiError::internal(format!("scan error: {e}")))?;

        Ok(projects
            .into_iter()
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
        let (mut page, next_cursor, total, page_jobs, _dir, _root_generation, fs, cache_scope) =
            self.list_sessions_skeleton(project_id, pagination).await?;

        // 并发提取每条 session 的 metadata：复用 `self.metadata_scan_semaphore`
        // 与 async `list_sessions` 共享同一把 8 容量信号量，保证 spec
        // `ipc-data-api/spec.md::Emit session metadata updates` "后台扫描并发度
        // 限制" Scenario 的全局 8 上限——HTTP 路径与 async 路径并发时也不会
        // 累加成 16+ 并发。metadata cache 内部有锁，多 task 共享安全。结果按
        // page_jobs 顺序一一映射回 page。
        let metas = futures::future::join_all(page_jobs.iter().map(|(id, path)| {
            let sem = self.metadata_scan_semaphore.clone();
            let cache = self.metadata_cache.clone();
            let path = path.clone();
            let fs = fs.clone();
            let id = id.clone();
            let scope = cache_scope.clone();
            async move {
                let _permit = sem.acquire_owned().await.ok()?;
                Some((
                    id,
                    extract_session_metadata_cached_via_fs(&cache, &fs, &scope, &path).await,
                ))
            }
        }))
        .await;
        // 按 sessionId 建索引：page_jobs 只包含 cache miss 的 session，page 还
        // 含 cache hit 的——之前用 `page.iter_mut().zip(metas)` 在 SSH 也走
        // page_jobs 后会把 metas 错位写到 hit 的 page 项上（hit 的 metadata 被
        // miss 的 metadata 覆盖）。改成 by-id lookup 安全。
        let metas_by_id: HashMap<String, SessionMetadata> = metas.into_iter().flatten().collect();
        for summary in &mut page {
            let Some(meta) = metas_by_id.get(&summary.session_id) else {
                continue;
            };
            summary.title = meta.title.clone();
            summary.message_count = meta.message_count;
            summary.is_ongoing = meta.is_ongoing;
            summary.git_branch = meta.git_branch.clone();
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
        let (page, next_cursor, total, page_jobs, dir, root_generation, fs, cache_scope) =
            self.list_sessions_skeleton(project_id, pagination).await?;

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

                let handle = tokio::spawn(scan_metadata_for_page(
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
                    fs,
                    cache_scope,
                    self.ssh_mgr.clone(),
                ));

                scans.insert(
                    scan_key,
                    ScanEntry {
                        generation: my_generation,
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
                (
                    session.id.clone(),
                    SessionSummary {
                        session_id: session.id,
                        project_id: project_id.to_owned(),
                        timestamp: session.last_modified,
                        message_count: 0,
                        title: None,
                        is_ongoing: false,
                        git_branch: None,
                        worktree_id: None,
                        worktree_name: None,
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();

        Ok(session_ids
            .iter()
            .filter_map(|id| by_id.remove(id))
            .collect())
    }

    async fn get_project_memory(&self, project_id: &str) -> Result<ProjectMemory, ApiError> {
        // SSH context 下 graceful degradation：远端 memory 文件读取暂不支持
        // （`discover_memory_layers` / `validate_memory_file_name` 内部走
        // 同步 tokio::fs，无法直接复用走远端 SFTP）。返回 has_memory=false
        // 让 UI 显示"无 memory"而非读宿主机错误数据。
        // followup：openspec/followups.md 已记 SSH context memory 支持（TODO）
        let (fs, _projects_dir) = self.active_fs_and_projects_dir().await?;
        if fs.kind() == cdt_discover::FsKind::Ssh {
            return Ok(ProjectMemory {
                project_id: project_id.to_owned(),
                has_memory: false,
                count: 0,
                default_file: None,
                layers: Vec::new(),
            });
        }
        let memory_dir = self.project_memory_dir(project_id).await?;
        let layers = discover_memory_layers(&memory_dir).await?;
        let default_file = layers
            .iter()
            .find(|layer| layer.kind == MemoryLayerKind::Index)
            .or_else(|| layers.first())
            .map(|layer| layer.file.clone());
        Ok(ProjectMemory {
            project_id: project_id.to_owned(),
            has_memory: !layers.is_empty(),
            count: layers.len(),
            default_file,
            layers,
        })
    }

    async fn read_memory_file(
        &self,
        project_id: &str,
        file: &str,
    ) -> Result<MemoryFileContent, ApiError> {
        // SSH context 下 graceful degradation——同 get_project_memory 同款理由。
        let (fs, _projects_dir) = self.active_fs_and_projects_dir().await?;
        if fs.kind() == cdt_discover::FsKind::Ssh {
            return Err(ApiError::not_found(format!(
                "memory file {file}: SSH context 下远端 memory 文件读取尚不支持"
            )));
        }
        let memory_dir = self.project_memory_dir(project_id).await?;
        let safe_file = validate_memory_file_name(file)?;
        let path = memory_dir.join(&safe_file);
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ApiError::not_found(format!("memory file {safe_file}: {e}")))?;
        Ok(MemoryFileContent {
            project_id: project_id.to_owned(),
            file: safe_file,
            file_path: path.to_string_lossy().into_owned(),
            content,
        })
    }

    async fn get_session_detail(
        &self,
        project_id: &str,
        session_id: &str,
    ) -> Result<SessionDetail, ApiError> {
        // 性能探针：拆 5 段计时——locate / parse / scan_subagents / build_chunks /
        // context+claude_md / serialize。`tracing::info!` 由订阅者过滤，开销极低。
        // 不要把这些段塞进一个汇总 log——分开打才能据此判断瓶颈走向。
        let t_total = std::time::Instant::now();

        // 路径解析以 scanner 持有的 projects_dir 为准，让集成测试可用 tmp 目录。
        // `path_decoder::get_projects_base_path()` 仅在 `ProjectScanner` 自身
        // 用默认路径构造时返回真实 home，scanner 已经统一了入口。
        let t_locate = std::time::Instant::now();
        let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        let mut scanner = ProjectScanner::new(fs.clone(), projects_dir.clone());
        let _ = scanner
            .scan()
            .await
            .map_err(|e| ApiError::internal(format!("scan error: {e}")))?;
        let project_dir = projects_dir.join(project_id);
        let sessions = scanner
            .list_sessions(project_id, &std::collections::BTreeSet::new())
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        let is_remote = fs.kind() == cdt_discover::FsKind::Ssh;

        let main_session = sessions.iter().find(|s| s.id == session_id);

        let (jsonl_path, last_modified, size) = if let Some(s) = main_session {
            (
                project_dir.join(format!("{session_id}.jsonl")),
                Some(s.last_modified),
                Some(s.size),
            )
        } else if !is_remote {
            let Some(path) = find_subagent_jsonl(&project_dir, session_id).await else {
                return Err(ApiError::not_found(format!("session {session_id}")));
            };
            let meta = tokio::fs::metadata(&path).await.ok();
            let modified = meta.as_ref().and_then(|m| m.modified().ok()).map(|t| {
                let dt: chrono::DateTime<chrono::Utc> = t.into();
                dt.timestamp_millis()
            });
            let size = meta.as_ref().map(std::fs::Metadata::len);
            (path, modified, size)
        } else {
            return Err(ApiError::not_found(format!("session {session_id}")));
        };
        let locate_ms = t_locate.elapsed().as_millis();

        let t_parse = std::time::Instant::now();
        let messages = if is_remote {
            let content = fs
                .read_to_string(&jsonl_path)
                .await
                .map_err(|e| ApiError::internal(format!("read error: {e}")))?;
            parse_jsonl_content(&content)
                .map_err(|e| ApiError::internal(format!("parse error: {e}")))?
        } else {
            parse_file(&jsonl_path)
                .await
                .map_err(|e| ApiError::internal(format!("parse error: {e}")))?
        };
        let parse_ms = t_parse.elapsed().as_millis();
        let message_count = messages.len();

        let t_scan = std::time::Instant::now();
        let candidates = if is_remote {
            Vec::new()
        } else if CROSS_PROJECT_SUBAGENT_SCAN {
            scan_subagent_candidates_cross_project(&projects_dir, &project_dir, session_id).await
        } else {
            scan_subagent_candidates(&project_dir, session_id).await
        };
        let scan_ms = t_scan.elapsed().as_millis();
        let candidate_count = candidates.len();

        let t_build = std::time::Instant::now();
        let messages_ongoing = cdt_analyze::check_messages_ongoing(&messages);
        // stale check 与 list_sessions 路径对齐（issue #94）：mtime > 5min 的
        // ongoing 视为 crashed/killed。
        let is_ongoing = if messages_ongoing && !is_remote {
            !crate::ipc::session_metadata::is_file_stale(&jsonl_path).await
        } else {
            messages_ongoing
        };
        let chunks = build_chunks_with_subagents(&messages, &candidates);
        let build_ms = t_build.elapsed().as_millis();
        let chunk_count = chunks.len();

        let t_ctx = std::time::Instant::now();
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
            &chunks,
            &cdt_analyze::context::ProcessSessionParams {
                project_root: Path::new(""),
                token_dictionaries: token_dicts,
                initial_claude_md_injections: &initial_claude_md,
            },
        );
        // 按 phase 切分 accumulated injections。每 phase 末尾的 AI group 在
        // stats_map 中持有该 phase 完整 accumulated（session.rs backfill 保证），
        // 中间 group 的 accumulated_injections 是 std::mem::take 后的空 Vec。
        let mut injections_by_phase = serde_json::Map::new();
        for phase in &ctx_result.phase_info.phases {
            let phase_injections = ctx_result
                .stats_map
                .get(&phase.last_ai_group_id)
                .map(|stats| stats.accumulated_injections.clone())
                .unwrap_or_default();
            injections_by_phase.insert(
                phase.phase_number.to_string(),
                serde_json::to_value(&phase_injections)
                    .unwrap_or(serde_json::Value::Array(Vec::new())),
            );
        }
        // contextInjections 字段语义：latest phase 的 accumulated injections
        // （向后兼容旧前端，等价于 injectionsByPhase[最大 phaseNumber]）。
        let context_injections = ctx_result
            .phase_info
            .phases
            .last()
            .and_then(|phase| ctx_result.stats_map.get(&phase.last_ai_group_id))
            .map(|stats| stats.accumulated_injections.clone())
            .and_then(|inj| serde_json::to_value(&inj).ok())
            .unwrap_or(serde_json::Value::Array(Vec::new()));
        let injections_by_phase_value = serde_json::Value::Object(injections_by_phase);
        let phase_info_value =
            serde_json::to_value(&ctx_result.phase_info).unwrap_or(serde_json::Value::Null);
        let ctx_ms = t_ctx.elapsed().as_millis();

        let t_serde = std::time::Instant::now();
        // IPC payload 瘦身：subagent.messages 默认裁剪为空，前端 SubagentCard
        // 展开时通过 `get_subagent_trace` 懒拉取。把 messages 抠空 + 设
        // `messages_omitted=true`；header_model / last_isolated_tokens /
        // is_shutdown_only 已由 resolver 阶段填充，可独立渲染 header。
        // ctx_result 已只持有 owned 数据（不借 chunks），此后 chunks 在本函数内
        // 不再被读取，可直接 move 进 OMIT pipeline 原地修改，省 1 次 Vec<Chunk>
        // 深拷贝（实测 1221 msg 会话 ~5-15ms + 数 MB 瞬态堆）。
        let chunks_for_payload = {
            let mut chunks = chunks;
            // CompactChunk 派生 token_delta / phase_number 必须在 OMIT 之前跑——
            // OMIT 不改 chunk 顺序也不改 responses[i].usage，前后顺序对算法本身
            // 无影响；但放在 OMIT 之前更接近"chunks 落定 → 派生 metadata → 应用
            // OMIT 瘦身"的清晰流水线。详见 change `compact-chunk-rendering-alignment`
            // 的 design.md D1c+D1d。
            apply_compact_derived(&mut chunks, COMPACT_DERIVED_ENABLED);
            // phase 3：image base64 OMIT 必须在 subagent OMIT 之前跑，否则
            // OMIT_SUBAGENT_MESSAGES=false 回滚路径下嵌套 messages 内的 image
            // 不会被裁。
            if OMIT_IMAGE_DATA {
                apply_image_omit(&mut chunks);
            }
            // phase 4：response.content OMIT 同样在 subagent OMIT 之前跑，
            // 覆盖 OMIT_SUBAGENT_MESSAGES=false 回滚路径下嵌套 messages 内的
            // AIChunk.responses[].content。
            if OMIT_RESPONSE_CONTENT {
                apply_response_content_omit(&mut chunks);
            }
            // phase 5：tool_exec.output OMIT 同上，覆盖嵌套 messages 内的
            // tool_executions[].output。
            if OMIT_TOOL_OUTPUT {
                apply_tool_output_omit(&mut chunks);
            }
            if OMIT_SUBAGENT_MESSAGES {
                for c in &mut chunks {
                    if let cdt_core::Chunk::Ai(ai) = c {
                        for sub in &mut ai.subagents {
                            sub.messages = Vec::new();
                            sub.messages_omitted = true;
                        }
                    }
                }
            }
            chunks
        };
        let detail = SessionDetail {
            session_id: session_id.to_owned(),
            project_id: project_id.to_owned(),
            chunks: serde_json::to_value(&chunks_for_payload).unwrap_or_default(),
            metrics: serde_json::json!({"message_count": message_count}),
            metadata: serde_json::json!({
                "last_modified": last_modified,
                "size": size,
            }),
            context_injections,
            injections_by_phase: injections_by_phase_value,
            phase_info: phase_info_value,
            is_ongoing,
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

        Ok(detail)
    }

    async fn find_session_project(&self, session_id: &str) -> Result<Option<String>, ApiError> {
        // FS 直扫覆盖默认 trait 实现（避免 O(项目数 × 会话数) 的全量
        // list_sessions）。匹配三种结构：
        //   - 主会话：`<projects_dir>/<encoded>/<session_id>.jsonl`
        //   - legacy subagent：`<projects_dir>/<encoded>/agent-<session_id>.jsonl`
        //   - 新结构 subagent：`<projects_dir>/<encoded>/<parent>/subagents/agent-<session_id>.jsonl`
        // 与 `find_subagent_jsonl` + `get_session_detail` 的查找口径一致。
        let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        let is_remote = fs.kind() == cdt_discover::FsKind::Ssh;
        let main_filename = format!("{session_id}.jsonl");
        if is_remote {
            // SSH 路径：通过 active provider 走远端 SFTP read_dir + exists
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
                // 远端 subagent 仅扫新结构 `<project_dir>/*/subagents/agent-<sid>.jsonl`
                // 旧 flat 结构需要逐文件 stat，远端 latency 不可接受——跳过
                if find_subagent_jsonl_via_fs(&*fs, &project_dir, session_id)
                    .await
                    .is_some()
                {
                    return Ok(Some(entry.name));
                }
            }
            return Ok(None);
        }
        // local 路径保留原 tokio::fs 实现
        let Ok(mut entries) = tokio::fs::read_dir(&projects_dir).await else {
            return Ok(None);
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let project_dir = entry.path();
            // 主会话快路径
            if tokio::fs::metadata(project_dir.join(&main_filename))
                .await
                .is_ok()
            {
                return Ok(entry.file_name().to_str().map(String::from));
            }
            // subagent 慢路径（与 `get_session_detail` fallback 一致）
            if find_subagent_jsonl(&project_dir, session_id)
                .await
                .is_some()
            {
                return Ok(entry.file_name().to_str().map(String::from));
            }
        }
        Ok(None)
    }

    async fn get_subagent_trace(
        &self,
        root_session_id: &str,
        subagent_session_id: &str,
    ) -> Result<serde_json::Value, ApiError> {
        // `get_subagent_trace` 调用方（Tauri command）只携带 sessionId，
        // 不带 projectId，所以需跨 `projects_dir` 扫。优先按
        // `{projects_dir}/*/{root_session_id}/subagents/agent-<sub>.jsonl`
        // 全局扫（新结构）；旧结构 fallback 走"找到 root jsonl 所在 project_dir
        // 后在该目录内查 flat agent jsonl"。
        //
        // 使用当前 active context 的 fs + projects_dir，避免 root 切换后继续扫描旧目录。
        let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        let is_remote = fs.kind() == cdt_discover::FsKind::Ssh;
        let (path, content): (std::path::PathBuf, Option<String>) = if is_remote {
            // SSH 路径：扫 projects_dir 找 root jsonl 所在 project_dir 后
            // 探测 `<project_dir>/<root>/subagents/agent-<sub>.jsonl` 新结构
            // （旧 flat 结构远端不支持——见 find_session_project 同款理由）
            let filename = format!("agent-{subagent_session_id}.jsonl");
            let Ok(entries) = fs.read_dir(&projects_dir).await else {
                return Ok(serde_json::Value::Array(Vec::new()));
            };
            let mut found: Option<std::path::PathBuf> = None;
            for entry in entries {
                if !entry.kind.is_dir() {
                    continue;
                }
                let project_dir = projects_dir.join(&entry.name);
                let candidate = project_dir
                    .join(root_session_id)
                    .join("subagents")
                    .join(&filename);
                if fs.exists(&candidate).await {
                    found = Some(candidate);
                    break;
                }
            }
            let Some(p) = found else {
                return Ok(serde_json::Value::Array(Vec::new()));
            };
            let body = fs
                .read_to_string(&p)
                .await
                .map_err(|e| ApiError::internal(format!("read error: {e}")))?;
            (p, Some(body))
        } else {
            let new_structure_path = if CROSS_PROJECT_SUBAGENT_SCAN {
                find_subagent_jsonl_cross_project(
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
                // 旧结构兜底：找 root jsonl 所在 project_dir 后在该目录扫 flat。
                let Ok(mut entries) = tokio::fs::read_dir(&projects_dir).await else {
                    return Ok(serde_json::Value::Array(Vec::new()));
                };
                let mut fallback: Option<std::path::PathBuf> = None;
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let project_dir = entry.path();
                    let root_jsonl = project_dir.join(format!("{root_session_id}.jsonl"));
                    if tokio::fs::metadata(&root_jsonl).await.is_ok() {
                        if let Some(p) =
                            find_subagent_jsonl(&project_dir, subagent_session_id).await
                        {
                            fallback = Some(p);
                        }
                        break;
                    }
                }
                let Some(p) = fallback else {
                    return Ok(serde_json::Value::Array(Vec::new()));
                };
                p
            };
            (path, None)
        };
        let messages = if let Some(body) = content {
            parse_jsonl_content(&body)
                .map_err(|e| ApiError::internal(format!("parse error: {e}")))?
        } else {
            parse_file(&path)
                .await
                .map_err(|e| ApiError::internal(format!("parse error: {e}")))?
        };
        let mut msgs = messages;
        for m in &mut msgs {
            m.is_sidechain = false;
        }
        let chunks = cdt_analyze::build_chunks(&msgs);
        serde_json::to_value(&chunks).map_err(|e| ApiError::internal(format!("{e}")))
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
        let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        let is_remote = fs.kind() == cdt_discover::FsKind::Ssh;
        let messages = if is_remote {
            let Some(jsonl_path) =
                locate_session_jsonl_via_fs(&*fs, &projects_dir, root_session_id, session_id).await
            else {
                tracing::warn!(target: "cdt_api::image", root_session_id, session_id, "jsonl not found (ssh)");
                return Ok(empty_data_uri());
            };
            let body = match fs.read_to_string(&jsonl_path).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(target: "cdt_api::image", error = %e, "ssh read jsonl failed; returning empty data URI");
                    return Ok(empty_data_uri());
                }
            };
            match parse_jsonl_content(&body) {
                Ok(m) => std::sync::Arc::new(m),
                Err(e) => {
                    tracing::warn!(target: "cdt_api::image", error = %e, "ssh parse failed; returning empty data URI");
                    return Ok(empty_data_uri());
                }
            }
        } else {
            let Some(jsonl_path) =
                locate_session_jsonl(&projects_dir, root_session_id, session_id).await
            else {
                tracing::warn!(target: "cdt_api::image", root_session_id, session_id, "jsonl not found");
                return Ok(empty_data_uri());
            };
            // parse 整个文件 → 找 chunk_uuid → 取 block_index 的 image。
            // 走 parsed-message LRU cache：命中时复用 Arc<Vec<ParsedMessage>>，
            // 跳过整文件 line-by-line parse；详 change `parsed-message-lru-cache`。
            let Some(messages) =
                extract_parsed_messages_cached(&self.parsed_msg_cache, &jsonl_path).await
            else {
                tracing::warn!(target: "cdt_api::image", "parse failed or stat error; returning empty data URI");
                return Ok(empty_data_uri());
            };
            messages
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
        let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        let is_remote = fs.kind() == cdt_discover::FsKind::Ssh;
        let messages = if is_remote {
            let Some(jsonl_path) =
                locate_session_jsonl_via_fs(&*fs, &projects_dir, root_session_id, session_id).await
            else {
                tracing::warn!(target: "cdt_api::tool_output", root_session_id, session_id, "jsonl not found (ssh)");
                return Ok(cdt_core::ToolOutput::Missing);
            };
            let body = match fs.read_to_string(&jsonl_path).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(target: "cdt_api::tool_output", error = %e, "ssh read jsonl failed; returning Missing");
                    return Ok(cdt_core::ToolOutput::Missing);
                }
            };
            match parse_jsonl_content(&body) {
                Ok(m) => std::sync::Arc::new(m),
                Err(e) => {
                    tracing::debug!(target: "cdt_api::tool_output", error = %e, "ssh parse failed; returning Missing");
                    return Ok(cdt_core::ToolOutput::Missing);
                }
            }
        } else {
            let Some(jsonl_path) =
                locate_session_jsonl(&projects_dir, root_session_id, session_id).await
            else {
                tracing::warn!(target: "cdt_api::tool_output", root_session_id, session_id, "jsonl not found");
                return Ok(cdt_core::ToolOutput::Missing);
            };

            // 走 parsed-message LRU cache：命中时复用 Arc<Vec<ParsedMessage>>，
            // 跳过整文件 line-by-line parse；详 change `parsed-message-lru-cache`。
            let Some(messages) =
                extract_parsed_messages_cached(&self.parsed_msg_cache, &jsonl_path).await
            else {
                tracing::debug!(target: "cdt_api::tool_output", "parse failed or stat error; returning Missing");
                return Ok(cdt_core::ToolOutput::Missing);
            };
            messages
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
            // 找不到就放占位条目，调用方按 metadata.status 决定是否过滤。
            let Ok(Some(project_id)) = self.find_session_project(sid).await else {
                results.push(SessionDetail {
                    session_id: sid.clone(),
                    project_id: String::new(),
                    chunks: serde_json::Value::Null,
                    metrics: serde_json::Value::Null,
                    metadata: serde_json::json!({"status": "not_found"}),
                    context_injections: serde_json::Value::Array(Vec::new()),
                    injections_by_phase: serde_json::Value::Object(serde_json::Map::new()),
                    phase_info: serde_json::Value::Null,
                    is_ongoing: false,
                });
                continue;
            };
            match self.get_session_detail(&project_id, sid).await {
                Ok(detail) => results.push(detail),
                Err(_) => results.push(SessionDetail {
                    session_id: sid.clone(),
                    project_id,
                    chunks: serde_json::Value::Null,
                    metrics: serde_json::Value::Null,
                    metadata: serde_json::json!({"status": "not_found"}),
                    context_injections: serde_json::Value::Array(Vec::new()),
                    injections_by_phase: serde_json::Value::Object(serde_json::Map::new()),
                    phase_info: serde_json::Value::Null,
                    is_ongoing: false,
                }),
            }
        }
        Ok(results)
    }

    // =========================================================================
    // 搜索
    // =========================================================================

    async fn search(&self, request: &SearchRequest) -> Result<serde_json::Value, ApiError> {
        if request.query.is_empty() {
            return Ok(serde_json::json!({
                "query": "",
                "results": [],
            }));
        }

        let max_results = 50;

        let project_id = request
            .project_id
            .as_deref()
            .ok_or_else(|| ApiError::validation("project_id is required for search"))?;

        let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        // active context = SSH 时启用 SearchConfig.is_ssh=true，开 stage-limit
        // 避免远端 SFTP 全量扫描（local 默认 is_ssh=false）
        let config = SearchConfig::from_fs_kind(fs.kind());
        let searcher = SessionSearcher::new(fs, self.search_cache.clone(), projects_dir);
        let result = searcher
            .search_sessions(project_id, &request.query, max_results, &config)
            .await
            .map_err(|e| ApiError::internal(format!("search error: {e}")))?;

        serde_json::to_value(&result).map_err(|e| ApiError::internal(format!("{e}")))
    }

    // =========================================================================
    // 配置 + 通知
    // =========================================================================

    async fn get_config(&self) -> Result<serde_json::Value, ApiError> {
        let mgr = self.config_mgr.lock().await;
        let config = mgr.get_config();
        serde_json::to_value(&config).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn update_config(
        &self,
        request: &ConfigUpdateRequest,
    ) -> Result<serde_json::Value, ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        let result = match request.section.as_str() {
            "general" => mgr.update_general(request.data.clone()).await,
            "display" => mgr.update_display(request.data.clone()).await,
            "notifications" => mgr.update_notifications(request.data.clone()).await,
            "ssh" => mgr.update_ssh(request.data.clone()).await,
            "httpServer" => mgr.update_http_server(request.data.clone()).await,
            "updater" => mgr.update_updater(request.data.clone()).await,
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
        serde_json::to_value(&config).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn get_notifications(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<serde_json::Value, ApiError> {
        let mgr = self.notif_mgr.lock().await;
        let result = mgr.get_notifications(limit, offset);
        serde_json::to_value(&result).map_err(|e| ApiError::internal(format!("{e}")))
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
            if let Some(prev) = previous_context_id.as_deref() {
                tracing::debug!(target: "cdt_ssh::lifecycle", phase = "cancel_prev_watcher", context_id = %prev);
                self.cancel_remote_watcher(prev).await;
            }
        }
        tracing::debug!(target: "cdt_ssh::lifecycle", phase = "ssh_mgr_switch_context");
        self.ssh_mgr
            .switch_context(target.clone())
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        if previous_context_id != target {
            if let Some(next) = target.as_deref() {
                tracing::debug!(target: "cdt_ssh::lifecycle", phase = "attach_new_watcher", context_id = %next);
                self.attach_remote_watcher(next).await;
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
            if previous_context_id.as_deref() != Some(target_context_id.as_str()) {
                if let Some(prev) = previous_context_id {
                    tracing::debug!(target: "cdt_ssh::lifecycle", phase = "cancel_prev_watcher", context_id = %prev);
                    self.cancel_remote_watcher(&prev).await;
                }
            }
            tracing::debug!(target: "cdt_ssh::lifecycle", phase = "ssh_mgr_connect");
            let context_id = self
                .ssh_mgr
                .connect(request.clone().into())
                .await
                .map_err(|e| ApiError::internal(format!("{e}")))?;
            if self.ssh_shutdown_generation.load(Ordering::SeqCst) == shutdown_generation {
                tracing::debug!(target: "cdt_ssh::lifecycle", phase = "attach_new_watcher", context_id = %context_id);
                self.attach_remote_watcher(&context_id).await;
                context_id
            } else {
                tracing::debug!(target: "cdt_ssh::lifecycle", phase = "shutdown_in_progress_aborting", context_id = %context_id);
                let _ = self.ssh_mgr.disconnect(&context_id).await;
                return Err(ApiError::internal("SSH shutdown in progress"));
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
        self.cancel_remote_watcher(context_id).await;
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
            .map_err(|e| ApiError::internal(format!("{e}")))?;
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
        // D3b：grouper 无状态轻量，每次 lazy 构造，避免 LocalDataApi 字段污染。
        // active context = SSH 时 SHALL NOT 用 LocalGitIdentityResolver——容器内远端
        // cwd 与本机宿主路径重合时（如 docker 挂载 `~/.claude` 复现场景），会读宿主机
        // `.git` 泄漏本地 gitBranch。SSH 路径用 NoopGitIdentityResolver 让 git 字段全 None。
        let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
        let is_remote = fs.kind() == cdt_discover::FsKind::Ssh;
        let mut scanner = ProjectScanner::new(fs, projects_dir);
        let projects = scanner
            .scan()
            .await
            .map_err(|e| ApiError::internal(format!("scan error: {e}")))?;
        if is_remote {
            let grouper = cdt_discover::WorktreeGrouper::new(NoopGitIdentityResolver);
            Ok(grouper.group_by_repository(projects).await)
        } else {
            let grouper =
                cdt_discover::WorktreeGrouper::new(cdt_discover::LocalGitIdentityResolver::new());
            Ok(grouper.group_by_repository(projects).await)
        }
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
        serde_json::to_value(&config).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn remove_trigger(&self, trigger_id: &str) -> Result<serde_json::Value, ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        let config = mgr
            .remove_trigger(trigger_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        serde_json::to_value(&config).map_err(|e| ApiError::internal(format!("{e}")))
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
        extract_parsed_messages_cached(&self.parsed_msg_cache, path).await
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

        let pairs: Vec<(String, String)> = projects
            .iter()
            .map(|p| (p.id.clone(), p.path.to_string_lossy().into_owned()))
            .collect();
        // 扫描涉及文件系统 I/O，放到 blocking 线程池避免阻塞 runtime
        let configs = tokio::task::spawn_blocking(move || {
            cdt_discover::agent_configs::read_agent_configs(&pairs)
        })
        .await
        .map_err(|e| ApiError::internal(format!("join error: {e}")))?;
        Ok(configs)
    }
}

async fn discover_memory_layers(memory_dir: &Path) -> Result<Vec<MemoryLayer>, ApiError> {
    let mut entries = match tokio::fs::read_dir(memory_dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(ApiError::internal(format!("read memory dir error: {e}"))),
    };

    let mut markdown_files = std::collections::BTreeSet::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| ApiError::internal(format!("read memory entry error: {e}")))?
    {
        let file_type = entry
            .file_type()
            .await
            .map_err(|e| ApiError::internal(format!("read memory file type error: {e}")))?;
        if !file_type.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        {
            markdown_files.insert(name);
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
        let index_content = tokio::fs::read_to_string(memory_dir.join("MEMORY.md"))
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

/// 失败 fallback：返回一个空 `data:` URI 占位。前端 `<img>` 加载会显示
/// broken-image，不阻塞 session 渲染。
fn empty_data_uri() -> String {
    "data:application/octet-stream;base64,".to_owned()
}

/// 完整 `data:` URI（落盘失败 / cache 目录未注入时 fallback）。
fn format_data_uri(media_type: &str, base64_data: &str) -> String {
    format!("data:{media_type};base64,{base64_data}")
}

/// 在 projects 根目录下定位 (`root_session_id`, `session_id`) 对应的 jsonl。
///
/// `session_id == root_session_id` 时直接找 root jsonl（在任一 `project_dir` 内）；
/// 不等时跨 `projects_dir` 扫 `{project_dir}/{root_session_id}/subagents/agent-<sub>.jsonl`
/// （新结构），命中即返；未命中再 fallback 到 root `project_dir` 内的 flat 旧结构。
/// SSH 远端版的 `locate_session_jsonl`——同语义但走 `FileSystemProvider`
/// 而不是 `tokio::fs`。远端只覆盖新结构 subagent（`{project}/{root}/subagents/agent-<sid>.jsonl`），
/// 旧 flat 结构需要 per-file stat，远端 latency 不可接受。
async fn locate_session_jsonl_via_fs(
    fs: &dyn FileSystemProvider,
    projects_dir: &Path,
    root_session_id: &str,
    session_id: &str,
) -> Option<std::path::PathBuf> {
    let entries = fs.read_dir(projects_dir).await.ok()?;
    if session_id == root_session_id {
        for entry in entries {
            if !entry.kind.is_dir() {
                continue;
            }
            let candidate = projects_dir
                .join(&entry.name)
                .join(format!("{root_session_id}.jsonl"));
            if fs.exists(&candidate).await {
                return Some(candidate);
            }
        }
        return None;
    }
    // subagent 新结构：`{projects_dir}/<project>/<root>/subagents/agent-<sid>.jsonl`
    let filename = format!("agent-{session_id}.jsonl");
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

/// SSH 远端版的 `find_subagent_jsonl`——给定 `project_dir` 扫
/// `<project_dir>/*/subagents/agent-<sid>.jsonl` 新结构。
async fn find_subagent_jsonl_via_fs(
    fs: &dyn FileSystemProvider,
    project_dir: &Path,
    session_id: &str,
) -> Option<std::path::PathBuf> {
    let filename = format!("agent-{session_id}.jsonl");
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

async fn locate_session_jsonl(
    projects_dir: &Path,
    root_session_id: &str,
    session_id: &str,
) -> Option<std::path::PathBuf> {
    // 主 session 自身：扫 projects_dir 找到任一 project_dir 含 root jsonl 即返。
    if session_id == root_session_id {
        let mut entries = tokio::fs::read_dir(projects_dir).await.ok()?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let root_jsonl = entry.path().join(format!("{root_session_id}.jsonl"));
            if tokio::fs::metadata(&root_jsonl).await.is_ok() {
                return Some(root_jsonl);
            }
        }
        return None;
    }

    // subagent：优先跨 project_dir 扫新结构。
    if CROSS_PROJECT_SUBAGENT_SCAN {
        if let Some(p) =
            find_subagent_jsonl_cross_project(projects_dir, root_session_id, session_id).await
        {
            return Some(p);
        }
    }

    // 旧结构兜底：找含 root jsonl 的 project_dir 后扫该目录的 flat agent jsonl。
    let mut entries = tokio::fs::read_dir(projects_dir).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let project_dir = entry.path();
        let root_jsonl = project_dir.join(format!("{root_session_id}.jsonl"));
        if tokio::fs::metadata(&root_jsonl).await.is_err() {
            continue;
        }
        if let Some(p) = find_subagent_jsonl(&project_dir, session_id).await {
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

/// SHA256 内容寻址 + 落盘到 cache 目录，返回 `asset://localhost/<absolute_path>`。
/// 失败时 fallback 返回 `data:` URI。
async fn materialize_image_asset(cache_dir: &Path, media_type: &str, base64_data: &str) -> String {
    use base64::Engine;
    use sha2::Digest;

    let bytes = match base64::engine::general_purpose::STANDARD.decode(base64_data) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(target: "cdt_api::image", error = %e, "base64 decode failed");
            return format_data_uri(media_type, base64_data);
        }
    };

    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let hash_hex: String = digest.iter().take(8).fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    });

    let ext = media_type_to_ext(media_type);
    let file_path = cache_dir.join(format!("{hash_hex}.{ext}"));

    if let Err(e) = tokio::fs::create_dir_all(cache_dir).await {
        tracing::warn!(target: "cdt_api::image", error = %e, dir = %cache_dir.display(), "create cache dir failed");
        return format_data_uri(media_type, base64_data);
    }

    if tokio::fs::metadata(&file_path).await.is_err() {
        if let Err(e) = tokio::fs::write(&file_path, &bytes).await {
            tracing::warn!(target: "cdt_api::image", error = %e, path = %file_path.display(), "write image cache failed");
            return format_data_uri(media_type, base64_data);
        }
    }

    // Windows 上 `file_path.display()` 含 `\`，Tauri asset protocol 按 POSIX URI
    // 解析 —— 手动归一为 `/` 保证 `asset://localhost/C:/Users/...` 格式。
    let url_path = file_path.to_string_lossy().replace('\\', "/");
    format!("asset://localhost/{url_path}")
}

fn media_type_to_ext(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}

/// 在 project 目录下查找指定 session id 的 subagent JSONL 文件。
///
/// 检查两种结构：
/// - 新：`{project_dir}/*/subagents/agent-{session_id}.jsonl`（扁平扫一层主 session 目录）
/// - 旧：`{project_dir}/agent-{session_id}.jsonl`
async fn find_subagent_jsonl(project_dir: &Path, session_id: &str) -> Option<std::path::PathBuf> {
    let filename = format!("agent-{session_id}.jsonl");

    // 旧结构：project_dir/agent-<id>.jsonl
    let flat = project_dir.join(&filename);
    if tokio::fs::metadata(&flat).await.is_ok() {
        return Some(flat);
    }

    // 新结构：project_dir/{parent_session}/subagents/agent-<id>.jsonl
    let mut entries = tokio::fs::read_dir(project_dir).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let candidate = entry.path().join("subagents").join(&filename);
        if tokio::fs::metadata(&candidate).await.is_ok() {
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
///
/// 跳过 `agent-acompact*` 前缀（compaction 类内部产物，不是真实 subagent）。
async fn scan_subagent_candidates_cross_project(
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
    if let Ok(mut entries) = tokio::fs::read_dir(projects_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            project_dirs.push(entry.path());
        }
    }
    let projects_scanned = project_dirs.len();

    // 第二遍：每个 project 并发探测 `<dir>/<root_session_id>/subagents/agent-*.jsonl`，
    // 用 Semaphore 限流 `METADATA_SCAN_CONCURRENCY=8` 路（与 metadata 扫描同口径，
    // 避免低核数机器上短脉冲 CPU 峰值过高，也压住打开 fd 数量）。
    // 单 task 内部仍顺序遍历 sub_entries，保证某 project 内候选顺序稳定。
    // 整体 task 顺序由 `join_all` 保证（与 project_dirs 同序），最终落到 candidates
    // 的顺序与原串行版本一致。
    let semaphore = Arc::new(Semaphore::new(METADATA_SCAN_CONCURRENCY));
    let scan_tasks = project_dirs.into_iter().map(|project_path| {
        let sem = semaphore.clone();
        let root_session_id = root_session_id.to_owned();
        async move {
            let _permit = sem.acquire_owned().await.ok()?;
            let new_dir = project_path.join(&root_session_id).join("subagents");
            let mut sub_entries = tokio::fs::read_dir(&new_dir).await.ok()?;
            let mut local: Vec<(cdt_core::SubagentCandidate, u128)> = Vec::new();
            while let Ok(Some(sub_entry)) = sub_entries.next_entry().await {
                let name = sub_entry.file_name();
                let name_str = name.to_string_lossy();
                if !(name_str.starts_with("agent-")
                    && name_str.ends_with(".jsonl")
                    && !name_str.starts_with("agent-acompact"))
                {
                    continue;
                }
                let t = std::time::Instant::now();
                if let Some(c) = parse_subagent_candidate(&sub_entry.path()).await {
                    local.push((c, t.elapsed().as_millis()));
                }
            }
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
    if let Ok(mut old_entries) = tokio::fs::read_dir(main_project_dir).await {
        while let Ok(Some(entry)) = old_entries.next_entry().await {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !(name_str.starts_with("agent-")
                && name_str.ends_with(".jsonl")
                && !name_str.starts_with("agent-acompact"))
            {
                continue;
            }
            let t = std::time::Instant::now();
            let Some(c) = parse_subagent_candidate(&entry.path()).await else {
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
/// 额外回退路径时显式叠加调用 `find_subagent_jsonl(&main_project_dir, ...)` 兜旧结构。
async fn find_subagent_jsonl_cross_project(
    projects_dir: &Path,
    root_session_id: &str,
    sub_session_id: &str,
) -> Option<std::path::PathBuf> {
    let filename = format!("agent-{sub_session_id}.jsonl");
    let Ok(mut entries) = tokio::fs::read_dir(projects_dir).await else {
        return None;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let candidate = entry
            .path()
            .join(root_session_id)
            .join("subagents")
            .join(&filename);
        if tokio::fs::metadata(&candidate).await.is_ok() {
            return Some(candidate);
        }
    }
    None
}

/// 扫描 subagent 候选文件，构建 `SubagentCandidate` 列表。
///
/// 扫描路径：
/// - 新结构：`{project_dir}/{session_id}/subagents/agent-*.jsonl`
/// - 旧结构：`{project_dir}/agent-*.jsonl`（需要读首行检查 parent session）
///
/// 扫描失败时静默返回空列表（warn 日志）。本函数仅扫主 `project_dir`，
/// 跨 `projects_dir` 的扫描走 `scan_subagent_candidates_cross_project`。
async fn scan_subagent_candidates(
    project_dir: &Path,
    session_id: &str,
) -> Vec<cdt_core::SubagentCandidate> {
    let mut candidates = Vec::new();
    let mut per_candidate_ms: Vec<u128> = Vec::new();

    // 新结构：{project_dir}/{session_id}/subagents/
    let new_dir = project_dir.join(session_id).join("subagents");
    if let Ok(mut entries) = tokio::fs::read_dir(&new_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("agent-")
                && name_str.ends_with(".jsonl")
                && !name_str.starts_with("agent-acompact")
            {
                let t = std::time::Instant::now();
                if let Some(c) = parse_subagent_candidate(&entry.path()).await {
                    per_candidate_ms.push(t.elapsed().as_millis());
                    candidates.push(c);
                }
            }
        }
    }

    // 旧结构：{project_dir}/agent-*.jsonl
    if let Ok(mut entries) = tokio::fs::read_dir(project_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("agent-")
                && name_str.ends_with(".jsonl")
                && !name_str.starts_with("agent-acompact")
            {
                let t = std::time::Instant::now();
                if let Some(c) = parse_subagent_candidate(&entry.path()).await {
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
async fn parse_subagent_candidate(path: &Path) -> Option<cdt_core::SubagentCandidate> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let file = tokio::fs::File::open(path).await.ok()?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.strip_prefix("agent-").unwrap_or(s).to_owned())
        .unwrap_or_default();
    let mut spawn_ts = None;
    let mut end_ts: Option<chrono::DateTime<chrono::Utc>> = None;
    let mut parent_session_id = None;
    let mut description_hint = None;
    let mut is_warmup = false;

    // 前 10 行提取元数据；之后继续扫描以记录最后一条 timestamp 作为 end_ts
    let mut line_count = 0;
    while let Ok(Some(line)) = lines.next_line().await {
        line_count += 1;
        let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        if let Some(ts_str) = val.get("timestamp").and_then(|v| v.as_str()) {
            let parsed = chrono::DateTime::parse_from_rfc3339(ts_str)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc));
            if let Some(ts) = parsed {
                if spawn_ts.is_none() {
                    spawn_ts = Some(ts);
                }
                end_ts = Some(ts);
            }
        }

        if line_count <= 10 {
            if parent_session_id.is_none() {
                if let Some(pid) = val.get("parentUuid").and_then(|v| v.as_str()) {
                    parent_session_id = Some(pid.to_owned());
                }
            }

            if let Some(aid) = val.get("agentId").and_then(|v| v.as_str()) {
                aid.clone_into(&mut session_id);
            }

            if val.get("type").and_then(|v| v.as_str()) == Some("user") {
                let content_val = val
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .unwrap_or(&val["content"]);
                if let Some(content) = content_val.as_str() {
                    if content == "Warmup" {
                        is_warmup = true;
                        break;
                    }
                    if description_hint.is_none() && !content.is_empty() {
                        description_hint = Some(content.chars().take(200).collect());
                    }
                }
            }
        }
    }

    if is_warmup {
        return None;
    }

    // 只有当最后一行时间戳晚于首行时才算已结束；否则视为仍在运行。
    // 注意：本字段仅用于 `compute_duration_ms` 与 resolver 内 OR 兜底；
    // `is_ongoing` 的真实判定走下方 `check_messages_ongoing` 五信号算法，
    // 与主 session（`get_session_detail` / `extract_session_metadata`）一致。
    let end_ts = match (spawn_ts, end_ts) {
        (Some(start), Some(end)) if end > start => Some(end),
        _ => None,
    };

    // 完整解析 subagent session，构建 Chunk 流供 UI 展示 ExecutionTrace。
    //
    // 注：subagent session 的所有消息对**父** session 而言是 sidechain，但对
    // subagent 自己来说不是——而 `build_chunks` 会跳过 `is_sidechain=true`。
    // 这里 clone 一份消息并清除 sidechain 标记，以便 Chunk 正常产出。
    // 对齐原版 `aiGroupHelpers.ts::computeSubagentPhaseBreakdown` 的处理。
    //
    // ongoing 判定：在清 sidechain 与 build_chunks 之前先用原始 `ParsedMessage`
    // 流跑 `check_messages_ongoing`——避免 followups.md "Subagent 状态判定"
    // 段记录的 impl-bug：仅看 `end_ts > spawn_ts` 会把"中断后无 assistant 收尾"
    // 的 subagent 误判为已完成，导致 `SubagentCard` 右上角误显示 ✓。
    let (is_ongoing, messages) = match parse_file(path).await {
        Ok(mut msgs) => {
            let ongoing = cdt_analyze::check_messages_ongoing(&msgs);
            for m in &mut msgs {
                m.is_sidechain = false;
            }
            (ongoing, cdt_analyze::build_chunks(&msgs))
        }
        Err(_) => (end_ts.is_none(), Vec::new()),
    };

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

// =============================================================================
// 测试：覆盖 Pin/Hide facade（走独立 impl 块的非 trait 方法）
// =============================================================================

#[cfg(test)]
mod tests {
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

    #[test]
    fn media_type_to_ext_known_and_unknown() {
        assert_eq!(media_type_to_ext("image/png"), "png");
        assert_eq!(media_type_to_ext("image/jpeg"), "jpg");
        assert_eq!(media_type_to_ext("image/gif"), "gif");
        assert_eq!(media_type_to_ext("image/webp"), "webp");
        assert_eq!(media_type_to_ext("application/x-future"), "bin");
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
        let cand = parse_subagent_candidate(&path).await.expect("candidate");
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
        let cand = parse_subagent_candidate(&path).await.expect("candidate");
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
        let cand = parse_subagent_candidate(&path).await.expect("candidate");
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

        let cands =
            scan_subagent_candidates_cross_project(&projects_dir, &main_pd, "root-uuid").await;
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
        let cands =
            scan_subagent_candidates_cross_project(&projects_dir, &main_pd, "root-uuid").await;
        assert_eq!(cands.len(), 1, "同 agent_id 跨目录重复应被 seen_ids 去重");
    }

    #[tokio::test]
    async fn scan_cross_project_empty_when_no_match() {
        let dir = tempdir().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        let main_pd = projects_dir.join("-ws-my-proj");
        std::fs::create_dir_all(&main_pd).unwrap();

        let cands =
            scan_subagent_candidates_cross_project(&projects_dir, &main_pd, "missing-root").await;
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

        let found = find_subagent_jsonl_cross_project(&projects_dir, "root-uuid", "sub-uuid").await;
        assert_eq!(found, Some(agent_path));
    }

    #[tokio::test]
    async fn find_subagent_jsonl_cross_project_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        std::fs::create_dir_all(projects_dir.join("-ws-empty")).unwrap();

        let found = find_subagent_jsonl_cross_project(&projects_dir, "root-uuid", "sub-uuid").await;
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

        let found = locate_session_jsonl(&projects_dir, "root-uuid", "root-uuid").await;
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

        let found = locate_session_jsonl(&projects_dir, "root-uuid", "sub-uuid").await;
        assert_eq!(
            found,
            Some(agent_path),
            "subagent 在 worktree pd 时 locate_session_jsonl 应跨目录找到"
        );
    }
}
