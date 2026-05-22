//! HTTP contract tests: 守护 IPC command ↔ HTTP route ↔ `BrowserTransport`
//! 路由表三方对齐，让"新加 IPC command 漏接 HTTP / 漏写 `transport.ts` case"
//! 这类两端不对齐 bug 在 CI 拦下来。
//!
//! 历史背景（PR `feat-dev-http-proxy-vite`）：浏览器 `?http=1` 模式下
//! `BrowserTransport` 用手写 cmd → URL 路由表（`ui/src/lib/transport.ts::
//! httpRequestForCommand`）+ 黑名单（`unsupportedBrowserCommands`）。任一缺
//! 配都不会编译失败，运行时才抛 `BrowserUnsupportedError` 或 404，往往要等
//! 用户报"功能在 http 模式下没用"才发现。本套契约 test 在 CI 拦：
//!
//! 1. 每个 `EXPECTED_TAURI_COMMANDS` 中的 command SHALL 满足以下二者之一：
//!    - 列在 `BROWSER_UNSUPPORTED_COMMANDS` 且 transport.ts unsupportedBrowserCommands 含同名条目
//!    - 在 transport.ts `httpRequestForCommand` 内有 `case "<cmd>":` 分支
//! 2. transport.ts `unsupportedBrowserCommands` 集合 SHALL 与
//!    `BROWSER_UNSUPPORTED_COMMANDS` 严格相等（多余 / 缺失都拒）。
//! 3. transport.ts 出现的每个 `case "<cmd>":` 分支 SHALL 对应
//!    `EXPECTED_TAURI_COMMANDS` 中的 command（拒绝错别字 / 重复 / 已删 cmd 残留）。

#[path = "contract_data.rs"]
mod contract_data;

use std::collections::BTreeSet;
use std::path::PathBuf;

use contract_data::{BROWSER_UNSUPPORTED_COMMANDS, EXPECTED_TAURI_COMMANDS};

fn transport_ts_path() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/cdt-api；transport.ts 在 ../../ui/src/lib/
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("ui")
        .join("src")
        .join("lib")
        .join("transport.ts")
}

fn read_transport_ts() -> String {
    let path = transport_ts_path();
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "failed to read transport.ts at {}: {e} —— contract test SHALL 跑在 repo \
             worktree 根下（CARGO_MANIFEST_DIR 推算路径），CI 默认满足；本机若 \
             cargo 子目录跑可显式 `cargo test -p cdt-api --test http_contract`",
            path.display()
        )
    })
}

/// 从 transport.ts `httpRequestForCommand` 函数体提取所有 `case "<cmd>":` 中的
/// cmd 名（去重）。**只扫这个函数**——transport.ts 还有 `normalizeHttpResponse`
/// / `mapPushEventName` / `subscribeEvents` 等函数也用 switch case，但它们的
/// case 是 **SSE 事件名**（如 `file_change`、`session_metadata_update`），与 IPC
/// command name 集合互不重叠，混入会导致契约 test 误报。
fn extract_transport_cases(src: &str) -> BTreeSet<String> {
    let mut cases = BTreeSet::new();
    let Some(fn_start) = src.find("function httpRequestForCommand(") else {
        panic!(
            "transport.ts SHALL 含 `function httpRequestForCommand(...)`——重命名后请同步契约 test"
        );
    };
    let body = &src[fn_start..];
    let Some(brace_open) = body.find('{') else {
        panic!("找不到 httpRequestForCommand 函数体起始 `{{`");
    };
    let mut depth = 0_i32;
    let mut end = body.len();
    for (i, ch) in body[brace_open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = brace_open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    for line in body[brace_open..end].lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("case \"")
            && let Some(close) = rest.find("\":")
        {
            cases.insert(rest[..close].to_string());
        }
    }
    cases
}

/// 从 transport.ts 提取 `unsupportedBrowserCommands` Set 字面量内的 cmd 名。
///
/// 形态：
/// ```ts
/// const unsupportedBrowserCommands = new Set([
///   "check_for_update",
///   "is_running_under_rosetta",
///   ...
/// ]);
/// ```
fn extract_transport_unsupported(src: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(start) = src.find("const unsupportedBrowserCommands = new Set([") else {
        panic!("transport.ts SHALL 有 `const unsupportedBrowserCommands = new Set([...])` 字面量");
    };
    let body = &src[start..];
    let Some(close) = body.find("]);") else {
        panic!("transport.ts unsupportedBrowserCommands literal 没找到 `]);` 闭合");
    };
    for line in body[..close].lines() {
        let trimmed = line.trim().trim_end_matches(',').trim();
        if let Some(stripped) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            names.insert(stripped.to_string());
        }
    }
    names
}

#[test]
fn every_ipc_command_either_has_transport_case_or_is_browser_unsupported() {
    let src = read_transport_ts();
    let cases = extract_transport_cases(&src);
    let unsupported_set: BTreeSet<&&str> = BROWSER_UNSUPPORTED_COMMANDS.iter().collect();

    let mut missing: Vec<String> = Vec::new();
    for cmd in EXPECTED_TAURI_COMMANDS {
        if unsupported_set.contains(cmd) {
            continue;
        }
        if !cases.contains(*cmd) {
            missing.push((*cmd).to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "以下 IPC command 缺 ui/src/lib/transport.ts httpRequestForCommand case，浏览器 \
         ?http=1 模式调用会抛 BrowserUnsupportedError：{missing:?}\n\
         修法：在 httpRequestForCommand switch 加对应 `case \"<cmd>\":` 分支并写 axum URL；\
         或如果该 command 不应在浏览器暴露，加到 `BROWSER_UNSUPPORTED_COMMANDS` + \
         transport.ts `unsupportedBrowserCommands` 双侧黑名单。"
    );
}

#[test]
fn browser_unsupported_set_matches_transport_blocklist() {
    let src = read_transport_ts();
    let transport_blocked = extract_transport_unsupported(&src);
    let rust_side: BTreeSet<String> = BROWSER_UNSUPPORTED_COMMANDS
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    let only_in_rust: Vec<&String> = rust_side.difference(&transport_blocked).collect();
    let only_in_ts: Vec<&String> = transport_blocked.difference(&rust_side).collect();

    assert!(
        only_in_rust.is_empty() && only_in_ts.is_empty(),
        "BROWSER_UNSUPPORTED_COMMANDS 与 transport.ts unsupportedBrowserCommands 不一致。\n\
         仅 Rust 侧（contract_data.rs）有：{only_in_rust:?}\n\
         仅 transport.ts 有：{only_in_ts:?}\n\
         两侧 SHALL 严格相等——任一漂移 = BrowserTransport 错过黑名单提前抛错的机会，\
         调用方拿到 404 / fetch error 难定位。"
    );
}

#[test]
fn every_transport_case_corresponds_to_known_ipc_command() {
    let src = read_transport_ts();
    let cases = extract_transport_cases(&src);
    let known: BTreeSet<&str> = EXPECTED_TAURI_COMMANDS.iter().copied().collect();

    let unknown: Vec<&String> = cases
        .iter()
        .filter(|c| !known.contains(c.as_str()))
        .collect();
    assert!(
        unknown.is_empty(),
        "transport.ts httpRequestForCommand 出现未知 command（错别字 / 已删 cmd 残留 / \
         Rust 侧未声明）：{unknown:?}\n\
         修法：检查 case 拼写；或在 contract_data.rs::EXPECTED_TAURI_COMMANDS 加该 command \
         + 在 src-tauri/src/lib.rs::invoke_handler! 注册。"
    );
}
