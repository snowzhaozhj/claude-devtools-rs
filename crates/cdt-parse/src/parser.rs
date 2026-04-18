//! JSONL → `ParsedMessage` 转换层。
//!
//! Spec：`openspec/specs/session-parsing/spec.md`。

use cdt_core::{
    ContentBlock, HardNoiseReason, MessageCategory, MessageContent, MessageType, ParsedMessage,
    TokenUsage, ToolCall, ToolResult,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::error::ParseError;
use crate::noise;

#[derive(Debug, Deserialize)]
struct RawEntry {
    #[serde(default)]
    uuid: Option<String>,
    #[serde(rename = "type", default)]
    entry_type: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(rename = "parentUuid", default)]
    parent_uuid: Option<String>,
    #[serde(rename = "isSidechain", default)]
    is_sidechain: Option<bool>,
    #[serde(rename = "isMeta", default)]
    is_meta: Option<bool>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(rename = "gitBranch", default)]
    git_branch: Option<String>,
    #[serde(rename = "userType", default)]
    user_type: Option<String>,
    #[serde(rename = "agentId", default)]
    agent_id: Option<String>,
    #[serde(rename = "requestId", default)]
    request_id: Option<String>,
    #[serde(rename = "toolUseResult", default)]
    tool_use_result: Option<serde_json::Value>,
    #[serde(rename = "isCompactSummary", default)]
    is_compact_summary: Option<bool>,
    #[serde(rename = "sourceToolUseID", default)]
    source_tool_use_id: Option<String>,
    #[serde(rename = "sourceToolAssistantUUID", default)]
    source_tool_assistant_uuid: Option<String>,
    #[serde(default)]
    message: Option<RawMessage>,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<MessageContent>,
    #[serde(default)]
    usage: Option<TokenUsage>,
    #[serde(default)]
    model: Option<String>,
}

/// 把一行 JSONL 解析成 `ParsedMessage`。
///
/// - 当 JSON 结构合法但缺 `uuid`、或 `type` 不在已知集合内时，返回
///   `Ok(None)`（TS 版 `SessionParser` 也是静默跳过）。
/// - JSON 语法错误返回 `Err(ParseError::MalformedLine)`。
/// - 字段形状不符时返回 `Err(ParseError::SchemaMismatch)`。
///
/// `line_number` 仅用于错误信息。没有文件上下文的调用方传 `0` 即可。
pub fn parse_entry_at(line: &str, line_number: usize) -> Result<Option<ParsedMessage>, ParseError> {
    if line.trim().is_empty() {
        return Ok(None);
    }

    let raw: RawEntry = serde_json::from_str(line).map_err(|e| ParseError::MalformedLine {
        line: line_number,
        source: e,
    })?;

    let Some(uuid) = raw.uuid.clone() else {
        return Ok(None);
    };

    let Some(message_type) = parse_message_type(raw.entry_type.as_deref()) else {
        return Ok(None);
    };

    let timestamp = parse_timestamp(raw.timestamp.as_deref());

    let (role, content, usage, model) = match raw.message {
        Some(m) => (m.role, m.content.unwrap_or_default(), m.usage, m.model),
        None => (None, MessageContent::default(), None, None),
    };

    let tool_calls = extract_tool_calls(&content);
    let tool_results = extract_tool_results(&content);

    let noise_reason = noise::classify_hard_noise(message_type, model.as_deref(), &content);
    let is_interrupt = noise_reason.is_none() && noise::is_interrupt_marker(message_type, &content);
    let is_compact_summary = raw.is_compact_summary.unwrap_or(false);
    let category = classify_category(message_type, noise_reason, is_interrupt, is_compact_summary);

    Ok(Some(ParsedMessage {
        uuid,
        parent_uuid: raw.parent_uuid,
        message_type,
        category,
        timestamp,
        role,
        content,
        usage,
        model,
        cwd: raw.cwd,
        git_branch: raw.git_branch,
        agent_id: raw.agent_id,
        is_sidechain: raw.is_sidechain.unwrap_or(false),
        is_meta: raw.is_meta.unwrap_or(false),
        user_type: raw.user_type,
        tool_calls,
        tool_results,
        source_tool_use_id: raw.source_tool_use_id,
        source_tool_assistant_uuid: raw.source_tool_assistant_uuid,
        is_compact_summary,
        request_id: raw.request_id,
        tool_use_result: raw.tool_use_result,
    }))
}

/// 给没有文件上下文的调用方准备的便捷包装，等价于 `parse_entry_at(line, 0)`。
pub fn parse_entry(line: &str) -> Result<Option<ParsedMessage>, ParseError> {
    parse_entry_at(line, 0)
}

fn parse_message_type(raw: Option<&str>) -> Option<MessageType> {
    match raw? {
        "user" => Some(MessageType::User),
        "assistant" => Some(MessageType::Assistant),
        "system" => Some(MessageType::System),
        "summary" => Some(MessageType::Summary),
        "file-history-snapshot" => Some(MessageType::FileHistorySnapshot),
        "queue-operation" => Some(MessageType::QueueOperation),
        _ => None,
    }
}

fn parse_timestamp(raw: Option<&str>) -> DateTime<Utc> {
    raw.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map_or_else(Utc::now, |dt| dt.with_timezone(&Utc))
}

fn classify_category(
    message_type: MessageType,
    noise: Option<HardNoiseReason>,
    is_interrupt: bool,
    is_compact: bool,
) -> MessageCategory {
    if let Some(reason) = noise {
        return MessageCategory::HardNoise(reason);
    }
    if is_interrupt {
        return MessageCategory::Interruption;
    }
    if is_compact {
        return MessageCategory::Compact;
    }
    match message_type {
        MessageType::User => MessageCategory::User,
        MessageType::Assistant => MessageCategory::Assistant,
        // 非会话型条目（system/summary/file-history-snapshot/queue-operation）
        // 在上面已经被分类成 hard noise，理论上走不到下面这几条分支；
        // 保留是为了避免未来分类逻辑调整时出现 `unreachable!()` panic。
        MessageType::System => MessageCategory::System,
        MessageType::Summary | MessageType::FileHistorySnapshot | MessageType::QueueOperation => {
            MessageCategory::HardNoise(HardNoiseReason::NonConversationalEntry)
        }
    }
}

fn extract_tool_calls(content: &MessageContent) -> Vec<ToolCall> {
    let MessageContent::Blocks(blocks) = content else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for block in blocks {
        if let ContentBlock::ToolUse { id, name, input } = block {
            let is_task = name == "Task" || name == "Agent";
            let (task_description, task_subagent_type) = if is_task {
                (
                    input
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned),
                    input
                        .get("subagent_type")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned),
                )
            } else {
                (None, None)
            };
            out.push(ToolCall {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
                is_task,
                task_description,
                task_subagent_type,
            });
        }
    }
    out
}

fn extract_tool_results(content: &MessageContent) -> Vec<ToolResult> {
    let MessageContent::Blocks(blocks) = content else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for block in blocks {
        if let ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } = block
        {
            out.push(ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: content.clone(),
                is_error: *is_error,
            });
        }
    }
    out
}
