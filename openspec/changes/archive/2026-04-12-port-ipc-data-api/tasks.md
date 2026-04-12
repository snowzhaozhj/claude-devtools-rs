## 1. 依赖 + 脚手架

- [x] 1.1 在 `cdt-api/Cargo.toml` 添加 `async-trait` 依赖
- [x] 1.2 建立 `cdt-api/src/ipc/` module 结构：`mod.rs`、`types.rs`、`error.rs`、`events.rs`、`traits.rs`
- [x] 1.3 `cargo build -p cdt-api` 确认编译通过

## 2. 类型定义

- [x] 2.1 在 `types.rs` 定义请求类型：`ListProjectsRequest`、`PaginatedSessionsRequest`、`SessionDetailRequest`、`SearchRequest`、`ConfigUpdateRequest`、`SshConnectRequest` 等
- [x] 2.2 定义响应类型：`ProjectInfo`、`SessionDetail`、`SearchResult`、`PaginatedResponse<T>`
- [x] 2.3 在 `error.rs` 定义 `ApiError { code: ApiErrorCode, message: String }` 和 `ApiErrorCode` 枚举（`ValidationError`/`NotFound`/`Internal`/`SshError`）
- [x] 2.4 在 `events.rs` 定义 `PushEvent` 枚举（`FileChange`/`TodoChange`/`NewNotification`/`SshStatusChange`）

## 3. DataApi trait

- [x] 3.1 在 `traits.rs` 定义 `DataApi` async trait，按 spec 8 个 Requirement 分组方法：
  - 项目/会话：`list_projects`、`list_sessions`、`get_session_detail`、`get_session_metrics`、`get_waterfall`、`get_subagent_detail`
  - 搜索：`search_session`、`search_project`、`search_all_projects`
  - 配置/通知：`get_config`、`update_config`、`get_notifications`、`mark_notification_read`
  - SSH/context：`list_contexts`、`switch_context`、`ssh_connect`、`ssh_disconnect`、`resolve_ssh_host`
  - 文件/验证：`validate_path`、`read_claude_md_files`、`read_mentioned_file`
  - 辅助：`get_sessions_by_ids`、`read_agent_configs`、`get_worktree_sessions`
- [x] 3.2 每个方法签名返回 `Result<T, ApiError>`

## 4. lib.rs 导出 + 集成

- [x] 4.1 在 `ipc/mod.rs` 和 `lib.rs` 通过 `pub use` 导出公开 API
- [x] 4.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 4.3 `cargo fmt --all`
- [x] 4.4 `cargo test -p cdt-api` 全测试通过

## 5. 文档 + 收尾

- [x] 5.1 更新根 `CLAUDE.md` 的 Capability→crate map：`ipc-data-api` → `done ✓`
- [x] 5.2 `openspec validate port-ipc-data-api --strict`
