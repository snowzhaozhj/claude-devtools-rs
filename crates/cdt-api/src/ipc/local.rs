//! `LocalDataApi`：`DataApi` trait 的本地文件系统实现。
//!
//! 组装底层 crate 调用，作为默认的数据 API 实现。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{Mutex, Semaphore, broadcast};
use tokio::task::{AbortHandle, JoinSet};

use cdt_analyze::build_chunks_with_subagents;
use cdt_config::{
    ConfigManager, DetectedError, NotificationManager, read_all_claude_md_files,
    read_mentioned_file as config_read_mentioned_file, validate_file_path,
};
use cdt_discover::{
    LocalFileSystemProvider, ProjectScanner, SearchConfig, SearchTextCache, SessionSearcher,
    path_decoder,
};
use cdt_parse::parse_file;
use cdt_ssh::{ActiveContext, SshConnectionManager, parse_ssh_config_file, resolve_host};
use cdt_watch::FileWatcher;

use super::error::ApiError;
use super::events::SessionMetadataUpdate;
use super::session_metadata::extract_session_metadata;
use super::traits::DataApi;
use super::types::{
    ConfigUpdateRequest, ContextInfo, PaginatedRequest, PaginatedResponse, ProjectInfo,
    ProjectSessionPrefs, SearchRequest, SessionDetail, SessionSummary, SshConnectRequest,
};
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

/// 本地文件系统 `DataApi` 实现。
pub struct LocalDataApi {
    scanner: Mutex<ProjectScanner>,
    searcher: SessionSearcher<LocalFileSystemProvider>,
    config_mgr: Arc<Mutex<ConfigManager>>,
    notif_mgr: Arc<Mutex<NotificationManager>>,
    ssh_mgr: Mutex<SshConnectionManager>,
    /// 自动通知管线的 `DetectedError` 广播发送端。仅在 `new_with_watcher`
    /// 构造下存在；`new()` 构造返回 `None`，此时 `subscribe_detected_errors`
    /// 返回一条永不发消息的 receiver（caller 代码统一）。
    error_tx: Option<broadcast::Sender<DetectedError>>,
    /// `list_sessions` 后台元数据扫描的广播发送端。`subscribe_session_metadata()`
    /// 返回 receiver，Tauri host 桥接为前端 `session-metadata-update` 事件。
    session_metadata_tx: broadcast::Sender<SessionMetadataUpdate>,
    /// 当前 per-project 进行中的元数据扫描句柄。`list_sessions` 调用前会
    /// abort 同 project 的旧扫描，避免事件串扰（详见 design.md decision 4）。
    active_scans: Arc<std::sync::Mutex<HashMap<String, AbortHandle>>>,
    /// `get_image_asset` 落盘 cache 目录。由 Tauri host 通过
    /// `new_with_image_cache` 注入 `app_cache_dir().join("cdt-images")`；
    /// `None` 时 `get_image_asset` fallback 到 `data:` URI（默认构造路径
    /// + 集成测试无 cache 目录依赖）。
    image_cache_dir: Option<std::path::PathBuf>,
}

