//! Teammate 消息检测与属性解析。
//!
//! Spec：`openspec/specs/team-coordination-metadata/spec.md`
//! §"`Detect teammate messages`"。

use std::sync::LazyLock;

use regex::Regex;

use cdt_core::{MessageContent, MessageType, ParsedMessage};

/// 匹配 `<teammate-message teammate_id="...">` 开头标签。
static TEAMMATE_MSG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^<teammate-message\s+teammate_id="([^"]+)""#)
        .expect("teammate message regex should compile")
});

/// 提取 teammate 属性的辅助 regex。
static ATTR_COLOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"color="([^"]+)""#).expect("color regex should compile"));
static ATTR_SUMMARY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"summary="([^"]+)""#).expect("summary regex should compile"));

/// Teammate 消息的解析属性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeammateAttrs {
    pub teammate_id: String,
    pub color: Option<String>,
    pub summary: Option<String>,
    pub body: String,
}

/// 从 `ParsedMessage` 提取文本内容（用于 teammate 检测）。
fn extract_text(msg: &ParsedMessage) -> Option<&str> {
    match &msg.content {
        MessageContent::Text(s) => Some(s.as_str()),
        MessageContent::Blocks(blocks) => {
            if blocks.len() == 1 {
                if let cdt_core::ContentBlock::Text { text } = &blocks[0] {
                    return Some(text.as_str());
                }
            }
            None
        }
    }
}

/// 检测消息是否是 teammate 消息。
pub fn is_teammate_message(msg: &ParsedMessage) -> bool {
    if msg.message_type != MessageType::User {
        return false;
    }
    extract_text(msg).is_some_and(|t| TEAMMATE_MSG_RE.is_match(t))
}

/// 解析 teammate 消息的属性。
pub fn parse_teammate_attrs(msg: &ParsedMessage) -> Option<TeammateAttrs> {
    if msg.message_type != MessageType::User {
        return None;
    }
    let text = extract_text(msg)?;
    let caps = TEAMMATE_MSG_RE.captures(text)?;
    let teammate_id = caps[1].to_owned();

    let color = ATTR_COLOR_RE.captures(text).map(|c| c[1].to_owned());
    let summary = ATTR_SUMMARY_RE.captures(text).map(|c| c[1].to_owned());

    // body 是闭合标签后的内容（如果有的话）
    let body = if let Some(end_tag_pos) = text.find('>') {
        let after = &text[end_tag_pos + 1..];
        after
            .strip_suffix("</teammate-message>")
            .unwrap_or(after)
            .to_owned()
    } else {
        String::new()
    };

    Some(TeammateAttrs {
        teammate_id,
        color,
        summary,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::*;
    use chrono::Utc;

    fn make_user_msg(content: MessageContent) -> ParsedMessage {
        ParsedMessage {
            uuid: "m1".into(),
            parent_uuid: None,
            message_type: MessageType::User,
            category: MessageCategory::User,
            timestamp: Utc::now(),
            role: None,
            content,
            usage: None,
            model: None,
            cwd: None,
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta: false,
            user_type: None,
            tool_calls: vec![],
            tool_results: vec![],
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            is_compact_summary: false,
            request_id: None,
        }
    }

    #[test]
    fn detect_teammate_string_content() {
        let msg = make_user_msg(MessageContent::Text(
            r#"<teammate-message teammate_id="alice" color="blue">hello</teammate-message>"#.into(),
        ));
        assert!(is_teammate_message(&msg));
    }

    #[test]
    fn detect_teammate_block_content() {
        let msg = make_user_msg(MessageContent::Blocks(vec![ContentBlock::Text {
            text: r#"<teammate-message teammate_id="bob">data</teammate-message>"#.into(),
        }]));
        assert!(is_teammate_message(&msg));
    }

    #[test]
    fn non_teammate_message() {
        let msg = make_user_msg(MessageContent::Text("hello world".into()));
        assert!(!is_teammate_message(&msg));
    }

    #[test]
    fn assistant_message_not_teammate() {
        let mut msg = make_user_msg(MessageContent::Text(
            r#"<teammate-message teammate_id="x">y</teammate-message>"#.into(),
        ));
        msg.message_type = MessageType::Assistant;
        msg.category = MessageCategory::Assistant;
        assert!(!is_teammate_message(&msg));
    }

    #[test]
    fn parse_attrs_full() {
        let msg = make_user_msg(MessageContent::Text(
            r#"<teammate-message teammate_id="alice" color="blue" summary="update">body text</teammate-message>"#.into(),
        ));
        let attrs = parse_teammate_attrs(&msg).unwrap();
        assert_eq!(attrs.teammate_id, "alice");
        assert_eq!(attrs.color, Some("blue".into()));
        assert_eq!(attrs.summary, Some("update".into()));
        assert_eq!(attrs.body, "body text");
    }

    #[test]
    fn parse_attrs_minimal() {
        let msg = make_user_msg(MessageContent::Text(
            r#"<teammate-message teammate_id="bob">content</teammate-message>"#.into(),
        ));
        let attrs = parse_teammate_attrs(&msg).unwrap();
        assert_eq!(attrs.teammate_id, "bob");
        assert!(attrs.color.is_none());
        assert!(attrs.summary.is_none());
        assert_eq!(attrs.body, "content");
    }
}
