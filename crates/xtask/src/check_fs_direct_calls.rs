//! `check-fs-direct-calls` subcommand —— H1 enforce 机制。
//!
//! 扫 `crates/*/src/**/*.rs` 内是否含 `tokio::fs::*` 直调，allowlist 从
//! `crates/cdt-fs/ALLOWLIST.md` 的 H1 Allowlist markdown table 读出。
//!
//! 设计：`openspec/changes/unify-fs-abstraction/design.md` D9 + D7。

use std::path::Path;
use std::process::ExitCode;

use crate::normalize_path;

/// `tokio::fs::*` 调用模式 —— 检查行内是否含其中任一字面量。
const FORBIDDEN_PATTERNS: &[&str] = &[
    "tokio::fs::metadata",
    "tokio::fs::read",
    "tokio::fs::read_to_string",
    "tokio::fs::read_dir",
    "tokio::fs::write",
    "tokio::fs::create_dir",
    "tokio::fs::create_dir_all",
    "tokio::fs::remove",
    "tokio::fs::remove_file",
    "tokio::fs::remove_dir",
    "tokio::fs::remove_dir_all",
    "tokio::fs::File::open",
    "tokio::fs::File::create",
];

pub fn run(workspace_root: &Path, args: &[String]) -> ExitCode {
    let warn_only = args.iter().any(|a| a == "--warn-only");

    let allowlist = match load_allowlist(workspace_root) {
        Ok(list) => list,
        Err(e) => {
            eprintln!("error: failed to parse allowlist from crates/cdt-fs/ALLOWLIST.md: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut hits: Vec<Hit> = Vec::new();
    let crates_root = workspace_root.join("crates");
    if !crates_root.exists() {
        eprintln!("error: crates/ root not found at {}", crates_root.display());
        return ExitCode::FAILURE;
    }
    walk_rs_files(&crates_root, workspace_root, &allowlist, &mut hits);

    if hits.is_empty() {
        println!("xtask: check-fs-direct-calls passed — 0 violation under crates/.");
        return ExitCode::SUCCESS;
    }

    let level = if warn_only { "warning" } else { "error" };
    for h in &hits {
        println!(
            "{level}: {} (H1 violation) -- '{}' at {}:{}",
            h.relpath, h.pattern, h.relpath, h.line_no
        );
        println!("  > {}", h.line);
    }
    println!(
        "xtask: check-fs-direct-calls found {} violation(s); allowlist source = crates/cdt-fs/ALLOWLIST.md",
        hits.len()
    );

    if warn_only {
        println!(
            "xtask: --warn-only is on (PR-A 过渡期默认行为)，exit 0；PR-D 后切到 fail-on-match"
        );
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

struct Hit {
    relpath: String,
    line_no: usize,
    line: String,
    pattern: &'static str,
}

/// Allowlist 条目 —— glob-lite 形式，支持 `**` / 单 `*` 通配，无正则。
#[derive(Debug)]
struct AllowEntry {
    pattern: String,
}

impl AllowEntry {
    fn matches(&self, relpath: &str) -> bool {
        glob_lite_match(&self.pattern, relpath)
    }
}

fn load_allowlist(workspace_root: &Path) -> Result<Vec<AllowEntry>, String> {
    let rules_path = workspace_root.join("crates/cdt-fs/ALLOWLIST.md");
    let text = std::fs::read_to_string(&rules_path)
        .map_err(|e| format!("read {}: {e}", rules_path.display()))?;

    // 找到 "### Allowlist" 标题（兼容前后不同标题层级，落到一个 H1 section 下表）。
    let needle_lower = "allowlist";
    let mut in_table = false;
    let mut header_passed = false;
    let mut found_marker = false;
    let mut entries = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if !found_marker {
            // 找到含 "Allowlist" 的 H3 / H2 / 段落标识
            if (l.starts_with("###") || l.starts_with("##"))
                && l.to_ascii_lowercase().contains(needle_lower)
            {
                found_marker = true;
            }
            continue;
        }
        if l.is_empty() {
            if in_table {
                break;
            }
            continue;
        }
        if l.starts_with("##") {
            break;
        }
        if l.starts_with('|') {
            if !header_passed {
                if l.contains("---") {
                    header_passed = true;
                    in_table = true;
                }
                continue;
            }
            if let Some(first) = extract_first_cell(l) {
                if !first.is_empty() {
                    entries.push(AllowEntry { pattern: first });
                }
            }
        } else if in_table {
            break;
        }
    }
    if entries.is_empty() {
        return Err(format!(
            "no allowlist entries parsed from {}; check the H1 Allowlist table format",
            rules_path.display()
        ));
    }
    Ok(entries)
}

fn extract_first_cell(row: &str) -> Option<String> {
    let trimmed = row.trim();
    let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
    let first = inner.split('|').next()?.trim();
    let cleaned = first.trim_matches('`').trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn walk_rs_files(dir: &Path, workspace_root: &Path, allow: &[AllowEntry], hits: &mut Vec<Hit>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if path.is_dir() {
            // 跳过 target / node_modules / .git 等大目录
            if matches!(name.as_ref(), "target" | "node_modules" | ".git") {
                continue;
            }
            walk_rs_files(&path, workspace_root, allow, hits);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let rel = path
                .strip_prefix(workspace_root)
                .map_or_else(|_| normalize_path(&path), normalize_path);
            if allow.iter().any(|e| e.matches(&rel)) {
                continue;
            }
            scan_file(&path, &rel, hits);
        }
    }
}

fn scan_file(path: &Path, rel: &str, hits: &mut Vec<Hit>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    // brace-depth tracking 识别 `#[cfg(test)] mod ... { ... }` span（codex 二审 M4）：
    // 旧实现单点 `find_test_mod_start` 截断文件尾部，多个 test mod / 中间夹业务
    // 代码会漏检。新实现精确收集 span 列表，仅跳过 span 内行，span 之后业务
    // 代码继续扫。
    //
    // 限制：brace count 不识别 string literal 中的 `{` `}` —— Rust 单测代码内
    // 此类 case 罕见，假阴性接受（dev tooling 性价比平衡）。
    let test_spans = collect_test_mod_spans(&text);
    let in_test_span = |i: usize| test_spans.iter().any(|(s, e)| i >= *s && i <= *e);

    for (i, line) in text.lines().enumerate() {
        if in_test_span(i) {
            continue;
        }
        // 跳过本行是注释（行首 trim 后以 // 开头）—— xtask 只关心实际调用
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        for pat in FORBIDDEN_PATTERNS {
            if line.contains(pat) {
                hits.push(Hit {
                    relpath: rel.to_string(),
                    line_no: i + 1,
                    line: trimmed.to_string(),
                    pattern: pat,
                });
                break;
            }
        }
    }
}

/// 返回所有 `#[cfg(test)] mod ... { ... }` 的 (`start_line`, `end_line`) span（0-based 含端）。
///
/// 仅识别"裸" `#[cfg(test)]`；不展开 `#[cfg(any(test, ...))]` 等组合属性
/// （Rust 惯例单元测试都用裸 cfg(test)）。brace tracking 用简单字符扫描，
/// **不**区分 string literal —— 单测代码内裸 `{` / `}` 在 string 内罕见，
/// 假阴性接受。
fn collect_test_mod_spans(text: &str) -> Vec<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() != "#[cfg(test)]" {
            i += 1;
            continue;
        }
        // 向后扫到 `mod ... ` 行（允许中间空行 / 其他 attribute）
        let mut j = i + 1;
        let mut mod_line = None;
        while j < lines.len() {
            let t = lines[j].trim();
            if t.is_empty() || t.starts_with("#[") {
                j += 1;
                continue;
            }
            if t.starts_with("mod ") {
                mod_line = Some(j);
            }
            break;
        }
        let Some(mod_idx) = mod_line else {
            i += 1;
            continue;
        };
        // brace tracking 从 mod_idx 开始，找首个 `{` 然后跟 depth 直到回 0
        let mut depth: i32 = 0;
        let mut found_open = false;
        let mut end_line = None;
        let mut k = mod_idx;
        while k < lines.len() {
            for ch in lines[k].chars() {
                if ch == '{' {
                    depth += 1;
                    found_open = true;
                } else if ch == '}' && found_open {
                    depth -= 1;
                    if depth == 0 {
                        end_line = Some(k);
                        break;
                    }
                }
            }
            if end_line.is_some() {
                break;
            }
            k += 1;
        }
        if let Some(end) = end_line {
            spans.push((i, end));
            i = end + 1;
        } else {
            // 异常：没找到 close brace（文件 truncate 等），保守跳到 mod_idx
            i = mod_idx + 1;
        }
    }
    spans
}

/// 轻量 glob：支持 `**`（任意层）、`*`（不含 `/` 的任意片段）、字面字符。
fn glob_lite_match(pattern: &str, target: &str) -> bool {
    glob_lite_match_inner(pattern.as_bytes(), target.as_bytes())
}

fn glob_lite_match_inner(pattern: &[u8], target: &[u8]) -> bool {
    if pattern.is_empty() {
        return target.is_empty();
    }
    match (pattern.first(), target.first()) {
        (Some(b'*'), _) => {
            // 区分 `**` 和 `*`
            if pattern.starts_with(b"**") {
                let rest_pat = &pattern[2..];
                let rest_pat = if let Some(b) = rest_pat.first() {
                    if *b == b'/' { &rest_pat[1..] } else { rest_pat }
                } else {
                    rest_pat
                };
                // ** 匹配零或多段；codex 二审 M5 修：每次跳过位置 SHALL 在
                // segment 边界（idx == 0 或 target[idx-1] == '/'），防止
                // `**/tests/**` 误匹配 `crates/foo/not_tests/x.rs` 这类子串。
                if rest_pat.is_empty() {
                    return true;
                }
                let mut idx = 0usize;
                loop {
                    let at_boundary = idx == 0 || target[idx - 1] == b'/';
                    if at_boundary && glob_lite_match_inner(rest_pat, &target[idx..]) {
                        return true;
                    }
                    if idx >= target.len() {
                        return false;
                    }
                    idx += 1;
                }
            } else {
                let rest_pat = &pattern[1..];
                let mut idx = 0usize;
                loop {
                    if glob_lite_match_inner(rest_pat, &target[idx..]) {
                        return true;
                    }
                    if idx >= target.len() {
                        return false;
                    }
                    if target[idx] == b'/' {
                        // `*` 不跨段
                        return glob_lite_match_inner(rest_pat, &target[idx..]);
                    }
                    idx += 1;
                }
            }
        }
        (Some(pc), Some(tc)) if pc == tc => glob_lite_match_inner(&pattern[1..], &target[1..]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_double_star_matches_any_depth() {
        assert!(glob_lite_match(
            "crates/cdt-watch/src/**",
            "crates/cdt-watch/src/watcher.rs"
        ));
        assert!(glob_lite_match(
            "**/tests/**",
            "crates/cdt-api/tests/ipc_contract.rs"
        ));
        assert!(glob_lite_match("**/tests/**", "tests/foo.rs"));
        assert!(!glob_lite_match(
            "crates/cdt-fs/src/local.rs",
            "crates/cdt-api/src/lib.rs"
        ));
    }

    #[test]
    fn glob_single_star_does_not_cross_slash() {
        assert!(glob_lite_match(
            "crates/*/Cargo.toml",
            "crates/cdt-fs/Cargo.toml"
        ));
        assert!(!glob_lite_match(
            "crates/*/Cargo.toml",
            "crates/cdt-fs/src/Cargo.toml"
        ));
    }

    #[test]
    fn glob_matches_xtask_trailing_slash() {
        assert!(glob_lite_match("xtask/", "xtask/"));
        assert!(!glob_lite_match("xtask/", "xtask/src/main.rs"));
        // 改进：xtask/ 在 allowlist 实际写成 xtask/，但我们扫的路径是 crates/xtask/src/main.rs
        // 该路径不会被这条命中——见 README/规则文件，实际是 crates/xtask/** 才匹配
        assert!(glob_lite_match(
            "crates/xtask/**",
            "crates/xtask/src/main.rs"
        ));
    }

    #[test]
    fn extract_first_cell_strips_backticks_and_pipes() {
        let row = "| `crates/cdt-fs/src/local.rs` | LocalFileSystemProvider 实现层 |";
        assert_eq!(
            extract_first_cell(row).as_deref(),
            Some("crates/cdt-fs/src/local.rs")
        );
    }

    #[test]
    fn glob_double_star_respects_segment_boundary() {
        // codex 二审 M5：`**/tests/**` SHALL NOT 误匹配 `not_tests/` 子串
        assert!(glob_lite_match("**/tests/**", "crates/foo/tests/bar.rs"));
        assert!(
            !glob_lite_match("**/tests/**", "crates/foo/not_tests/bar.rs"),
            "不应误匹配 not_tests/ 中的 tests 子串"
        );
        assert!(
            !glob_lite_match("**/tests/**", "crates/foo/testsuite/bar.rs"),
            "不应误匹配 testsuite/ 前缀"
        );
    }

    #[test]
    fn glob_trailing_double_star_matches_only_under_prefix() {
        assert!(glob_lite_match(
            "crates/xtask/**",
            "crates/xtask/src/main.rs"
        ));
        assert!(!glob_lite_match(
            "crates/xtask/**",
            "crates/cdt-fs/src/main.rs"
        ));
        assert!(
            !glob_lite_match("crates/xtask/**", "other-crates/xtask/src/main.rs"),
            "不应误匹配 other-crates/xtask/ 子串"
        );
    }

    #[test]
    fn collect_test_mod_spans_handles_multiple_and_mid_file_modules() {
        let src = r#"
fn business_a() { }

#[cfg(test)]
mod tests_a {
    fn test1() { }
}

fn business_b() {
    tokio::fs::write("path", b"x").await.unwrap();
}

#[cfg(test)]
mod tests_b {
    use super::*;
    fn test2() { }
}
"#;
        let spans = collect_test_mod_spans(src);
        assert_eq!(spans.len(), 2, "SHALL 识别两个独立 test mod");
        // 验证 business_b 行不在任一 span 内（M4 修法核心点）
        let business_b_line = src.lines().position(|l| l.contains("business_b")).unwrap();
        assert!(
            !spans
                .iter()
                .any(|(s, e)| business_b_line >= *s && business_b_line <= *e),
            "中间业务函数 SHALL NOT 被 span 跳过"
        );
    }
}
