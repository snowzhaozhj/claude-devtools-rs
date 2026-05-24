## MODIFIED Requirements

### Requirement: Watch remote project directories via SFTP polling

系统 SHALL 在 SSH context 处于 `connected` 状态时启动远端文件变更感知 polling watcher：每 3 秒调用一轮 SFTP `read_dir(<remote_home>/.claude/projects/<project_id>/)` + 对每个 `.jsonl` 文件 `stat` 取 size 与 mtime，与上轮 baseline 比较差异（新增 / size 变化 / 删除）后通过与本地 watcher 相同的 `FileChangeEvent` schema 广播事件。第一次 poll SHALL 不触发任何事件（建 baseline 用）。系统 SHALL 额外每 30 秒运行一次 catch-up 比较作为兜底。SHALL 在 `ssh_disconnect` 时停止 watcher 与释放 SFTP 资源。

watcher SHALL 把每轮 poll 失败按错误特征分到三类（在 polling 层 `with_retry` 之后做语义升级，**不**改 `cdt-ssh::SftpClient` trait 错误分类）：

- `Permanent`：错误消息含 `session closed` / `eof` / `broken pipe` / `epipe` / `connection reset` / `econnreset` 任一关键字（不区分 `Other` / `Transient` 来源——`provider::is_transient_io_reason` 把 `broken pipe` / `connection reset` / `epipe` 归 Transient，`with_retry` 3 次后仍是 transport-dead 即视同 channel 真死）
- `Timeout`：错误消息含 `timeout` / `etimedout` / `timed out` / `eagain` / `would block` 任一关键字（来自 `provider::is_transient_io_reason` 列表减去 transport-dead 子集；含 `would block` 即 `std::io::ErrorKind::WouldBlock`，与 EAGAIN 同源——不纳入 timeout 类会让"反复 WouldBlock"序列只能落 OtherTransient 重置计数，与 timeout 漏检对称）
- `OtherTransient`：其它 `Transient` / `Other` / `NoSuchFile` / `PermissionDenied`（含 `Status::Failure` 的 `error_message` 等不带 transport-dead / timeout 关键字的失败）

watcher SHALL 维护两个独立 counter：

- `consecutive_permanent: u32`，阈值 `PERMANENT_FAILURE_THRESHOLD = 3`（≈ 9s 持续 transport 错误）
- `consecutive_timeout: u32`，阈值 `TIMEOUT_FAILURE_THRESHOLD = 6`（≈ 18s 持续 timeout，远高于网络瞬时抖动 1-3s window，远低于用户感知 sidebar 僵死的 60s）

counter 演化规则（codex 二审收紧 reset 规则，避免攻击序列推迟 dead_signal）：

- `Ok` / `OtherTransient`：两 counter 都 SHALL reset 为 0（唯一 reset 入口；只有"channel 真活着"的强证据才清零）
- `Permanent`：仅 `consecutive_permanent += 1`，**不动** `consecutive_timeout`
- `Timeout`：仅 `consecutive_timeout += 1`，**不动** `consecutive_permanent`

任一 counter ≥ 自己阈值时，watcher SHALL `dead_signal.notify_one()` + 跳出主 loop。

理由：早期"互斥重置"规则被 `5T → 1P → 5T → 1P → ...` 攻击序列利用让 timeout 永不达 6；新规则下 dead-向量单调累积，攻击序列只能拖延无法阻止——`5T + 1P` 后下一轮 `1T` 即触发（`timeout=6 ≥ 6`）。

`scan_once` 内 sub-project 子目录 `read_dir` 失败时：

- `NoSuchFile` / `PermissionDenied`：silent skip 该 project（保持现有容错）
- 其它错误经 `classify_failure` 分类——`Permanent` SHALL 让整个 `scan_once` 返 `Err(SftpClientError::*)` escalate 到顶层 counter（避免 sub-project channel-dead 错误被静默吞掉、watcher 误以为 baseline 完整后下轮报"全部 session deleted"事件）；`Timeout` / `OtherTransient` 仍 silent skip 该 project，留下次 catch-up 重试

#### Scenario: First poll establishes baseline without events

- **WHEN** SSH context 刚切到 `connected` 状态，watcher 启动后第一次 poll
- **AND** 远端项目目录有 5 个 session JSONL 文件
- **THEN** watcher SHALL NOT emit 任何 `FileChangeEvent`
- **AND** 内部 baseline `BTreeMap<PathBuf, FileFingerprint>` SHALL 含 5 个条目

#### Scenario: Subsequent poll detects size change

- **WHEN** 第二次 poll 中某文件 size 从 1024 增长到 2048
- **THEN** watcher SHALL emit 一条 `FileChangeEvent { project_id, session_id, deleted: false }`
- **AND** baseline 中该文件 fingerprint SHALL 被更新

#### Scenario: Polling stops on disconnect

