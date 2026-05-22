//! 测试共享常量：IPC command 权威列表 + 浏览器 runtime 黑名单。
//!
//! 由 `tests/ipc_contract.rs` 与 `tests/http_contract.rs` 通过 `#[path]`
//! 共享，避免两份契约 test 各维护一份漂移。本文件本身不放 `#[test]` 函数；
//! cargo 仍会把它当独立 test target 编一次（跑 0 tests），可接受。

#![allow(dead_code)]

/// 与 `src-tauri/src/lib.rs::invoke_handler!` 同步——任何新加 `#[tauri::command]`
/// 必须在此追加，并通过 `ipc_contract` 的 meta-tests 校验长度 / 去重 / server-mode
/// 子集关系。命令名顺序无要求，但去重在 meta-test 内强制。
pub const EXPECTED_TAURI_COMMANDS: &[&str] = &[
    "list_projects",
    "list_sessions",
    "get_session_summaries_by_ids",
    "get_session_detail",
    "get_project_memory",
    "read_memory_file",
    "add_memory",
    "delete_memory",
    "get_subagent_trace",
    "get_image_asset",
    "get_tool_output",
    "search_sessions",
    "get_config",
    "update_config",
    "get_notifications",
    "mark_notification_read",
    "delete_notification",
    "mark_all_notifications_read",
    "clear_notifications",
    "add_trigger",
    "remove_trigger",
    "read_agent_configs",
    "ssh_connect",
    "ssh_disconnect",
    "ssh_test_connection",
    "ssh_get_state",
    "ssh_get_config_hosts",
    "ssh_resolve_host",
    "ssh_save_last_connection",
    "ssh_get_last_connection",
    "list_contexts",
    "switch_context",
    "get_active_context",
    "pin_session",
    "unpin_session",
    "hide_session",
    "unhide_session",
    "get_project_session_prefs",
    "check_for_update",
    "is_running_under_rosetta",
    "list_repository_groups",
    "get_worktree_sessions",
    "list_group_sessions",
    "list_wsl_distros",
    "http_server_start",
    "http_server_stop",
    "http_server_status",
];

/// 浏览器 runtime 不实现的 IPC command 集合——这些 command 在 `?http=1` 模式
/// 下 `BrowserTransport` 主动抛 `BrowserUnsupportedError`，**不**应有 HTTP route
/// 也**不**应在 `httpRequestForCommand` 加 case：
///
/// - `check_for_update` / `is_running_under_rosetta`：依赖 Tauri runtime API
/// - `http_server_start/stop/status`：server-mode 控制本身，浏览器没必要套娃
/// - `read_agent_configs`：依赖 Tauri filesystem permission（暂未 mirror）
/// - `add_memory` / `delete_memory`：**pre-existing gap**——`get_project_memory`
///   和 `read_memory_file` 已有 HTTP route，但 add / delete 当时 mirror 漏掉。
///   本契约 test 引入时（PR <feat-dev-http-proxy-vite>）出于"不扩 PR scope"先
///   silence；后续单 PR 补 `POST /api/projects/{id}/memory-files/add` 和
///   `/delete` 路由 + transport case 时 SHALL 同步从此名单移出。
///
/// 与 `ui/src/lib/transport.ts::unsupportedBrowserCommands` 同步——`http_contract.rs`
/// 校验两侧一致。
pub const BROWSER_UNSUPPORTED_COMMANDS: &[&str] = &[
    "check_for_update",
    "is_running_under_rosetta",
    "http_server_start",
    "http_server_stop",
    "http_server_status",
    "read_agent_configs",
    "add_memory",
    "delete_memory",
];
