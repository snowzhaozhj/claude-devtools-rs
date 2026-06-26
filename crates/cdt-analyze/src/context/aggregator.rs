//! 6 类 context injection 的纯函数聚合器。
//!
//! 每个 `aggregate_*` / `create_*` 函数都是"输入一段 chunk 子结构 + 一点
//! 元数据，输出 `Option<ContextInjection>`"，失败返回 `None`（表示本轮没
//! 有对应 injection），不报错、不 panic。
//!
//! 设计决策见 `openspec/changes/port-context-tracking/design.md` §决策 4。

use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

use cdt_core::{
    AIChunk, ContextInjection, MessageContent, SemanticStep, TaskCoordinationBreakdown,
    TaskCoordinationInjection, TaskCoordinationKind, ThinkingTextBreakdown, ThinkingTextInjection,
    ThinkingTextKind, ToolExecution, ToolOutput, ToolOutputInjection, ToolTokenBreakdown,
    UserChunk, UserMessageInjection, estimate_content_tokens, estimate_tokens,
};

use crate::team::summary::TASK_COORDINATION_TOOL_NAMES;

#[must_use]
pub(super) fn is_task_coordination_tool(name: &str) -> bool {
    TASK_COORDINATION_TOOL_NAMES.contains(&name)
}

/// 把单个 `ToolExecution` 的 call + result token 估算出来。
///
/// `call_tokens`：对 `tool.input` 做 `estimate_content_tokens`。
/// `result_tokens`：对 `tool.output`：`Text` 直接估字符串，`Structured` 估其
/// JSON 序列化结果，`Missing` 归 0。
fn estimate_tool_tokens(tool: &ToolExecution) -> u64 {
    let call = estimate_content_tokens(&tool.input) as u64;
    let result = match &tool.output {
        ToolOutput::Text { text } => estimate_tokens(text) as u64,
        ToolOutput::Structured { value } => estimate_content_tokens(value) as u64,
        ToolOutput::Missing => 0,
    };
    call + result
}

// injection id 由所属 AIChunk 的 chunkId（即 `ai_group_id`，被打断 turn 则为
// UserChunk.chunkId）派生，而非 turn 序号（design D7，codex C2）。多个 AIChunk 折叠进同一
// turn 序号后，按 turn 序号拼 id 会撞车（两个 group 共享 turnIndex 0 → 同名 `*-ai-0`）；
// 按 chunkId 拼则每个 group 唯一。每类每 AIChunk 至多产一条聚合 injection，故 `{类别}-{chunkId}`
// 唯一。

fn tool_output_id(ai_group_id: &str) -> String {
    format!("tool-output-{ai_group_id}")
}

fn thinking_text_id(ai_group_id: &str) -> String {
    format!("thinking-text-{ai_group_id}")
}

fn task_coord_id(ai_group_id: &str) -> String {
    format!("task-coord-{ai_group_id}")
}

fn user_message_id(ai_group_id: &str) -> String {
    format!("user-msg-{ai_group_id}")
}

/// 聚合 `tool-output` 桶 —— 不含 7 个 task coordination 工具。
#[must_use]
pub(super) fn aggregate_tool_outputs(
    ai_chunk: &AIChunk,
    turn_index: u32,
    ai_group_id: &str,
) -> Option<ContextInjection> {
    let mut breakdown = Vec::new();
    let mut total: u64 = 0;

    for tool in &ai_chunk.tool_executions {
        if is_task_coordination_tool(&tool.tool_name) {
            continue;
        }
        let tokens = estimate_tool_tokens(tool);
        if tokens == 0 {
            continue;
        }
        let display_name = if tool.tool_name == "Task" {
            "Task (Subagent)".to_string()
        } else {
            tool.tool_name.clone()
        };
        breakdown.push(ToolTokenBreakdown {
            tool_name: display_name,
            token_count: tokens,
            is_error: tool.is_error,
            tool_use_id: Some(tool.tool_use_id.clone()),
        });
        total += tokens;
    }

    if total == 0 {
        return None;
    }

    Some(ContextInjection::ToolOutput(ToolOutputInjection {
        id: tool_output_id(ai_group_id),
        turn_index,
        ai_group_id: ai_group_id.to_string(),
        estimated_tokens: total,
        tool_count: breakdown.len(),
        tool_breakdown: breakdown,
    }))
}

