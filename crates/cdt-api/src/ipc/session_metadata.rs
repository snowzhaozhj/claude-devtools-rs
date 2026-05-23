//! 轻量 session 元数据提取：标题 + 消息计数。
//!
//! 标题语义见 spec `openspec/specs/ipc-data-api/spec.md`：
//! - 跳过 `is_meta` / `<local-command-*>` 命令输出 / `[Request interrupted by user` 起首消息
//! - 带非空 `<command-args>` 的 slash 直接作 title；空 / 无 args 的 slash 走 `command_fallback`
//! - teammate-message 主导消息优先取 `summary` 属性；其它走 `sanitize_for_title`
//! - `sanitize_for_title` 移除 8 个 system tag + `teammate-message` + `Read the output file…` 指令
//! - 字符数 ≤ `TITLE_MAX_CHARS` (500，Unicode `char` 计数)
//!
//! 其它字段：
//! - 消息计数：user + 对应 assistant 轮次配对计数，过滤规则对齐原版 `isParsedUserChunkMessage`
//! - `isOngoing`：`check_messages_ongoing` + 文件 mtime stale check（5 分钟）

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex as StdMutex, OnceLock};
use std::time::{Duration, SystemTime};

use cdt_fs::{ContextId, FileSystemProvider};
use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};

use cdt_core::message::{ContentBlock, MessageCategory, MessageContent, ParsedMessage};
use cdt_parse::parse_entry_at;

use crate::cache_signature::FileSignature;

/// 文件 mtime 距 now 超过此阈值则即便消息序列结构上为 ongoing 也强制判 done。
/// 5 分钟，对齐原版 `STALE_SESSION_THRESHOLD_MS`。
pub const STALE_SESSION_THRESHOLD: Duration = Duration::from_secs(5 * 60);

/// scanner 用 `BufReader` 容量 —— 与 SFTP `SSH_FXP_READ` reply 单消息上限对齐。
/// 详 change `unify-fs-direct-calls` design D5：32 KiB 单 `BufReader` fill = 单 SFTP READ
/// message；64 KiB 强制底层拆 2× SFTP READ 无收益；默认 8 KiB 在 SSH 5MB jsonl 需 ~640 RTTs。
const SCANNER_BUF_BYTES: usize = 32 * 1024;

/// `SessionSummary.title` 最大字符数（Unicode `char` 计数，非 byte）。
/// 见 spec `ipc-data-api/spec.md` §`Title length is bounded by TITLE_MAX_CHARS constant`。
pub const TITLE_MAX_CHARS: usize = 500;

/// 中断消息字面量前缀。用户上一轮按 ESC 时 Claude Code 注入。
/// 见 spec `ipc-data-api/spec.md` §`Sanitize title against interruption and task-output instructions`。
const REQUEST_INTERRUPTED_PREFIX: &str = "[Request interrupted by user";

/// `task-notification` 后系统注入的"读取输出文件"指令模式。
/// 对齐 TS 原版 `contentSanitizer.ts::TASK_OUTPUT_INSTRUCTION_PATTERN`。
fn task_output_instruction_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r" ?Read the output file to retrieve the result: \S+")
            .expect("task-output regex 字面量合法")
    })
}

/// 提取结果。
pub struct SessionMetadata {
    pub title: Option<String>,
    pub message_count: usize,
    /// 会话是否仍在进行。计算方式见
    /// `cdt_analyze::check_messages_ongoing`。
    pub is_ongoing: bool,
    /// 会话最后一条携带 `git_branch` 的消息行所记录的分支名。
    /// 与原版 `claude-devtools/src/renderer/utils/sessionExporter.ts:304`
    /// 的 `session.gitBranch` 取值方式一致——反映会话最后所在 git 分支。
    pub git_branch: Option<String>,
}

/// 扫描标题时读取的最大行数（与原版 `maxLines: 200` 对齐）。
const TITLE_MAX_LINES: usize = 200;

/// 原版 `SYSTEM_OUTPUT_TAGS`（`messageTags.ts`）：以这些标签起首的 user
/// 内容是命令输出 / 系统注入，不算用户输入。
const SYSTEM_OUTPUT_TAG_PREFIXES: &[&str] = &[
    "<local-command-stdout>",
    "<local-command-stderr>",
    "<local-command-caveat>",
    "<system-reminder>",
];

/// 是否计入 `messageCount` 的真实 user-chunk 消息——对齐原版
/// `claude-devtools/src/main/types/messages.ts::isParsedUserChunkMessage`。
///
/// hard noise / interrupt / synthetic 由 cdt-parse 分类剥离到别的
/// `MessageCategory`；剩余 `MessageCategory::User` 中本函数再过滤：
/// - `is_meta = true` → 排除
/// - 任一 Text block / Text content 中含 `<teammate-message teammate_id="...">` → 排除
///   （复用 `cdt_analyze::contains_teammate_message`，其 regex 要求 `teammate_id` 属性，
///   与原版 `isParsedTeammateMessage` 行为一致；用户写的纯字面量
///   `<teammate-message>note</teammate-message>` 不会误判）
/// - Text 起首（trim 前导空白后）匹配 `SYSTEM_OUTPUT_TAG_PREFIXES` → 排除
/// - Blocks 不含任何 `Text` / `Image` block（纯 `tool_result`-only "工具结果回传"行）→ 排除
/// - Blocks 中任一 Text block **不**经 trim 直接 `starts_with` `SYSTEM_OUTPUT_TAG_PREFIXES`
///   → 排除（与原版 `messages.ts:211-216` 对 array text block 不 trim 一致）
///
/// 详见 `openspec/specs/sidebar-navigation/spec.md` §"会话项展示"。
fn is_user_chunk_message(msg: &ParsedMessage) -> bool {
    if msg.category != MessageCategory::User {
        return false;
    }
    if msg.is_meta {
        return false;
    }
    if cdt_analyze::contains_teammate_message(msg) {
        return false;
    }
    match &msg.content {
        MessageContent::Text(s) => {
            let trimmed = s.trim_start();
            if trimmed.is_empty() {
                return false;
            }
            !starts_with_system_output_tag(trimmed)
        }
        MessageContent::Blocks(blocks) => {
            let has_user_content = blocks
                .iter()
                .any(|b| matches!(b, ContentBlock::Text { .. } | ContentBlock::Image { .. }));
            if !has_user_content {
                return false;
            }
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    // 原版 messages.ts:213 对 array text block 用 textBlock.text.startsWith(tag)，
                    // 不做 trim——保持与原版一致以避免 messageCount 与原版差异
                    // （codex 二审第二轮发现的 bug）。
                    if starts_with_system_output_tag(text) {
                        return false;
                    }
                }
            }
            true
        }
    }
}

fn starts_with_system_output_tag(text: &str) -> bool {
    SYSTEM_OUTPUT_TAG_PREFIXES
        .iter()
        .any(|tag| text.starts_with(tag))
}

/// 扫描 JSONL 文件，提取标题和消息计数（薄 wrapper：复用 `extract_session_metadata_with_ongoing`）。
///
/// 标题只扫描前 `TITLE_MAX_LINES` 行；消息计数扫描全文件。
///
/// path-only Local-only 兼容入口。SSH-aware 调用方 SHALL 用 `extract_session_metadata_with_ongoing(fs, path)`。
pub async fn extract_session_metadata(path: &Path) -> SessionMetadata {
    let fs = cdt_fs::local_handle();
    extract_session_metadata_with_ongoing(&*fs, path).await.0
}

/// `extract_session_metadata` 的内部实现，额外暴露 `messages_ongoing` 中间值
/// 给 cache 写入路径使用 —— 详见 change `multi-session-cpu-cache` D8。
///
/// `is_ongoing = messages_ongoing && !is_file_stale(fs, path)`。缓存只存
/// `messages_ongoing`（不随时间变），`is_ongoing` 在 lookup 时由当前 wall clock
/// 实时合成 stale 状态。
///
/// 通过 `fs.open_read(path)` 拿 `Box<dyn AsyncRead + Send + Unpin>`，`BufReader` 容量
/// 32 KiB 与 SFTP packet 上限对齐（详 change `unify-fs-direct-calls` design D5）。
pub(crate) async fn extract_session_metadata_with_ongoing(
    fs: &dyn FileSystemProvider,
    path: &Path,
) -> (SessionMetadata, bool) {
    let Ok(reader) = fs.open_read(path).await else {
        return (
            SessionMetadata {
                title: None,
                message_count: 0,
                is_ongoing: false,
                git_branch: None,
            },
            false,
        );
    };

    let reader = BufReader::with_capacity(SCANNER_BUF_BYTES, reader);
    let mut lines = reader.lines();

    let mut title: Option<String> = None;
    let mut command_fallback: Option<String> = None;
    let mut message_count: usize = 0;
    let mut awaiting_ai = false;
    let mut line_number: usize = 0;
    // ongoing 流式判定状态机——逐行喂消息，避免在内存中保留全量 ParsedMessage Vec
    // （详见 change `metadata-streaming-ongoing` 与 spec ipc-data-api §
    // `extract_session_metadata` 流式判定 isOngoing 不收集全量消息向量）。
    let mut ongoing_sm = cdt_analyze::IsOngoingStateMachine::new();
    // 取最后一条非空 git_branch（与原版 sessionExporter.ts 取值一致）
    let mut last_git_branch: Option<String> = None;

    while let Ok(Some(line)) = lines.next_line().await {
        line_number += 1;
        if line.trim().is_empty() {
            continue;
        }

        let Ok(Some(msg)) = parse_entry_at(&line, line_number) else {
            continue;
        };

        if let Some(branch) = &msg.git_branch {
            // 过滤字面 "HEAD" —— 原版 Claude Code 在 detached HEAD（worktree
            // 检出某 commit / rebase 中等）状态下会把字面字符串 "HEAD" 写进
            // JSONL 的 `gitBranch` 字段，对用户没有可读语义。与
            // `worktree_grouper::parse_head_branch` detached → None 保持一致：
            // 一律返 None 让 sidebar 隐藏分支 chip（spec sidebar-navigation §
            // "gitBranch 为 null SHALL NOT 渲染该 chip"）。
            if !branch.is_empty() && branch != "HEAD" {
                last_git_branch = Some(branch.clone());
            }
        }

        // --- 消息计数（对齐原版 isParsedUserChunkMessage 过滤；详见
        //     `is_user_chunk_message` doc 与 spec sidebar-navigation
        //     §"会话项展示"）---
        if is_user_chunk_message(&msg) {
            message_count += 1;
            awaiting_ai = true;
        } else if awaiting_ai
            && msg.category == MessageCategory::Assistant
            && msg.model.as_deref() != Some("<synthetic>")
            && !msg.is_sidechain
        {
            message_count += 1;
            awaiting_ai = false;
        }

        // --- 标题提取（只在前 TITLE_MAX_LINES 行内）---
        // spec：`ipc-data-api/spec.md` §`Title prefers slash command with non-empty args ...`
        //                              §`Sanitize title against interruption and task-output instructions`
        if line_number <= TITLE_MAX_LINES
            && title.is_none()
            && msg.category == MessageCategory::User
            && !msg.is_meta
        {
            let text = extract_text(&msg.content);
            if !text.is_empty() {
                let trimmed = text.trim_start();
                if is_command_output(&text) {
                    // 跳过命令输出
                } else if trimmed.starts_with(REQUEST_INTERRUPTED_PREFIX) {
                    // 跳过用户中断标记（既不进 title 也不进 fallback）
                } else if is_command_content(&text) {
                    match extract_command_parts(&text) {
                        Some((slash_name, args)) if !args.is_empty() => {
                            // 带非空 args 的 slash 直接作 title
                            let display = format!("{slash_name} {args}");
                            title = Some(truncate_str(&display, TITLE_MAX_CHARS));
                        }
                        Some((slash_name, _)) if command_fallback.is_none() => {
                            command_fallback = Some(slash_name);
                        }
                        _ => {}
                    }
                } else if let Some(summary) = extract_teammate_summary_title(&text) {
                    // teammate-message 包裹的消息：优先取 `summary` 属性作为标题
                    title = Some(truncate_str(&summary, TITLE_MAX_CHARS));
                } else {
                    let sanitized = sanitize_for_title(&text);
                    if !sanitized.is_empty() {
                        title = Some(truncate_str(&sanitized, TITLE_MAX_CHARS));
                    }
                }
            }
        }

        ongoing_sm.feed(&msg);
    }

    // 没有真实用户消息时用 slash 命令后备
    if title.is_none() {
        title = command_fallback;
    }

    let messages_ongoing = ongoing_sm.finalize();
    let is_ongoing = if messages_ongoing {
        !is_file_stale(fs, path).await
    } else {
        false
    };

    (
        SessionMetadata {
            title,
            message_count,
            is_ongoing,
            git_branch: last_git_branch,
        },
        messages_ongoing,
    )
}

