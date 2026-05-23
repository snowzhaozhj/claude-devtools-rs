## Context

GitHub issue #231 报告两条独立但同根因的 SFTP 失效检测漏洞，根因都是"SFTP 单 channel 在 hang / 大量 read_dir 下半死，cdt 容错策略不主动重连"：

**现状关键代码**（`crates/cdt-ssh/src/polling_watcher.rs`）：
```rust
fn is_permanent_sftp_failure(err: &SftpClientError) -> bool {
    let s = err.to_string().to_ascii_lowercase();
    s.contains("session closed") || s.contains("eof") || s.contains("broken pipe")
        || s.contains("epipe") || s.contains("connection reset") || s.contains("econnreset")
}
```

`Transient("timeout")` 字符串渲染为 `transient sftp error: timeout`，上述 6 个关键字一个都不命中。复现日志（`pkill -STOP sshd` 60s）：
```
polling scan failed (skipping this round) error=transient sftp error: timeout permanent=false
```

watcher 6 轮后仍 `consecutive_permanent=0`，永不达 `PERMANENT_FAILURE_THRESHOLD=3`。

**第二条路径**（`scan_once` line 423-431）—— sub-project read_dir 失败时 `Other(reason)` 也只 warn + continue，不向上 escalate。

**第三条路径**（`crates/cdt-discover/src/project_scanner.rs::scan` line 109-117）—— SSH 模式 `for dir_name in dirs { match self.scan_project_dir... { Err(err) => warn + continue } }`，无论是单文件 IO error 还是 channel 全死，行为一致：silent skip 凑半成品列表返给 IPC caller。

**约束**：
- `cdt-discover` 不依赖 `cdt-ssh`（crate 边界硬约束），失效检测不能直接 `use cdt_ssh::SftpClientError`
- `FsError` 已是跨后端抽象，应通过它表达"channel 可能死了"语义
- 现有测试 `transient_errors_do_not_trigger_dead_signal`（line 1097-1132）显式断言纯 timeout 不应触发 dead_signal——本 change 的解法 SHALL 与此意图兼容（瞬时 timeout 仍不该立即 fire dead，只有"持续"timeout 才触发）

## Goals / Non-Goals

**Goals:**
- `polling_watcher` 在 `pkill -STOP sshd` 60s 后**SHALL**触发 `dead_signal` 让 active context 自愈切回 Local（issue 主诉求）
- `project_scanner` SSH 分支拿到 channel-dead 错误时**SHALL** abort 整轮 scan 而非凑半成品列表
- 现有 `transient_errors_do_not_trigger_dead_signal` 测试意图保留：纯瞬时 timeout（< 阈值轮）不该误触发 dead_signal
- 复用 `scripts/repro/repro-ssh-dead-channel.sh` 作为手工回归（CI 不跑 docker）；单元测试用 `FakeSftpClient` 覆盖 timeout 累计 + 子目录 escalate 两条路径

**Non-Goals:**
- 不实现"自动重连"逻辑——dead_signal 触发后由 `LocalDataApi::perform_polling_self_heal_disconnect` 走 disconnect 路径，用户手动 reconnect（保留现有自愈契约不动）
- 不改 IPC `list_repository_groups` 的返回类型（不引入 `partial: bool` 字段）——issue 列的"错误传播到 IPC partial"作为后续独立 change（design.md 仅记录此决策不在本 change 范围）
- 不评估"SFTP channel 复用上限 + post-recover 重连策略"（issue 第 4 点）——同样作为后续独立 change
- 不改 `cdt-ssh::SftpClient` trait 错误分类（保持 `Transient/Other/NoSuchFile/PermissionDenied` 四态）——polling 层在 with_retry 后做语义升级即可

## Decisions

### D1：把 `is_permanent_sftp_failure -> bool` 重构为 `classify_failure -> PollFailureKind` 三态

**决策**：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PollFailureKind {
    /// 永久 transport 错误（session closed / eof / broken pipe / epipe /
    /// connection reset / econnreset 任一关键字命中）—— 累计 PERMANENT counter
    Permanent,
    /// 纯 timeout / etimedout / timed out / eagain 类瞬时——累计独立 TIMEOUT
    /// counter（高阈值），不触发 PERMANENT counter
    Timeout,
    /// 其它非 retryable 终态（NoSuchFile / PermissionDenied / Unsupported）
    /// 或 with_retry 兜底返的非 transport-dead Transient——本轮 silent skip
    /// 不计任何 counter
    OtherTransient,
}

