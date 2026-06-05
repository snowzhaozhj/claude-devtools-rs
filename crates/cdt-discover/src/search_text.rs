//! 共用文本提取 + grep helper。
//!
//! 核心契约：只匹配 JSON string leaf value，不匹配 object key。

/// 从 `serde_json::Value` 递归提取 string leaf 文本，bounded。
///
/// 只提取 `Value::String` leaf，不提取 object key。超过 `max_bytes` 后停止收集。
pub fn json_value_to_search_text(value: &serde_json::Value, max_bytes: usize) -> String {
    let mut buf = String::new();
    collect_leaves(value, &mut buf, max_bytes);
    buf
}

fn collect_leaves(value: &serde_json::Value, buf: &mut String, max_bytes: usize) {
    if buf.len() >= max_bytes {
        return;
    }
    match value {
        serde_json::Value::String(s) => {
            let remaining = max_bytes.saturating_sub(buf.len());
            if s.len() <= remaining {
                buf.push_str(s);
            } else {
                let truncated: String = s.chars().take(remaining).collect();
                buf.push_str(&truncated);
            }
            buf.push(' ');
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_leaves(v, buf, max_bytes);
                if buf.len() >= max_bytes {
                    return;
                }
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values() {
                collect_leaves(v, buf, max_bytes);
                if buf.len() >= max_bytes {
                    return;
                }
            }
        }
        _ => {}
    }
}

/// 递归检查 `serde_json::Value` 的 string leaf 是否包含 needle（已预转小写）。
///
/// 只检查 `Value::String` leaf，不匹配 object key。
pub fn json_value_contains(value: &serde_json::Value, needle_lower: &str) -> bool {
    match value {
        serde_json::Value::String(s) => s.to_lowercase().contains(needle_lower),
        serde_json::Value::Array(arr) => {
            arr.iter().any(|v| json_value_contains(v, needle_lower))
        }
        serde_json::Value::Object(map) => {
            map.values().any(|v| json_value_contains(v, needle_lower))
        }
        _ => false,
    }
}

/// Grep 匹配器——隔离匹配策略与遍历逻辑。
pub enum GrepMatcher {
    Literal { needle_lower: String },
}

impl GrepMatcher {
    pub fn literal(needle: &str) -> Self {
        Self::Literal {
            needle_lower: needle.to_lowercase(),
        }
    }

    pub fn matches(&self, haystack: &str) -> bool {
        match self {
            Self::Literal { needle_lower } => haystack.to_lowercase().contains(needle_lower),
        }
    }

    pub fn matches_json_value(&self, value: &serde_json::Value) -> bool {
        match self {
            Self::Literal { needle_lower } => json_value_contains(value, needle_lower),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn search_text_extracts_string_leaves() {
        let val = json!({"command": "mw switch get", "description": "Get switch"});
        let text = json_value_to_search_text(&val, 8192);
        assert!(text.contains("mw switch get"));
        assert!(text.contains("Get switch"));
    }

    #[test]
    fn search_text_skips_object_keys() {
        let val = json!({"command": "hello"});
        let text = json_value_to_search_text(&val, 8192);
        assert!(!text.contains("command"));
        assert!(text.contains("hello"));
    }

    #[test]
    fn search_text_truncates_at_max_bytes() {
        let val = json!({"a": "x".repeat(5000), "b": "y".repeat(5000)});
        let text = json_value_to_search_text(&val, 4096);
        assert!(text.len() <= 4096 + 2);
    }

    #[test]
    fn search_text_handles_nested_arrays() {
        let val = json!([["nested", "array"], "flat"]);
        let text = json_value_to_search_text(&val, 8192);
        assert!(text.contains("nested"));
        assert!(text.contains("array"));
        assert!(text.contains("flat"));
    }

    #[test]
    fn contains_matches_string_leaf() {
        let val = json!({"command": "mw switch get carts2"});
        assert!(json_value_contains(&val, "mw switch"));
    }

    #[test]
    fn contains_does_not_match_object_key() {
        let val = json!({"command": "hello"});
        assert!(!json_value_contains(&val, "command"));
    }

    #[test]
    fn contains_is_case_insensitive() {
        let val = json!({"x": "MixedCase Value"});
        assert!(json_value_contains(&val, "mixedcase"));
    }

    #[test]
    fn contains_handles_nested_structure() {
        let val = json!({"outer": {"inner": [{"deep": "target_value"}]}});
        assert!(json_value_contains(&val, "target_value"));
        assert!(!json_value_contains(&val, "deep"));
    }

    #[test]
    fn contains_returns_false_for_numbers_and_bools() {
        let val = json!({"n": 42, "b": true, "null_val": null});
        assert!(!json_value_contains(&val, "42"));
        assert!(!json_value_contains(&val, "true"));
    }

    #[test]
    fn grep_matcher_literal_basic() {
        let m = GrepMatcher::literal("hello");
        assert!(m.matches("say HELLO world"));
        assert!(!m.matches("goodbye"));
    }

    #[test]
    fn grep_matcher_json_value() {
        let m = GrepMatcher::literal("switch");
        let val = json!({"cmd": "mw switch get"});
        assert!(m.matches_json_value(&val));

        let val2 = json!({"cmd": "git push"});
        assert!(!m.matches_json_value(&val2));
    }
}
