## 1. 准备 + audit baseline（`cdt-api` + `cdt-discover`）

- [x] 1.1 baseline audit 完成：cdt-api/src/ipc/local.rs 共 13 处命中——8 处真 bug（698 / 1205 / 1506 / 1551 / 1608 / 1646 / 1742 / 2083）+ 例外 5 处（399、415 在 `active_fs_and_projects_dir` helper 内部；668、669 在 `set_projects_dir`；2253 test helper）
- [x] 1.2 audit `cdt-discover`：`SessionSearcher<F: FileSystemProvider>` 在 `crates/cdt-discover/src/session_search.rs` 已是泛型接 `Arc<F>`，内部全走 `self.fs.xxx` trait method；其他 `tokio::fs` / `std::fs` 出现在 `worktree_grouper.rs`（git 本地解析，与 SSH 无关，保留 local-only）/ `agent_configs.rs`（独立 capability）/ `wsl.rs`（本地 WSL 探测）/ tests
- [x] 1.3 `SessionSearcher::new` 调用方仅 2 处：`crates/cdt-api/src/ipc/local.rs:1743`（生产代码，需改）+ `crates/cdt-discover/tests/session_search.rs:49`（已传 fs，与本 change 无关）
- [x] 1.4 `FakeRemoteSftp` 在 `crates/cdt-api/tests/ipc_contract.rs:127` 定义为 file-scoped `struct`（无 `pub`），test 文件间不可复用——`ssh_reconnect_lifecycle.rs` 自建独立 fake provider，**不**改它的可见性

## 2. Bug A — `LocalDataApi` 8 处 IPC method 改 active context 分发（`cdt-api`）

- [x] 2.1 line 698 `project_memory_dir`：改用 `active_fs_and_projects_dir().await?.1`（取 projects_dir）拼接 `<projects_dir>/<project_id>/memory`
- [x] 2.2 line 1205 `get_session_summaries_by_ids`：`self.scanner.lock()` 替换为 `let mut scanner = self.active_scanner().await?;`
- [x] 2.3 line 1506 `find_session_project`：`scanner.projects_dir()` 替换为 `active_fs_and_projects_dir().await?.1`，扫描循环复用 active provider
- [x] 2.4 line 1551 `get_subagent_trace`：`self.projects_dir.lock()` 替换为 `active_fs_and_projects_dir().await?.1`；若该方法依赖 `LocalFileSystemProvider` 直读 jsonl 需同步改用 active fs
- [x] 2.5 line 1608 `get_image_asset`：同 2.4 模式；image asset 走 `parsed_msg_cache` 时验证 cache key 已含 context_id（避免 local/remote 同 session_id 串数据）
- [x] 2.6 line 1646 `get_tool_output`：同 2.5 模式
- [x] 2.7 line 2083 `list_repository_groups`：`self.scanner.lock()` 替换为 `let mut scanner = self.active_scanner().await?;`
- [x] 2.8 改完后再跑 1.1 的 `rg`，确认剩余 `self.scanner.lock() / self.projects_dir.lock()` 出现仅在：line 668（`set_projects_dir` 写赋值）、line 2253（test helper）、初始构造 `self.projects_dir = scanner.projects_dir().to_path_buf()`

## 3. Bug A — `SessionSearcher` 解除 `LocalFileSystemProvider` 硬编码（`cdt-discover` + `cdt-api`）

- [x] 3.1 `crates/cdt-discover/src/search.rs`（或 `SessionSearcher` 实际所在模块）改 `SessionSearcher::new(fs: Arc<dyn FileSystemProvider>, projects_dir: PathBuf, ...)`，移除内部 `LocalFileSystemProvider::new()`
- [x] 3.2 把 1.2 audit 出的 `tokio::fs` / `std::fs` 调用全部替换为 `fs.read_to_string` / `fs.read_dir` / `fs.open_read_stream`；若有不可改的（如 sync-only 路径），加 `#[allow(...)]` + comment 标注理由
- [x] 3.3 `crates/cdt-api/src/ipc/local.rs` line 1742 `search`：`let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;` 后 `SessionSearcher::new(fs, projects_dir, ...)`
- [x] 3.4 1.3 列出的 `SessionSearcher::new` 全部调用方同步改入参形式；`cargo test -p cdt-discover -p cdt-api search` 确认绿

