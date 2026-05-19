## ADDED Requirements

### Requirement: Reconnect lifecycle preserves SFTP session integrity

`LocalDataApi` 在 `ssh_connect` / `switch_context` / `ssh_disconnect` 路径上 SHALL 保证：旧 `RemotePollingWatcher` 在 `SshSessionManager` 做任何 lifecycle 动作（`connect` / `disconnect` / `switch_context`）之前已完成 cancel-and-join，使新调用路径不可能拿到指向已关闭 SftpSession 的旧 `Arc<Mutex<SftpSession>>`。

实施约束（与 PR #171 现有实现一致，本 Requirement 主要为加自动化回归屏障）：

- 三处调用路径 SHALL 持 `ssh_watcher_ops: Mutex<()>` 序列化整段 cancel-then-mutate 操作
- `cancel_remote_watcher(prev_context_id).await` SHALL 在 `ssh_mgr.connect / switch_context / disconnect` 之前调用
- `attach_remote_watcher(new_context_id).await` SHALL 在 `ssh_mgr` 完成插入新 `SshSessionResources` 之后调用，且与 `ssh_shutdown_generation` 双检（shutdown 中途的 attach 被丢弃）
- watcher 归属保持在 `LocalDataApi.remote_watchers`，`SshSessionManager` 不直接管 watcher 生命周期（保持 crate 边界：`cdt-ssh` 不依赖 `cdt-api` 的 broadcast tx）

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

### Requirement: Polling watcher exits promptly on cancellation

`RemotePollingWatcher::run_polling_loop` SHALL 在 `cancel_token.cancelled()` 触发时立即跳出主 loop（不等满 `POLL_INTERVAL` 或 `CATCH_UP_INTERVAL`）。当前实现使用 `tokio::select!` 同时 await `cancel_token.cancelled()` 与两个 interval tick，本 Requirement 把这一行为固化为契约。in-flight 的 `sftp.read_dir(...)` 自然完成，cancel 中断点在每次 select 入口；这是 spec `Read sessions and files over SSH with same contract` 的补强。

#### Scenario: cancel 在 sleep 阶段触发时 watcher 立即退出（paused time）

- **WHEN** 测试设置 `tokio::test(start_paused = true)`
- **AND** watcher task 在 `poll_interval.tick()` 的 await 状态
- **AND** 调用方触发 `cancel_token.cancel()`
- **THEN** `tokio::time::timeout(Duration::from_millis(100), watcher.cancel_and_join()).await` SHALL 返回 `Ok(())`（即 join 在 paused-time 维度的 100ms 内完成）
- **AND** 测试**不**通过推进时钟来让 watcher 退出（验证 cancel 本身而非 timer 触发）

#### Scenario: cancel 在 in-flight read_dir 时按现有逻辑退出

- **WHEN** watcher task 正在 await `sftp.read_dir(...)`（远端 SFTP I/O）
- **AND** 调用方触发 `cancel_token.cancel()`
- **THEN** 当前 read_dir 完成后，下一次 `tokio::select!` 入口 SHALL 命中 `cancel_token.cancelled()` 分支并跳出循环
- **AND** 本 Requirement **不**强制中断 in-flight SFTP request（保留 SFTP 协议层的礼貌断开）
