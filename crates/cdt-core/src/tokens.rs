//! Rough token estimation shared across crates.
//!
//! `context-tracking` 把这个函数作为 workspace 里的唯一 token 估计入口，
//! 任何需要粗略 token 数的 crate 都应该走这里，不要自己再写一份。
//!
//! Spec：`openspec/specs/context-tracking/spec.md` 的
//! `Estimate token counts with a Unicode-scalar heuristic` Requirement。
//!
//! 算法：`⌈scalar_count(text) / 4⌉`，其中 `scalar_count` 数 Unicode scalar
//! values —— 与 JS `str.length`（UTF-16 code unit）对 ASCII / BMP 完全一致，
//! 对多字节 CJK 也在同量级，避免 Rust `str::len()`（字节数）导致中文文本
//! 被高估 3 倍的问题。

use serde_json::Value;

/// 用 `⌈chars/4⌉` 估计一段字符串的 token 数。空串返回 `0`。
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.chars().count().div_ceil(4)
}

/// 估计任意 JSON 值的 token 数。
///
/// * 字符串：直接 [`estimate_tokens`]
/// * null：`0`
/// * 其他（数字 / bool / 数组 / 对象）：`serde_json::to_string` 后再估
#[must_use]
pub fn estimate_content_tokens(value: &Value) -> usize {
    match value {
        Value::Null => 0,
        Value::String(s) => estimate_tokens(s),
        other => {
            let s = serde_json::to_string(other).unwrap_or_default();
            estimate_tokens(&s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_length_16_is_four_tokens() {
        assert_eq!(estimate_tokens("abcdefghijklmnop"), 4);
    }

    #[test]
    fn empty_text_is_zero() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn three_spaces_round_up_to_one() {
        assert_eq!(estimate_tokens("   "), 1);
    }

    #[test]
    fn cjk_counts_by_scalar_not_byte() {
        assert_eq!(estimate_tokens("你好世界"), 1);
    }

    #[test]
    fn estimate_content_tokens_handles_json_array() {
        let v: Value = serde_json::json!([1, 2, 3]);
        assert_eq!(estimate_content_tokens(&v), 2); // "[1,2,3]" → 7 chars → 2 tokens
    }

    #[test]
    fn estimate_content_tokens_handles_null() {
        assert_eq!(estimate_content_tokens(&Value::Null), 0);
    }

    #[test]
    fn estimate_content_tokens_handles_string() {
        let v = Value::String("hello world".into());
        assert_eq!(estimate_content_tokens(&v), 3);
    }
}
