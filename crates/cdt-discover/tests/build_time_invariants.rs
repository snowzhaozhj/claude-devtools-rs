//! Build-time grep 断言：阻止生产代码回退到 `ProjectScanner::new(` 便利构造。
//!
//! change `simplify-repository-as-project::D4` 要求生产路径走
//! `new_with_semaphore` 注入共享 semaphore，避免 N 个 scanner × 64 fd 击穿
//! macOS 默认 256 软上限。只允许 `#[cfg(test)]` 或 `crates/<x>/tests/` 目录
//! 下调用便利构造。

use std::fs;
use std::path::{Path, PathBuf};

/// 扫描 `src/` 文件树（**不**含 `tests/` / `examples/`，那些是 dev 上下文），
/// 拒绝任何 `ProjectScanner::new(` 非测试调用。
fn assert_no_naked_new_calls(crate_src_root: &Path) {
    let mut offenders: Vec<(PathBuf, usize, String)> = Vec::new();
    walk_rs(crate_src_root, &mut |path: &Path| {
        let Ok(body) = fs::read_to_string(path) else {
            return;
        };
        let mut in_cfg_test_mod = false;
        let mut brace_depth_at_entry: i32 = 0;
        let mut brace_depth: i32 = 0;
        for (idx, raw) in body.lines().enumerate() {
            let line = raw.trim();
            // 极简启发：`#[cfg(test)]` 后下一非空行的 `mod`/`fn` 进入测试上下文，
            // 直到对应大括号归零退出。
            if line.starts_with("#[cfg(test)]") || line.starts_with("#[cfg(any(test") {
                in_cfg_test_mod = true;
                brace_depth_at_entry = brace_depth;
                continue;
            }
            for ch in raw.chars() {
                if ch == '{' {
                    brace_depth += 1;
                }
                if ch == '}' {
                    brace_depth -= 1;
                    if in_cfg_test_mod && brace_depth <= brace_depth_at_entry {
                        in_cfg_test_mod = false;
                    }
                }
            }
            if in_cfg_test_mod {
                continue;
            }
            // 跳过注释 / doc / spec 引用行
            if line.starts_with("//") {
                continue;
            }
            if raw.contains("ProjectScanner::new(") && !raw.contains("new_with_semaphore") {
                offenders.push((path.to_path_buf(), idx + 1, raw.to_string()));
            }
        }
    });

    assert!(
        offenders.is_empty(),
        "Found naked `ProjectScanner::new(` in non-test source — \
         spec `simplify-repository-as-project::D4` requires `new_with_semaphore` \
         for production callers:\n{}",
        offenders
            .iter()
            .map(|(p, n, l)| format!("  {}:{} {}", p.display(), n, l.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn walk_rs(root: &Path, cb: &mut dyn FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk_rs(&p, cb);
        } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
            cb(&p);
        }
    }
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/cdt-discover → 两级 parent 即 workspace 根。
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn cdt_cli_src_does_not_call_project_scanner_new() {
    let root = workspace_root().join("crates").join("cdt-cli").join("src");
    assert_no_naked_new_calls(&root);
}

#[test]
fn cdt_api_src_does_not_call_project_scanner_new() {
    let root = workspace_root().join("crates").join("cdt-api").join("src");
    assert_no_naked_new_calls(&root);
}

#[test]
fn cdt_api_examples_does_not_call_project_scanner_new() {
    // examples 是 dev 上下文但本 spec 把它列入"生产侧 hygiene"，让回归不会
    // 偷偷在 example 里悄悄复活旧调用。
    let root = workspace_root()
        .join("crates")
        .join("cdt-api")
        .join("examples");
    if root.exists() {
        assert_no_naked_new_calls(&root);
    }
}