/// 聚合 `task-coordination` 桶 —— 仅含 7 个 task coordination 工具。
///
/// `teammate_message` display item 本 port 不实现（需要
/// `team-coordination-metadata` 的身份识别），TS 里的那一路先留空。
#[must_use]
pub(super) fn aggregate_task_coordination(
    ai_chunk: &AIChunk,
    turn_index: u32,
    ai_group_id: &str,
) -> Option<ContextInjection> {
    let mut breakdown = Vec::new();
    let mut total: u64 = 0;

    for tool in &ai_chunk.tool_executions {
        if !is_task_coordination_tool(&tool.tool_name) {
            continue;
        }
        let tokens = estimate_tool_tokens(tool);
        if tokens == 0 {
            continue;
        }

        let (kind, label) = if tool.tool_name == "SendMessage" {
            let recipient = tool
                .input
                .get("recipient")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let label = if recipient.is_empty() {
                "SendMessage".to_string()
            } else {
                format!("SendMessage → {recipient}")
            };
            (TaskCoordinationKind::SendMessage, label)
        } else {
            (TaskCoordinationKind::TaskTool, tool.tool_name.clone())
        };

        breakdown.push(TaskCoordinationBreakdown {
            kind,
            token_count: tokens,
            label,
            tool_name: Some(tool.tool_name.clone()),
        });
        total += tokens;
    }

    if total == 0 {
        return None;
    }

    Some(ContextInjection::TaskCoordination(
        TaskCoordinationInjection {
            id: task_coord_id(ai_group_id),
            turn_index,
            ai_group_id: ai_group_id.to_string(),
            estimated_tokens: total,
            breakdown,
        },
    ))
}

/// 聚合 `thinking-text` 桶 —— `SemanticStep::Thinking` + `SemanticStep::Text`。
#[must_use]
pub(super) fn aggregate_thinking_text(
    ai_chunk: &AIChunk,
    turn_index: u32,
    ai_group_id: &str,
) -> Option<ContextInjection> {
    let mut thinking_tokens: u64 = 0;
    let mut text_tokens: u64 = 0;

    for step in &ai_chunk.semantic_steps {
        match step {
            SemanticStep::Thinking { text, .. } => {
                thinking_tokens += estimate_tokens(text) as u64;
            }
            SemanticStep::Text { text, .. } => {
                text_tokens += estimate_tokens(text) as u64;
            }
            _ => {}
        }
    }

    let total = thinking_tokens + text_tokens;
    if total == 0 {
        return None;
    }

    let mut breakdown = Vec::new();
    if thinking_tokens > 0 {
        breakdown.push(ThinkingTextBreakdown {
            kind: ThinkingTextKind::Thinking,
            token_count: thinking_tokens,
        });
    }
    if text_tokens > 0 {
        breakdown.push(ThinkingTextBreakdown {
            kind: ThinkingTextKind::Text,
            token_count: text_tokens,
        });
    }

    Some(ContextInjection::ThinkingText(ThinkingTextInjection {
        id: thinking_text_id(ai_group_id),
        turn_index,
        ai_group_id: ai_group_id.to_string(),
        estimated_tokens: total,
        breakdown,
    }))
}

/// 从 `UserChunk.content` 提取纯文本用于 token 估计。
#[must_use]
fn user_chunk_text(user: &UserChunk) -> String {
    match &user.content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut out = String::new();
            for b in blocks {
                if let cdt_core::ContentBlock::Text { text } = b {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(text);
                }
            }
            out
        }
    }
}