#[allow(dead_code)]
pub(crate) fn extract_session_metadata_from_parsed(
    messages: &[ParsedMessage],
    is_stale: bool,
) -> SessionMetadata {
    let mut title: Option<String> = None;
    let mut command_fallback: Option<String> = None;
    let mut message_count: usize = 0;
    let mut awaiting_ai = false;
    let mut ongoing_sm = cdt_analyze::IsOngoingStateMachine::new();
    let mut last_git_branch: Option<String> = None;

    for (idx, msg) in messages.iter().enumerate() {
        if let Some(branch) = &msg.git_branch {
            if !branch.is_empty() && branch != "HEAD" {
                last_git_branch = Some(branch.clone());
            }
        }

        if is_user_chunk_message(msg) {
            message_count += 1;
            awaiting_ai = true;
        } else if awaiting_ai
            && msg.category == MessageCategory::Assistant
            && msg.model.as_deref() != Some("<synthetic>")
            && !msg.is_sidechain
        {
            message_count += 1;
            awaiting_ai = false;
        }

        if idx < TITLE_MAX_LINES
            && title.is_none()
            && msg.category == MessageCategory::User
            && !msg.is_meta
        {
            let text = extract_text(&msg.content);
            if !text.is_empty() {
                let trimmed = text.trim_start();
                if is_command_output(&text) || trimmed.starts_with(REQUEST_INTERRUPTED_PREFIX) {
                } else if is_command_content(&text) {
                    match extract_command_parts(&text) {
                        Some((slash_name, args)) if !args.is_empty() => {
                            let display = format!("{slash_name} {args}");
                            title = Some(truncate_str(&display, TITLE_MAX_CHARS));
                        }
                        Some((slash_name, _)) if command_fallback.is_none() => {
                            command_fallback = Some(slash_name);
                        }
                        _ => {}
                    }
                } else if let Some(summary) = extract_teammate_summary_title(&text) {
                    title = Some(truncate_str(&summary, TITLE_MAX_CHARS));
                } else {
                    let sanitized = sanitize_for_title(&text);
                    if !sanitized.is_empty() {
                        title = Some(truncate_str(&sanitized, TITLE_MAX_CHARS));
                    }
                }
            }
        }

        ongoing_sm.feed(msg);
    }

    if title.is_none() {
        title = command_fallback;
    }

    SessionMetadata {
        title,
        message_count,
        is_ongoing: ongoing_sm.finalize() && !is_stale,
        git_branch: last_git_branch,
    }
}

// ============================================================================
// metadata 缓存（详 change `multi-session-cpu-cache` design D3b/D8）
//
// 缓存值不直接存 `is_ongoing` —— 该字段含 wall-clock 时间敏感判定（5 分钟 stale
// 阈值），命中时由 `is_session_stale(signature.mtime, now)` 实时合成。缓存只存
// `messages_ongoing` 中间值（基于消息序列结构判定，不随时间变）。
// ============================================================================

/// metadata 缓存容量上限。
///
/// 从 PR `multi-session-cpu-cache` 的 200 提升到 2000：本 change
/// `metadata-cache-context-prefix` 把 cache key 升级为 `(ContextId, PathBuf)`
/// 后，单个 cache 实例需要同时容纳 Local + 多个 SSH host 的 entry —— 200
/// 容量在多 context 共享下会让 SSH cache 几次列表查询就被挤光。详 design D4。
pub const METADATA_CACHE_CAPACITY: usize = 2000;

/// 单条缓存记录：`FileSignature` + 各字段（不含时间敏感的 `is_ongoing`）。
#[derive(Debug, Clone)]
pub(crate) struct MetadataCacheEntry {
    pub(crate) signature: FileSignature,
    pub(crate) title: Option<String>,
    pub(crate) message_count: usize,
    pub(crate) messages_ongoing: bool,
    pub(crate) git_branch: Option<String>,
}

/// cache key —— `(ContextId, PathBuf)` tuple，按 PR-A spec
/// `openspec/specs/fs-abstraction/spec.md` §`fs-related cache 必须采用'单实例 + ContextId key 前缀'拓扑`
/// 钉死的契约：单实例 cache + key 含 `ContextId` 前缀。Local vs SSH / 不同 SSH host
/// 间天然由 `ContextId` 的 `Hash`/`Eq` 隔离，不串扰。
type MetadataCacheKey = (ContextId, PathBuf);

/// `LocalDataApi` 持有的 metadata LRU 缓存。**不**用全局单例（详 design D3b）。
#[derive(Debug)]
pub struct MetadataCache {
    map: HashMap<MetadataCacheKey, MetadataCacheEntry>,
    order: VecDeque<MetadataCacheKey>,
    capacity: usize,
}

impl Default for MetadataCache {
    fn default() -> Self {
        Self::new(METADATA_CACHE_CAPACITY)
    }
}

impl MetadataCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    fn lookup(&mut self, ctx: &ContextId, path: &Path) -> Option<MetadataCacheEntry> {
        // HashMap key 是 owned `(ContextId, PathBuf)` tuple，无法用 `(&ContextId, &Path)`
        // 直接 get；克隆 key 用于 lookup 是常规模式（ContextId ~300 bytes + PathBuf ~120
        // bytes 短暂分配，每次 cache hit 几百 ns 可忽略，相对 fs.stat 微秒级开销）。
        let key = (ctx.clone(), path.to_path_buf());
        let entry = self.map.get(&key)?.clone();
        if let Some(pos) = self.order.iter().position(|k| k == &key) {
            let k = self.order.remove(pos).expect("position 已校验");
            self.order.push_front(k);
        }
        Some(entry)
    }

    /// 用调用方提供的 `FileSignature` 直接查 cache —— 跳过内部 stat。
    ///
    /// 用于 list 后台 batch 校验路径：调用方先 `fs.read_dir_with_metadata(parent)`
    /// 一次拿全 dir 内 entry 的 metadata，再批量 lookup，避免 N 次串行 stat
    /// （详 change `unify-fs-direct-calls` design D3）。
    ///
    /// signature 字段 byte-equal 才命中；mismatch 返 None。命中时 LRU bump 到队首。
    #[allow(dead_code)]
    pub(crate) fn lookup_with_known_signature(
        &mut self,
        ctx: &ContextId,
        path: &Path,
        signature: &FileSignature,
    ) -> Option<MetadataCacheEntry> {
        let key = (ctx.clone(), path.to_path_buf());
        let entry = self.map.get(&key)?.clone();
        if entry.signature != *signature {
            return None;
        }
        if let Some(pos) = self.order.iter().position(|k| k == &key) {
            let k = self.order.remove(pos).expect("position 已校验");
            self.order.push_front(k);
        }
        Some(entry)
    }

    /// hot path cache hit trust —— 不校验 signature 直接返当前 entry。
    ///
    /// 调用方语义：信任 cache 内容立即返回 UI 渲染（0 fs op），signature 校验
    /// 由后台 batch task 异步跑（详 change `unify-fs-direct-calls` design D3）。
    /// 命中时 LRU bump。
    pub(crate) fn lookup_trust_cached(
        &mut self,
        ctx: &ContextId,
        path: &Path,
    ) -> Option<MetadataCacheEntry> {
        let key = (ctx.clone(), path.to_path_buf());
        let entry = self.map.get(&key)?.clone();
        if let Some(pos) = self.order.iter().position(|k| k == &key) {
            let k = self.order.remove(pos).expect("position 已校验");
            self.order.push_front(k);
        }
        Some(entry)
    }

    fn insert(&mut self, key: MetadataCacheKey, entry: MetadataCacheEntry) {
        if self.map.contains_key(&key) {
            self.map.insert(key.clone(), entry);
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                let k = self.order.remove(pos).expect("position 已校验");
                self.order.push_front(k);
            }
            return;
        }

        if self.map.len() >= self.capacity {
            if let Some(evicted) = self.order.pop_back() {
                self.map.remove(&evicted);
            }
        }

        self.map.insert(key.clone(), entry);
        self.order.push_front(key);
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }
}

