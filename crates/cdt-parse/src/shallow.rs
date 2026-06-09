use cdt_core::TokenUsage;
use serde::Deserialize;

#[derive(Debug, Default)]
pub struct ShallowSessionStats {
    pub message_count: usize,
    pub usage: TokenUsage,
    pub tool_names: Vec<String>,
    pub error_count: usize,
    pub model: Option<String>,
}

#[derive(Deserialize)]
struct ShallowEntry {
    #[serde(rename = "type", default)]
    entry_type: Option<String>,
    #[serde(default)]
    message: Option<ShallowMessage>,
}

#[derive(Deserialize)]
struct ShallowMessage {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    usage: Option<TokenUsage>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    content: Option<serde_json::Value>,
}

pub fn parse_session_shallow(lines: &[String]) -> ShallowSessionStats {
    let mut stats = ShallowSessionStats::default();

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<ShallowEntry>(line) else {
            continue;
        };

        if !matches!(
            entry.entry_type.as_deref(),
            Some("assistant" | "user" | "conversation")
        ) {
            continue;
        }

        stats.message_count += 1;

        let Some(msg) = entry.message else {
            continue;
        };

        if msg.role.as_deref() == Some("assistant") {
            if let Some(usage) = msg.usage {
                stats.usage.input_tokens += usage.input_tokens;
                stats.usage.output_tokens += usage.output_tokens;
                stats.usage.cache_creation_input_tokens += usage.cache_creation_input_tokens;
                stats.usage.cache_read_input_tokens += usage.cache_read_input_tokens;
            }
            if stats.model.is_none() {
                stats.model = msg.model;
            }

            if let Some(content) = msg.content {
                extract_tool_info(&content, &mut stats);
            }
        } else if msg.role.as_deref() == Some("user") {
            if let Some(content) = msg.content {
                extract_error_info(&content, &mut stats);
            }
        }
    }

    stats
}

fn extract_tool_info(content: &serde_json::Value, stats: &mut ShallowSessionStats) {
    let Some(blocks) = content.as_array() else {
        return;
    };
    for block in blocks {
        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
            if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                stats.tool_names.push(name.to_string());
            }
        }
    }
}

fn extract_error_info(content: &serde_json::Value, stats: &mut ShallowSessionStats) {
    let Some(blocks) = content.as_array() else {
        return;
    };
    for block in blocks {
        if block.get("type").and_then(|t| t.as_str()) == Some("tool_result")
            && block
                .get("is_error")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        {
            stats.error_count += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_assistant_line_with_type(
        entry_type: &str,
        usage: Option<(i64, i64)>,
        tools: &[&str],
    ) -> String {
        let mut content = Vec::new();
        for name in tools {
            content.push(
                serde_json::json!({"type": "tool_use", "name": name, "id": "t1", "input": {}}),
            );
        }
        let mut msg = serde_json::json!({
            "role": "assistant",
            "content": content,
            "model": "claude-sonnet-4-20250514",
        });
        if let Some((inp, out)) = usage {
            msg["usage"] = serde_json::json!({
                "input_tokens": inp,
                "output_tokens": out,
                "cache_creation_input_tokens": 0,
                "cache_read_input_tokens": 0,
            });
        }
        serde_json::json!({
            "type": entry_type,
            "uuid": "u1",
            "timestamp": "2026-01-01T00:00:00Z",
            "message": msg,
        })
        .to_string()
    }

    fn make_user_line_with_errors(entry_type: &str, error_count: usize) -> String {
        let mut content = Vec::new();
        for _ in 0..error_count {
            content.push(serde_json::json!({
                "type": "tool_result",
                "tool_use_id": "t1",
                "is_error": true,
                "content": "error"
            }));
        }
        serde_json::json!({
            "type": entry_type,
            "uuid": "u2",
            "timestamp": "2026-01-01T00:00:01Z",
            "message": {
                "role": "user",
                "content": content,
            },
        })
        .to_string()
    }

    #[test]
    fn shallow_parse_real_jsonl_format() {
        let lines = vec![
            make_assistant_line_with_type("assistant", Some((100, 50)), &["Bash", "Read"]),
            make_user_line_with_errors("user", 1),
            make_assistant_line_with_type("assistant", Some((200, 100)), &["Write"]),
        ];
        let stats = parse_session_shallow(&lines);
        assert_eq!(stats.message_count, 3);
        assert_eq!(stats.usage.input_tokens, 300);
        assert_eq!(stats.usage.output_tokens, 150);
        assert_eq!(stats.tool_names, vec!["Bash", "Read", "Write"]);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.model.as_deref(), Some("claude-sonnet-4-20250514"));
    }

    #[test]
    fn shallow_parse_legacy_conversation_format() {
        let lines = vec![make_assistant_line_with_type(
            "conversation",
            Some((100, 50)),
            &["Bash"],
        )];
        let stats = parse_session_shallow(&lines);
        assert_eq!(stats.message_count, 1);
        assert_eq!(stats.usage.input_tokens, 100);
        assert_eq!(stats.tool_names, vec!["Bash"]);
    }

    #[test]
    fn shallow_parse_cache_tokens() {
        let line = serde_json::json!({
            "type": "assistant",
            "uuid": "u1",
            "timestamp": "2026-01-01T00:00:00Z",
            "message": {
                "role": "assistant",
                "content": [],
                "model": "claude-sonnet-4-20250514",
                "usage": {
                    "input_tokens": 6,
                    "output_tokens": 205,
                    "cache_creation_input_tokens": 104_641,
                    "cache_read_input_tokens": 0,
                },
            },
        })
        .to_string();
        let stats = parse_session_shallow(&[line]);
        assert_eq!(stats.usage.input_tokens, 6);
        assert_eq!(stats.usage.cache_creation_input_tokens, 104_641);
        assert_eq!(stats.usage.cache_read_input_tokens, 0);
    }

    #[test]
    fn shallow_parse_skips_non_conversation_types() {
        let lines = vec![
            serde_json::json!({"type": "custom-title", "title": "test"}).to_string(),
            serde_json::json!({"type": "permission-mode", "mode": "auto"}).to_string(),
            make_assistant_line_with_type("assistant", Some((10, 5)), &[]),
        ];
        let stats = parse_session_shallow(&lines);
        assert_eq!(stats.message_count, 1);
        assert_eq!(stats.usage.input_tokens, 10);
    }

    #[test]
    fn shallow_parse_skips_bad_lines() {
        let lines = vec![
            "not json".to_string(),
            String::new(),
            make_assistant_line_with_type("assistant", Some((10, 5)), &[]),
        ];
        let stats = parse_session_shallow(&lines);
        assert_eq!(stats.message_count, 1);
        assert_eq!(stats.usage.input_tokens, 10);
    }
}
