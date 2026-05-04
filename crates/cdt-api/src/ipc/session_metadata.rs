//! 轻量 session 元数据提取：标题 + 消息计数。
//!
//! 与原版 `metadataExtraction.ts` 的 `analyzeSessionFileMetadata` 对齐：
//! - 标题：第一条非 `is_meta`、非命令输出的 user 消息（清洗后截取前 200 字符）
//! - 消息计数：user + 对应 assistant 轮次配对计数
//! - `isOngoing`：`check_messages_ongoing` 结果再叠加 stale check（文件
//!   mtime 距 now > 5 分钟视为 crashed/killed），对齐
//!   `claude-devtools/src/main/services/discovery/ProjectScanner.ts`
//!   `STALE_SESSION_THRESHOLD_MS = 5 * 60 * 1000`（issue #94）

use std::path::Path;
use std::time::{Duration, SystemTime};

use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

use cdt_core::message::{ContentBlock, MessageCategory, MessageContent, ParsedMessage};
use cdt_parse::parse_entry_at;

/// 文件 mtime 距 now 超过此阈值则即便消息序列结构上为 ongoing 也强制判 done。
/// 5 分钟，对齐原版 `STALE_SESSION_THRESHOLD_MS`。
pub const STALE_SESSION_THRESHOLD: Duration = Duration::from_secs(5 * 60);

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

/// 扫描 JSONL 文件，提取标题和消息计数。
///
/// 标题只扫描前 `TITLE_MAX_LINES` 行；消息计数扫描全文件。
pub async fn extract_session_metadata(path: &Path) -> SessionMetadata {
    let Ok(file) = File::open(path).await else {
        return SessionMetadata {
            title: None,
            message_count: 0,
            is_ongoing: false,
            git_branch: None,
        };
    };

    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut title: Option<String> = None;
    let mut command_fallback: Option<String> = None;
    let mut message_count: usize = 0;
    let mut awaiting_ai = false;
    let mut line_number: usize = 0;
    let mut all_messages: Vec<cdt_core::ParsedMessage> = Vec::new();
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
            if !branch.is_empty() {
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
        if line_number <= TITLE_MAX_LINES
            && title.is_none()
            && msg.category == MessageCategory::User
            && !msg.is_meta
        {
            let text = extract_text(&msg.content);
            if !text.is_empty() {
                if is_command_output(&text) {
                    // 跳过命令输出
                } else if is_command_content(&text) {
                    if command_fallback.is_none() {
                        command_fallback = extract_command_display(&text);
                    }
                } else if let Some(summary) = extract_teammate_summary_title(&text) {
                    // teammate-message 包裹的消息：优先取 `summary` 属性作为标题
                    title = Some(truncate_str(&summary, 200));
                } else {
                    let sanitized = sanitize_for_title(&text);
                    if !sanitized.is_empty() {
                        title = Some(truncate_str(&sanitized, 200));
                    }
                }
            }
        }

        all_messages.push(msg);
    }

    // 没有真实用户消息时用 slash 命令后备
    if title.is_none() {
        title = command_fallback;
    }

    let messages_ongoing = cdt_analyze::check_messages_ongoing(&all_messages);
    let is_ongoing = if messages_ongoing {
        !is_file_stale(path).await
    } else {
        false
    };

    SessionMetadata {
        title,
        message_count,
        is_ongoing,
        git_branch: last_git_branch,
    }
}

/// 异步读 file mtime 并判定是否超过 stale 阈值。
/// stat 失败时回退到 `false`（不强制 stale，保守保留 `messages_ongoing` 的判定）。
pub async fn is_file_stale(path: &Path) -> bool {
    let Ok(meta) = tokio::fs::metadata(path).await else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    is_session_stale(modified, SystemTime::now())
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

/// 提取 slash 命令为 "/name args" 格式。
fn extract_command_display(content: &str) -> Option<String> {
    let name = extract_tag_content(content, "command-name")?;
    let name = name.strip_prefix('/').unwrap_or(&name);
    let display = format!("/{name}");
    if let Some(args) = extract_tag_content(content, "command-args") {
        if !args.is_empty() {
            return Some(format!("{display} {args}"));
        }
    }
    Some(display)
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
fn sanitize_for_title(text: &str) -> String {
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
    s.trim().to_string()
}

/// 若 `text` trim 后以 `<teammate-message` 起首，提取 `summary="..."` 属性
/// 内容作为标题候选；非 teammate 主导消息或无 summary 属性返回 `None`。
///
/// Spec：`openspec/specs/ipc-data-api/spec.md`
/// §`Strip teammate-message tags from session title`。
fn extract_teammate_summary_title(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("<teammate-message") {
        return None;
    }
    // 取开 tag 范围内的属性串
    let tag_end = trimmed.find('>')?;
    let attrs = &trimmed[..tag_end];
    // summary="..."
    let idx = attrs.find("summary=\"")?;
    let after = &attrs[idx + "summary=\"".len()..];
    let close = after.find('"')?;
    let summary = after[..close].trim();
    if summary.is_empty() {
        None
    } else {
        Some(summary.to_string())
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
    fn teammate_no_summary_returns_none() {
        let text = r#"<teammate-message teammate_id="alice" color="blue">body</teammate-message>"#;
        let summary = extract_teammate_summary_title(text);
        assert!(summary.is_none());
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
}
