//! 跨平台路径比较 helper —— **Windows 大小写不敏感、其它平台精确**。
//!
//! 这是整个 workspace 中跨平台路径比较的**唯一来源**：cdt-watch、cdt-config、
//! cdt-discover 内部其他模块需要做路径比较 / hash 时 SHALL `use
//! cdt_discover::path_compare::*`，**禁止**自行实现 lowercase / equality 逻辑。
//!
//! Windows 规范化策略：ASCII lowercase（`u8::to_ascii_lowercase`）。
//! 这是与 TS 原版 `pathValidation.ts::normalizeForCompare` 行为的**有意近似**——
//! TS `String.prototype.toLowerCase()` 走 Unicode default case mapping，
//! 本实现仅覆盖 ASCII。详 `openspec/changes/.../design.md::D2`。
//!
//! Spec：`openspec/specs/project-discovery/spec.md::Compare paths case-insensitively
//! on Windows` + `openspec/specs/file-watching/spec.md::Route watch events
//! case-insensitively on Windows`。

use std::borrow::Cow;
use std::path::Path;
#[cfg(target_os = "windows")]
use std::path::PathBuf;

/// 规范化路径供跨平台比较使用。
///
/// * Windows：返回 owned `PathBuf`，所有 ASCII 字母转小写
/// * 非 Windows：直接借出原 `Path`（零拷贝）
#[must_use]
pub fn normalize_path_for_compare(p: &Path) -> Cow<'_, Path> {
    #[cfg(target_os = "windows")]
    {
        let s = p.to_string_lossy();
        let lower = s.to_ascii_lowercase();
        Cow::Owned(PathBuf::from(lower))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Cow::Borrowed(p)
    }
}

/// 规范化字符串路径（callsite 已经持有 `&str` 时避免 `Path` 转换开销）。
#[must_use]
pub fn normalize_path_string_for_compare(s: &str) -> Cow<'_, str> {
    #[cfg(target_os = "windows")]
    {
        Cow::Owned(s.to_ascii_lowercase())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Cow::Borrowed(s)
    }
}

/// 跨平台路径相等 —— Windows 上 ASCII 大小写不敏感。
#[must_use]
pub fn paths_equal(a: &Path, b: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        normalize_path_for_compare(a) == normalize_path_for_compare(b)
    }
    #[cfg(not(target_os = "windows"))]
    {
        a == b
    }
}

/// 跨平台路径前缀匹配 —— Windows 上 ASCII 大小写不敏感。
#[must_use]
pub fn path_starts_with(haystack: &Path, prefix: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        normalize_path_for_compare(haystack).starts_with(normalize_path_for_compare(prefix))
    }
    #[cfg(not(target_os = "windows"))]
    {
        haystack.starts_with(prefix)
    }
}

/// 跨平台版 `Path::strip_prefix`。Windows 上当前缀大小写不一致时仍能剥离，
/// 返回的相对路径**保留 haystack 原始大小写**（不被规范化）。
///
/// 当 `haystack` 在跨平台规范化后仍不以 `prefix` 起首时返回 `None`。
#[must_use]
pub fn path_strip_prefix<'a>(haystack: &'a Path, prefix: &Path) -> Option<&'a Path> {
    #[cfg(target_os = "windows")]
    {
        let p_norm = normalize_path_for_compare(prefix);
        let prefix_components = p_norm.components().count();
        let mut iter = haystack.components();
        let mut consumed = PathBuf::new();
        for _ in 0..prefix_components {
            consumed.push(iter.next()?);
        }
        if normalize_path_for_compare(&consumed) == p_norm {
            Some(iter.as_path())
        } else {
            None
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        haystack.strip_prefix(prefix).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    mod windows {
        use super::*;

        #[test]
        fn paths_equal_ignores_case() {
            assert!(paths_equal(
                Path::new(r"C:\Users\Alice\app"),
                Path::new(r"c:\users\alice\app")
            ));
        }

        #[test]
        fn paths_equal_distinguishes_different_paths() {
            assert!(!paths_equal(
                Path::new(r"C:\Users\Alice\app"),
                Path::new(r"C:\Users\Bob\app")
            ));
        }

        #[test]
        fn path_starts_with_ignores_case() {
            assert!(path_starts_with(
                Path::new(r"C:\Users\Alice\app\sub.jsonl"),
                Path::new(r"c:\users\alice\app")
            ));
        }

        #[test]
        fn path_starts_with_rejects_non_prefix() {
            assert!(!path_starts_with(
                Path::new(r"C:\Users\Bob\app"),
                Path::new(r"c:\users\alice")
            ));
        }

        #[test]
        fn normalize_path_string_lowercases() {
            let n = normalize_path_string_for_compare("C:\\Users\\ALICE");
            assert_eq!(&*n, "c:\\users\\alice");
        }

        #[test]
        fn path_strip_prefix_returns_original_case_remainder() {
            let h = Path::new(r"C:\Users\Alice\app\sess.jsonl");
            let p = Path::new(r"c:\users\alice");
            let rest = path_strip_prefix(h, p).expect("should strip case-insensitively");
            // 返回的相对路径保留原始大小写
            assert_eq!(rest, Path::new(r"app\sess.jsonl"));
        }

        #[test]
        fn path_strip_prefix_rejects_non_match() {
            assert!(
                path_strip_prefix(Path::new(r"C:\Users\Bob"), Path::new(r"c:\users\alice"))
                    .is_none()
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    mod unix {
        use super::*;

        #[test]
        fn paths_equal_is_case_sensitive() {
            assert!(!paths_equal(
                Path::new("/Users/alice/app"),
                Path::new("/Users/Alice/app")
            ));
        }

        #[test]
        fn paths_equal_matches_exact() {
            assert!(paths_equal(
                Path::new("/Users/alice/app"),
                Path::new("/Users/alice/app")
            ));
        }

        #[test]
        fn path_starts_with_is_case_sensitive() {
            assert!(!path_starts_with(
                Path::new("/Users/Alice/app/sub.jsonl"),
                Path::new("/Users/alice/app")
            ));
        }

        #[test]
        fn path_starts_with_matches_exact() {
            assert!(path_starts_with(
                Path::new("/Users/alice/app/sub.jsonl"),
                Path::new("/Users/alice/app")
            ));
        }

        #[test]
        fn normalize_path_string_returns_borrowed() {
            let n = normalize_path_string_for_compare("/Users/Alice");
            assert_eq!(&*n, "/Users/Alice");
            // Unix 平台下 Cow 应是 Borrowed
            assert!(matches!(n, Cow::Borrowed(_)));
        }

        #[test]
        fn path_strip_prefix_exact_match() {
            let h = Path::new("/Users/alice/app/sess.jsonl");
            let p = Path::new("/Users/alice");
            let rest = path_strip_prefix(h, p).expect("exact prefix");
            assert_eq!(rest, Path::new("app/sess.jsonl"));
        }

        #[test]
        fn path_strip_prefix_rejects_case_mismatch_on_unix() {
            // Unix 上字节精确——大小写漂移视为不匹配
            assert!(
                path_strip_prefix(Path::new("/Users/Alice/app"), Path::new("/Users/alice"))
                    .is_none()
            );
        }
    }
}