/// `extract_session_metadata_cached` 的 lookup-only 变体：只查 cache + 校验
/// `FileSignature`，miss 或 stat 失败返回 `None`，**不**触发扫描。
///
/// 用于 `list_sessions_skeleton` 的 fast-path：cache 命中时骨架阶段直接带元数据
/// 返回，规避 `session-metadata-update` event 在前端 listener 注册前 fire-and-forget
/// 丢失的 race（详见 fix `session-title-race` 修复说明）。
///
/// stat 走 `FileSystemProvider::stat`（详 change `metadata-cache-context-prefix`
/// design D2 / D3），让 Local 与 SSH 两条 fs backend 共用同一 cache 抽象——
/// 调用方按"当前 active context"传 fs + ctx，cache key 加 `ContextId` 前缀防串扰。
pub(crate) async fn try_lookup_cached_metadata(
    cache: &StdMutex<MetadataCache>,
    fs: &dyn FileSystemProvider,
    context_id: &ContextId,
    path: &Path,
) -> Option<SessionMetadata> {
    let Ok(meta) = fs.stat(path).await else {
        cdt_telemetry::counter!("metadata.cache.stat_err").inc();
        return None;
    };
    let sig = FileSignature::from_fs_metadata(&meta);
    let entry = {
        let lookup = cache
            .lock()
            .expect("metadata cache mutex poisoned")
            .lookup(context_id, path);
        if let Some(e) = lookup {
            e
        } else {
            cdt_telemetry::counter!("metadata.cache.miss").inc();
            return None;
        }
    };
    if entry.signature != sig {
        cdt_telemetry::counter!("metadata.cache.sig_mismatch").inc();
        return None;
    }
    cdt_telemetry::counter!("metadata.cache.hit").inc();
    let is_ongoing = entry.messages_ongoing && !is_session_stale(sig.mtime, SystemTime::now());
    Some(SessionMetadata {
        title: entry.title,
        message_count: entry.message_count,
        is_ongoing,
        git_branch: entry.git_branch,
    })
}

/// `extract_session_metadata` 的缓存 wrapper —— `LocalDataApi` 持有 cache 实例。
///
/// 命中：返回基于缓存合成的 `SessionMetadata`（`is_ongoing` 用 `messages_ongoing`
/// 与 `is_session_stale(signature.mtime, now)` 实时合成）。
/// miss / stat 失败：调 uncached `extract_session_metadata_with_ongoing` 重扫，
/// 成功后写缓存。stat 失败不写缓存。
///
/// stat 走 `FileSystemProvider::stat`（详 change `metadata-cache-context-prefix`
/// design D2 / D8）；cache miss 后的扫描路径仍是 `tokio::fs::File::open`，本
/// change scope 边界——SSH callsite 当前不经过此 wrapper（详 spec
/// `ipc-data-api/spec.md` Scenario `本 change 不强制切 scanner 路径`），完整
/// SSH 接入 + scanner 切 `fs.open_read` 留 PR-D。
pub(crate) async fn extract_session_metadata_cached(
    cache: &StdMutex<MetadataCache>,
    fs: &dyn FileSystemProvider,
    context_id: &ContextId,
    path: &Path,
) -> SessionMetadata {
    // SSH 远端 mtime 与本机 `SystemTime::now()` 跨 clock domain，5min 阈值不可
    // 比对（远端时钟回拨/时差产生 false positive/negative）；SSH callsite SHALL
    // skip stale check 让 `is_ongoing = messages_ongoing`（详 change
    // `unify-fs-direct-calls` design D2 line 2171 / codex 二审 H1 + ADR 同 SSH
    // policy fork）。Local context 仍按 `messages_ongoing && !stale` 合成。
    let backend_skips_stale = matches!(fs.kind(), cdt_discover::FsKind::Ssh);
    let new_sig = fs
        .stat(path)
        .await
        .ok()
        .map(|m| FileSignature::from_fs_metadata(&m));

    if let Some(sig) = new_sig {
        let cached = cache
            .lock()
            .expect("metadata cache mutex poisoned")
            .lookup(context_id, path);
        if let Some(entry) = cached {
            if entry.signature == sig {
                let is_ongoing = if backend_skips_stale {
                    entry.messages_ongoing
                } else {
                    entry.messages_ongoing && !is_session_stale(sig.mtime, SystemTime::now())
                };
                return SessionMetadata {
                    title: entry.title,
                    message_count: entry.message_count,
                    is_ongoing,
                    git_branch: entry.git_branch,
                };
            }
        }
    }

    let (mut meta, messages_ongoing) = extract_session_metadata_with_ongoing(fs, path).await;
    if backend_skips_stale {
        // scanner 内部已对 Local 路径做 stale check 合并；SSH 走外层 skip 后用
        // `messages_ongoing` 覆写（scanner 内部 `is_file_stale` 也用本机时钟，跨
        // clock domain 不可信）。
        meta.is_ongoing = messages_ongoing;
    }

    if let Some(sig) = new_sig {
        cache.lock().expect("metadata cache mutex poisoned").insert(
            (context_id.clone(), path.to_path_buf()),
            MetadataCacheEntry {
                signature: sig,
                title: meta.title.clone(),
                message_count: meta.message_count,
                messages_ongoing,
                git_branch: meta.git_branch.clone(),
            },
        );
    }

    meta
}

/// 异步读 file mtime 并判定是否超过 stale 阈值。
/// stat 失败时回退到 `false`（不强制 stale，保守保留 `messages_ongoing` 的判定）。
///
/// 通过 `FileSystemProvider::stat` 走当前 active context 的 fs（Local / SSH）。
/// 注：SSH context 下远端 mtime 与本机 `SystemTime::now()` 跨 clock domain，
/// 不可比对——SSH callsite SHALL 通过外层 policy fork 跳过此判定（详 change
/// `unify-fs-direct-calls` design D2 line 2171 + tasks 6.4 ADR）。
pub async fn is_file_stale(fs: &dyn FileSystemProvider, path: &Path) -> bool {
    let Ok(meta) = fs.stat(path).await else {
        return false;
    };
    is_session_stale(meta.mtime, SystemTime::now())
}

/// 纯函数版本：给定文件 mtime 与"当前时刻"判定 session 是否 stale。
/// `now` 早于 `file_modified`（时钟回拨等异常）时返回 `false`。
pub fn is_session_stale(file_modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(file_modified)
        .is_ok_and(|elapsed| elapsed >= STALE_SESSION_THRESHOLD)
}

fn extract_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut parts = Vec::new();
            for block in blocks {
                if let cdt_core::ContentBlock::Text { text } = block {
                    if !text.is_empty() {
                        parts.push(text.as_str());
                    }
                }
            }
            parts.join("\n")
        }
    }
}

fn is_command_content(content: &str) -> bool {
    content.starts_with("<command-name>") || content.starts_with("<command-message>")
}

fn is_command_output(content: &str) -> bool {
    content.starts_with("<local-command-stdout>") || content.starts_with("<local-command-stderr>")
}

/// 提取 slash 命令的 `(/name, args_trimmed)` 两部分。
///
/// - `/name`：永远带前导 `/`（即便原文 `<command-name>` 内未含 `/`）
/// - `args_trimmed`：`<command-args>...</command-args>` 内容 trim 后的字符串；
///   tag 缺失 / 自闭合 / 内容仅空白时 SHALL 返回空字符串
///
/// 返回 `None` 仅当 `<command-name>` tag 缺失或内容空。
///
/// 见 spec `ipc-data-api/spec.md` §`Title prefers slash command with non-empty args ...`。
fn extract_command_parts(content: &str) -> Option<(String, String)> {
    let name = extract_tag_content(content, "command-name")?;
    let name = name.strip_prefix('/').unwrap_or(&name);
    let slash_name = format!("/{name}");
    let args = extract_tag_content(content, "command-args").unwrap_or_default();
    Some((slash_name, args))
}