fn classify_failure(err: &SftpClientError) -> PollFailureKind { ... }
```

`is_permanent_sftp_failure` 作为 backward-compat shim 调 `matches!(classify_failure(err), PollFailureKind::Permanent)` 保留——现有公共 fn 签名不动让 backward 测试不破。

**为什么不直接加第二个 bool helper**：单一函数 + enum 比 `is_permanent` + `is_timeout` 两个 bool 更难写出"两个都返 true"的歧义；switch on enum 让外层 match 完整覆盖 + clippy 拦未来漏 case。

**关键字选择**：`Timeout` 命中 `timeout` / `etimedout` / `timed out` / `eagain`（来自 `provider.rs::is_transient_io_reason` 的瞬时清单减去 transport-dead 子集）；其它 Transient 字符串（如 `Status::Failure` 的 `error_message`）走 OtherTransient。

### D2：独立 `consecutive_timeout` counter + `TIMEOUT_FAILURE_THRESHOLD=6`

**决策**：

```rust
pub const PERMANENT_FAILURE_THRESHOLD: u32 = 3;     // 不动
pub const TIMEOUT_FAILURE_THRESHOLD: u32 = 6;       // 新增

let mut consecutive_permanent: u32 = 0;
let mut consecutive_timeout: u32 = 0;

