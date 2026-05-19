## Context

SSH context 切换后 UI 三个症状：项目 dropdown 切换无反应 / 会话列表数据不对 / 会话详情打不开。诊断阶段用 `docker openssh-server` 容器 + `ssh docker-ssh` alias 在本机复现，并通过 server-mode HTTP API（`127.0.0.1:3456`）旁路 UI 验证：

```
curl /api/contexts/active   → { id: "docker-ssh", kind: "ssh", isActive: true }
curl /api/repository-groups → { gitBranch: "main", worktrees=[ 本机宿主路径 ] }
```

`LocalGitIdentityResolver` 只能在本地解 `.git`（容器内既没装 git 也没 `.git`），证明 `list_repository_groups` 在 active context = SSH 时仍走 local。同时后端日志显示 `cdt_discover::project_scanner` 与 `cdt_watch::ssh_polling` 都报 `sftp error: session closed`——但 codex 二审验证 `RemotePollingWatcher::run_polling_loop` 已 cancel-aware（`tokio::select!` 含 `cancel_token.cancelled() / poll_interval.tick() / catch_up_interval.tick()`），`LocalDataApi::cancel_remote_watcher` 已做 cancel-and-join（PR #171），ssh_connect / switch_context / ssh_disconnect 三处都已调用——**Bug B 的 lifecycle 路径在 PR #171 后已修**。

剩余的两层独立 bug：

**Bug A — IPC method 没按 active context 分发**：Explore subagent 全文件 audit 出 8 处 method 直接锁 `self.scanner` / `self.projects_dir`：

| Line | 方法 |
|---|---|
| 698 | `project_memory_dir` |
| 1205 | `get_session_summaries_by_ids` |
| 1506 | `find_session_project` |
| 1551 | `get_subagent_trace` |
| 1608 | `get_image_asset` |
| 1646 | `get_tool_output` |
| 1742 | `search`（还硬编码 `LocalFileSystemProvider::new()`）|
| 2083 | `list_repository_groups` |

已正确实现（走 active provider）：line 742 `list_sessions_paginated` / line 1073 `list_sessions` / line 1289 `get_session_detail` / line 1091 `list_sessions_sync`。

**Bug B' — 测试覆盖缺口**：`crates/cdt-api/tests/ipc_contract.rs::active_ssh_context_reads_remote_projects_and_sessions` 只测了 list_projects / list_sessions_sync / get_session_detail 三处的 active context 分发，没覆盖剩余 8 处；`ssh_reconnect` 路径 lifecycle 测试完全缺失，因此 PR #171 修复未被自动化回归保护。

## Goals / Non-Goals

**Goals:**

- 修复 8 处 `LocalDataApi` IPC method 在 SSH context 下走 local provider 的 bug
- `search` 解除 `LocalFileSystemProvider` 硬编码，`SessionSearcher` 接 `Arc<dyn FileSystemProvider>`
- 扩展 `ipc_contract::active_ssh_context_reads_remote_projects_and_sessions` 覆盖全部 11 个 method（3 已实现 + 8 待修）
- 新增 `ssh_reconnect_lifecycle` 集成测试为 PR #171 lifecycle 加自动化回归

**Non-Goals:**

- 不动 `SshSessionManager` 内部 lifecycle（PR #171 已修复）
- 不改 `RemotePollingWatcher::run_polling_loop`（已 cancel-aware）
- 不动 `ipc_contract.rs` 的 schema 字段——本 change 只扩展 active context 路由测试覆盖范围，contract 字段不变
- 不改 UI 端 store 失效逻辑（前端切换 context 后已通过 `context_changed` event SSE 触发重拉，本 change 不涉及）
- 不重写 SSH 鉴权 / SFTP polling 主流程
- 不 backport 到 v0.5 之前的 tag

## Decisions

### D1 — 统一 active context 分发：helper 方式