impl LocalDataApi {
    pub fn new(
        scanner: ProjectScanner,
        config_mgr: ConfigManager,
        notif_mgr: NotificationManager,
        ssh_mgr: SshConnectionManager,
    ) -> Self {
        let fs = std::sync::Arc::new(LocalFileSystemProvider::new());
        let cache = std::sync::Arc::new(Mutex::new(SearchTextCache::new()));
        let searcher = SessionSearcher::new(fs, cache);
        let (session_metadata_tx, _) =
            broadcast::channel::<SessionMetadataUpdate>(METADATA_BROADCAST_CAPACITY);
        Self {
            scanner: Mutex::new(scanner),
            searcher,
            config_mgr: Arc::new(Mutex::new(config_mgr)),
            notif_mgr: Arc::new(Mutex::new(notif_mgr)),
            ssh_mgr: Mutex::new(ssh_mgr),
            error_tx: None,
            session_metadata_tx,
            active_scans: Arc::new(std::sync::Mutex::new(HashMap::new())),
            image_cache_dir: None,
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
        ssh_mgr: SshConnectionManager,
        watcher: &FileWatcher,
        projects_dir: std::path::PathBuf,
    ) -> Self {
        let fs = std::sync::Arc::new(LocalFileSystemProvider::new());
        let cache = std::sync::Arc::new(Mutex::new(SearchTextCache::new()));
        let searcher = SessionSearcher::new(fs, cache);

        let config_mgr = Arc::new(Mutex::new(config_mgr));
        let notif_mgr = Arc::new(Mutex::new(notif_mgr));
        let (error_tx, _) = broadcast::channel::<DetectedError>(256);

        let pipeline = NotificationPipeline::new(
            watcher.subscribe_files(),
            config_mgr.clone(),
            notif_mgr.clone(),
            error_tx.clone(),
            projects_dir,
        );
        tokio::spawn(pipeline.run());

        let (session_metadata_tx, _) =
            broadcast::channel::<SessionMetadataUpdate>(METADATA_BROADCAST_CAPACITY);

        Self {
            scanner: Mutex::new(scanner),
            searcher,
            config_mgr,
            notif_mgr,
            ssh_mgr: Mutex::new(ssh_mgr),
            error_tx: Some(error_tx),
            session_metadata_tx,
            active_scans: Arc::new(std::sync::Mutex::new(HashMap::new())),
            image_cache_dir: None,
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
        ),
        ApiError,
    > {
        let scanner = self.scanner.lock().await;
        let sessions = scanner
            .list_sessions(project_id, &std::collections::BTreeSet::new())
            .await
            .map_err(|e| ApiError::internal(format!("list sessions error: {e}")))?;
        let projects_dir = scanner.projects_dir().to_path_buf();
        drop(scanner);

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
        for s in page_sessions {
            let jsonl_path = dir.join(format!("{}.jsonl", s.id));
            page.push(SessionSummary {
                session_id: s.id.clone(),
                project_id: project_id.to_owned(),
                timestamp: s.last_modified,
                message_count: 0,
                title: None,
                is_ongoing: false,
            });
            page_jobs.push((s.id, jsonl_path));
        }

        let page_len = page.len();
        let next_cursor = if offset + page_len < total {
            Some((offset + page_len).to_string())
        } else {
            None
        };

        Ok((page, next_cursor, total, page_jobs, dir))
    }
}

/// 后台扫描某页 session 的元数据，每扫完一个 broadcast 一条
/// `SessionMetadataUpdate`。并发度受 `METADATA_SCAN_CONCURRENCY` 限流。
///
/// 任务结束时（无论正常完成还是被 abort）从 `active_scans` 移除自己的
/// `AbortHandle`。`broadcast::send` 在无订阅者时返回 `Err`，本函数静默
/// 忽略——元数据更新本质上是 fire-and-forget。
async fn scan_metadata_for_page(
    project_id: String,
    dir: std::path::PathBuf,
    page_jobs: Vec<(String, std::path::PathBuf)>,
    tx: broadcast::Sender<SessionMetadataUpdate>,
    active_scans: Arc<std::sync::Mutex<HashMap<String, AbortHandle>>>,
    cleanup_key: String,
) {
    let _ = dir; // dir 当前由 page_jobs 内的 jsonl_path 携带，保留参数为未来扩展（如懒构造路径）
    let semaphore = Arc::new(Semaphore::new(METADATA_SCAN_CONCURRENCY));
    let mut set = JoinSet::new();

    for (session_id, jsonl_path) in page_jobs {
        let permit_sem = semaphore.clone();
        let tx = tx.clone();
        let project_id = project_id.clone();
        set.spawn(async move {
            let Ok(_permit) = permit_sem.acquire_owned().await else {
                return;
            };
            let meta = extract_session_metadata(&jsonl_path).await;
            let _ = tx.send(SessionMetadataUpdate {
                project_id,
                session_id,
                title: meta.title,
                message_count: meta.message_count,
                is_ongoing: meta.is_ongoing,
            });
        });
    }

    while set.join_next().await.is_some() {}

    if let Ok(mut scans) = active_scans.lock() {
        scans.remove(&cleanup_key);
    }
}

#[async_trait]
impl DataApi for LocalDataApi {
    // =========================================================================
    // 项目 + 会话
    // =========================================================================

    async fn list_projects(&self) -> Result<Vec<ProjectInfo>, ApiError> {
        let mut scanner = self.scanner.lock().await;
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
        let (mut page, next_cursor, total, page_jobs, _dir) =
            self.list_sessions_skeleton(project_id, pagination).await?;

        for (summary, (_id, path)) in page.iter_mut().zip(page_jobs.iter()) {
            let meta = extract_session_metadata(path).await;
            summary.title = meta.title;
            summary.message_count = meta.message_count;
            summary.is_ongoing = meta.is_ongoing;
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
        let (page, next_cursor, total, page_jobs, dir) =
            self.list_sessions_skeleton(project_id, pagination).await?;

        // 取消同 project 的旧扫描，避免事件串扰
        if let Ok(mut scans) = self.active_scans.lock() {
            if let Some(handle) = scans.remove(project_id) {
                handle.abort();
            }
        }

        if !page_jobs.is_empty() {
            let tx = self.session_metadata_tx.clone();
            let project_id_owned = project_id.to_owned();
            let active_scans = self.active_scans.clone();
            let pid_for_cleanup = project_id_owned.clone();

            let handle = tokio::spawn(scan_metadata_for_page(
                project_id_owned,
                dir,
                page_jobs,
                tx,
                active_scans.clone(),
                pid_for_cleanup,
            ));

            if let Ok(mut scans) = self.active_scans.lock() {
                scans.insert(project_id.to_owned(), handle.abort_handle());
            }
        }

        Ok(PaginatedResponse {
            items: page,
            next_cursor,
            total,
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

        let projects_dir = path_decoder::get_projects_base_path();
        let project_dir = projects_dir.join(project_id);

        let t_locate = std::time::Instant::now();
        let scanner = self.scanner.lock().await;
        let sessions = scanner
            .list_sessions(project_id, &std::collections::BTreeSet::new())
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        drop(scanner);

        let main_session = sessions.iter().find(|s| s.id == session_id);

        let (jsonl_path, last_modified, size) = if let Some(s) = main_session {
            (
                project_dir.join(format!("{session_id}.jsonl")),
                Some(s.last_modified),
                Some(s.size),
            )
        } else if let Some(path) = find_subagent_jsonl(&project_dir, session_id).await {
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
        let messages = parse_file(&jsonl_path)
            .await
            .map_err(|e| ApiError::internal(format!("parse error: {e}")))?;
        let parse_ms = t_parse.elapsed().as_millis();
        let message_count = messages.len();

        let t_scan = std::time::Instant::now();
        let candidates = scan_subagent_candidates(&project_dir, session_id).await;
        let scan_ms = t_scan.elapsed().as_millis();
        let candidate_count = candidates.len();

        let t_build = std::time::Instant::now();
        let messages_ongoing = cdt_analyze::check_messages_ongoing(&messages);
        // stale check 与 list_sessions 路径对齐（issue #94）：mtime > 5min 的
        // ongoing 视为 crashed/killed。
        let is_ongoing = if messages_ongoing {
            !crate::ipc::session_metadata::is_file_stale(&jsonl_path).await
        } else {
            false
        };
        let chunks = build_chunks_with_subagents(&messages, &candidates);
        let build_ms = t_build.elapsed().as_millis();
        let chunk_count = chunks.len();

        let t_ctx = std::time::Instant::now();
        let project_root = messages.iter().find_map(|m| m.cwd.as_deref()).unwrap_or("");
        let initial_claude_md = build_claude_md_from_filesystem(project_root).await;
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
        let context_injections = ctx_result
            .phase_info
            .phases
            .last()
            .and_then(|phase| ctx_result.stats_map.get(&phase.last_ai_group_id))
            .map(|stats| &stats.accumulated_injections)
            .and_then(|inj| serde_json::to_value(inj).ok())
            .unwrap_or(serde_json::Value::Array(Vec::new()));
        let ctx_ms = t_ctx.elapsed().as_millis();

        let t_serde = std::time::Instant::now();
        // IPC payload 瘦身：subagent.messages 默认裁剪为空，前端 SubagentCard
        // 展开时通过 `get_subagent_trace` 懒拉取。把 messages 抠空 + 设
        // `messages_omitted=true`；header_model / last_isolated_tokens /
        // is_shutdown_only 已由 resolver 阶段填充，可独立渲染 header。
        let chunks_for_payload = {
            let mut cloned = chunks.clone();
            // phase 3：image base64 OMIT 必须在 subagent OMIT 之前跑，否则
            // OMIT_SUBAGENT_MESSAGES=false 回滚路径下嵌套 messages 内的 image
            // 不会被裁。
            if OMIT_IMAGE_DATA {
                apply_image_omit(&mut cloned);
            }
            // phase 4：response.content OMIT 同样在 subagent OMIT 之前跑，
            // 覆盖 OMIT_SUBAGENT_MESSAGES=false 回滚路径下嵌套 messages 内的
            // AIChunk.responses[].content。
            if OMIT_RESPONSE_CONTENT {
                apply_response_content_omit(&mut cloned);
            }
            // phase 5：tool_exec.output OMIT 同上，覆盖嵌套 messages 内的
            // tool_executions[].output。
            if OMIT_TOOL_OUTPUT {
                apply_tool_output_omit(&mut cloned);
            }
            if OMIT_SUBAGENT_MESSAGES {
                for c in &mut cloned {
                    if let cdt_core::Chunk::Ai(ai) = c {
                        for sub in &mut ai.subagents {
                            sub.messages = Vec::new();
                            sub.messages_omitted = true;
                        }
                    }
                }
            }
            cloned
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

    async fn get_subagent_trace(
        &self,
        root_session_id: &str,
        subagent_session_id: &str,
    ) -> Result<serde_json::Value, ApiError> {
        // 跨所有 project 找 root session 所在目录——`get_subagent_trace` 调用
        // 方（Tauri command）只携带 sessionId，不带 projectId，所以这里需要
        // scan。`scan_subagents_for` 在 cdt-discover 里有现成实现，但这里
        // 直接复用 `find_subagent_jsonl` + `path_decoder` 即可保持简单。
        let projects_dir = path_decoder::get_projects_base_path();
        let Ok(mut entries) = tokio::fs::read_dir(&projects_dir).await else {
            return Ok(serde_json::Value::Array(Vec::new()));
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let project_dir = entry.path();
            // root session 自身存在 → 才在该 project 里查 subagent
            let root_jsonl = project_dir.join(format!("{root_session_id}.jsonl"));
            if tokio::fs::metadata(&root_jsonl).await.is_ok() {
                if let Some(path) = find_subagent_jsonl(&project_dir, subagent_session_id).await {
                    let messages = parse_file(&path)
                        .await
                        .map_err(|e| ApiError::internal(format!("parse error: {e}")))?;
                    let mut msgs = messages;
                    for m in &mut msgs {
                        m.is_sidechain = false;
                    }
                    let chunks = cdt_analyze::build_chunks(&msgs);
                    return serde_json::to_value(&chunks)
                        .map_err(|e| ApiError::internal(format!("{e}")));
                }
                break;
            }
        }
        Ok(serde_json::Value::Array(Vec::new()))
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
        let projects_dir = path_decoder::get_projects_base_path();
        let Some(jsonl_path) =
            locate_session_jsonl(&projects_dir, root_session_id, session_id).await
        else {
            tracing::warn!(target: "cdt_api::image", root_session_id, session_id, "jsonl not found");
            return Ok(empty_data_uri());
        };

        // parse 整个文件 → 找 chunk_uuid → 取 block_index 的 image。
        let messages = match parse_file(&jsonl_path).await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(target: "cdt_api::image", error = %e, "parse failed");
                return Ok(empty_data_uri());
            }
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
        let projects_dir = path_decoder::get_projects_base_path();
        let Some(jsonl_path) =
            locate_session_jsonl(&projects_dir, root_session_id, session_id).await
        else {
            tracing::warn!(target: "cdt_api::tool_output", root_session_id, session_id, "jsonl not found");
            return Ok(cdt_core::ToolOutput::Missing);
        };

        let messages = match parse_file(&jsonl_path).await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(target: "cdt_api::tool_output", error = %e, "parse failed");
                return Ok(cdt_core::ToolOutput::Missing);
            }
        };

        // build_chunks 后线性 scan tool_executions 找 tool_use_id 匹配
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
            // 简化：尝试查找，找不到就跳过
            match self.get_session_detail("", sid).await {
                Ok(detail) => results.push(detail),
                Err(_) => results.push(SessionDetail {
                    session_id: sid.clone(),
                    project_id: String::new(),
                    chunks: serde_json::Value::Null,
                    metrics: serde_json::Value::Null,
                    metadata: serde_json::json!({"status": "not_found"}),
                    context_injections: serde_json::Value::Array(Vec::new()),
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

        let config = SearchConfig::default();
        let max_results = 50;

        let project_id = request
            .project_id
            .as_deref()
            .ok_or_else(|| ApiError::validation("project_id is required for search"))?;

        let result = self
            .searcher
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
            "httpServer" => mgr.update_http_server(request.data.clone()).await,
            _ => {
                return Err(ApiError::validation(format!(
                    "unknown section: {}",
                    request.section
                )));
            }
        };
        let config = result.map_err(|e| ApiError::internal(format!("{e}")))?;
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
        let mgr = self.ssh_mgr.lock().await;
        let active = mgr.get_active_context();

        let mut contexts = vec![ContextInfo {
            id: "local".into(),
            kind: "local".into(),
            is_active: matches!(active, ActiveContext::Local),
            host: None,
        }];

        for status in mgr.get_all_statuses() {
            contexts.push(ContextInfo {
                id: status.context_id.clone(),
                kind: "ssh".into(),
                is_active: matches!(active, ActiveContext::Ssh(id) if id == &status.context_id),
                host: status.host.clone(),
            });
        }

        Ok(contexts)
    }

    async fn switch_context(&self, context_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.ssh_mgr.lock().await;
        if context_id == "local" {
            mgr.set_active_context(ActiveContext::Local);
        } else {
            mgr.set_active_context(ActiveContext::Ssh(context_id.to_owned()));
        }
        Ok(())
    }

    async fn ssh_connect(
        &self,
        request: &SshConnectRequest,
    ) -> Result<serde_json::Value, ApiError> {
        let config_path = cdt_ssh::default_ssh_config_path();
        let configs = parse_ssh_config_file(&config_path).await;
        let host_config = resolve_host(&configs, &request.host_alias)
            .ok_or_else(|| ApiError::not_found(format!("SSH host: {}", request.host_alias)))?;

        let context_id = request
            .context_id
            .clone()
            .unwrap_or_else(|| request.host_alias.clone());

        let mut mgr = self.ssh_mgr.lock().await;
        let status = mgr.register_connection(&context_id, &host_config);
        serde_json::to_value(status).map_err(|e| ApiError::internal(format!("{e}")))
    }

    async fn ssh_disconnect(&self, context_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.ssh_mgr.lock().await;
        mgr.disconnect(context_id);
        Ok(())
    }

    async fn resolve_ssh_host(&self, alias: &str) -> Result<serde_json::Value, ApiError> {
        let config_path = cdt_ssh::default_ssh_config_path();
        let configs = parse_ssh_config_file(&config_path).await;
        let host = resolve_host(&configs, alias)
            .ok_or_else(|| ApiError::not_found(format!("SSH host: {alias}")))?;
        Ok(serde_json::json!({
            "hostname": host.hostname,
            "user": host.user,
            "port": host.port,
            "identityFiles": host.identity_files,
        }))
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
        let result = read_all_claude_md_files(Path::new(project_root)).await;
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

    async fn get_worktree_sessions(&self, _group_id: &str) -> Result<serde_json::Value, ApiError> {
        // 简化：worktree session 需要 WorktreeGrouper，暂返回空
        Ok(serde_json::json!([]))
    }
}

// =============================================================================
// Trigger CRUD（非 trait 方法，供 Tauri commands 直接调用）
// =============================================================================

impl LocalDataApi {
    /// 添加 trigger，返回更新后的 `AppConfig`。
    pub async fn add_trigger(
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

    /// 删除 trigger，返回更新后的 `AppConfig`。
    pub async fn remove_trigger(&self, trigger_id: &str) -> Result<serde_json::Value, ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        let config = mgr
            .remove_trigger(trigger_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        serde_json::to_value(&config).map_err(|e| ApiError::internal(format!("{e}")))
    }

    /// Pin 一个 session（project + session 维度），写入配置文件。
    pub async fn pin_session(&self, project_id: &str, session_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        mgr.pin_session(project_id, session_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    /// 取消 pin，写入配置文件。
    pub async fn unpin_session(&self, project_id: &str, session_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        mgr.unpin_session(project_id, session_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    /// 隐藏一个 session，写入配置文件。
    pub async fn hide_session(&self, project_id: &str, session_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        mgr.hide_session(project_id, session_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    /// 取消隐藏，写入配置文件。
    pub async fn unhide_session(&self, project_id: &str, session_id: &str) -> Result<(), ApiError> {
        let mut mgr = self.config_mgr.lock().await;
        mgr.unhide_session(project_id, session_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))
    }

    /// 返回当前 project 的 pin/hide session id 列表，供前端首次 load 时 prime `$state`。
    ///
    /// 列表顺序保持 `ConfigManager` 内部的"最近在前"约定（pin 用 `pinned_at` 倒序插入、
    /// hide 用 `hidden_at` 倒序插入）。
    pub async fn get_project_session_prefs(
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

/// 从文件系统扫描 CLAUDE.md 文件，构建 `ClaudeMdContextInjection` 列表。
async fn build_claude_md_from_filesystem(project_root: &str) -> Vec<cdt_core::ContextInjection> {
    use cdt_config::claude_md::Scope;

    let files = read_all_claude_md_files(Path::new(project_root)).await;
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
            let display_name = info
                .path
                .rsplit('/')
                .next()
                .unwrap_or(&info.path)
                .to_owned();
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
/// `session_id == root_session_id` 时直接找 root jsonl；不等时去 root 同 project
/// 的 `subagents/agent-<sub>.jsonl`（与 `find_subagent_jsonl` 同样支持新旧两种结构）。
async fn locate_session_jsonl(
    projects_dir: &Path,
    root_session_id: &str,
    session_id: &str,
) -> Option<std::path::PathBuf> {
    let mut entries = tokio::fs::read_dir(projects_dir).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let project_dir = entry.path();
        let root_jsonl = project_dir.join(format!("{root_session_id}.jsonl"));
        if tokio::fs::metadata(&root_jsonl).await.is_err() {
            continue;
        }
        if session_id == root_session_id {
            return Some(root_jsonl);
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

/// 扫描 subagent 候选文件，构建 `SubagentCandidate` 列表。
///
/// 扫描路径：
/// - 新结构：`{project_dir}/{session_id}/subagents/agent-*.jsonl`
/// - 旧结构：`{project_dir}/agent-*.jsonl`（需要读首行检查 parent session）
///
/// 扫描失败时静默返回空列表（warn 日志）。
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
        let mut config_mgr = ConfigManager::new(Some(config_path));
        config_mgr.load().await.unwrap();
        let notif_mgr = NotificationManager::new(None);
        let ssh_mgr = SshConnectionManager::new();
        let scanner = ProjectScanner::new(local_handle(), std::path::PathBuf::from("/tmp"));
        LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr)
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
            }],
            slash_commands: Vec::new(),
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
            output_omitted: false,
            output_bytes: None,
        }
    }

    fn make_ai_chunk_with_tool(exec: cdt_core::ToolExecution) -> cdt_core::Chunk {
        cdt_core::Chunk::Ai(cdt_core::AIChunk {
            timestamp: ts(),
            duration_ms: None,
            responses: Vec::new(),
            metrics: cdt_core::ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: vec![exec],
            subagents: Vec::new(),
            slash_commands: Vec::new(),
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
            }],
            slash_commands: Vec::new(),
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
}
