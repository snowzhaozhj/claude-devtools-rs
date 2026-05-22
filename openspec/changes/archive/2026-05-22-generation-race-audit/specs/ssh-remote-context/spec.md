# ssh-remote-context spec delta

## MODIFIED Requirements

### Requirement: Reconnect lifecycle preserves SFTP session integrity

`LocalDataApi` 在 `ssh_connect` / `switch_context` / `ssh_disconnect` 路径上 SHALL 保证：旧 `RemotePollingWatcher` 在 `SshSessionManager` 做任何 lifecycle 动作（`connect` / `disconnect` / `switch_context`）之前已完成 cancel-and-join，使新调用路径不可能拿到指向已关闭 SftpSession 的旧 `Arc<Mutex<SftpSession>>`。

实施约束（与 PR #171 现有实现一致，本 Requirement 主要为加自动化回归屏障）：

- **5 处 mutate 入口** SHALL 在 mutate 之前持 `ssh_watcher_ops: Mutex<()>`：`ssh_connect` / `switch_context` / `ssh_disconnect` / `reconfigure_claude_root` / `shutdown_ssh_all`。`shutdown_ssh_all` 实现 SHALL 用 `lock().await` 而非 `try_lock()`——`try_lock` 失败时绕过锁直接 mutate `ssh_mgr` 会破坏与 refresh 路径同锁互斥的前提（详 change `generation-race-audit` codex commit-stage Bug 2 修订）。
- `cancel_remote_watcher(prev_context_id).await` SHALL 在 `ssh_mgr.connect / switch_context / disconnect` 之前调用
- `attach_remote_watcher(new_context_id).await` SHALL 在 `ssh_mgr` 完成插入新 `SshSessionResources` 之后调用，且与 `ssh_shutdown_generation` 双检（shutdown 中途的 attach 被丢弃）
- watcher 归属保持在 `LocalDataApi.remote_watchers`，`SshSessionManager` 不直接管 watcher 生命周期（保持 crate 边界：`cdt-ssh` 不依赖 `cdt-api` 的 broadcast tx）

**bump-first 顺序契约（不变）**：5 处路径 SHALL 在 `ssh_mgr.connect / switch_context / disconnect` / `ssh_mgr.shutdown_all` / 写新 `projects_dir` 这一步 await **之前** 完成 `context_generation.fetch_add(1, SeqCst)`（`reconfigure_claude_root` 同时 bump `root_generation`）。理由：保留这一顺序让任何 in-flight `list_sessions_skeleton` / `build_group_session_page` 在 spawn 时记录的 `expected_context_generation` 立即失效（broadcast 前 check `current != expected` → silent drop）。如果反转为"先 await 后 bump"，await 期间 ssh_mgr 状态部分切换但 generation 未 bump，in-flight scan task 的 broadcast 校验 `current == expected` 仍通过，会向前端串扰旧 ctx 的 metadata update。

**派生 cache 写入的双重校验**：依赖 `worktree_meta_cache` 等"全局 flat-key、随 list_repository_groups 刷新"的派生 cache 的实现 SHALL 在 cache 写入路径前用 `ssh_watcher_ops` 锁与本路径互斥，并在锁内做 (current ContextId == captured ContextId) **AND** (current `context_generation` == captured `context_generation`) 双重校验——任一 mismatch 时 SHALL skip 写入（safe degrade，不污染派生 cache）。详 `ipc-data-api::SessionSummary 增加 worktree 元信息字段` Requirement 的"映射缓存刷新约束"段。理由：bump-first 顺序使 `context_generation` 在 `ssh_mgr.switch_context` 网络 RTT 期间已经领先于实际 ssh_mgr 状态；caller 的 generation pre/post snapshot 都可能整段落在该 window 内（pre = post = bumped 后值），漏判 "context 已切"。**单 ctx-equality** 又无法识别"同 host 快速 disconnect+reconnect 期间 ContextId 等价但 generation bumped 两次"边角；**单 generation-equality** 无法识别"reconfigure_claude_root 改 Local projects_dir 但 ssh_mgr.active 不变"边角。结构性修法是 refresh 路径用同锁同步读 ssh_mgr active + 重建 ContextId（含 Local 时的 projects_dir 字段）+ 二次比较 captured generation 做综合判断。

#### Scenario: 同 host 重连后 list_repository_groups 仍返回远端数据

