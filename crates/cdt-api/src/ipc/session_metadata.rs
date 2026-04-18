//! 轻量 session 元数据提取：标题 + 消息计数。
//!
//! 与原版 `metadataExtraction.ts` 的 `analyzeSessionFileMetadata` 对齐：
//! - 标题：第一条非 `is_meta`、非命令输出的 user 消息（清洗后截取前 200 字符）
//! - 消息计数：user + 对应 assistant 轮次配对计数

use std::path::Path;

use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

use cdt_core::message::{MessageCategory, MessageContent};
use cdt_parse::parse_entry_at;

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

    let is_ongoing = cdt_analyze::check_messages_ongoing(&all_messages);

    SessionMetadata {
        title,
        message_count,
        is_ongoing,
    }
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
    s.trim().to_string()
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}
