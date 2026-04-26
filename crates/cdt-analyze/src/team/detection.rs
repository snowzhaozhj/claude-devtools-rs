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

/// 全局匹配每个完整的 `<teammate-message ...>body</teammate-message>` 块。
/// 与原版 `claude-devtools/src/shared/utils/teammateMessageParser.ts` 的
/// `TEAMMATE_BLOCK_RE` 同算法。
///
/// Captures: 1=`teammate_id`、2=剩余 attributes 字串、3=inner body。
static TEAMMATE_BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<teammate-message\s+teammate_id="([^"]+)"([^>]*)>([\s\S]*?)</teammate-message>"#)
        .expect("teammate block regex should compile")
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
///
/// 容错前导空白 / 换行（部分 JSONL 把 teammate-message 包在 trim 前未清理的
/// whitespace 里），用 `trim_start` 后再匹配。
pub fn is_teammate_message(msg: &ParsedMessage) -> bool {
    if msg.message_type != MessageType::User {
        return false;
    }
    extract_text(msg).is_some_and(|t| TEAMMATE_MSG_RE.is_match(t.trim_start()))
}

/// 解析 teammate 消息的属性（兼容入口：仅返回首个 block）。
///
/// 多 block 场景请用 [`parse_all_teammate_attrs`]。
pub fn parse_teammate_attrs(msg: &ParsedMessage) -> Option<TeammateAttrs> {
    parse_all_teammate_attrs(msg).into_iter().next()
}

/// 解析一条 user 消息中**所有** `<teammate-message>` 块。
///
/// 与原版 `claude-devtools/src/shared/utils/teammateMessageParser.ts::parseAllTeammateMessages`
/// 同算法：用全局 regex 扫整段 text，每个 `<teammate-message ...>body</teammate-message>`
/// 块独立产出一条 [`TeammateAttrs`]。一条 user 消息内含 N 块时返回 N 条。
///
/// 修复 bug：旧 [`parse_teammate_attrs`] 实现取 "首个 `>` 之后到末尾" 的策略，
/// 多 block 时会把所有 block 串到一个 body 里，丢失后续 block。
pub fn parse_all_teammate_attrs(msg: &ParsedMessage) -> Vec<TeammateAttrs> {
    if msg.message_type != MessageType::User {
        return Vec::new();
    }
    let Some(text) = extract_text(msg) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for caps in TEAMMATE_BLOCK_RE.captures_iter(text) {
        let teammate_id = caps[1].to_owned();
        let attrs_str = &caps[2];
        let body = caps[3].trim().to_owned();
        let color = ATTR_COLOR_RE.captures(attrs_str).map(|c| c[1].to_owned());
        let summary = ATTR_SUMMARY_RE.captures(attrs_str).map(|c| c[1].to_owned());
        out.push(TeammateAttrs {
            teammate_id,
            color,
            summary,
            body,
        });
    }
    out
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
            tool_use_result: None,
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
    fn detect_with_leading_whitespace() {
        // 部分 JSONL 把 teammate-message 包在前导 whitespace / 换行里
        let msg = make_user_msg(MessageContent::Text(
            "  \n\t<teammate-message teammate_id=\"alice\">body</teammate-message>".into(),
        ));
        assert!(
            is_teammate_message(&msg),
            "前导 whitespace 不应让 is_teammate_message 漏判"
        );
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

    #[test]
    fn parse_all_returns_each_block_separately() {
        // 一条 user 消息含 3 个 teammate-message 块 → 应返回 3 条独立 attrs
        let raw = r#"<teammate-message teammate_id="alice" color="blue" summary="hi a">body alice</teammate-message><teammate-message teammate_id="bob" color="green" summary="hi b">body bob</teammate-message><teammate-message teammate_id="charlie" color="orange" summary="hi c">body charlie</teammate-message>"#;
        let msg = make_user_msg(MessageContent::Text(raw.into()));
        let all = parse_all_teammate_attrs(&msg);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].teammate_id, "alice");
        assert_eq!(all[0].body, "body alice");
        assert_eq!(all[0].summary.as_deref(), Some("hi a"));
        assert_eq!(all[1].teammate_id, "bob");
        assert_eq!(all[1].body, "body bob");
        assert_eq!(all[2].teammate_id, "charlie");
        assert_eq!(all[2].body, "body charlie");
        assert_eq!(all[2].color.as_deref(), Some("orange"));
    }

    #[test]
    fn parse_all_with_text_between_blocks() {
        // 多个 block 之间夹杂文本（罕见但需健壮）→ 仍各自解出
        let raw = r#"<teammate-message teammate_id="alice">A</teammate-message>some noise<teammate-message teammate_id="bob">B</teammate-message>"#;
        let msg = make_user_msg(MessageContent::Text(raw.into()));
        let all = parse_all_teammate_attrs(&msg);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].teammate_id, "alice");
        assert_eq!(all[0].body, "A");
        assert_eq!(all[1].teammate_id, "bob");
        assert_eq!(all[1].body, "B");
    }

    #[test]
    fn parse_all_returns_empty_for_non_teammate() {
        let msg = make_user_msg(MessageContent::Text("plain user text".into()));
        let all = parse_all_teammate_attrs(&msg);
        assert!(all.is_empty());
    }

    #[test]
    fn parse_attrs_compatibility_returns_first_block() {
        // 兼容入口：含 N 块时只返回第一块
        let raw = r#"<teammate-message teammate_id="alice">A</teammate-message><teammate-message teammate_id="bob">B</teammate-message>"#;
        let msg = make_user_msg(MessageContent::Text(raw.into()));
        let attrs = parse_teammate_attrs(&msg).unwrap();
        assert_eq!(attrs.teammate_id, "alice");
        assert_eq!(attrs.body, "A");
    }
}
