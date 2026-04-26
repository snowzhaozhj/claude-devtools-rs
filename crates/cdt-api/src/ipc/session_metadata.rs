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

use cdt_core::message::{MessageCategory, MessageContent};
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
}

/// 扫描标题时读取的最大行数（与原版 `maxLines: 200` 对齐）。
const TITLE_MAX_LINES: usize = 200;

/// 扫描 JSONL 文件，提取标题和消息计数。
///
/// 标题只扫描前 `TITLE_MAX_LINES` 行；消息计数扫描全文件。
pub async fn extract_session_metadata(path: &Path) -> SessionMetadata {
    let Ok(file) = File::open(path).await else {
        return SessionMetadata {
            title: None,
            message_count: 0,
            is_ongoing: false,
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

    while let Ok(Some(line)) = lines.next_line().await {
        line_number += 1;
        if line.trim().is_empty() {
            continue;
        }

        let Ok(Some(msg)) = parse_entry_at(&line, line_number) else {
            continue;
        };

        // --- 消息计数（与原版配对逻辑对齐）---
        if msg.category == MessageCategory::User && !msg.is_meta {
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
}
