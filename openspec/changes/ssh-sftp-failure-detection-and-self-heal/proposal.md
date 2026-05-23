## Why

GitHub issue #231（2026-05-22 docker `cdt-ssh-test` `pkill -STOP sshd` 60s 复现）暴露 SFTP 失效检测两条独立漏洞：

1. **`polling_watcher.rs::is_permanent_sftp_failure` 不识别 timeout**：`cdt-ssh::provider::classify_sftp_error` 把 `SftpError::Timeout` 归到 `SftpClientError::Transient("timeout")`，错误字符串 `transient sftp error: timeout` 不含 `session closed` / `eof` / `broken pipe` / `epipe` / `connection reset` 任一关键字，**永远**不计入 `consecutive_permanent` counter，**永远**达不到 `PERMANENT_FAILURE_THRESHOLD=3`，**永远**不发 `dead_signal`。复现日志：6 次 `polling scan failed (skipping this round) error=transient sftp error: timeout permanent=false`，`active.kind` 始终 `ssh`，`list_sessions` 走死 SFTP 等 30s curl timeout 才放弃。

2. **`project_scanner.rs` SSH 分支 silent skip permanent error**：`scan_project_dir` 单 project read_dir 报 `session closed` 时只 `tracing::warn!` 后 continue，错误信号**不**传导到 `ssh_mgr` 也**不**返回到 IPC caller。`polling_watcher::scan_once` 顶层 read_dir 失败能进 counter，但子目录 read_dir 失败（line 423-431 路径）也是同样静默 continue——单 project 永久错误不会让外层 scan_once 返 permanent。

后果：用户 sidebar 看到不完整列表 + UI 表现"还在加载 SSH 数据"，自愈路径瘫痪到只能等用户手动 disconnect。修法已在 issue 提出方向：timeout 用独立高阈值 counter（避免误杀网络抖动）、scanner 主动 escalate permanent 错误、复用 `scripts/repro/repro-ssh-dead-channel.sh` 作回归 fixture。

## What Changes

- **`polling_watcher.rs`**：把 `is_permanent_sftp_failure -> bool` 重构为 `classify_failure(&SftpClientError) -> PollFailureKind`，返三态 `Permanent | Timeout | OtherTransient`；`PollOutcome` 同步加 `Timeout` 变体；`run_polling_loop` 维护**独立** `consecutive_timeout: u32` counter，阈值 `TIMEOUT_FAILURE_THRESHOLD=6`（≈ 18s 持续 timeout 视同 dead，避开瞬时抖动；`PERMANENT_FAILURE_THRESHOLD=3` 不动），任一 counter 达阈值同样触发 `dead_signal` + 退出 loop。
- **`polling_watcher.rs::scan_once`**：单 project 子目录 read_dir 失败时若 `classify_failure` 命中 `Permanent` SHALL 让整个 `scan_once` 返 `Err(SftpClientError::Other(...))`（让外层 counter 累计），**不再**对子目录 permanent 错误 silent continue；timeout / 其他 transient 仍 silent skip 该 project（保留容错）。
- **`cdt-fs::FsError`**：新增 `is_likely_channel_dead(&self) -> bool` 元方法——返 true 当：`Disconnected` 任意 / `TransientExhausted { last_reason }` 含 transport-dead 关键字（`session closed` / `eof` / `broken pipe` / `epipe` / `connection reset` / `econnreset`） / `Io { source.kind() }` 是 `BrokenPipe` / `ConnectionReset` / `ConnectionAborted`。仅作语义元方法，不改 `is_retryable` / `should_invalidate_cache` 行为。
- **`project_scanner.rs`**：SSH 分支（line 109-117）单 project scan 错误处理升级——`FsError::is_likely_channel_dead() == true` SHALL 立即 `return Err(DiscoverError::Fs(err))` abort 整轮 scan（让 `list_repository_groups` 拿到 hard error 触发上层自愈），**不再**对 channel-dead 错误 silent continue 凑半成品列表；其它 FsError（含普通 `NotFound` / 单文件 IO error）保留现有 `tracing::warn!` + 跳过该 project 行为。
- **`scripts/repro/repro-ssh-dead-channel.sh`**：保留作为回归 fixture，单测路径用 `FakeSftpClient` 模拟 `Transient("timeout")` 序列 + `Other("session closed")` 子目录失败序列覆盖 polling_watcher / scanner 双修法。

无 IPC 字段语义改动；无前端契约改动。

## Capabilities

### New Capabilities
（无）

### Modified Capabilities

- `ssh-remote-context`：把"SFTP 永久错误检测"从单一 `consecutive_permanent` counter 升级为分类 + 双 counter（permanent / timeout）；扩展 "Polling watcher 自愈触发" Requirement 描述 timeout 路径；新增 "Watcher 内子目录 permanent 错误 escalate 而非 silent continue" Scenario。
- `project-discovery`：在 `Scan Claude projects directory` Requirement 加 "SSH 模式下 channel-dead 错误 SHALL abort 整轮 scan 而非 silent skip" Scenario。
- `fs-abstraction`：在 `FsError 提供错误语义元方法` Requirement 加 `is_likely_channel_dead` 元方法 Scenario。

## Impact

**代码**：
- `crates/cdt-ssh/src/polling_watcher.rs`：核心修改（约 +60 / -20 行 + 新增 5 个测试）
- `crates/cdt-fs/src/error.rs`：加 `is_likely_channel_dead` + 单测（约 +30 行）
- `crates/cdt-discover/src/project_scanner.rs`：SSH 分支错误处理 + 集成测试（约 +20 行）
- `openspec/specs/ssh-remote-context/spec.md` / `project-discovery/spec.md` / `fs-abstraction/spec.md`：spec delta 同步

**性能**：无回归——失效检测路径只在错误时跑；常态零开销。

**外部依赖**：无新增。

**回归 fixture**：`scripts/repro/repro-ssh-dead-channel.sh`（手工 docker fixture，非 CI）。

**用户感知**：sshd hang 60s 后用户**SHALL**看到"自动切回 Local + 重连提示"而非"sidebar 残缺 + 30s curl timeout"。
