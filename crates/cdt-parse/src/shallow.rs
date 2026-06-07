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

        if entry.entry_type.as_deref() != Some("conversation") {
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
        }
    }

    stats
}

fn extract_tool_info(content: &serde_json::Value, stats: &mut ShallowSessionStats) {
    let Some(blocks) = content.as_array() else {
        return;
    };
    for block in blocks {
        match block.get("type").and_then(|t| t.as_str()) {
            Some("tool_use") => {
                if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                    stats.tool_names.push(name.to_string());
                }
            }
            Some("tool_result")
                if block
                    .get("is_error")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false) =>
            {
                stats.error_count += 1;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_assistant_line(usage: Option<(i64, i64)>, tools: &[(&str, bool)]) -> String {
        let mut content = Vec::new();
        for (name, is_error) in tools {
            content.push(
                serde_json::json!({"type": "tool_use", "name": name, "id": "t1", "input": {}}),
            );
            content.push(serde_json::json!({"type": "tool_result", "tool_use_id": "t1", "is_error": is_error, "content": "ok"}));
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
            "type": "conversation",
            "uuid": "u1",
            "timestamp": "2026-01-01T00:00:00Z",
            "message": msg,
        })
        .to_string()
    }

    #[test]
    fn shallow_parse_extracts_usage_and_tools() {
        let lines = vec![
            make_assistant_line(Some((100, 50)), &[("Bash", false), ("Read", true)]),
            make_assistant_line(Some((200, 100)), &[("Write", false)]),
        ];
        let stats = parse_session_shallow(&lines);
        assert_eq!(stats.message_count, 2);
        assert_eq!(stats.usage.input_tokens, 300);
        assert_eq!(stats.usage.output_tokens, 150);
        assert_eq!(stats.tool_names, vec!["Bash", "Read", "Write"]);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.model.as_deref(), Some("claude-sonnet-4-20250514"));
    }

    #[test]
    fn shallow_parse_skips_bad_lines() {
        let lines = vec![
            "not json".to_string(),
            String::new(),
            make_assistant_line(Some((10, 5)), &[]),
        ];
        let stats = parse_session_shallow(&lines);
        assert_eq!(stats.message_count, 1);
        assert_eq!(stats.usage.input_tokens, 10);
    }
}