/// 噪声标签（含内容）：preview 显示前应剥离，否则截断 80 字符时残缺
/// 标签（如 `<task-notification><task-id>...`）会原文露给用户。
///
/// 与 `cdt-api::ipc::session_metadata::sanitize_for_title` 列表一致。
const PREVIEW_NOISE_TAGS: &[&str] = &[
    "system-reminder",
    "local-command-caveat",
    "task-notification",
    "command-name",
    "command-message",
    "command-args",
    "local-command-stdout",
    "local-command-stderr",
];

/// sanitize 全清空后 preview 显示的占位符，避免 UI 同框出现
/// "(空 preview, 大 token)" 矛盾——告诉用户这条 turn 是系统注入而非
/// 自己的输入。
const PREVIEW_PLACEHOLDER_SYSTEM_ONLY: &str = "(含系统注入)";

/// 每个噪声标签的完整闭合块 regex；`replace_all` 单趟 O(N) 替代旧
/// `find + replace_range` 的 O(N²) 循环，长 `<system-reminder>` 注入
/// 场景下不卡 UI。
static NOISE_BLOCK_RES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    PREVIEW_NOISE_TAGS
        .iter()
        .map(|tag| {
            Regex::new(&format!(r"<{tag}>[\s\S]*?</{tag}>"))
                .expect("noise block regex should compile")
        })
        .collect()
});

/// `<teammate-message ... teammate_id="..." ...>body</teammate-message>` 块。
///
/// teammate 块对用户而言已由 `SendMessage` 卡片渲染，preview 露原文标签字面
/// 是噪声；按"剥离整段（含 body）"处理。原始 `create_user_message_injection`
/// 的上游（`stats.rs` 直接遍历 `Chunk::User`）并未走
/// `is_user_chunk_message` gate，所以"normal text + teammate-message 块"
/// 的混合消息会进到这里，必须自己剥离。
///
/// `teammate_id` 用 `[^>]*?` 非贪婪允许任意 attr 顺序（如 `summary` /
/// `color` 在前）；否则 main regex miss → `UNCLOSED_NOISE_RE` 兜底会把开
/// tag 之后**包括 close tag 后面的真实文本**一起截掉。
static TEAMMATE_NOISE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<teammate-message\s+[^>]*?teammate_id="[^"]+"[^>]*>[\s\S]*?</teammate-message>"#)
        .expect("teammate noise regex should compile")
});

/// 未闭合噪声标签兜底：闭合块 regex 替换完后剩下的开 tag 一定是未闭合
/// （否则会被前两轮匹配掉），从首次匹配处截到末尾。
static UNCLOSED_NOISE_RE: LazyLock<Regex> = LazyLock::new(|| {
    let mut alts: Vec<String> = PREVIEW_NOISE_TAGS.iter().map(|t| format!("{t}>")).collect();
    alts.push(r"teammate-message\s".to_string());
    let pattern = format!(r"<(?:{})", alts.join("|"));
    Regex::new(&pattern).expect("unclosed noise regex should compile")
});

fn sanitize_preview(text: &str) -> String {
    let mut s: Cow<'_, str> = Cow::Borrowed(text);
    for re in NOISE_BLOCK_RES.iter() {
        if re.is_match(&s) {
            s = Cow::Owned(re.replace_all(&s, "").into_owned());
        }
    }
    if TEAMMATE_NOISE_RE.is_match(&s) {
        s = Cow::Owned(TEAMMATE_NOISE_RE.replace_all(&s, "").into_owned());
    }
    if let Some(m) = UNCLOSED_NOISE_RE.find(&s) {
        s = Cow::Owned(s[..m.start()].to_string());
    }
    s.trim().to_string()
}

