## 1. `cdt-fs::FsError::is_likely_channel_dead`

- [ ] 1.1 在 `crates/cdt-fs/src/error.rs::FsError` impl 块加 `is_likely_channel_dead(&self) -> bool` 方法（按 design D4 规则：`Disconnected` 恒 true / `TransientExhausted` 含 transport-dead 关键字 / `Io` source kind 是 `BrokenPipe`/`ConnectionReset`/`ConnectionAborted`）
- [ ] 1.2 加单测 `is_likely_channel_dead_classifies_disconnected` / `is_likely_channel_dead_classifies_transient_exhausted_with_transport_dead_keyword` / `is_likely_channel_dead_pure_timeout_returns_false` / `is_likely_channel_dead_io_kinds` / `is_likely_channel_dead_notfound_utf8_unsupported_returns_false` 覆盖 spec delta 五个 Scenario
- [ ] 1.3 跑 `cargo test -p cdt-fs --lib` 确保新测试通过 + 老测试不破

## 2. `cdt-ssh::polling_watcher` 三态分类 + 双 counter

- [ ] 2.1 在 `crates/cdt-ssh/src/polling_watcher.rs` 加 `PollFailureKind` enum（`Permanent` / `Timeout` / `OtherTransient`）+ `classify_failure(&SftpClientError) -> PollFailureKind` 私有 fn（按 design D1 关键字清单）
- [ ] 2.2 把 `PollOutcome` 的 `Transient` 拆为 `Timeout` / `OtherTransient`（保留 `Ok` / `Permanent`），更新 `run_one_pass` 返回值映射
- [ ] 2.3 加 `pub const TIMEOUT_FAILURE_THRESHOLD: u32 = 6;`（`PERMANENT_FAILURE_THRESHOLD` 不动，注释引用 issue #231 + design.md D2）
- [ ] 2.4 重写 `update_permanent_counter` 为 `update_counters(outcome, &mut consecutive_permanent, &mut consecutive_timeout)` 按 design D2 规则演化两个 counter；返 `bool` 表示"任一计数已达阈值"
- [ ] 2.5 在 `run_polling_loop` 同步引入 `consecutive_timeout: u32 = 0` 局部变量；poll 与 catch-up 两条 tick 分支都调 `update_counters` + 任一阈值达到即 `dead_signal.notify_one()` 退出（注意 `notify_one` 用法与现有保持一致，写好两处的 `tracing::warn!` 含计数器命名让 ops 可定位）
- [ ] 2.6 把 `is_permanent_sftp_failure(err) -> bool` 改写为 `matches!(classify_failure(err), PollFailureKind::Permanent)` 一行代理（保留公共 fn 签名 backward-compat，design D5）
- [ ] 2.7 修订 eager baseline scan 的错误 bump 路径：`is_permanent_sftp_failure` 调用换为 `classify_failure` 三态分流（baseline 失败若是 Timeout 也累 `consecutive_timeout`，与正常 poll 一致）
- [ ] 2.8 修订 `scan_once` 内 sub-project read_dir 错误处理（line 411-432）：保留 `NoSuchFile` / `PermissionDenied` 的 silent skip；其它错误经 `classify_failure` 分流 —— `Permanent` SHALL `return Err(err)` escalate，`Timeout` / `OtherTransient` 仍 silent skip 该 project（design D3）

## 3. `cdt-ssh::polling_watcher` 单测扩展

- [ ] 3.1 加 `classify_failure_classifies_three_kinds` 覆盖 timeout / permanent / other 三态分类
- [ ] 3.2 加 `timeout_threshold_triggers_dead_signal_at_6_consecutive` —— `tokio::test(start_paused = true)` + 严格驱动顺序：每轮 `tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await; tokio::task::yield_now().await; tokio::task::yield_now().await;`（共 ×6 轮喂入 `Transient("timeout")` snapshot）后断言 `dead_signal.notified()` 在 100ms 内 ready + watcher join 立即返
- [ ] 3.3 加 `timeout_below_threshold_does_not_trigger` —— 5 轮 timeout（5 < 6）后 `dead.notified()` 50ms timeout 必须 Err
- [ ] 3.4 加 `timeout_counter_resets_on_intervening_success` —— 5 timeout + 1 ok + 5 timeout 不触发；同样按 advance + yield + yield 顺序驱动每轮
- [ ] 3.5 加 `mixed_permanent_timeout_sequence_still_triggers` —— 验证 codex 二审收紧的 reset 规则：5 timeout + 1 permanent + 1 timeout 序列 SHALL 触发（`consecutive_timeout=6` 在第 7 轮）；反向 2 permanent + 1 timeout + 1 permanent SHALL 触发
- [ ] 3.6 加 `subdir_permanent_error_escalates_scan_once` —— FakeSftpClient 顶层 read_dir 成功 + sub-project read_dir 返 `Other("session closed")`，断言外层 `consecutive_permanent` 累计；3 轮后触发 dead_signal
- [ ] 3.7 修订老测试 `transient_errors_do_not_trigger_dead_signal`：序列从 5 轮 timeout/eagain → 现在改为 5 轮纯 EAGAIN（OtherTransient）+ 注释更新引用 issue #231，确保不与新行为冲突；测试名重命名为 `pure_eagain_resets_counters_no_dead_signal`
- [ ] 3.8 跑 `cargo test -p cdt-ssh --lib polling_watcher` 全部通过

## 4. `cdt-discover::ProjectScanner` SSH 分支 channel-dead fail-fast

- [ ] 4.1 在 `crates/cdt-discover/src/project_scanner.rs::scan` SSH 分支（line 109-117）改写：`Err(DiscoverError::Fs(err))` 分流——`err.is_likely_channel_dead()` 为 true SHALL `tracing::error!` + `return Err(DiscoverError::Fs(err))` abort 整轮；其它仍 `tracing::warn!` + continue
- [ ] 4.2 加 fake SSH provider 测试 `ssh_channel_dead_aborts_scan` —— 注入 `FsError::Disconnected` 让第一个 sub-project scan_project_dir 返该错误，断言 scan 整体返 Err 且 all_projects 为空
- [ ] 4.3 加测试 `ssh_transient_exhausted_with_transport_dead_aborts_scan` —— `TransientExhausted { last_reason: "session closed" }` 同样 abort
- [ ] 4.4 加测试 `ssh_pure_timeout_does_not_abort` —— `TransientExhausted { last_reason: "timeout" }` 走 silent skip 路径，scan 返 Ok 含其它 sub-project
- [ ] 4.5 加测试 `ssh_notfound_does_not_abort` —— `FsError::NotFound` 走 silent skip 保留现有行为
- [ ] 4.6 跑 `cargo test -p cdt-discover --lib project_scanner` 全部通过

## 5. 集成 + 回归

- [ ] 5.1 跑 `just preflight`（fmt + clippy + test + spec-validate）一把梭
- [ ] 5.2 跑 `cargo test --workspace --release` 跨 crate 集成测试（cdt-api / cdt-ssh interplay）
- [ ] 5.3 在 PR 描述里记录"手工回归未跑（CI 不跑 docker），开发者后续按需调 `bash scripts/repro/repro-ssh-dead-channel.sh` 验 dead_signal 60s 内 fire"——CI 不能跑 docker fixture 不阻塞 merge

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