fn update_counters(outcome: PollOutcome, perm: &mut u32, timeout: &mut u32) {
    match outcome {
        PollOutcome::Ok | PollOutcome::OtherTransient => {
            *perm = 0;
            *timeout = 0;
        }
        PollOutcome::Permanent => { *perm = perm.saturating_add(1); *timeout = 0; }
        PollOutcome::Timeout => { *timeout = timeout.saturating_add(1); *perm = 0; }
    }
    // 任一计数达阈值 → caller 触发 dead_signal
}
```

阈值 **6 = 6 × 3s ≈ 18s** 持续 timeout 才视同 channel 已死，远高于网络抖动典型 1-3s window，但远低于"用户感知 sidebar 已僵死"的 60s。issue 提议 6 次（≈ 18s）—— 与 issue 一致，避开过度调优。

**为什么不复用同 counter**：现有测试 `transient_errors_do_not_trigger_dead_signal` 显式断言"纯 timeout 5 轮不触发"——若把 timeout 计入同 counter（哪怕阈值高），测试要重写且语义变模糊（"timeout 当 permanent 算"）。独立 counter 让"timeout 单独高阈值"语义清晰可读。

**为什么 reset 规则是"任一非该类成功 / OtherTransient → 两 counter 都 reset"**：与现有 `Ok / Transient → reset; Permanent → bump` 行为对齐——单次 OK 即视为 channel 活着，不该让旧的 timeout 半累计状态影响后续判定。但 Permanent 出现时 timeout reset（避免"3 timeout + 1 permanent + 3 timeout = dead"误判，应该是 permanent 自走 permanent 路径）；Timeout 出现时 permanent reset（同理）。

### D3：`scan_once` 子目录 read_dir 永久错误 escalate 到顶层错误

**决策**：

```rust
// scan_once 内 line 411-432 区域改写：
let session_entries = match client.read_dir(&proj_dir_str).await {
    Ok(entries) => entries,
    Err(SftpClientError::NoSuchFile | SftpClientError::PermissionDenied) => continue,
    Err(err) => {
        match classify_failure(&err) {
            PollFailureKind::Permanent => {
                // sub-project read_dir 报永久错误 → 整个 scan_once 视为永久失败
                tracing::warn!(
                    target: "cdt_watch::ssh_polling",
                    project = %proj.name,
                    error = %err,
                    "permanent sftp error reading project dir; escalating scan_once",
                );
                return Err(err);
            }
            PollFailureKind::Timeout | PollFailureKind::OtherTransient => {
                tracing::warn!(
                    target: "cdt_watch::ssh_polling",
                    project = %proj.name,
                    error = %err,
                    "transient sftp error reading project dir; skipping",
                );
                continue;
            }
        }
    }
};
```

**理由**：sub-project read_dir 报 `session closed` 已是 channel 半死的强信号，silent continue 让 watcher 误判"baseline 完整 + 仅丢一两个 project"——下轮 baseline diff 会误报"消失的 project / session 全部 deleted"事件。escalate 让 counter 累计 + dead_signal 自然触发，与顶层 read_dir 失败语义统一。

**为什么子目录的 timeout 不 escalate**：单 project 的临时 timeout（典型大目录 readdir 跨包 RTT 抖动）误杀面太大；保持 silent skip 该 project 让其它 project 仍可见，留下次 catch-up scan 重试。

### D4：`FsError::is_likely_channel_dead` 元方法 + scanner SSH 分支 fail-fast

**决策**：在 `cdt-fs::FsError` 加：

```rust
impl FsError {
    /// 该错误是否暗示底层 transport channel 已死 / 半死。
    /// caller（典型 ProjectScanner SSH 分支）SHALL 据此 fail-fast 而非凑半
    /// 成品列表，让上层（list_repository_groups → IPC caller）拿到 hard
    /// error 触发自愈路径而非误以为 scan 已完成。
    #[must_use]
    pub fn is_likely_channel_dead(&self) -> bool {
        match self {
            FsError::Disconnected { .. } => true,
            FsError::TransientExhausted { last_reason, .. } => {
                let s = last_reason.to_ascii_lowercase();
                s.contains("session closed") || s.contains("eof")
                    || s.contains("broken pipe") || s.contains("epipe")
                    || s.contains("connection reset") || s.contains("econnreset")
            }
            FsError::Io { source, .. } => matches!(
                source.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
            ),
            FsError::NotFound(_) | FsError::Utf8 { .. } | FsError::Unsupported(_) => false,
        }
    }
}
```

`project_scanner.rs::scan` SSH 分支：

```rust
if self.fs.kind() == FsKind::Ssh {
    for dir_name in dirs {
        match self.scan_project_dir(&dir_name).await {
            Ok(projects) => all_projects.extend(projects),
            Err(DiscoverError::Fs(err)) if err.is_likely_channel_dead() => {
                tracing::error!(
                    dir = %dir_name,
                    error = %err,
                    "ssh channel appears dead; aborting full scan to surface error",
                );
                return Err(DiscoverError::Fs(err));
            }
            Err(err) => {
                tracing::warn!(dir = %dir_name, error = ?err, "skip unreadable project dir");
            }
        }
    }
}
```

**为什么单独加 `is_likely_channel_dead` 而不复用 `is_retryable`**：`is_retryable` 已被 `Disconnected` 用作 `true`，但 `TransientExhausted` 是 `false`（"已经重试过了"）；`is_retryable` 的语义是"该不该再 retry"，与"该不该 fail-fast"不同。一个文件 NotFound 也不 retry 但不该 abort scan；语义不能复用。

**为什么 timeout 不命中 `is_likely_channel_dead`**：scanner 与 watcher 节奏不同——scanner 是用户触发的 IPC（一次性），timeout 在 with_retry 3 次后仍 timeout（即 `TransientExhausted { last_reason: "timeout" }`）反而是"远端短暂不可达"，让 scanner abort 给用户报"扫描失败"比 silent skip 更刺眼。但本 change 保守只命中 transport-dead 关键字，纯 timeout 仍走 silent skip 路径——保留现有行为（issue 第 4 点的"自动重连"留 follow-up，那里再决定 timeout escalate 与否）。

### D5：保留 `is_permanent_sftp_failure` 公共签名 backward-compat

`is_permanent_sftp_failure(err) -> bool` 是 `pub fn`（line 242 没 `pub`，实际是 mod-private），但 mod 内单测和外部 mod 引用都假定它存在。本 change 保留它作为 `matches!(classify_failure(err), PollFailureKind::Permanent)` 的 alias——避免一次 PR 同时改公有 fn 名 + 行为，让 review diff 集中在新逻辑。

### D6：单测策略

新增 `polling_watcher.rs::tests` 覆盖：
1. `classify_failure_classifies_timeout_as_separate_kind` —— 单测三态分类
2. `timeout_threshold_triggers_dead_signal_at_6_consecutive` —— 6 轮 `Transient("timeout")` 后 dead_signal fire
3. `timeout_counter_resets_on_intervening_success` —— 5 timeout + 1 ok + 5 timeout 不触发
4. `timeout_below_threshold_does_not_trigger` —— 5 timeout 不触发（**替换**老 `transient_errors_do_not_trigger_dead_signal` 的扩展形态：保留 EAGAIN 不触发；timeout 部分行为变更，从"never trigger"改为"need 6+"）
5. `permanent_in_subdir_escalates_scan_once` —— 子目录 `Other("session closed")` 让外层 scan 算 permanent 一次

`cdt-fs::error::tests` 加：
- `is_likely_channel_dead_classifies_disconnected_and_dead_keywords`

`cdt-discover::project_scanner::tests` 加：
- `ssh_channel_dead_aborts_scan` —— 用 fake SSH provider 注入 `FsError::Disconnected` 第一个 project 即 abort，all_projects 空 + Err 返回

### D7：`is_permanent_sftp_failure` 现有测试意图变更点说明

现有测试 `transient_errors_do_not_trigger_dead_signal`（line 1097-1132）输入：
```rust
Err(SftpClientError::Transient("ETIMEDOUT".into())),
Err(SftpClientError::Transient("timeout".into())),
Err(SftpClientError::Transient("EAGAIN".into())),
Err(SftpClientError::Transient("ETIMEDOUT".into())),
Err(SftpClientError::Transient("ETIMEDOUT".into())),
```
共 5 轮（baseline + 4 poll tick）。新行为下：
- 4 个 ETIMEDOUT/timeout 计入 `consecutive_timeout`（baseline 也命中即 5 轮 timeout 中 4 是 ETIMEDOUT/timeout / 1 是 EAGAIN）
- EAGAIN 走哪类？`is_transient_io_reason` 列表含 EAGAIN 但不含 transport-dead 关键字——按 D1 分类，EAGAIN 走 OtherTransient，**reset** 两个 counter
- 序列：T(1) T(2) E(reset) T(1) T(2) → 最终 timeout=2 < 6，**仍**不触发——与现有断言兼容

修订后测试名改为 `pure_eagain_resets_timeout_counter` 更精确表达意图；额外加新测试覆盖"6 个连续 timeout 触发"。

## Risks / Trade-offs

- **[误杀]：18s timeout 阈值仍可能在远端慢盘 readdir 时误杀**
  → Mitigation：实测远端最坏 readdir RTT < 1s，6 × 3s = 18s 远超过单 readdir 最坏 case。若实践中误杀频发，调高 `TIMEOUT_FAILURE_THRESHOLD` 到 10（30s）即可，**不**需要改架构。

- **[行为变更]：现有 `transient_errors_do_not_trigger_dead_signal` 测试断言被弱化**
  → Mitigation：D7 列出修订点；测试名重命名 + 注释引用 issue #231 说明语义升级，让未来 reviewer 不困惑。

- **[scanner abort 让冷启动半失败更刺眼]**：之前用户连不稳定 SSH 时"sidebar 缺一两个 project"，现在变成"sidebar 完全空 + 错误提示"
  → Mitigation：channel-dead 错误本来就是用户该感知的故障；sidebar 半破坏反而误导用户以为"扫完了"。后续配合 list_repository_groups 加 partial 字段（follow-up change）让"非 channel-dead 的单 project 失败"仍能展示其它 project + 错误徽标，但本 change 不做。

- **[reset 规则争议]：Permanent 出现时为什么 reset timeout（而不累加）**
  → Mitigation：`update_counters` 文档注释明确写"Permanent / Timeout 互斥重置"决策；如果未来发现"timeout 后跟一个 permanent 应该让 dead 提前触发"再调，先按最简洁规则上线。

- **[backward-compat shim 增加维护负担]**
  → Mitigation：`is_permanent_sftp_failure` 仅 1 行（`matches!`），维护成本接近 0；后续清理可在独立 cleanup PR 做。

## Migration Plan

1. 实现 + 单测一次性 PR——无 feature flag / 无渐进 rollout（行为契约级修复，应立即生效）
2. 手工回归：跑 `bash scripts/repro/repro-ssh-dead-channel.sh` 在 docker `cdt-ssh-test` 容器上验 dead_signal 60s 内 fire
3. release notes 引用 issue #231 让用户知道"sshd hang 时 cdt 会自动切回 Local"行为变化
4. 无 rollback 计划——若发现误杀，bump `TIMEOUT_FAILURE_THRESHOLD` 而非整体回退

## Open Questions

无。issue 列的"评估 SFTP channel 复用上限 + post-recover 重连策略"明确不在本 change scope（D2 Non-Goals 已声明）。