/// 从 `<tag>content</tag>` 提取 content。
fn extract_tag_content(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    let inner = text[start..end].trim();
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

/// 简单清洗：移除噪声标签（含内容），用于标题。
///
/// `<teammate-message ...>...</teammate-message>` 的 attributes 形式靠
/// 通用前缀匹配剥除（与无 attribute 的 7 个标签共享算法）。
///
/// `Read the output file to retrieve the result: <path>` 系统指令清洗 SHALL
/// 仅在原文确实含 `<task-notification>` tag 时触发——避免误删用户在普通消息
/// 中手写的同字面量（如教程引用）。详 codex 二审反馈与 spec
/// `ipc-data-api/spec.md` §`Sanitize title against interruption and task-output instructions`。
fn sanitize_for_title(text: &str) -> String {
    let had_task_notification = text.contains("<task-notification>");
    let mut s = text.to_string();
    let tags = [
        "system-reminder",
        "local-command-caveat",
        "task-notification",
        "command-name",
        "command-message",
        "command-args",
        "local-command-stdout",
        "local-command-stderr",
    ];
    for tag in tags {
        loop {
            let open = format!("<{tag}>");
            let close = format!("</{tag}>");
            let Some(start) = s.find(&open) else { break };
            if let Some(rel_end) = s[start..].find(&close) {
                s.replace_range(start..start + rel_end + close.len(), "");
            } else {
                // 没有闭合标签，移除从 open 开始到末尾
                s.truncate(start);
                break;
            }
        }
    }
    // teammate-message 含 attributes（teammate_id / color / summary），用前缀
    // 模式 `<teammate-message ` 匹配开 tag。
    loop {
        let close = "</teammate-message>";
        let Some(start) = s
            .find("<teammate-message ")
            .or_else(|| s.find("<teammate-message>"))
        else {
            break;
        };
        if let Some(rel_end) = s[start..].find(close) {
            s.replace_range(start..start + rel_end + close.len(), "");
        } else {
            s.truncate(start);
            break;
        }
    }
    // task-notification 后系统注入的 "Read the output file to retrieve the result: <path>"
    // 指令残留（task-notification tag 已被上面循环剥除），按正则移除。
    // 仅在原文含 `<task-notification>` 时触发，防止用户在普通消息中手写同字面量被吞。
    if had_task_notification {
        let cleaned = task_output_instruction_regex()
            .replace_all(&s, "")
            .into_owned();
        return cleaned.trim().to_string();
    }
    s.trim().to_string()
}

/// 若 `text` trim 后以 `<teammate-message` 起首，按优先级提取标题候选：
/// 1. `summary="..."` 属性（如有非空值）
/// 2. fallback：tag body 文本（`<teammate-message ...>body</teammate-message>`
///    中 body 部分 trim 后非空）
///
/// 非 teammate 主导消息或两者都失败时返回 `None`。
///
/// 历史踩坑：原实现只看 `summary` 属性，没 summary 时 fallback 走
/// `sanitize_for_title`，那里把 `<teammate-message ...>...</teammate-message>`
/// **整段含 body** 剥除 → title 变空。结果是 teammate-message 主导消息
/// 无 summary 属性时（典型场景：用户发 teammate body 但没标 summary），
/// 该 session 的 title 永久卡在 null，UI fallback 到 sessionId 前缀
/// （2026-05-21 修复）。
///
/// Spec：`openspec/specs/ipc-data-api/spec.md`
/// §`Strip teammate-message tags from session title`。
fn extract_teammate_summary_title(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("<teammate-message") {
        return None;
    }
    let tag_end = trimmed.find('>')?;
    let attrs = &trimmed[..tag_end];
    // 1. 优先 summary="..."
    if let Some(idx) = attrs.find("summary=\"") {
        let after = &attrs[idx + "summary=\"".len()..];
        if let Some(close) = after.find('"') {
            let summary = after[..close].trim();
            if !summary.is_empty() {
                return Some(summary.to_string());
            }
        }
    }
    // 2. fallback：提取 body 文本（截到 `</teammate-message>` 或文本末尾）。
    let after_open_tag = &trimmed[tag_end + 1..];
    let body = if let Some(close_pos) = after_open_tag.find("</teammate-message>") {
        &after_open_tag[..close_pos]
    } else {
        after_open_tag
    };
    let body_trimmed = body.trim();
    if body_trimmed.is_empty() {
        None
    } else {
        Some(body_trimmed.to_string())
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_secs_after(base: SystemTime, secs: u64) -> SystemTime {
        base + Duration::from_secs(secs)
    }

    #[test]
    fn freshly_written_session_is_not_stale() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let modified = at_secs_after(now, 0);
        assert!(!is_session_stale(modified, now));
    }

    #[test]
    fn session_at_4min_59s_is_not_stale() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let modified = now - Duration::from_secs(4 * 60 + 59);
        assert!(!is_session_stale(modified, now));
    }

    #[test]
    fn session_at_5min_exactly_is_stale() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let modified = now - STALE_SESSION_THRESHOLD;
        assert!(is_session_stale(modified, now));
    }

    #[test]
    fn session_far_in_past_is_stale() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let modified = now - Duration::from_secs(7 * 24 * 60 * 60);
        assert!(is_session_stale(modified, now));
    }

    #[test]
    fn clock_skew_with_future_mtime_is_not_stale() {
        // file_modified > now（NTP 漂移 / 时区错配等）：保守判 not stale。
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let modified = now + Duration::from_secs(60);
        assert!(!is_session_stale(modified, now));
    }

    // ---- teammate-message title sanitize ----
    //
    // Spec：`openspec/specs/ipc-data-api/spec.md`
    // §`Strip teammate-message tags from session title`。

    #[test]
    fn teammate_summary_extracted_when_message_solely_wrapped() {
        let text = r#"<teammate-message teammate_id="alice" color="blue" summary="Set up project">body</teammate-message>"#;
        let summary = extract_teammate_summary_title(text);
        assert_eq!(summary.as_deref(), Some("Set up project"));
    }

    #[test]
    fn teammate_no_summary_falls_back_to_body() {
        // teammate-message 主导消息但无 summary 属性时 SHALL fallback 到 body
        // 文本作 title（2026-05-21 修复：原实现返 None 让 sanitize_for_title
        // 把整段含 body 一起剥光，导致 title 永久 null）。
        let text =
            r#"<teammate-message teammate_id="alice" color="blue">body text</teammate-message>"#;
        let title = extract_teammate_summary_title(text);
        assert_eq!(title.as_deref(), Some("body text"));
    }

    #[test]
    fn teammate_empty_body_returns_none() {
        // body 为空且无 summary → 仍返 None（不当 title）。
        let text = r#"<teammate-message teammate_id="alice"></teammate-message>"#;
        let title = extract_teammate_summary_title(text);
        assert!(title.is_none());
    }

    #[test]
    fn non_teammate_message_returns_none() {
        let text = "Hello team, please respond.";
        let summary = extract_teammate_summary_title(text);
        assert!(summary.is_none());
    }

    #[test]
    fn sanitize_strips_teammate_message_tag() {
        let text = r#"Hello team. <teammate-message teammate_id="alice" summary="x">body</teammate-message> please continue."#;
        let result = sanitize_for_title(text);
        assert!(
            !result.contains("<teammate-message"),
            "sanitize 后不应残留 <teammate-message 字面量: {result:?}"
        );
        assert!(
            !result.contains("</teammate-message>"),
            "sanitize 后不应残留 </teammate-message> 字面量: {result:?}"
        );
        assert!(
            result.starts_with("Hello team."),
            "应保留前置正文: {result:?}"
        );
        assert!(
            result.ends_with("please continue."),
            "应保留后置正文: {result:?}"
        );
    }

    #[test]
    fn sanitize_handles_teammate_without_attributes() {
        // 边界：自闭合 attributes 缺失（罕见）
        let text = r"prefix<teammate-message>inner</teammate-message>suffix";
        let result = sanitize_for_title(text);
        assert_eq!(result, "prefixsuffix");
    }

    // ---- git_branch extraction ----
    //
    // Spec：`openspec/specs/ipc-data-api/spec.md`
    // §`Expose git branch on session summary and metadata updates`。

    fn write_jsonl(dir: &std::path::Path, lines: &[&str]) -> std::path::PathBuf {
        let path = dir.join("s.jsonl");
        std::fs::write(&path, lines.join("\n")).unwrap();
        path
    }

    fn user_line(uuid: &str, ts: &str, branch: Option<&str>) -> String {
        let branch_field = branch.map_or(String::new(), |b| format!(r#""gitBranch":"{b}","#));
        format!(
            r#"{{"type":"user","uuid":"{uuid}","timestamp":"{ts}","sessionId":"sid","cwd":"/tmp",{branch_field}"message":{{"role":"user","content":"hi"}}}}"#
        )
    }

    #[tokio::test]
    async fn extract_takes_last_non_empty_git_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_line("u1", "2026-05-03T10:00:00.000Z", Some("main")),
                &user_line("u2", "2026-05-03T10:01:00.000Z", None),
                &user_line("u3", "2026-05-03T10:02:00.000Z", Some("feat/x")),
                &user_line("u4", "2026-05-03T10:03:00.000Z", Some("feat/y")),
                &user_line("u5", "2026-05-03T10:04:00.000Z", None),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        assert_eq!(meta.git_branch.as_deref(), Some("feat/y"));
    }

    #[tokio::test]
    async fn extract_returns_none_when_no_git_branch_anywhere() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_line("u1", "2026-05-03T10:00:00.000Z", None),
                &user_line("u2", "2026-05-03T10:01:00.000Z", None),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        assert!(meta.git_branch.is_none());
    }

    #[tokio::test]
    async fn extract_skips_empty_string_git_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_line("u1", "2026-05-03T10:00:00.000Z", Some("main")),
                &user_line("u2", "2026-05-03T10:01:00.000Z", Some("")),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        assert_eq!(meta.git_branch.as_deref(), Some("main"));
    }

    /// detached HEAD 时原版 Claude Code 把字面 "HEAD" 写进 `gitBranch` 字段，
    /// 对用户无可读语义。extract 应跳过该值，与 `worktree_grouper::parse_head_branch`
    /// detached → None 保持一致，避免会话列表显示 "HEAD" 当成分支名。
    #[tokio::test]
    async fn extract_skips_head_literal_git_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_line("u1", "2026-05-03T10:00:00.000Z", Some("main")),
                &user_line("u2", "2026-05-03T10:01:00.000Z", Some("HEAD")),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        // 即便 "HEAD" 出现在最后一条非空 branch 位置，也 SHALL 跳过它，保留前面的 "main"
        assert_eq!(meta.git_branch.as_deref(), Some("main"));
    }

    #[tokio::test]
    async fn extract_returns_none_when_only_head_literal() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_line("u1", "2026-05-03T10:00:00.000Z", Some("HEAD")),
                &user_line("u2", "2026-05-03T10:01:00.000Z", Some("HEAD")),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        assert!(meta.git_branch.is_none());
    }

    // ---- messageCount: isParsedUserChunkMessage parity ----
    //
    // Spec：`openspec/specs/sidebar-navigation/spec.md` §"会话项展示"
    //   消息计数语义：对齐原版 `isParsedUserChunkMessage` 过滤逻辑。

    fn assistant_line(uuid: &str, ts: &str) -> String {
        format!(
            r#"{{"type":"assistant","uuid":"{uuid}","timestamp":"{ts}","sessionId":"sid","cwd":"/tmp","message":{{"role":"assistant","model":"claude-sonnet","content":[{{"type":"text","text":"answer"}}]}}}}"#
        )
    }

    fn assistant_tool_use_line(uuid: &str, ts: &str, tool_id: &str) -> String {
        format!(
            r#"{{"type":"assistant","uuid":"{uuid}","timestamp":"{ts}","sessionId":"sid","cwd":"/tmp","message":{{"role":"assistant","model":"claude-sonnet","content":[{{"type":"tool_use","id":"{tool_id}","name":"Bash","input":{{"command":"ls"}}}}]}}}}"#
        )
    }

    fn user_tool_result_line(uuid: &str, ts: &str, tool_id: &str) -> String {
        format!(
            r#"{{"type":"user","uuid":"{uuid}","timestamp":"{ts}","sessionId":"sid","cwd":"/tmp","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{tool_id}","content":"ok"}}]}}}}"#
        )
    }

    fn user_text_line(uuid: &str, ts: &str, text: &str) -> String {
        let escaped = text.replace('"', "\\\"");
        format!(
            r#"{{"type":"user","uuid":"{uuid}","timestamp":"{ts}","sessionId":"sid","cwd":"/tmp","message":{{"role":"user","content":"{escaped}"}}}}"#
        )
    }

    fn user_blocks_line(uuid: &str, ts: &str, content_json: &str) -> String {
        format!(
            r#"{{"type":"user","uuid":"{uuid}","timestamp":"{ts}","sessionId":"sid","cwd":"/tmp","message":{{"role":"user","content":{content_json}}}}}"#
        )
    }

    #[tokio::test]
    async fn message_count_excludes_tool_result_only_user_rows() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_text_line("u1", "2026-05-03T10:00:00.000Z", "hi"),
                &assistant_tool_use_line("a1", "2026-05-03T10:00:01.000Z", "tu1"),
                &user_tool_result_line("u2", "2026-05-03T10:00:02.000Z", "tu1"),
                &assistant_line("a2", "2026-05-03T10:00:03.000Z"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        // 真实 user-chunk 1 条 + 配对 assistant 1 条 = 2；tool_result-only 行不计入
        assert_eq!(meta.message_count, 2);
    }

    #[tokio::test]
    async fn message_count_includes_text_plus_tool_result_mixed_user_row() {
        let tmp = tempfile::tempdir().unwrap();
        let mixed_blocks = r#"[{"type":"text","text":"please continue"},{"type":"tool_result","tool_use_id":"tu1","content":"ok"}]"#;
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_blocks_line("u1", "2026-05-03T10:00:00.000Z", mixed_blocks),
                &assistant_line("a1", "2026-05-03T10:00:01.000Z"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        // text + tool_result 混合 → 含 text block → 计入
        assert_eq!(meta.message_count, 2);
    }

    #[tokio::test]
    async fn message_count_includes_image_only_user_row() {
        let tmp = tempfile::tempdir().unwrap();
        let image_blocks = r#"[{"type":"image","source":{"type":"base64","media_type":"image/png","data":"AAA"}}]"#;
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_blocks_line("u1", "2026-05-03T10:00:00.000Z", image_blocks),
                &assistant_line("a1", "2026-05-03T10:00:01.000Z"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        // image block 也算用户输入 → 计入
        assert_eq!(meta.message_count, 2);
    }

    #[tokio::test]
    async fn message_count_excludes_is_meta_user_rows() {
        let tmp = tempfile::tempdir().unwrap();
        let meta_line = r#"{"type":"user","uuid":"u1","timestamp":"2026-05-03T10:00:00.000Z","sessionId":"sid","cwd":"/tmp","isMeta":true,"message":{"role":"user","content":"system bootstrap"}}"#;
        let path = write_jsonl(
            tmp.path(),
            &[
                meta_line,
                &user_text_line("u2", "2026-05-03T10:00:01.000Z", "hi"),
                &assistant_line("a1", "2026-05-03T10:00:02.000Z"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        // isMeta=true user 行不计入；剩下真实 user + assistant = 2
        assert_eq!(meta.message_count, 2);
    }

    #[tokio::test]
    async fn message_count_excludes_non_empty_command_output_user_row() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_text_line("u1", "2026-05-03T10:00:00.000Z", "/help"),
                &user_text_line(
                    "u2",
                    "2026-05-03T10:00:01.000Z",
                    "<local-command-stdout>some help text</local-command-stdout>",
                ),
                &assistant_line("a1", "2026-05-03T10:00:02.000Z"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        // 非空 stdout 起首的 user 行（cdt-parse 不归 noise，但语义是命令输出）
        // SHALL NOT 计入；真实 slash command + 配对 assistant = 2
        assert_eq!(meta.message_count, 2);
    }

    #[tokio::test]
    async fn message_count_excludes_teammate_message_user_row() {
        let tmp = tempfile::tempdir().unwrap();
        let teammate_text =
            r#"<teammate-message teammate_id=\"alice\" summary=\"x\">hello</teammate-message>"#;
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_text_line("u1", "2026-05-03T10:00:00.000Z", "hi"),
                &assistant_line("a1", "2026-05-03T10:00:01.000Z"),
                &user_text_line("u2", "2026-05-03T10:00:02.000Z", teammate_text),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        // teammate-message 不产 UserChunk → 不计入；u1 + a1 = 2
        assert_eq!(meta.message_count, 2);
    }

    // ---- codex 二轮回归 ----
    //
    // 修 commit 29f6389 中三处与原版 isParsedUserChunkMessage 不一致：

    #[tokio::test]
    async fn message_count_blocks_text_block_does_not_trim_before_tag_match() {
        // codex bug 1：Blocks 中 Text block 检查 system tag 时**不**应 trim_start，
        // 与原版 messages.ts:213 `textBlock.text.startsWith(tag)` 一致。
        // 反例：text 以 " \n<local-command-stdout>..." 起首——原版**不** trim 数组
        // 内 text，所以 startsWith 不命中 → 计入；本仓修前会 trim 后命中 → 漏算。
        let tmp = tempfile::tempdir().unwrap();
        let blocks =
            r#"[{"type":"text","text":" \n<local-command-stdout>x</local-command-stdout>"}]"#;
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_blocks_line("u1", "2026-05-03T10:00:00.000Z", blocks),
                &assistant_line("a1", "2026-05-03T10:00:01.000Z"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        // Blocks 内 text block 前导空白不影响 system-tag 匹配（原版不 trim 数组内 text），
        // 所以这条计入；u1 + a1 = 2
        assert_eq!(meta.message_count, 2);
    }

    #[tokio::test]
    async fn message_count_excludes_teammate_in_non_first_text_block() {
        // codex bug 2：teammate 检测应遍历**所有** Text block，不只是首个。
        // 反例：blocks = [text "prefix", text "<teammate-message ...>...</teammate-message>"]
        // 原版 isParsedTeammateMessage 用 content.some(...) 命中第二个 → 排除；
        // 本仓修前只看首个 block "prefix" 不命中 → 多算。
        let tmp = tempfile::tempdir().unwrap();
        let blocks = r#"[{"type":"text","text":"prefix"},{"type":"text","text":"<teammate-message teammate_id=\"alice\" summary=\"x\">body</teammate-message>"}]"#;
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_text_line("u0", "2026-05-03T10:00:00.000Z", "hi"),
                &assistant_line("a0", "2026-05-03T10:00:01.000Z"),
                &user_blocks_line("u1", "2026-05-03T10:00:02.000Z", blocks),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        // 含 teammate 的 user 行（第二 block）不计入；u0 + a0 = 2
        assert_eq!(meta.message_count, 2);
    }

    #[tokio::test]
    async fn message_count_includes_literal_teammate_tag_without_id_attr() {
        // codex bug 3：teammate 检测应要求 `teammate_id="..."` 属性
        // （原版 regex `^<teammate-message\s+teammate_id="([^"]+)"`）。
        // 反例：用户在文本中写字面量 `<teammate-message>note</teammate-message>`
        // （没 teammate_id 属性，是普通文本里的标签字面量）原版 regex 不匹配
        // → 计入；本仓修前用 `starts_with("<teammate-message")` 误判 → 漏算。
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_text_line(
                    "u1",
                    "2026-05-03T10:00:00.000Z",
                    "<teammate-message>note</teammate-message>",
                ),
                &assistant_line("a1", "2026-05-03T10:00:01.000Z"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        // 字面量 teammate tag（无 teammate_id 属性）= 普通用户文本 → 计入；u1 + a1 = 2
        assert_eq!(meta.message_count, 2);
    }

    // ========================================================================
    // metadata cache 行为单测 —— 覆盖 spec
    // `ipc-data-api/spec.md::extract_session_metadata 按 FileSignature 缓存`
    // 与 `metadata 缓存 ownership 由 LocalDataApi 持有` 的全部 Scenario
    // ========================================================================

    fn make_cache() -> StdMutex<MetadataCache> {
        StdMutex::new(MetadataCache::default())
    }

    fn test_local_ctx(root: &std::path::Path) -> ContextId {
        ContextId::local(root.to_path_buf())
    }

    fn test_ssh_ctx() -> ContextId {
        ContextId::ssh(
            cdt_fs::HostSignature::from_ssh_config_fields(&cdt_fs::SshConfigDigestInput {
                hostname: "host-a.example".into(),
                port: 22,
                user: "alice".into(),
                identity_files: vec![],
                proxyjump: None,
                proxycommand: None,
                hostkeyalias: None,
            }),
            PathBuf::from("/remote/home/.claude/projects"),
        )
    }

    fn test_ssh_ctx_b() -> ContextId {
        ContextId::ssh(
            cdt_fs::HostSignature::from_ssh_config_fields(&cdt_fs::SshConfigDigestInput {
                hostname: "host-b.example".into(),
                port: 22,
                user: "bob".into(),
                identity_files: vec![],
                proxyjump: None,
                proxycommand: None,
                hostkeyalias: None,
            }),
            PathBuf::from("/remote/home-b/.claude/projects"),
        )
    }

    #[tokio::test]
    async fn cached_hit_returns_cached_metadata_without_rereading() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_text_line("u1", "2026-05-03T10:00:00.000Z", "hello world"),
                &assistant_line("a1", "2026-05-03T10:00:01.000Z"),
            ],
        );

        let cache = make_cache();
        let m1 = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        assert_eq!(m1.message_count, 2);

        // 缓存应已写入
        assert_eq!(cache.lock().unwrap().len(), 1);

        // 第二次：FileSignature 不变命中。改变文件内容后再次调用 cached
        // 不会读取——这里通过比较返回结果与缓存一致间接验证（不真改文件）
        let m2 = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        assert_eq!(m1.message_count, m2.message_count);
        assert_eq!(m1.title, m2.title);
        assert_eq!(m1.git_branch, m2.git_branch);
    }

    #[tokio::test]
    async fn cached_miss_when_file_size_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line("u1", "2026-05-03T10:00:00.000Z", "hi")],
        );

        let cache = make_cache();
        let m1 = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        assert_eq!(m1.message_count, 1);

        // append 新内容让 size 变化 → cache miss → 重扫
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        std::fs::write(
            &path,
            format!(
                "{}\n{}\n",
                user_text_line("u1", "2026-05-03T10:00:00.000Z", "hi"),
                user_text_line("u2", "2026-05-03T10:00:02.000Z", "second"),
            ),
        )
        .unwrap();

        let m2 = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        assert_eq!(m2.message_count, 2, "size 变化后应重扫");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cached_miss_when_inode_changes_via_rename() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line("u1", "2026-05-03T10:00:00.000Z", "first")],
        );

        let cache = make_cache();
        let m1 = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        assert_eq!(m1.message_count, 1);

        // 准备替换文件（不同内容）
        let replacement = tmp.path().join("replace.jsonl");
        std::fs::write(
            &replacement,
            format!(
                "{}\n{}\n",
                user_text_line("u9", "2026-05-03T10:00:00.000Z", "renamed"),
                assistant_line("a9", "2026-05-03T10:00:01.000Z"),
            ),
        )
        .unwrap();
        std::fs::rename(&replacement, &path).unwrap();

        let m2 = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        assert_eq!(
            m2.message_count, 2,
            "rename 替换（inode 变化）必须重扫，message_count 应反映新内容"
        );
    }

    #[tokio::test]
    async fn cached_stat_failure_falls_through_no_write() {
        let cache = make_cache();
        // 不存在的 path → stat 失败 → 走 uncached → 返回空 metadata，不写缓存
        let m = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &ContextId::local(PathBuf::from("/")),
            std::path::Path::new("/nonexistent/missing.jsonl"),
        )
        .await;
        assert_eq!(m.message_count, 0);
        assert!(m.title.is_none());

        assert_eq!(cache.lock().unwrap().len(), 0, "stat 失败不应写缓存");
    }

    // -------- MetadataCache LRU + bump 行为 --------

    fn dummy_entry(size: u64) -> MetadataCacheEntry {
        MetadataCacheEntry {
            signature: FileSignature {
                mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(size),
                size,
                #[cfg(unix)]
                identity: crate::cache_signature::FileIdentity::Unix { dev: 1, ino: size },
                #[cfg(not(unix))]
                identity: crate::cache_signature::FileIdentity::None,
            },
            title: None,
            message_count: usize::try_from(size).unwrap_or(0),
            messages_ongoing: false,
            git_branch: None,
        }
    }

    #[test]
    fn metadata_cache_evicts_lru_when_over_capacity() {
        let mut cache = MetadataCache::new(2);
        let ctx = ContextId::local(PathBuf::from("/"));
        cache.insert((ctx.clone(), PathBuf::from("/a")), dummy_entry(1));
        cache.insert((ctx.clone(), PathBuf::from("/b")), dummy_entry(2));
        cache.insert((ctx.clone(), PathBuf::from("/c")), dummy_entry(3));
        // /a 应被淘汰
        assert!(cache.lookup(&ctx, std::path::Path::new("/a")).is_none());
        assert!(cache.lookup(&ctx, std::path::Path::new("/b")).is_some());
        assert!(cache.lookup(&ctx, std::path::Path::new("/c")).is_some());
        assert!(cache.len() <= 2);
    }

    #[test]
    fn metadata_cache_lookup_bumps_hit_to_front() {
        let mut cache = MetadataCache::new(2);
        let ctx = ContextId::local(PathBuf::from("/"));
        cache.insert((ctx.clone(), PathBuf::from("/a")), dummy_entry(1));
        cache.insert((ctx.clone(), PathBuf::from("/b")), dummy_entry(2));
        // lookup /a → bump 到队首
        assert!(cache.lookup(&ctx, std::path::Path::new("/a")).is_some());
        cache.insert((ctx.clone(), PathBuf::from("/c")), dummy_entry(3));
        assert!(
            cache.lookup(&ctx, std::path::Path::new("/a")).is_some(),
            "命中后 bump 队首，不应被淘汰"
        );
        assert!(cache.lookup(&ctx, std::path::Path::new("/b")).is_none());
    }

    #[test]
    fn local_vs_ssh_keys_do_not_collide() {
        // Spec `ipc-data-api/spec.md` §`Local 与 SSH 同字面 path 不串扰` —— 同
        // path 但不同 `ContextId` 必须各占独立 cache slot，互不串扰。
        let mut cache = MetadataCache::new(10);
        let local_ctx = ContextId::local(PathBuf::from("/home/u/.claude/projects"));
        let ssh_ctx = ContextId::ssh(
            cdt_fs::HostSignature::from_ssh_config_fields(&cdt_fs::SshConfigDigestInput {
                hostname: "remote".into(),
                port: 22,
                user: "u".into(),
                identity_files: vec![],
                proxyjump: None,
                proxycommand: None,
                hostkeyalias: None,
            }),
            PathBuf::from("/home/u/.claude/projects"),
        );
        // 字面同 path，但 ContextId 不等
        let same_path = PathBuf::from("/foo/s.jsonl");
        cache.insert((local_ctx.clone(), same_path.clone()), dummy_entry(1));
        cache.insert((ssh_ctx.clone(), same_path.clone()), dummy_entry(2));
        assert_eq!(cache.len(), 2, "Local 与 SSH 各占一个 slot");
        let local_hit = cache.lookup(&local_ctx, &same_path).unwrap();
        let ssh_hit = cache.lookup(&ssh_ctx, &same_path).unwrap();
        assert_eq!(local_hit.message_count, 1);
        assert_eq!(ssh_hit.message_count, 2, "SSH lookup 必须命中 SSH entry");
    }

    #[test]
    fn lru_capacity_2000_evicts_lru_with_mixed_context() {
        // Spec `ipc-data-api/spec.md` §`缓存超过容量按 LRU 淘汰` —— 容量上限是
        // 跨 ContextId 全局总和。混插 Local + SSH 共 2001 条 → 最早一条被淘汰。
        let mut cache = MetadataCache::new(2000);
        let local_ctx = ContextId::local(PathBuf::from("/local"));
        let ssh_ctx = test_ssh_ctx();
        for i in 0..1000 {
            cache.insert(
                (local_ctx.clone(), PathBuf::from(format!("/p{i}"))),
                dummy_entry(u64::try_from(i).unwrap()),
            );
        }
        for i in 0..1000 {
            cache.insert(
                (ssh_ctx.clone(), PathBuf::from(format!("/p{i}"))),
                dummy_entry(u64::try_from(i + 5000).unwrap()),
            );
        }
        assert_eq!(cache.len(), 2000);
        // 再插一条让 Local 的第一条（最久未访问）被淘汰
        cache.insert(
            (local_ctx.clone(), PathBuf::from("/p9999")),
            dummy_entry(9999),
        );
        assert_eq!(cache.len(), 2000, "容量上限");
        assert!(
            cache
                .lookup(&local_ctx, std::path::Path::new("/p0"))
                .is_none(),
            "最久未访问的 Local /p0 应被淘汰"
        );
        assert!(
            cache
                .lookup(&ssh_ctx, std::path::Path::new("/p999"))
                .is_some()
        );
    }

    #[test]
    fn switch_context_does_not_clear_cache() {
        // Spec `ipc-data-api/spec.md` §`ssh_disconnect 不清 cache` 的纯 cache 层验证：
        // 写入 Local entry 后，模拟"切到 SSH"（直接构造 SSH ctx 查），SSH lookup
        // miss；切回 Local lookup 仍命中 —— ContextId 隔离 + cache 永久未被清。
        let mut cache = MetadataCache::new(10);
        let local_ctx = ContextId::local(PathBuf::from("/local"));
        let ssh_ctx = test_ssh_ctx();
        cache.insert((local_ctx.clone(), PathBuf::from("/foo")), dummy_entry(1));
        // 切到 SSH 视角：同字面 path 必 miss
        assert!(
            cache
                .lookup(&ssh_ctx, std::path::Path::new("/foo"))
                .is_none()
        );
        // 切回 Local 视角：仍命中
        assert!(
            cache
                .lookup(&local_ctx, std::path::Path::new("/foo"))
                .is_some()
        );
    }

    #[test]
    fn different_ssh_hosts_do_not_collide() {
        // Spec `ipc-data-api/spec.md` §`不同 SSH host 之间不串扰`。
        let mut cache = MetadataCache::new(10);
        let ssh_a = test_ssh_ctx();
        let ssh_b = test_ssh_ctx_b();
        assert_ne!(ssh_a, ssh_b, "fixture 必须产不同 ContextId");
        cache.insert((ssh_a.clone(), PathBuf::from("/foo")), dummy_entry(1));
        cache.insert((ssh_b.clone(), PathBuf::from("/foo")), dummy_entry(2));
        assert_eq!(
            cache
                .lookup(&ssh_a, std::path::Path::new("/foo"))
                .unwrap()
                .message_count,
            1
        );
        assert_eq!(
            cache
                .lookup(&ssh_b, std::path::Path::new("/foo"))
                .unwrap()
                .message_count,
            2
        );
    }

    // -------- stale 实时合成 --------
    //
    // Scenario `缓存命中后实时重算 stale 状态`：缓存的 messages_ongoing=true
    // 但 wall-clock 距 mtime 推进 > 5min 时，is_ongoing 应为 false 而 cache
    // 不被 invalidate。
    //
    // 直接构造 MetadataCacheEntry + lookup 验证：mtime 设置为远古 → 任何
    // 当前 wall-clock 都 > 5 分钟 → is_ongoing 合成为 false。

    #[tokio::test]
    async fn cached_hit_synthesizes_is_ongoing_with_fresh_stale_check() {
        let tmp = tempfile::tempdir().unwrap();
        // 这个 fixture 让 messages_ongoing = true（通过用户消息后没有配对 assistant
        // 的方式让 cdt_analyze::check_messages_ongoing 返回 true）
        // 简单起见：只一条 user 消息，无 assistant 回应
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line("u1", "2026-05-03T10:00:00.000Z", "hi")],
        );

        let cache = make_cache();
        let m1 = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        // 第一次：刚写入，mtime 接近 now，is_ongoing 取决于 messages_ongoing 与 stale
        // 不强断言 m1.is_ongoing，重点是缓存的 messages_ongoing 中间值
        let _ = m1;

        // 把缓存条目的 mtime 改成远古，模拟"缓存命中但 wall-clock 推进 > 5 分钟"
        {
            let mut guard = cache.lock().unwrap();
            let key = (test_local_ctx(tmp.path()), path.clone());
            if let Some(entry) = guard.map.get_mut(&key) {
                entry.signature.mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
                entry.messages_ongoing = true;
            }
        }
        // 修改文件 mtime 让 stat 与缓存中的（被改成远古）一致
        let _ = filetime_set_old(&path);

        let m2 = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        // 命中后实时合成：mtime 远古 → stale → is_ongoing = false
        assert!(
            !m2.is_ongoing,
            "缓存命中但 mtime 远古时 is_ongoing 应实时合成为 false"
        );
    }

    /// 把文件 mtime 改成 `UNIX_EPOCH + 1_000_000s` 以匹配上面测试构造的"远古" cache 条目。
    /// 用 `File::set_modified`（Rust 1.75+ stable）跨平台可用。
    fn filetime_set_old(path: &Path) -> std::io::Result<()> {
        let f = std::fs::OpenOptions::new().write(true).open(path)?;
        f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000))?;
        Ok(())
    }

    // ========================================================================
    // change `session-title-extraction-fix` 新增 title 规则单测
    // spec: openspec/specs/ipc-data-api/spec.md
    //   §`Title prefers slash command with non-empty args ...`
    //   §`Sanitize title against interruption and task-output instructions`
    //   §`Title length is bounded by TITLE_MAX_CHARS constant`
    //   §`Title algorithm changes do not invalidate MetadataCache`
    // ========================================================================

    fn slash_user_line(uuid: &str, ts: &str, name: &str, args: &str) -> String {
        // 用 JSON Blocks 形式以避免双引号转义复杂；保持与原版 JSONL 兼容。
        let content =
            format!("<command-name>{name}</command-name><command-args>{args}</command-args>");
        user_text_line(uuid, ts, &content)
    }

    #[tokio::test]
    async fn slash_with_non_empty_args_used_as_title() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &slash_user_line(
                    "u1",
                    "2026-05-03T10:00:00.000Z",
                    "/impeccable",
                    "生成设计规范",
                ),
                &user_text_line("u2", "2026-05-03T10:00:01.000Z", "提一下PR"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        assert_eq!(meta.title.as_deref(), Some("/impeccable 生成设计规范"));
    }

    #[tokio::test]
    async fn slash_with_empty_args_falls_back_to_next_message() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &slash_user_line("u1", "2026-05-03T10:00:00.000Z", "/clear", ""),
                &user_text_line("u2", "2026-05-03T10:00:01.000Z", "今天的工作"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        assert_eq!(meta.title.as_deref(), Some("今天的工作"));
    }

    #[tokio::test]
    async fn slash_without_args_tag_uses_fallback_when_no_other_message() {
        let tmp = tempfile::tempdir().unwrap();
        // 无 <command-args> tag
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line(
                "u1",
                "2026-05-03T10:00:00.000Z",
                "<command-name>/help</command-name>",
            )],
        );
        let meta = extract_session_metadata(&path).await;
        assert_eq!(meta.title.as_deref(), Some("/help"));
    }

    #[tokio::test]
    async fn interrupted_message_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_text_line(
                    "u1",
                    "2026-05-03T10:00:00.000Z",
                    "[Request interrupted by user during tooling cycle]",
                ),
                &user_text_line("u2", "2026-05-03T10:00:01.000Z", "继续"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        assert_eq!(meta.title.as_deref(), Some("继续"));
    }

    #[tokio::test]
    async fn read_output_file_instruction_stripped() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line(
                "u1",
                "2026-05-03T10:00:00.000Z",
                "<task-notification>已完成</task-notification> Read the output file to retrieve the result: /tmp/result.txt",
            )],
        );
        let meta = extract_session_metadata(&path).await;
        let title = meta.title.unwrap_or_default();
        assert!(
            !title.contains("Read the output file"),
            "title 不应含 Read the output file: {title:?}"
        );
        assert!(
            !title.contains("/tmp/result.txt"),
            "title 不应含路径: {title:?}"
        );
    }

    #[tokio::test]
    async fn read_output_file_literal_in_user_text_not_stripped_without_task_notification() {
        // 用户在普通消息中手写 "Read the output file..." 字面量（无 <task-notification>
        // 标签上下文）SHALL NOT 被 sanitize 误吞。codex 二审反馈。
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line(
                "u1",
                "2026-05-03T10:00:00.000Z",
                "参考一下教程：Read the output file to retrieve the result: /tutorial.txt",
            )],
        );
        let meta = extract_session_metadata(&path).await;
        let title = meta.title.unwrap_or_default();
        assert!(
            title.contains("Read the output file"),
            "用户手写字面量 SHALL 保留: {title:?}"
        );
        assert!(
            title.contains("/tutorial.txt"),
            "用户手写路径 SHALL 保留: {title:?}"
        );
    }

    #[tokio::test]
    async fn read_output_file_multi_match_all_stripped() {
        let tmp = tempfile::tempdir().unwrap();
        let content = "<task-notification>x</task-notification> Read the output file to retrieve the result: /a 中间文本 Read the output file to retrieve the result: /b";
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line("u1", "2026-05-03T10:00:00.000Z", content)],
        );
        let meta = extract_session_metadata(&path).await;
        let title = meta.title.unwrap_or_default();
        assert!(
            !title.contains("Read the output file"),
            "多匹配均应移除: {title:?}"
        );
        assert!(!title.contains("/a"), "路径 /a 应被移除: {title:?}");
        assert!(!title.contains("/b"), "路径 /b 应被移除: {title:?}");
    }

    #[tokio::test]
    async fn slash_with_long_args_truncated_at_max_chars() {
        let long_args: String = "测".repeat(700);
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&slash_user_line(
                "u1",
                "2026-05-03T10:00:00.000Z",
                "/foo",
                &long_args,
            )],
        );
        let meta = extract_session_metadata(&path).await;
        let title = meta.title.unwrap_or_default();
        assert!(
            title.chars().count() <= TITLE_MAX_CHARS,
            "title 字符数 {} 应 <= {}",
            title.chars().count(),
            TITLE_MAX_CHARS
        );
    }

    #[tokio::test]
    async fn plain_text_long_title_truncated_at_max_chars() {
        let long_text: String = "字".repeat(700);
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line(
                "u1",
                "2026-05-03T10:00:00.000Z",
                &long_text,
            )],
        );
        let meta = extract_session_metadata(&path).await;
        let title = meta.title.unwrap_or_default();
        assert!(
            title.chars().count() <= TITLE_MAX_CHARS,
            "title 字符数 {} 应 <= {}",
            title.chars().count(),
            TITLE_MAX_CHARS
        );
    }

    // ---- 边界 / early-exit ----

    #[tokio::test]
    async fn slash_with_self_closing_command_args_treated_as_no_args() {
        // 自闭合 `<command-args/>` —— `extract_tag_content` 只识别 `<tag>...</tag>`，
        // 走"无 args"路径 → 进 fallback；有第二条 user → title = 第二条
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_text_line(
                    "u1",
                    "2026-05-03T10:00:00.000Z",
                    "<command-name>/foo</command-name><command-args/>",
                ),
                &user_text_line("u2", "2026-05-03T10:00:01.000Z", "真正的输入"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        assert_eq!(meta.title.as_deref(), Some("真正的输入"));
    }

    #[tokio::test]
    async fn sanitized_to_only_whitespace_falls_back() {
        // 第一条 user 仅含 system-reminder 标签 → sanitize 后空白 → 跳过 title
        // 第二条 user 是真实输入 → 作 title
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_text_line(
                    "u1",
                    "2026-05-03T10:00:00.000Z",
                    "<system-reminder>internal</system-reminder>",
                ),
                &user_text_line("u2", "2026-05-03T10:00:01.000Z", "真实主题"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        assert_eq!(meta.title.as_deref(), Some("真实主题"));
    }

    #[tokio::test]
    async fn title_once_set_does_not_get_overridden() {
        // 第一条 user 已是有效 title T1；第二条 user T2、第三条带 args slash；
        // title 应保持 T1（验证 `title.is_none()` early-exit gate）
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[
                &user_text_line("u1", "2026-05-03T10:00:00.000Z", "T1"),
                &user_text_line("u2", "2026-05-03T10:00:01.000Z", "T2"),
                &slash_user_line("u3", "2026-05-03T10:00:02.000Z", "/foo", "after-title-args"),
            ],
        );
        let meta = extract_session_metadata(&path).await;
        assert_eq!(meta.title.as_deref(), Some("T1"));
    }

    // ---- 缓存兼容性：算法变更不主动 invalidate ----
    // spec: §`Title algorithm changes do not invalidate MetadataCache`

    #[tokio::test]
    async fn cache_hit_returns_legacy_title_without_recomputing() {
        // 手动写入一个 cache entry，title 字段是"旧规则算出的"字面量；签名匹配
        // 时 SHALL 直接返回该 title，不会被新算法覆盖。
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&slash_user_line(
                "u1",
                "2026-05-03T10:00:00.000Z",
                "/impeccable",
                "新规则会用这串",
            )],
        );
        let fs_meta = cdt_fs::local_handle().stat(&path).await.unwrap();
        let sig = FileSignature::from_fs_metadata(&fs_meta);

        let cache = make_cache();
        let ctx = test_local_ctx(tmp.path());
        // 模拟旧版本缓存：title 写一个完全不同的字面量
        cache.lock().unwrap().insert(
            (ctx.clone(), path.clone()),
            MetadataCacheEntry {
                signature: sig,
                title: Some("旧规则算出的 title".to_string()),
                message_count: 7,
                messages_ongoing: false,
                git_branch: None,
            },
        );

        let m = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        assert_eq!(m.title.as_deref(), Some("旧规则算出的 title"));
        assert_eq!(
            m.message_count, 7,
            "命中 cache 不重扫，message_count 来自 cache"
        );
    }

    #[tokio::test]
    async fn cache_miss_after_signature_change_uses_new_algorithm() {
        // 先写入 cache 一个旧 title；append 文件让 signature 变化触发重扫；
        // 重扫应用新算法（带 args slash 直接作 title）。
        let tmp = tempfile::tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            &[&user_text_line("u1", "2026-05-03T10:00:00.000Z", "old")],
        );

        let cache = make_cache();
        // 第一次扫填入 cache
        let m1 = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        assert_eq!(m1.title.as_deref(), Some("old"));

        // 把 cache 里的 title 篡改为模拟"按旧算法算出来的不同字面量"，签名仍匹配
        {
            let mut guard = cache.lock().unwrap();
            let key = (test_local_ctx(tmp.path()), path.clone());
            if let Some(entry) = guard.map.get_mut(&key) {
                entry.title = Some("legacy title from old algo".to_string());
            }
        }
        // 命中：返回篡改后的旧 title
        let m_legacy = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        assert_eq!(
            m_legacy.title.as_deref(),
            Some("legacy title from old algo")
        );

        // 现在 append 行让 signature 变化，触发重扫；新内容含带 args slash
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        std::fs::write(
            &path,
            format!(
                "{}\n{}\n",
                user_text_line("u1", "2026-05-03T10:00:00.000Z", "old"),
                slash_user_line("u2", "2026-05-03T10:00:02.000Z", "/impeccable", "新规则",),
            ),
        )
        .unwrap();

        let m_new = extract_session_metadata_cached(
            &cache,
            &*cdt_fs::local_handle(),
            &test_local_ctx(tmp.path()),
            &path,
        )
        .await;
        // 新扫描时第一条仍是 "old"（按 early-exit gate 它先被赋 title）。
        // 关键 assertion：cache 已用 *新算法* 重算并写回，不再是 legacy 字面量
        assert_ne!(
            m_new.title.as_deref(),
            Some("legacy title from old algo"),
            "signature 变化后 SHALL 重扫，不再返回篡改后的旧 cache"
        );
        assert_eq!(m_new.title.as_deref(), Some("old"));
    }

    // ========================================================================
    // fake-SSH metadata cache perf bench —— counter-based assertion
    //
    // Spec `ipc-data-api/spec.md` §`相同 (ContextId, path) FileSignature 不变命中缓存`
    // 的运行时含义：cache hit 后 SHALL NOT 调 `open_read` 或 `read_to_string`，
    // 仅需 `stat` 校验 signature。本测试用 fake-SSH provider 模拟 50ms RTT，
    // counter-based assertion 验证 hit 路径 fs op 形态。
    //
    // 标 `#[ignore]`——CI 不跑；本地 `cargo test -p cdt-api --lib
    // ipc::session_metadata::tests::ssh_cache_hit_skips -- --ignored --nocapture` 验收。
    // 详 change `metadata-cache-context-prefix` design EXTRA-4。
    // ========================================================================

    use cdt_fs::{
        DirEntry as CdtDirEntry, FsError, FsKind, FsMetadata, InstrumentedFs, with_fs_counter,
    };
    use std::collections::HashMap as PerfHashMap;
    use std::path::Path as StdPath;
    use std::sync::Arc as StdArc;
    use std::sync::Mutex as StdSyncMutex;
    use tokio::io::AsyncRead;

    struct FakeSshFs {
        latency: Duration,
        files: StdSyncMutex<PerfHashMap<PathBuf, (u64, SystemTime, String)>>,
    }

    impl FakeSshFs {
        fn new(latency: Duration) -> Self {
            Self {
                latency,
                files: StdSyncMutex::new(PerfHashMap::new()),
            }
        }
        fn insert(&self, path: PathBuf, size: u64, mtime: SystemTime, content: String) {
            self.files
                .lock()
                .expect("FakeSshFs mutex")
                .insert(path, (size, mtime, content));
        }
    }

    #[async_trait::async_trait]
    impl FileSystemProvider for FakeSshFs {
        fn kind(&self) -> FsKind {
            FsKind::Ssh
        }
        async fn exists(&self, path: &StdPath) -> bool {
            tokio::time::sleep(self.latency).await;
            self.files
                .lock()
                .expect("FakeSshFs mutex")
                .contains_key(path)
        }
        async fn read_dir(&self, _path: &StdPath) -> Result<Vec<CdtDirEntry>, FsError> {
            tokio::time::sleep(self.latency).await;
            Ok(vec![])
        }
        async fn read_to_string(&self, path: &StdPath) -> Result<String, FsError> {
            tokio::time::sleep(self.latency).await;
            self.files
                .lock()
                .expect("FakeSshFs mutex")
                .get(path)
                .map(|(_, _, c)| c.clone())
                .ok_or_else(|| FsError::NotFound(path.to_path_buf()))
        }
        async fn stat(&self, path: &StdPath) -> Result<FsMetadata, FsError> {
            tokio::time::sleep(self.latency).await;
            self.files
                .lock()
                .expect("FakeSshFs mutex")
                .get(path)
                .map(|(size, mtime, _)| FsMetadata {
                    size: *size,
                    mtime: *mtime,
                    identity: None,
                })
                .ok_or_else(|| FsError::NotFound(path.to_path_buf()))
        }
        async fn read_lines_head(
            &self,
            path: &StdPath,
            max: usize,
        ) -> Result<Vec<String>, FsError> {
            tokio::time::sleep(self.latency).await;
            let content = self
                .files
                .lock()
                .expect("FakeSshFs mutex")
                .get(path)
                .map(|(_, _, c)| c.clone())
                .ok_or_else(|| FsError::NotFound(path.to_path_buf()))?;
            Ok(content.lines().take(max).map(str::to_owned).collect())
        }
        async fn open_read(
            &self,
            path: &StdPath,
        ) -> Result<Box<dyn AsyncRead + Send + Unpin>, FsError> {
            tokio::time::sleep(self.latency).await;
            let content = self
                .files
                .lock()
                .expect("FakeSshFs mutex")
                .get(path)
                .map(|(_, _, c)| c.clone())
                .ok_or_else(|| FsError::NotFound(path.to_path_buf()))?;
            Ok(Box::new(std::io::Cursor::new(content.into_bytes())))
        }
        async fn write_atomic(&self, _path: &StdPath, _content: &[u8]) -> Result<(), FsError> {
            unimplemented!("FakeSshFs perf bench fixture 当前不走写路径")
        }
        async fn create_dir_all(&self, _path: &StdPath) -> Result<(), FsError> {
            unimplemented!("FakeSshFs perf bench fixture 当前不走写路径")
        }
        async fn remove_file(&self, _path: &StdPath) -> Result<(), FsError> {
            unimplemented!("FakeSshFs perf bench fixture 当前不走写路径")
        }
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "perf bench, dev-only; run via `cargo test --lib ssh_cache_hit_skips -- --ignored --nocapture`"]
    async fn ssh_cache_hit_skips_open_read_and_read_to_string() {
        // 不模拟真 RTT（tokio test-util 未启用，virtual time 不可用）；assertion
        // 完全基于 counter——latency=0 让本机跑时 sub-second 完成，CI 不跑（#[ignore]）。
        const N: usize = 500;
        let latency = Duration::from_millis(0);
        let fake = FakeSshFs::new(latency);
        let base = PathBuf::from("/fake/ssh/home/.claude/projects/-fake-project");
        let mtime_base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);

        let paths: Vec<PathBuf> = (0..N)
            .map(|i| base.join(format!("session-{i}.jsonl")))
            .collect();
        for (i, p) in paths.iter().enumerate() {
            fake.insert(
                p.clone(),
                64 + u64::try_from(i).unwrap(),
                mtime_base + Duration::from_secs(u64::try_from(i).unwrap()),
                String::new(),
            );
        }

        let instrumented = StdArc::new(InstrumentedFs::new(fake));
        let ctx = test_ssh_ctx();
        let cache = StdArc::new(StdMutex::new(MetadataCache::default()));

        // 第一轮 500 次：全部 miss（cache 为空）→ stat + 内部扫描（scanner 走
        // `tokio::fs::File::open(path)` 在 fake path 上失败，返空 metadata，
        // 但 cache 仍按 signature 写入 entry，让第二轮命中）。
        let cache_for_miss = cache.clone();
        let fs_for_miss = instrumented.clone();
        let ctx_for_miss = ctx.clone();
        let paths_for_miss = paths.clone();
        let ((), miss_counts) = with_fs_counter(move || async move {
            for p in &paths_for_miss {
                let _ = extract_session_metadata_cached(
                    &cache_for_miss,
                    &*fs_for_miss,
                    &ctx_for_miss,
                    p,
                )
                .await;
            }
        })
        .await;

        // 第二轮 500 次：全部 hit → 仅 fs.stat 校验 signature，绝不应再调
        // open_read / read_to_string。
        let cache_for_hit = cache.clone();
        let fs_for_hit = instrumented.clone();
        let ctx_for_hit = ctx.clone();
        let paths_for_hit = paths.clone();
        let ((), hit_counts) = with_fs_counter(move || async move {
            for p in &paths_for_hit {
                let _ =
                    extract_session_metadata_cached(&cache_for_hit, &*fs_for_hit, &ctx_for_hit, p)
                        .await;
            }
        })
        .await;

        eprintln!("[perf_metadata_cache_ssh_hit] miss={miss_counts:?} hit={hit_counts:?}");

        assert_eq!(
            hit_counts.open_read, 0,
            "cache hit SHALL NOT 调 fs.open_read（实际：{}）",
            hit_counts.open_read
        );
        assert_eq!(
            hit_counts.read_to_string, 0,
            "cache hit SHALL NOT 调 fs.read_to_string（实际：{}）",
            hit_counts.read_to_string
        );
        assert_eq!(
            hit_counts.stat,
            u32::try_from(N).unwrap(),
            "cache hit 每个 path SHALL 仍调一次 fs.stat 校验 signature"
        );
    }
}
