//! Trigger pattern 匹配工具。
//!
//! 对应 TS `TriggerMatcher.ts`。提供 regex cache + pattern/ignore 匹配。

use std::cell::RefCell;
use std::num::NonZeroUsize;

use lru::LruCache;
use regex::Regex;

use crate::regex_safety::validate_regex_pattern;
use cdt_core::{ContentBlock, MessageContent};

const MAX_CACHE_SIZE: usize = 500;

thread_local! {
    static REGEX_CACHE: RefCell<LruCache<String, Option<Regex>>> =
        RefCell::new(LruCache::new(
            NonZeroUsize::new(MAX_CACHE_SIZE).expect("cache size > 0"),
        ));
}

/// 从 cache 获取或编译 regex。无效 pattern 缓存为 `None`。
fn get_cached_regex(pattern: &str, flags: &str) -> Option<Regex> {
    let key = format!("{pattern}\0{flags}");

    REGEX_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.get(&key) {
            return cached.clone();
        }

        let validation = validate_regex_pattern(pattern);
        let regex = if validation.valid {
            let full = if flags.contains('i') {
                format!("(?i){pattern}")
            } else {
                pattern.to_owned()
            };
            Regex::new(&full).ok()
        } else {
            None
        };

        cache.put(key, regex.clone());
        regex
    })
}

/// 检查 content 是否匹配 pattern（大小写不敏感）。
pub fn matches_pattern(content: &str, pattern: &str) -> bool {
    get_cached_regex(pattern, "i").is_some_and(|re| re.is_match(content))
}

/// 检查 content 是否匹配任一 ignore pattern。
pub fn matches_ignore_patterns(content: &str, ignore_patterns: Option<&[String]>) -> bool {
    let Some(patterns) = ignore_patterns else {
        return false;
    };
    if patterns.is_empty() {
        return false;
    }

    for pattern in patterns {
        if get_cached_regex(pattern, "i").is_some_and(|re| re.is_match(content)) {
            return true;
        }
    }
    false
}

/// 从 `tool_use` 的 input 中按 `match_field` 提取字段值。
pub fn extract_tool_use_field(input: &serde_json::Value, match_field: &str) -> Option<String> {
    let val = input.get(match_field)?;
    if let Some(s) = val.as_str() {
        Some(s.to_owned())
    } else {
        Some(val.to_string())
    }
}

/// 从 `MessageContent` 提取 `ContentBlock` 列表。
pub fn get_content_blocks(content: &MessageContent) -> Vec<&ContentBlock> {
    match content {
        MessageContent::Blocks(blocks) => blocks.iter().collect(),
        MessageContent::Text(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_simple_pattern() {
        assert!(matches_pattern("hello ERROR world", "error"));
    }

    #[test]
    fn no_match_pattern() {
        assert!(!matches_pattern("hello world", "ERROR"));
    }

    #[test]
    fn matches_case_insensitive() {
        assert!(matches_pattern("Hello World", "hello"));
    }

    #[test]
    fn ignore_patterns_match() {
        let patterns = vec![r"user doesn't want".to_owned(), r"interrupted".to_owned()];
        assert!(matches_ignore_patterns(
            "The user doesn't want to proceed",
            Some(&patterns),
        ));
    }

    #[test]
    fn ignore_patterns_no_match() {
        let patterns = vec![r"something".to_owned()];
        assert!(!matches_ignore_patterns("other text", Some(&patterns)));
    }

    #[test]
    fn ignore_patterns_none() {
        assert!(!matches_ignore_patterns("anything", None));
    }

    #[test]
    fn extract_string_field() {
        let input = serde_json::json!({"command": "ls -la", "timeout": 5000});
        assert_eq!(
            extract_tool_use_field(&input, "command"),
            Some("ls -la".into())
        );
    }

    #[test]
    fn extract_number_field() {
        let input = serde_json::json!({"timeout": 5000});
        assert_eq!(
            extract_tool_use_field(&input, "timeout"),
            Some("5000".into())
        );
    }

    #[test]
    fn extract_missing_field() {
        let input = serde_json::json!({"a": 1});
        assert!(extract_tool_use_field(&input, "b").is_none());
    }

    #[test]
    fn get_content_blocks_from_blocks() {
        let content = MessageContent::Blocks(vec![ContentBlock::Text { text: "hi".into() }]);
        assert_eq!(get_content_blocks(&content).len(), 1);
    }

    #[test]
    fn get_content_blocks_from_text() {
        let content = MessageContent::Text("hi".into());
        assert!(get_content_blocks(&content).is_empty());
    }

    #[test]
    fn regex_cache_eviction() {
        // 填充超过 cache 大小的 pattern
        for i in 0..=MAX_CACHE_SIZE {
            let pat = format!("pattern{i}");
            matches_pattern("test", &pat);
        }
        // 不应 panic，且最早的条目应被 evict
    }
}