- **WHEN** 用户调 `ssh_disconnect`
- **THEN** 该 context 的 polling task SHALL 在 1s 内退出（cancellation token）
- **AND** SFTP channel SHALL 被关闭

#### Scenario: Watcher tolerates short transient SFTP errors below threshold

- **WHEN** 某轮 poll 中 `read_dir` 返回瞬时 timeout 错误（`Transient("timeout")`）
- **AND** `consecutive_timeout` 累计 < `TIMEOUT_FAILURE_THRESHOLD = 6`
- **THEN** watcher SHALL 跳过本轮，下一轮（3s 后）再尝试
- **AND** SHALL NOT 因单次失败而停止 watcher 或断开 SSH
- **AND** SHALL NOT 触发 `dead_signal`

#### Scenario: Sustained timeout triggers dead_signal at 6 consecutive

- **WHEN** SFTP `read_dir` 连续 6 轮 poll 都返 `Transient("timeout")` 类错误（典型场景：远端 `pkill -STOP sshd` 导致 SFTP 协议层 hang 但 TCP 未断）
- **THEN** watcher SHALL 在第 6 轮后 `dead_signal.notify_one()` 并跳出主 loop
- **AND** 触发 `LocalDataApi` monitor task 走 `perform_polling_self_heal_disconnect` 把 active context 切回 `Local`
- **AND** wall time SHALL ≈ 18s（6 × `POLL_INTERVAL=3s`），远低于 issue #231 报告的"用户走死 SFTP 等 30s curl timeout 才放弃"

#### Scenario: Permanent transport error triggers dead_signal at 3 consecutive

- **WHEN** SFTP `read_dir` 连续 3 轮 poll 都返含 `session closed` / `broken pipe` / `connection reset` 等 transport-dead 关键字的错误（无论来源是 `SftpClientError::Other` 还是 `Transient`）
- **THEN** watcher SHALL 在第 3 轮后 `dead_signal.notify_one()` 并跳出主 loop
- **AND** wall time SHALL ≈ 9s（3 × `POLL_INTERVAL`）

#### Scenario: Timeout counter resets on intervening success

- **WHEN** 5 轮 timeout（`consecutive_timeout = 5`）后下一轮 `read_dir` 成功
- **THEN** `consecutive_timeout` SHALL 立即 reset 为 0
- **AND** 后续即使再来 5 轮 timeout 也 SHALL NOT 触发 `dead_signal`（因为新 streak < 6）

#### Scenario: Permanent and timeout counters accumulate independently (mixed sequence still triggers)

- **WHEN** 5 轮 timeout 后来 1 轮 permanent
- **THEN** `consecutive_timeout` SHALL = 5（不被 permanent 重置）；`consecutive_permanent` SHALL = 1
- **AND** 下一轮 timeout SHALL 让 `consecutive_timeout = 6 ≥ TIMEOUT_FAILURE_THRESHOLD` → 立即触发 `dead_signal`
- **AND** 反向同理：2 轮 permanent + 1 轮 timeout 后 `consecutive_permanent = 2`、`consecutive_timeout = 1`；下一轮 permanent 让 `consecutive_permanent = 3 ≥ PERMANENT_FAILURE_THRESHOLD` → 触发
- **AND** 攻击序列 `5T → 1P → 5T → 1P → ...` SHALL **不能**永远推迟 dead_signal——任一 dead 向量单调累积

#### Scenario: OtherTransient errors do not trigger dead_signal

- **WHEN** 连续 10 轮 poll 返 `Transient("EAGAIN")` 等不含 transport-dead 与 timeout 关键字的错误
- **THEN** 两 counter 都 SHALL reset 为 0（`OtherTransient` 不计任一计数）
- **AND** SHALL NOT 触发 `dead_signal`
- **AND** watcher SHALL 持续运行等下一轮恢复

#### Scenario: Sub-project read_dir permanent error escalates to scan_once failure

- **WHEN** 顶层 `read_dir(<remote_home>/.claude/projects/)` 成功，但其中一个 sub-project `read_dir(<base>/<project_id>/)` 返 `Other("session closed")` 永久错误
- **THEN** `scan_once` SHALL 立即 return `Err(SftpClientError::Other(...))` 而非 silent skip 该 project
- **AND** 外层 polling loop 经 `classify_failure` 把该错误归 `Permanent` → `consecutive_permanent += 1`
- **AND** 连续 3 轮 sub-project permanent 错误 SHALL 触发 `dead_signal`

#### Scenario: Sub-project read_dir timeout / NoSuchFile silent skip 仍保留

- **WHEN** 顶层 `read_dir` 成功，sub-project A 返 `NoSuchFile`，sub-project B 返 `Transient("timeout")`，sub-project C 成功
- **THEN** `scan_once` SHALL 跳过 A 与 B，处理 C 后正常返 `Ok(BTreeMap)`
- **AND** baseline 仅含 C 的条目（A / B 缺失视同未变更，下轮 catch-up 自然重试）
- **AND** 不 escalate 任何错误到外层 counter