**决策**：所有"读项目/会话/搜索/产物"类 IPC method SHALL 通过 `active_scanner()` / `active_fs_and_projects_dir()` helper 切到当前 active context 的 provider；root config 重置类（`set_projects_dir` / `reconfigure_claude_root`）保持 local。

**改动**：8 处 method body 替换：

```rust
// Before
let scanner = self.scanner.lock().await;

// After
let mut scanner = self.active_scanner().await?;
```

或：

```rust
// Before
let projects_dir = self.projects_dir.lock().await.clone();

// After
let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
```

**替代方案 A**：直接 swap `self.scanner` 的 provider（ssh_connect/disconnect 时切换）。**否决**——`self.scanner` 是 invalidate / file-watcher cache 依赖的稳定 local instance；切换 provider 会让 invalidation 逻辑变成"按 context 维度多副本"，复杂度爆炸。

**替代方案 B**：`ContextScopedScannerCache`（按 `context_id` 缓存 + generation 失效）。**当前否决**——本 change 先用 helper 把 bug 收住；如果后续观察到 SSH context scan 性能不足（多次重建 ephemeral scanner 开销可见），再单独 propose change 引入 per-context cache。helper 方式每次 lazy 构造，对 SSH SFTP 这种网络 I/O 主导的场景几乎不增加成本（构造一个 ProjectScanner 是纯 struct，没 I/O）。

### D2 — `SessionSearcher` 接 `Arc<dyn FileSystemProvider>`

**决策**：将 `crates/cdt-discover/src/search.rs::SessionSearcher` 从内部硬编码 `LocalFileSystemProvider::new()` 改为构造时接受 `Arc<dyn FileSystemProvider>` 入参。`LocalDataApi::search` 调用点改：

```rust
let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;
let searcher = SessionSearcher::new(fs, projects_dir, ...);
```

**前置 audit**：tasks.md 1.2 要求 `rg "tokio::fs|std::fs|parse_file|LocalFileSystemProvider" crates/cdt-discover/src`，所有命中要么走 `FileSystemProvider` trait method，要么明确标注为 local-only 不可走 SSH。已知 `cdt_parse::parse_entry_at` 是 sync 行解析，不读 fs（用 caller 传 `&str` line），不会有 search 路径被 sync fs API 卡住的问题。

**替代方案**：SSH context 下 `search` 返 `code: not_supported`。**否决**——SSH 用户也需要搜远端 session（远端就是 SFTP 文件读，与 local 唯一区别是 read latency）。

### D3 — Audit `ssh_connect / switch_context / ssh_disconnect` cancel 顺序（不改 SshSessionManager）

**决策**：watcher 归属保持在 `LocalDataApi.remote_watchers`；`SshSessionManager` 不涉及 watcher。本 change SHALL 仅做 audit + 加测试，**不**改 `SshSessionManager::connect/disconnect` 内部逻辑。

audit 三处 cancel 调用顺序（应已正确，本 change 加 reproducer 测试形成自动化回归）：

- `LocalDataApi::ssh_connect`（line 1888）：取 `ssh_watcher_ops` 锁 → if prev != target: `cancel_remote_watcher(prev).await` → `ssh_mgr.connect()`（内部 disconnect prev session）→ generation 不变时 `attach_remote_watcher(new)` → 释放锁
- `LocalDataApi::switch_context`（line 1865）：同上模式
- `LocalDataApi::ssh_disconnect`（line 1932）：先 `cancel_remote_watcher(context_id).await` → 再 `ssh_mgr.disconnect()`

**关键不变量**：cancel-and-join 在 `ssh_mgr` 任何动作之前完成（保证 watcher task 不会再读旧 SftpSession Arc）；attach_remote_watcher 在新连接 insert 到 sessions map 后才调（拿到的是 new provider）。

**替代方案**：把 watcher 移入 `SshSessionManager`（让 manager 自管 watcher lifecycle）。**否决**——watcher 的事件流（`broadcast::Receiver<FileEvent>`）属于 IPC 层关注的事，跨进 `cdt-ssh` 会破坏 crate 边界（cdt-ssh 不应依赖 cdt-api 的 broadcast tx）；当前 ownership 切分合理。

