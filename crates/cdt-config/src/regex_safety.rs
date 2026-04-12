//! `ReDoS` 防护 regex 校验。
//!
//! 移植 TS `regexValidation.ts`：长度限制、危险 pattern 检测、
//! 括号平衡、`regex::Regex::new` 语法验证。
//!
//! Rust 的 `regex` crate 本身保证 O(n) 执行（不使用 backtracking），
//! 但仍保留校验以与 TS 行为一致并拦截明显误写。

use regex::Regex;
use std::sync::LazyLock;

/// 单个 regex pattern 的最大允许长度。
const MAX_PATTERN_LENGTH: usize = 100;

/// 危险 pattern 列表（嵌套量词、重叠替代等）。
static DANGEROUS_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // 嵌套量词：(a+)+, (a*)+, (a+)*, (a*)*
        r"\([^)]{0,50}[+*][^)]{0,50}\)[+*]",
        // 重叠替代 + 量词：(a|a)+
        r"\([^)|]{0,50}\|[^)]{0,50}\)[+*]",
        // 多量词：a{1,}+
        r"[+*]\{",
        r"\}[+*]",
        // 反向引用 + 量词
        r"\\[1-9][+*]",
        // 超长字符类 + 量词
        r"\[[^\]]{20}\][+*]",
    ]
    .iter()
    .map(|p| Regex::new(p).expect("dangerous pattern regex should compile"))
    .collect()
});

/// 校验结果。
#[derive(Debug, Clone)]
pub struct RegexValidationResult {
    pub valid: bool,
    pub error: Option<String>,
}

impl RegexValidationResult {
    fn ok() -> Self {
        Self {
            valid: true,
            error: None,
        }
    }

    fn fail(msg: impl Into<String>) -> Self {
        Self {
            valid: false,
            error: Some(msg.into()),
        }
    }
}

/// 检查括号是否平衡。
fn are_brackets_balanced(pattern: &str) -> bool {
    let mut stack: Vec<char> = Vec::new();
    let mut escaped = false;
    let mut in_char_class = false;

    for ch in pattern.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }

        if ch == '[' && !in_char_class {
            in_char_class = true;
            stack.push(ch);
            continue;
        }
        if ch == ']' && in_char_class {
            in_char_class = false;
            if stack.last() != Some(&'[') {
                return false;
            }
            stack.pop();
            continue;
        }
        if in_char_class {
            continue;
        }

        match ch {
            '(' | '{' => stack.push(ch),
            ')' => {
                if stack.last() != Some(&'(') {
                    return false;
                }
                stack.pop();
            }
            '}' => {
                if stack.last() != Some(&'{') {
                    return false;
                }
                stack.pop();
            }
            _ => {}
        }
    }

    stack.is_empty()
}

/// 校验 regex pattern 的安全性和正确性。
///
/// 检查：
/// 1. 非空字符串
/// 2. 长度限制（≤ 100）
/// 3. 危险 pattern 检测（嵌套量词等）
/// 4. 括号平衡
/// 5. `regex::Regex::new` 语法验证
pub fn validate_regex_pattern(pattern: &str) -> RegexValidationResult {
    if pattern.is_empty() {
        return RegexValidationResult::fail("Pattern must be a non-empty string");
    }

    if pattern.len() > MAX_PATTERN_LENGTH {
        return RegexValidationResult::fail(format!(
            "Pattern too long (max {MAX_PATTERN_LENGTH} characters)"
        ));
    }

    for dangerous in DANGEROUS_PATTERNS.iter() {
        if dangerous.is_match(pattern) {
            return RegexValidationResult::fail(
                "Pattern contains constructs that could cause performance issues",
            );
        }
    }

    if !are_brackets_balanced(pattern) {
        return RegexValidationResult::fail("Pattern has unbalanced brackets");
    }

    if let Err(e) = Regex::new(pattern) {
        return RegexValidationResult::fail(format!("Invalid regex syntax: {e}"));
    }

    RegexValidationResult::ok()
}

/// 便利函数：校验后编译 regex，失败返回 `None`。
pub fn create_safe_regex(pattern: &str, case_insensitive: bool) -> Option<Regex> {
    let result = validate_regex_pattern(pattern);
    if !result.valid {
        return None;
    }

    let full = if case_insensitive {
        format!("(?i){pattern}")
    } else {
        pattern.to_owned()
    };

    Regex::new(&full).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_simple_pattern() {
        let r = validate_regex_pattern(r"hello\s+world");
        assert!(r.valid);
        assert!(r.error.is_none());
    }

    #[test]
    fn empty_pattern_rejected() {
        let r = validate_regex_pattern("");
        assert!(!r.valid);
        assert!(r.error.as_deref().unwrap().contains("non-empty"));
    }

    #[test]
    fn too_long_pattern_rejected() {
        let long = "a".repeat(101);
        let r = validate_regex_pattern(&long);
        assert!(!r.valid);
        assert!(r.error.as_deref().unwrap().contains("too long"));
    }

    #[test]
    fn nested_quantifier_rejected() {
        let r = validate_regex_pattern("(a+)+");
        assert!(!r.valid);
        assert!(r.error.as_deref().unwrap().contains("performance"));
    }

    #[test]
    fn unbalanced_brackets_rejected() {
        let r = validate_regex_pattern("(abc");
        assert!(!r.valid);
        assert!(r.error.as_deref().unwrap().contains("unbalanced"));
    }

    #[test]
    fn invalid_regex_syntax_rejected() {
        let r = validate_regex_pattern("[invalid");
        assert!(!r.valid);
    }

    #[test]
    fn create_safe_regex_works() {
        let re = create_safe_regex("hello", true);
        assert!(re.is_some());
        assert!(re.unwrap().is_match("HELLO world"));
    }

    #[test]
    fn create_safe_regex_rejects_bad_pattern() {
        let re = create_safe_regex("(a+)+", false);
        assert!(re.is_none());
    }
}