- **WHEN** 调用方依次执行：`insert_test_ssh_context("ctx-a", fake_provider_v1)` → `list_repository_groups`（断言成功）→ `ssh_disconnect("ctx-a")` → `insert_test_ssh_context("ctx-a", fake_provider_v2)` 同名重新注册 → `list_repository_groups`
- **THEN** 第二次 `list_repository_groups` SHALL 成功返回 `RepositoryGroup`
- **AND** 返回值 SHALL 与 `fake_provider_v2` 提供的 fixture 一致（不复用 v1 的旧数据）
- **AND** 调用过程 SHALL NOT 抛 `Err` 含 `session closed` 字符串

#### Scenario: 切换到新 host 时旧 watcher 先 cancel-and-join 再 mutate

- **WHEN** active context 是 `Ssh<host_a>` 且其 watcher 正在运行
- **AND** 调用方请求 `ssh_connect(host_b)` 切换到新 host
- **THEN** `LocalDataApi::ssh_connect` SHALL 在调 `ssh_mgr.connect` 之前完成 `cancel_remote_watcher("host_a").await`
- **AND** cancel-and-join 完成后才执行 `ssh_mgr.connect`（内部会 disconnect `host_a` 的 SshSessionResources，旧 SftpSession Arc ref count 此时降为 0）
- **AND** `host_b` 上线后任何对 `host_b` provider 的查询 SHALL 拿到 fresh Arc，**不会**返回 `host_a` 的 closed session

#### Scenario: switch_context bump-first 顺序保留以防 in-flight scan 串扰

- **WHEN** active context = `Ssh<host_a>` 且有一个 in-flight `list_sessions_skeleton` 已 spawn `scan_metadata_for_page` 后台 task（task 持 `expected_context_generation = N`）
- **AND** 调用方触发 `switch_context("local")`
- **THEN** `switch_context` 实现 SHALL 先 `context_generation.fetch_add(1, SeqCst)`（gen N→N+1）再 await `ssh_mgr.switch_context(None)`
- **AND** 后台 task 后续每次 `tx.send(SessionMetadataUpdate)` 前 SHALL load `context_generation`，发现 `N+1 != N` → silent drop update
- **AND** 用户 SHALL NOT 在切到 Local 后看到 host_a 的 metadata broadcast 串扰

#### Scenario: 派生 cache 写入识别 captured ctx 与当前 active 不一致时 skip

- **WHEN** 调用方 task A 触发 `switch_context("local")`，进入 `ssh_mgr.switch_context(None).await` 期间（gen 已 bump 到 N+1，但 ssh_mgr.active_context_id() 仍返 `host_a`）
- **AND** 调用方 task B 并发调 `list_repository_groups()`
- **AND** task B 的内部 `active_fs_and_policy().await` 拿到 captured_ctx = `Ssh<host_a>` + captured_generation = `N+1`
- **WHEN** task B 进入 `refresh_worktree_meta_cache` 路径，先获取 `ssh_watcher_ops` 锁
- **THEN** 锁拿到时 `ssh_mgr.switch_context` 已完成（task A 也持同锁，task A 完成 mutate + 释放锁后 task B 才能拿到）
- **AND** 锁内 `ssh_mgr.active_context_id().await` SHALL 返回 `None`（Local active）；重建的 ContextId = `Local { projects_dir }` 与 captured_ctx = `Ssh<host_a>` mismatch
- **AND** task B SHALL skip refresh + 写 `tracing::debug!`（含 captured/current ctx + 两个 generation 值），不调用 `worktree_meta_cache.write().clear()`
- **AND** task B 仍 SHALL 把 host_a 的 groups 返给 caller（caller 拿 self-consistent 旧数据；下次 IPC 自然刷新到 Local）

#### Scenario: 同 host 快速 disconnect+reconnect 期间生成 generation mismatch 触发 skip

- **WHEN** active context = `Ssh<host_a>`，调用方 task B 进入 `list_repository_groups_inner()` 拿到 captured_ctx = `Ssh<host_a>` + captured_generation = `N`
- **AND** 调用方 task A 在 task B inner 完成之后、wrapper 拿锁之前完成 `ssh_disconnect("host_a")`（gen N→N+1）+ `ssh_connect("host_a")` 同 host 重连（gen N+1→N+2）
- **THEN** task B wrapper 拿锁后 重建的 ContextId = `Ssh<host_a>` 与 captured_ctx 全等（同 HostSignature 派生的 ContextId 相等），但 `current_generation = N+2` ≠ `captured_generation = N` → generation mismatch
- **AND** task B SHALL skip refresh —— 避免 task B inner 用旧 host_a session 拿到的 groups 覆盖新 session 应有的最新 mapping
