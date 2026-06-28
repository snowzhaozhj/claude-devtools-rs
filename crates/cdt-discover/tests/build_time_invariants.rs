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

/// `list_group_sessions` 在 k-way merge 之前 SHALL 跨 worktree 按 sessionId
/// 去重（保留 `last_modified` 最大的 wt 版本）。否则同一 `<sid>.jsonl` 因为
/// main + linked worktree 两份 encoded path 被两次 enumerate 时，page 内
/// 同一 sessionId 出现两次，前端 `{#each ... (sessionId)}` 报
/// `each_key_duplicate` 整段列表崩。
///
/// 该不变量靠源码 grep 防回归——多 worktree group 集成测需要真 git
/// `common-dir` + linked worktree fixture，留作 followup。
#[test]
fn list_group_sessions_dedupes_sessions_across_worktrees() {
    let path = workspace_root()
        .join("crates")
        .join("cdt-api")
        .join("src")
        .join("ipc")
        .join("local.rs");
    let body = fs::read_to_string(&path).expect("read cdt-api/src/ipc/local.rs");
    assert!(
        body.contains("best_wt_for_sid"),
        "list_group_sessions 内 cross-worktree sessionId dedup 标记 `best_wt_for_sid` \
         不存在于 {}——同一 sessionId 跨 worktree 重复会让前端 each_key_duplicate 列表崩。",
        path.display()
    );
}

/// issue #546：`src-tauri` 桌面后端只需 self-update 工具（已提取到 `cdt-install`），
/// **不得**直接依赖 `cdt-cli`——否则会透传整棵 CLI-only 依赖树（`cdt-analyze` /
/// `cdt-query` / `rmcp` / `clap` …）进桌面 app，徒增编译时间与 bundle 体积却无任何
/// 运行时收益。共享的下载/解压/校验逻辑走 `cdt-install`。本不变量靠 manifest grep
/// 防回归（PR #544 曾因给 cdt-cli 加 cdt-analyze 依赖而无意放大该透传）。
#[test]
fn src_tauri_does_not_depend_on_cdt_cli() {
    let path = workspace_root().join("src-tauri").join("Cargo.toml");
    let body = fs::read_to_string(&path).expect("read src-tauri/Cargo.toml");
    let depends_on_cli = body.lines().any(|l| {
        let trimmed = l.trim_start();
        !trimmed.starts_with('#')
            && trimmed
                .strip_prefix("cdt-cli")
                .is_some_and(|rest| rest.trim_start().starts_with('='))
    });
    assert!(
        !depends_on_cli,
        "src-tauri/Cargo.toml 直接依赖 cdt-cli——issue #546 要求桌面后端只依赖 \
         cdt-install，避免透传整棵 CLI-only 依赖树（cdt-analyze / cdt-query / rmcp / \
         clap …）进桌面 bundle。共享 self-update 逻辑请走 cdt-install。"
    );
}