### D4 — 测试覆盖

**决策**：加 1 个扩展 + 2 个新集成测试。

1. **扩展 `crates/cdt-api/tests/ipc_contract.rs::active_ssh_context_reads_remote_projects_and_sessions`**：在现有 list_projects / list_sessions_sync / get_session_detail 之外，补 8 处 method 各 1 个断言（复用同一 fake SSH context setup）：
   - `list_repository_groups`：返回单个 `RepositoryGroup`，worktree 是 fake fixture 提供的远端项目（不出现本地宿主路径）
   - `find_session_project(session_id)`：返回 fake fixture 的 project_id
   - `get_session_summaries_by_ids`：返回 fake fixture 的 summaries 列表
   - `project_memory_dir(project_id)`：路径以 `<remote_home>` 为根
   - `get_subagent_trace` / `get_image_asset` / `get_tool_output`：fake provider 的 `read_file` 调用计数 ≥ 1
   - `search(query)`：fake provider 的 `read_to_string` / `open_read_stream` 计数 ≥ 1

2. **新增 `crates/cdt-api/tests/ssh_reconnect_lifecycle.rs`**：复用 `FakeRemoteSftp` 写一个 lifecycle reproducer：
   - 步骤：`insert_test_ssh_context("ctx-a", ...)` → `list_repository_groups`（assert ok）→ `ssh_disconnect("ctx-a")` → `insert_test_ssh_context("ctx-a", ...)` 重新注册 → `list_repository_groups`（assert ok）
   - 行为断言：第二次调用返回与第一次同形态的 RepositoryGroup；不依赖日志字符串匹配（codex P0 [2]）
   - 这个测试**既**验证 D3 audit 结果稳定，**也**是 Bug B 的最终回归屏障——如果未来 PR 误改了 cancel 顺序，此测试会复现"closed session" panic / Err 立即拒绝合并

3. **可选**：在 `crates/cdt-ssh/src/polling_watcher.rs::tests` 加 `cancel_during_long_poll`（`#[tokio::test(start_paused = true)]` + `tokio::time::pause` + `tokio::time::timeout(Duration::from_millis(100), join)`），断言 cancel 后 watcher 立即退出（不依赖 wall-clock）。**当前判定**：run_polling_loop 已 cancel-aware，此测试只是把现有保护写成单测；如 tasks 6.3 实施工作量 > 30min 可降级为 P1（先不加）。

## Risks / Trade-offs

- **风险**：8 处 method 改动有可能漏改类似的 9 / 10 处。**缓解**：tasks.md 把 `rg "self\\.scanner\\.lock\\|self\\.projects_dir\\.lock" crates/cdt-api/src/ipc/local.rs` 固化为 task 1.1（baseline）与 2.8（after-fix），二审 prompt 也专门列。
- **风险**：`SessionSearcher::new` 改签名会让所有调用者编译失败。**缓解**：tasks 1.2 先列全调用方；预期 ≤ 3 处（IPC 主调用 + 1-2 处单测）。
- **风险**：D4 测试 6.1 扩展的 `active_ssh_context_reads_remote_projects_and_sessions` 单 test 变长（11 个断言）。**缓解**：可拆为 11 个独立 `#[tokio::test]`（每个共享 setup helper），review 友好。
- **Trade-off**：本 change 一次性补 8 处 + 测试覆盖 + lifecycle reproducer，PR 改动行数 +400 行（其中 +280 测试）。codex 二审通过后预期 1 轮合入。

## Migration Plan

无 migration——本 change 是 bug 修复，对 IPC schema 与 UI 端无契约变化。`v0.5.5 main` 直接合入，下次 release 携带。

## Open Questions

无（D4 reproducer 测试就是 Bug B 的最终确认手段；写好如果 fails 说明 lifecycle 还需进一步收紧，pass 则 PR #171 修复被自动化回归保护）。
