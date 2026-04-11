//! Hard-noise 分类器。
//!
//! 见 spec `openspec/specs/session-parsing/spec.md` §"Classify hard noise
//! messages"。分类器观察已反序列化的消息字段以及原始 `model` 字符串，
//! 判断该消息是否必须从任何面向用户的渲染中过滤掉。

use cdt_core::{ContentBlock, HardNoiseReason, MessageContent, MessageType};

const LOCAL_COMMAND_CAVEAT_TAG: &str = "<local-command-caveat>";
const SYSTEM_REMINDER_TAG: &str = "<system-reminder>";
const LOCAL_COMMAND_STDOUT_EMPTY: &str = "<local-command-stdout></local-command-stdout>";
const LOCAL_COMMAND_STDERR_EMPTY: &str = "<local-command-stderr></local-command-stderr>";
const INTERRUPT_PREFIX: &str = "[Request interrupted by user";

/// 若消息属于 hard noise 则返回 `Some(reason)`，否则返回 `None`。
pub(crate) fn classify_hard_noise(
    message_type: MessageType,
    model: Option<&str>,
    content: &MessageContent,
) -> Option<HardNoiseReason> {
    match message_type {
        MessageType::System
        | MessageType::Summary
        | MessageType::FileHistorySnapshot
        | MessageType::QueueOperation => {
            return Some(HardNoiseReason::NonConversationalEntry);
        }
        MessageType::Assistant => {
            if model == Some("<synthetic>") {
                return Some(HardNoiseReason::SyntheticAssistant);
            }
        }
        MessageType::User => {}
    }

    if message_type == MessageType::User {
        if let Some(reason) = classify_user_content(content) {
            return Some(reason);
        }
    }

    None
}

fn classify_user_content(content: &MessageContent) -> Option<HardNoiseReason> {
    let text = extract_user_text(content)?;
    let trimmed = text.trim();

    if trimmed.starts_with(INTERRUPT_PREFIX) {
        return Some(HardNoiseReason::InterruptMarker);
    }

    if trimmed == LOCAL_COMMAND_STDOUT_EMPTY || trimmed == LOCAL_COMMAND_STDERR_EMPTY {
        return Some(HardNoiseReason::EmptyCommandOutput);
    }

    if wraps_tag(trimmed, LOCAL_COMMAND_CAVEAT_TAG) {
        return Some(HardNoiseReason::LocalCommandCaveatOnly);
    }

    if wraps_tag(trimmed, SYSTEM_REMINDER_TAG) {
        return Some(HardNoiseReason::SystemReminderOnly);
    }

    None
}

/// 从用户消息正文里抽取可显示文本：content 是 legacy 字符串或
/// 含至少一个 text block 的数组时，返回拼接后的文本；否则返回 `None`。
fn extract_user_text(content: &MessageContent) -> Option<String> {
    match content {
        MessageContent::Text(s) => Some(s.clone()),
        MessageContent::Blocks(blocks) => {
            let mut acc = String::new();
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    if !acc.is_empty() {
                        acc.push('\n');
                    }
                    acc.push_str(text);
                }
            }
            if acc.is_empty() { None } else { Some(acc) }
        }
    }
}

/// 若 `text` 完整被一对 `<tag>…</tag>` 包裹、且包裹外没有任何非空白
/// 字符，则返回 true。用于匹配 spec 中"仅被 X 包裹"的场景。
fn wraps_tag(text: &str, open_tag: &str) -> bool {
    let close_tag = format!("</{}", &open_tag[1..]);
    if !text.starts_with(open_tag) {
        return false;
    }
    let after_close = match text.rfind(&close_tag) {
        Some(idx) => &text[idx + close_tag.len()..],
        None => return false,
    };
    after_close.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_content(s: &str) -> MessageContent {
        MessageContent::Text(s.into())
    }

    #[test]
    fn system_entry_is_noise() {
        assert_eq!(
            classify_hard_noise(MessageType::System, None, &text_content("")),
            Some(HardNoiseReason::NonConversationalEntry)
        );
    }

    #[test]
    fn synthetic_assistant_is_noise() {
        assert_eq!(
            classify_hard_noise(
                MessageType::Assistant,
                Some("<synthetic>"),
                &text_content("")
            ),
            Some(HardNoiseReason::SyntheticAssistant)
        );
    }

    #[test]
    fn normal_assistant_is_not_noise() {
        assert_eq!(
            classify_hard_noise(
                MessageType::Assistant,
                Some("claude-opus-4-6"),
                &MessageContent::Blocks(vec![ContentBlock::Text { text: "hi".into() }])
            ),
            None
        );
    }

    #[test]
    fn interrupt_marker_is_noise() {
        assert_eq!(
            classify_hard_noise(
                MessageType::User,
                None,
                &text_content("[Request interrupted by user for tool use]")
            ),
            Some(HardNoiseReason::InterruptMarker)
        );
    }

    #[test]
    fn caveat_only_is_noise() {
        assert_eq!(
            classify_hard_noise(
                MessageType::User,
                None,
                &text_content("<local-command-caveat>hi</local-command-caveat>")
            ),
            Some(HardNoiseReason::LocalCommandCaveatOnly)
        );
    }

    #[test]
    fn system_reminder_only_is_noise() {
        assert_eq!(
            classify_hard_noise(
                MessageType::User,
                None,
                &text_content("<system-reminder>x</system-reminder>")
            ),
            Some(HardNoiseReason::SystemReminderOnly)
        );
    }

    #[test]
    fn empty_stdout_is_noise() {
        assert_eq!(
            classify_hard_noise(
                MessageType::User,
                None,
                &text_content("<local-command-stdout></local-command-stdout>")
            ),
            Some(HardNoiseReason::EmptyCommandOutput)
        );
    }

    #[test]
    fn plain_user_text_is_not_noise() {
        assert_eq!(
            classify_hard_noise(MessageType::User, None, &text_content("hello")),
            None
        );
    }
}