/// 从 `UserChunk` 产出 `user-message` injection。
///
/// token 估计走原文（含噪声标签——其字符确实占 Claude 上下文窗口），
/// 但 `text_preview` 先剥离 `<task-notification>` / `<system-reminder>` /
/// `<teammate-message ...>` 等系统标签再截 80 字符，避免 UI context 面板
/// 显示残缺标签字面量（如 `<task-notification><task-id>...`）。
///
/// sanitize 后为空时（整条消息全是系统注入）显示
/// [`PREVIEW_PLACEHOLDER_SYSTEM_ONLY`] 占位符，避免 UI 同框出现
/// "空 preview + 大 token" 矛盾。
#[must_use]
pub(super) fn create_user_message_injection(
    user: &UserChunk,
    turn_index: u32,
    ai_group_id: &str,
) -> Option<ContextInjection> {
    let text = user_chunk_text(user);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let tokens = estimate_tokens(&text) as u64;
    if tokens == 0 {
        return None;
    }
    let sanitized = sanitize_preview(&text);
    let preview_source = if sanitized.is_empty() {
        PREVIEW_PLACEHOLDER_SYSTEM_ONLY
    } else {
        sanitized.as_str()
    };
    let char_count = preview_source.chars().count();
    let preview: String = preview_source.chars().take(80).collect();
    let preview = if char_count > 80 {
        format!("{preview}…")
    } else {
        preview
    };

    Some(ContextInjection::UserMessage(UserMessageInjection {
        id: user_message_id(ai_group_id),
        turn_index,
        ai_group_id: ai_group_id.to_string(),
        estimated_tokens: tokens,
        text_preview: preview,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{
        AssistantResponse, ChunkMetrics, MessageContent, SemanticStep, ToolExecution, ToolOutput,
    };
    use chrono::{DateTime, Utc};

    fn ts() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn empty_ai() -> AIChunk {
        AIChunk {
            chunk_id: "ai:a1:0".into(),
            timestamp: ts(),
            duration_ms: None,
            responses: Vec::<AssistantResponse>::new(),
            metrics: ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: Vec::new(),
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        }
    }

    fn make_tool(name: &str, input: serde_json::Value, output: &str) -> ToolExecution {
        ToolExecution {
            tool_use_id: format!("tu-{name}"),
            tool_name: name.to_string(),
            input,
            output: ToolOutput::Text {
                text: output.to_string(),
            },
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

    #[test]
    fn aggregate_tool_outputs_skips_task_coordination_tools() {
        let mut ai = empty_ai();
        ai.tool_executions.push(make_tool(
            "Bash",
            serde_json::json!({"cmd":"ls -la"}),
            "file1\nfile2\nfile3",
        ));
        ai.tool_executions.push(make_tool(
            "SendMessage",
            serde_json::json!({"recipient":"bob","body":"hi"}),
            "ack",
        ));
        let inj = aggregate_tool_outputs(&ai, 0, "ai-0").expect("has tool output");
        match inj {
            ContextInjection::ToolOutput(x) => {
                assert_eq!(x.tool_count, 1);
                assert_eq!(x.tool_breakdown[0].tool_name, "Bash");
                assert!(x.estimated_tokens > 0);
            }
            _ => panic!("expected ToolOutput"),
        }
    }

    #[test]
    fn aggregate_tool_outputs_returns_none_when_only_task_tools() {
        let mut ai = empty_ai();
        ai.tool_executions.push(make_tool(
            "TaskCreate",
            serde_json::json!({"subject":"x"}),
            "ok",
        ));
        assert!(aggregate_tool_outputs(&ai, 0, "ai-0").is_none());
    }

    #[test]
    fn aggregate_task_coordination_labels_send_message_with_recipient() {
        let mut ai = empty_ai();
        ai.tool_executions.push(make_tool(
            "SendMessage",
            serde_json::json!({"recipient":"alice","body":"hi"}),
            "ack",
        ));
        let inj = aggregate_task_coordination(&ai, 1, "ai-1").unwrap();
        match inj {
            ContextInjection::TaskCoordination(x) => {
                assert_eq!(x.breakdown.len(), 1);
                assert_eq!(x.breakdown[0].label, "SendMessage → alice");
                assert!(matches!(
                    x.breakdown[0].kind,
                    TaskCoordinationKind::SendMessage
                ));
            }
            _ => panic!("expected TaskCoordination"),
        }
    }

    #[test]
    fn aggregate_thinking_text_sums_both_kinds() {
        let mut ai = empty_ai();
        ai.semantic_steps.push(SemanticStep::Thinking {
            text: "abcdefghijklmnop".into(), // 16 chars → 4 tokens
            timestamp: ts(),
        });
        ai.semantic_steps.push(SemanticStep::Text {
            text: "abcdefgh".into(), // 8 chars → 2 tokens
            timestamp: ts(),
        });
        let inj = aggregate_thinking_text(&ai, 2, "ai-2").unwrap();
        match inj {
            ContextInjection::ThinkingText(x) => {
                assert_eq!(x.estimated_tokens, 6);
                assert_eq!(x.breakdown.len(), 2);
            }
            _ => panic!("expected ThinkingText"),
        }
    }

    #[test]
    fn aggregate_thinking_text_returns_none_when_empty() {
        let ai = empty_ai();
        assert!(aggregate_thinking_text(&ai, 0, "ai-0").is_none());
    }

    #[test]
    fn create_user_message_injection_handles_text_and_trims_preview() {
        let user = UserChunk {
            chunk_id: "u1".into(),
            uuid: "u1".into(),
            timestamp: ts(),
            duration_ms: None,
            content: MessageContent::Text("a".repeat(120)),
            metrics: ChunkMetrics::zero(),
        };
        let inj = create_user_message_injection(&user, 0, "ai-0").unwrap();
        match inj {
            ContextInjection::UserMessage(x) => {
                assert!(x.estimated_tokens > 0);
                assert!(x.text_preview.chars().count() <= 81); // 80 + …
                assert!(x.text_preview.ends_with('…'));
            }
            _ => panic!("expected UserMessage"),
        }
    }

    #[test]
    fn create_user_message_injection_returns_none_on_empty() {
        let user = UserChunk {
            chunk_id: "u1".into(),
            uuid: "u1".into(),
            timestamp: ts(),
            duration_ms: None,
            content: MessageContent::Text("   ".into()),
            metrics: ChunkMetrics::zero(),
        };
        assert!(create_user_message_injection(&user, 0, "ai-0").is_none());
    }

    fn user_with_text(text: &str) -> UserChunk {
        UserChunk {
            chunk_id: "u1".into(),
            uuid: "u1".into(),
            timestamp: ts(),
            duration_ms: None,
            content: MessageContent::Text(text.into()),
            metrics: ChunkMetrics::zero(),
        }
    }

    #[test]
    fn preview_strips_complete_task_notification_block() {
        let raw = "<task-notification><task-id>abc</task-id><status>done</status></task-notification>真正的用户输入";
        let user = user_with_text(raw);
        let inj = create_user_message_injection(&user, 0, "ai-0").unwrap();
        let ContextInjection::UserMessage(x) = inj else {
            panic!("expected UserMessage");
        };
        assert_eq!(
            x.text_preview, "真正的用户输入",
            "preview 应剥离完整 <task-notification> 块: {:?}",
            x.text_preview
        );
        assert!(
            x.estimated_tokens >= estimate_tokens(raw) as u64,
            "token 估计仍用原文（含标签）: {}",
            x.estimated_tokens
        );
    }

    #[test]
    fn preview_strips_truncated_task_notification_when_no_close_tag() {
        let raw = "<task-notification><task-id>bhzpt4awl</task-id> <tool-use-id>toolu_vrtx_01HSPRz_long_content_without_close_tag";
        let user = user_with_text(raw);
        let inj = create_user_message_injection(&user, 0, "ai-0").unwrap();
        let ContextInjection::UserMessage(x) = inj else {
            panic!("expected UserMessage");
        };
        // 未闭合 <task-notification> 整段剥离 → sanitize 空 → 显示占位符
        // （而非空字符串，避免 UI 显示 "Turn N + 大 token" 而无任何内容线索）
        assert_eq!(x.text_preview, "(含系统注入)");
    }

    #[test]
    fn preview_strips_system_reminder_keeping_trailing_text() {
        let user = user_with_text(
            "<system-reminder>noise about hooks</system-reminder>hello world after reminder",
        );
        let inj = create_user_message_injection(&user, 0, "ai-0").unwrap();
        let ContextInjection::UserMessage(x) = inj else {
            panic!("expected UserMessage");
        };
        assert_eq!(x.text_preview, "hello world after reminder");
    }

    #[test]
    fn preview_strips_multiple_consecutive_task_notifications() {
        let raw = "<task-notification><task-id>a</task-id></task-notification><task-notification><task-id>b</task-id></task-notification>tail";
        let user = user_with_text(raw);
        let inj = create_user_message_injection(&user, 0, "ai-0").unwrap();
        let ContextInjection::UserMessage(x) = inj else {
            panic!("expected UserMessage");
        };
        assert_eq!(x.text_preview, "tail");
    }

    #[test]
    fn preview_strips_teammate_message_block_with_attributes() {
        let raw = r#"prefix text <teammate-message teammate_id="alice" color="blue" summary="hi">实际正文</teammate-message> suffix"#;
        let user = user_with_text(raw);
        let inj = create_user_message_injection(&user, 0, "ai-0").unwrap();
        let ContextInjection::UserMessage(x) = inj else {
            panic!("expected UserMessage");
        };
        assert_eq!(
            x.text_preview, "prefix text  suffix",
            "teammate-message 整块（含 body）应剥离"
        );
    }

    #[test]
    fn preview_uses_placeholder_when_message_is_entirely_system_injection() {
        let raw = "<system-reminder>".to_string() + &"x".repeat(2000) + "</system-reminder>";
        let user = user_with_text(&raw);
        let inj = create_user_message_injection(&user, 0, "ai-0").unwrap();
        let ContextInjection::UserMessage(x) = inj else {
            panic!("expected UserMessage");
        };
        assert_eq!(
            x.text_preview, "(含系统注入)",
            "全噪声消息 preview 应为占位符而非空字符串"
        );
        assert!(
            x.estimated_tokens > 0,
            "token 估计仍走原文以保留 context 统计"
        );
    }

    #[test]
    fn preview_handles_mixed_noise_kinds_in_one_message() {
        let raw = r#"<system-reminder>r1</system-reminder>真实输入<task-notification><task-id>t</task-id></task-notification><teammate-message teammate_id="a">tm</teammate-message>尾巴"#;
        let user = user_with_text(raw);
        let inj = create_user_message_injection(&user, 0, "ai-0").unwrap();
        let ContextInjection::UserMessage(x) = inj else {
            panic!("expected UserMessage");
        };
        assert_eq!(x.text_preview, "真实输入尾巴");
    }

    #[test]
    fn preview_strips_teammate_message_when_teammate_id_is_not_first_attribute() {
        // codex 第二轮发现：原 regex 要求 teammate_id 紧跟 `<teammate-message`，
        // 若 summary / color 在前会 miss → UNCLOSED_NOISE_RE 兜底截掉块后真实
        // 文本。修法：[^>]*? 允许 teammate_id 在任意 attr 位置。
        let raw = r#"前文 <teammate-message summary="hi" teammate_id="alice" color="blue">body</teammate-message> 后文"#;
        let user = user_with_text(raw);
        let inj = create_user_message_injection(&user, 0, "ai-0").unwrap();
        let ContextInjection::UserMessage(x) = inj else {
            panic!("expected UserMessage");
        };
        assert_eq!(
            x.text_preview, "前文  后文",
            "teammate_id 不在第一位也应剥离整段，且不能误吃 close tag 后的文本",
        );
    }
}