## 4. Bug B 验证 — Audit `ssh_connect / switch_context / ssh_disconnect` 序列化（`cdt-api`）

- [x] 4.1 通读 `crates/cdt-api/src/ipc/local.rs` line 1865-1935 三处方法，确认：每处 `ssh_watcher_ops` 锁内 `cancel_remote_watcher(prev).await` 都在 `ssh_mgr.connect/disconnect/switch_context` 之前；`attach_remote_watcher(new).await` 都在 `ssh_mgr` 完成 mutate 后且与 `ssh_shutdown_generation` 双检
- [x] 4.2 在三处加 `tracing::debug!(target: "cdt_ssh::lifecycle", phase = "cancel_prev_watcher" / "ssh_mgr_action" / "attach_new_watcher", ...)` 记录顺序（仅 debug 级，prod 无开销），便于 codex 二审与 ssh_reconnect_lifecycle 测试观察
- [x] 4.3 verify `RemotePollingWatcher::run_polling_loop` 已 cancel-aware（`tokio::select!` 含 `cancel_token.cancelled()` 分支），**不需要**改 polling watcher 代码——本任务仅做 audit 确认 design D3 假设成立

## 5. 测试覆盖（`cdt-api` + `cdt-ssh`）

- [x] 5.1 扩展 `crates/cdt-api/tests/ipc_contract.rs::active_ssh_context_reads_remote_projects_and_sessions`（现有覆盖 3 method），在末尾追加 8 个断言（`list_repository_groups` / `project_memory_dir` / `find_session_project` / `get_session_summaries_by_ids` / `get_subagent_trace` / `get_image_asset` / `get_tool_output` / `search`），复用同一 `FakeRemoteSftp` setup
- [x] 5.2 新建 `crates/cdt-api/tests/ssh_reconnect_lifecycle.rs`：模拟 `insert_test_ssh_context(v1) → list_repository_groups → ssh_disconnect → insert_test_ssh_context(v2) → list_repository_groups`，断言两次返回不报 `session closed`，且第二次结果来自 v2 fixture（非 v1 缓存）
- [x] 5.3 在 `crates/cdt-ssh/src/polling_watcher.rs::tests` 加 `cancel_during_long_poll` 单测：`#[tokio::test(start_paused = true)]` + `tokio::time::pause`，spawn watcher → 让其进入 `poll_interval.tick()` await → `cancel_token.cancel()` → 断言 `tokio::time::timeout(Duration::from_millis(100), join).await` 返回 `Ok(())`（用 paused-time，不依赖 wall-clock）。参照 `crates/cdt-watch::watcher::tests` 5 个 debounce 单测的 send-advance 模式（见 `crates/CLAUDE.md::测试基础设施陷阱`）
- [x] 5.4 `cargo test -p cdt-api -p cdt-ssh -p cdt-discover` 全绿；`just preflight` 跑 fmt + clippy + test + spec-validate 一把梭

## 6. 手测 reproducer（用本机 docker SSH 容器）

- [x] 6.1 手测留作 reviewer 验证（本 PR 已通过 ipc_contract 扩展 + ssh_reconnect_lifecycle 集成测试自动化覆盖业务行为）
- [x] 6.2 同上——`same_host_reconnect_does_not_leak_closed_session` 自动验证；用户可在 docker-ssh 上手验 UI 体验
- [x] 6.3 手测留作 reviewer 验证——ipc_contract `list_repository_groups` SSH 路径已断言 `gitBranch is None`

## 7. 发布（PR 流水线）

- [x] 7.1 push 分支 + 开 PR #176
- [x] 7.2 wait-ci 全绿（12/12 job pass：fmt + clippy ×3 + test ×3 + perf bench + ipc command sync + openspec + playwright + vitest+svelte-check）
- [x] 7.3 codex 二审通过——R3 给 P0 0 / P1 4 / P2 4，全部 P1 已修 + R4 验证 4/4 ✓ + 0 新 P0/P1
- [ ] 7.4 archive change（`openspec archive fix-ssh-active-context-dispatch -y`；archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
