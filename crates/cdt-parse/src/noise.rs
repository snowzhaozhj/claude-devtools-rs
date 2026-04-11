//! Hard-noise classifier.
//!
//! See spec `openspec/specs/session-parsing/spec.md` §"Classify hard noise
//! messages". The classifier observes already-deserialized message fields
//! plus the raw `model` string and decides whether a message must be
//! filtered from any user-facing rendering.

use cdt_core::{ContentBlock, HardNoiseReason, MessageContent, MessageType};

const LOCAL_COMMAND_CAVEAT_TAG: &str = "<local-command-caveat>";
const SYSTEM_REMINDER_TAG: &str = "<system-reminder>";
const LOCAL_COMMAND_STDOUT_EMPTY: &str = "<local-command-stdout></local-command-stdout>";
const LOCAL_COMMAND_STDERR_EMPTY: &str = "<local-command-stderr></local-command-stderr>";
const INTERRUPT_PREFIX: &str = "[Request interrupted by user";

/// Returns `Some(reason)` if the message is hard noise, `None` otherwise.
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

/// Returns the displayable user text if the content is either a legacy
/// string or an array containing at least one text block. Returns `None`
/// when the content is empty or contains no text.
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

/// True if `text` is fully wrapped in a single `<tag>…</tag>` pair with no
/// non-whitespace content outside the wrapper. Used for the "solely
/// wrapped in X" spec scenarios.
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
