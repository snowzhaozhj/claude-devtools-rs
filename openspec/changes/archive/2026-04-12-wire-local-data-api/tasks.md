## 1. 依赖

- [x] 1.1 在 `cdt-cli/Cargo.toml` 添加 `cdt-discover`、`cdt-config`、`cdt-ssh` 依赖

## 2. LocalDataApi 实现

- [x] 2.1 在 `cdt-api/src/ipc/` 新增 `local.rs`，定义 `LocalDataApi` struct
- [x] 2.2 实现项目/会话查询方法：`list_projects`、`list_sessions`、`get_session_detail`、`get_sessions_by_ids`
- [x] 2.3 实现搜索方法：`search`
- [x] 2.4 实现配置/通知方法：`get_config`、`update_config`、`get_notifications`、`mark_notification_read`
- [x] 2.5 实现 SSH/context 方法：`list_contexts`、`switch_context`、`ssh_connect`、`ssh_disconnect`、`resolve_ssh_host`
- [x] 2.6 实现文件/验证方法：`validate_path`、`read_claude_md_files`、`read_mentioned_file`
- [x] 2.7 实现辅助方法：`read_agent_configs`、`get_worktree_sessions`
- [x] 2.8 在 `ipc/mod.rs` 和 `lib.rs` 导出 `LocalDataApi`

## 3. cdt-cli 接线

- [x] 3.1 重写 `cdt-cli/src/main.rs`：初始化 tracing → 构造各 manager → `LocalDataApi` → `start_server`
- [x] 3.2 `cargo build -p cdt-cli` 确认编译通过

## 4. 质量校验

- [x] 4.1 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 4.2 `cargo fmt --all`
- [x] 4.3 `cargo test --workspace`
- [x] 4.4 `openspec validate wire-local-data-api --strict`
