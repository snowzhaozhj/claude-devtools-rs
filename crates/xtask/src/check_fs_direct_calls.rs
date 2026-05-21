//! `check-fs-direct-calls` subcommand —— H1 enforce 机制。
//!
//! 扫 `crates/*/src/**/*.rs` 内是否含 `tokio::fs::*` 直调，allowlist 从
//! `.claude/rules/fs-abstraction.md` 的 H1 Allowlist markdown table 读出。
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
            eprintln!("error: failed to parse allowlist from .claude/rules/fs-abstraction.md: {e}");
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
        "xtask: check-fs-direct-calls found {} violation(s); allowlist source = .claude/rules/fs-abstraction.md",
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
    let rules_path = workspace_root.join(".claude/rules/fs-abstraction.md");
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
    // 启发式：找到第一个 `#[cfg(test)]` 后跟 `mod ` 的行号，从该行起视为
    // 单元测试 mod（Rust 惯例放在文件底部），跳过这些行。
    // 反例覆盖：worktree_grouper.rs / project_path_resolver.rs 等 src 内
    // 单元测试的 fs setup —— 严格按 H1 它们是 test code，不算业务 violation。
    let test_mod_start = find_test_mod_start(&text);

    for (i, line) in text.lines().enumerate() {
        if let Some(start) = test_mod_start {
            if i >= start {
                break;
            }
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

/// 返回 `#[cfg(test)]` 后跟 `mod ` 的 line index（0-based），无则 `None`。
///
/// 注意：仅识别"裸" `#[cfg(test)]`；不展开 `#[cfg(any(test, ...))]` 等组合
/// 属性——若未来需要可扩展，目前 Rust 单元测试约定都用裸 cfg(test)。
fn find_test_mod_start(text: &str) -> Option<usize> {
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == "#[cfg(test)]" {
            // 允许中间空行 / 其他 attribute；扫到第一个非空非 attribute 行
            for (j, next) in lines.iter().enumerate().skip(i + 1) {
                let trimmed = next.trim();
                if trimmed.is_empty() || trimmed.starts_with("#[") {
                    continue;
                }
                if trimmed.starts_with("mod ") {
                    return Some(j);
                }
                break;
            }
        }
    }
    None
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
                // ** 匹配零或多段
                if rest_pat.is_empty() {
                    return true;
                }
                let mut idx = 0usize;
                loop {
                    if glob_lite_match_inner(rest_pat, &target[idx..]) {
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
}
