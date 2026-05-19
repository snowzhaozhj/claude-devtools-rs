## Why

SSH context 切换后 UI 三个症状同时出现（顶部项目 dropdown 切换无反应 / 会话列表数据不对 / 会话详情打不开），定位为两层独立 bug：`LocalDataApi` 多个 IPC method 没按 active context 分发到 SSH provider（永远走 local 数据），以及 SFTP session 在某个时点变成 `closed` 后没被替换，watcher / scanner 持有的 `Arc<Mutex<SftpSession>>` 失效。SSH 远程上下文是 v0.4 主推卖点，当前实现导致核心数据流半成品状态。

## What Changes

- **统一 active context 分发契约**：所有"读项目/会话/搜索/产物"类 IPC method（共 8 处）SHALL 通过 `active_scanner()` / `active_fs_and_projects_dir()` 切到当前 active context 的 provider；root config 重置类（`set_projects_dir` / `reconfigure_claude_root`）保持 local。
- **`search` 解除 LocalFileSystemProvider 硬编码**：`SessionSearcher` 改成接 `Arc<dyn FileSystemProvider>` 入参；SSH context 下走远端 fs。
- **SFTP lifecycle 加自动化回归**：codex 二审验证 PR #171 后 `LocalDataApi::ssh_connect / switch_context / ssh_disconnect` 的 cancel-and-join 顺序 + `RemotePollingWatcher::run_polling_loop` 的 `tokio::select!` cancel-aware 主 loop 都已正确——本 change **不**改 lifecycle 实现，只 audit 三处调用顺序 + 加 `tracing::debug!` lifecycle 日志 + 加 reproducer 集成测试形成自动化回归屏障。
- **测试覆盖扩展**：扩展 `crates/cdt-api/tests/ipc_contract.rs::active_ssh_context_reads_remote_projects_and_sessions`（现有覆盖 3 method）补 8 处断言；新增 `crates/cdt-api/tests/ssh_reconnect_lifecycle.rs` round-trip 测试（insert → list → disconnect → re-insert → list，断言不报 session closed）；新增 `crates/cdt-ssh/src/polling_watcher.rs::tests::cancel_during_long_poll` 用 paused time 验证 cancel 延迟边界（不依赖 wall-clock）。

## Capabilities

### New Capabilities

无新 capability。

### Modified Capabilities

- `ipc-data-api`：新增 SHALL 要求"所有读项目/会话/搜索/产物类 IPC method 在 active context = SSH 时走远端 provider"——覆盖 list_repository_groups / project_memory_dir / find_session_project / get_image_asset / get_tool_output / get_subagent_trace / search / get_session_summaries_by_ids 八处。
- `ssh-remote-context`：新增 SHALL 要求 reconnect 时 SftpSession Arc 替换原子性 + watcher cancel-and-join 的延迟上界（poll loop 内 cancel-aware select）。

## Impact

- **代码**：
  - `crates/cdt-api/src/ipc/local.rs` 8 处 method body 改路由
  - `crates/cdt-discover/src/search.rs`（或对应 SessionSearcher 模块）签名改 `Arc<dyn FileSystemProvider>` 入参
  - `crates/cdt-api/src/ipc/local.rs::ssh_connect / switch_context / ssh_disconnect` 三处加 `tracing::debug!(target: "cdt_ssh::lifecycle", ...)` 步骤日志（不改业务逻辑）
  - **不改** `crates/cdt-ssh/src/session.rs::SshSessionManager`（PR #171 已修 lifecycle）
  - **不改** `crates/cdt-ssh/src/polling_watcher.rs::run_polling_loop`（已 cancel-aware）
- **测试**：
  - `crates/cdt-api/tests/` 新增 `ssh_active_context_dispatch.rs`（8 处 method 各 1 个测试，覆盖 SSH context 路由 contract）
  - `crates/cdt-api/tests/` 新增 `ssh_reconnect_lifecycle.rs`（connect→disconnect→reconnect→list 不报 session closed）
  - `crates/cdt-ssh/src/polling_watcher.rs::tests` 加 cancel-during-long-poll 单测
- **IPC 契约**：无字段改动（只是路由实现 bug 修复）；`ipc_contract.rs` 不需要更新。
- **依赖**：无新 crate / 版本变化。
- **性能**：SSH context 下数据流首次工作正常，无 local 性能回归。
- **不向下兼容**：无 BREAKING——只是修 bug，行为更符合 spec。
