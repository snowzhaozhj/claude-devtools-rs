//! 远端 SFTP polling watcher（3s 间隔 + 30s catch-up timer）。
//!
//! Spec：`openspec/specs/ssh-remote-context/spec.md` `Requirement: Watch remote project
//! directories via SFTP polling`。
//!
//! 设计要点（design.md D5）：
//! - 主 polling 周期 [`POLL_INTERVAL`]=3s；catch-up [`CATCH_UP_INTERVAL`]=30s 兜底
//! - 指纹 = size + mtime（mtime 缺失时退化为 size-only 并 once-per-path debug 日志）
//! - 第一次 scan 仅建 baseline 不发事件
//! - 瞬时 SFTP 错误（含顶层 `read_dir`）跳过本轮，不停 watcher
//! - 取消信号 1s 内退出（[`CANCEL_DEADLINE`]）
//!
//! 通过 [`crate::SftpClient`] trait 访问远端——生产路径与
//! [`crate::SshFileSystemProvider`] 共享同一 `RusshSftpClient`，测试可注入 fake
//! 模拟 5 类差异。
//!
//! 通过 `broadcast::Sender<FileChangeEvent>` 把事件喂入与 `cdt-watch::FileWatcher`
//! 同一通道，下游订阅者无须感知 local vs ssh。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use cdt_core::FileChangeEvent;
use cdt_discover::EntryKind;
use tokio::sync::{Notify, broadcast};
use tokio::task::JoinHandle;
use tokio::time::{Instant, MissedTickBehavior, interval_at};

use crate::provider::{SftpClient, SftpClientError};

/// 远端文件指纹 baseline 条目：size + mtime（mtime 缺失时退化为 size-only）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFingerprint {
    pub size: u64,
    pub mtime: Option<SystemTime>,
}

/// 主 polling 周期（design.md D5）。
pub const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// catch-up 兜底周期（design.md D5）。
pub const CATCH_UP_INTERVAL: Duration = Duration::from_secs(30);

/// 取消 token 退出最大延迟（spec Scenario "Polling stops on disconnect"）。
pub const CANCEL_DEADLINE: Duration = Duration::from_secs(1);

/// 连续多少轮"永久性 SFTP 错误"后判定 SFTP channel 已死，触发 `dead_signal`。
/// 永久错误特征：错误消息含 `session closed` / `Eof` / `BrokenPipe` /
/// `Broken pipe` 等不可恢复字样。
///
/// 阈值取 3 = 让单次瞬时网络抖动不会误触发（瞬时已被 `SftpClientError::Transient`
/// 与 `with_retry` 内部消化）；3 × `POLL_INTERVAL` ≈ 9s 内连续报错才认为
/// channel 真的死了，调用方（cdt-api `attach_remote_watcher`）SHALL 借此信号
/// 触发 `ssh_mgr.disconnect`，避免 `active_context_id()` 与底层 SFTP liveness
/// 长期撒谎。
pub const PERMANENT_FAILURE_THRESHOLD: u32 = 3;

/// 连续多少轮"timeout 类瞬时错误"后判定 SFTP channel 已半死，触发 `dead_signal`。
///
/// 修 GitHub issue #231：`SftpError::Timeout` 经 `provider::classify_sftp_error`
/// 归 `SftpClientError::Transient("timeout")`，错误字符串不含 `session closed` 等
/// 永久关键字，永远不计 `consecutive_permanent`，永远不发 `dead_signal`。复现
/// 场景：`pkill -STOP sshd` 让 SFTP 协议层 hang 但 TCP 未断，watcher 6+ 轮
/// `transient sftp error: timeout permanent=false` 仍判 channel 活。
///
/// 阈值取 6 = 让短暂网络抖动（典型 1-3s）不误触发，但远低于"用户感知 sidebar
/// 已僵死"的 60s——6 × `POLL_INTERVAL` ≈ 18s 持续 timeout 视同 channel 真的
/// 死了。issue #231 提议同值。
pub const TIMEOUT_FAILURE_THRESHOLD: u32 = 6;

/// 取消令牌——多 owner 可 clone，调 [`CancelToken::cancel`] 通知所有
/// 等待 [`CancelToken::cancelled`] 的 future 立即退出。
///
/// 自实现而非引入 `tokio_util::sync::CancellationToken`：避免新增 workspace dep；
/// 仅 30 行 API，一次性事件 + 多 awaiter 的语义足够。
#[derive(Clone, Default, Debug)]
pub struct CancelToken {
    inner: Arc<CancelInner>,
}

#[derive(Default, Debug)]
struct CancelInner {
    flag: AtomicBool,
    notify: Notify,
}

impl CancelToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 触发取消——只有第一次调用真正广播；后续调用幂等。
    pub fn cancel(&self) {
        if !self.inner.flag.swap(true, Ordering::SeqCst) {
            self.inner.notify.notify_waiters();
        }
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.flag.load(Ordering::SeqCst)
    }

    /// 等待取消。已取消时立即 ready；未取消时注册 wake 等 [`cancel`] 触发。
    ///
    /// race-free：先注册 `notified` future（pin 后 enable），再检查 flag——
    /// 若取消发生在两者之间，`notify_waiters` 已唤醒已注册者；若取消发生
    /// 在 flag 检查后，未来 cancel 会唤醒此 future。
    ///
    /// [`cancel`]: Self::cancel
    pub async fn cancelled(&self) {
        loop {
            if self.is_cancelled() {
                return;
            }
            let notified = self.inner.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            notified.await;
            if self.is_cancelled() {
                return;
            }
        }
    }
}

/// `RemotePollingWatcher` 句柄——cancel + join 入口。
///
/// 持有 [`CancelToken`]（cancel signal）+ tokio task 的 [`JoinHandle`] +
/// [`Notify`] `dead_signal` —— connection manager 在 disconnect 时调 [`cancel`]
/// 通知 watcher 退出；watcher 内连续 [`PERMANENT_FAILURE_THRESHOLD`] 轮永久
/// 错误时会 `dead_signal.notify_one()`，调用方可订阅该信号触发自愈逻辑
/// （详 `openspec/followups.md` 的 SSH/SFTP keepalive 条目）。
///
/// [`cancel`]: Self::cancel
#[derive(Debug)]
pub struct RemoteWatcherHandle {
    cancel_token: CancelToken,
    join_handle: JoinHandle<()>,
    dead_signal: Arc<Notify>,
}

impl RemoteWatcherHandle {
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    /// 订阅 watcher 的 "SFTP channel 已死" 信号——连续
    /// [`PERMANENT_FAILURE_THRESHOLD`] 轮永久错误后由 watcher 内部 `notify_one`。
    /// 调用方（`cdt-api` `attach_remote_watcher`）在 `notified().await` 后 SHALL
    /// 触发 `ssh_mgr.disconnect(context_id)` 让 active context 同步翻转回 Local，
    /// 避免 `list_xxx` IPC 继续走死 SFTP channel 返空。
    #[must_use]
    pub fn dead_signal(&self) -> Arc<Notify> {
        Arc::clone(&self.dead_signal)
    }

    /// 取消 + 等待 task 退出（最长 [`CANCEL_DEADLINE`]）。超时调 abort 兜底。
    pub async fn cancel_and_join(self) {
        self.cancel_token.cancel();
        let mut join_handle = self.join_handle;
        if (tokio::time::timeout(CANCEL_DEADLINE, &mut join_handle).await).is_err() {
            tracing::warn!(
                target: "cdt_watch::ssh_polling",
                "remote polling watcher did not exit within {:?}; aborting",
                CANCEL_DEADLINE,
            );
            join_handle.abort();
            let _ = join_handle.await;
        }
    }

    /// 仅在测试 / 显式等待时使用——直接 await join 不带超时。
    pub async fn join(self) {
        let _ = self.join_handle.await;
    }
}

/// 远端 polling watcher——持有 client + `projects_root` + sender，spawn 一个
/// tokio task 跑 3s + 30s 双 timer 主循环。
pub struct RemotePollingWatcher;

impl RemotePollingWatcher {
    /// Spawn 一个 polling task，立即 eager scan 建 baseline（不发事件），
    /// 之后每 [`POLL_INTERVAL`] 跑一轮增量 diff，每 [`CATCH_UP_INTERVAL`]
    /// 跑一次兜底全量 scan + diff。返回的 [`RemoteWatcherHandle`] 由调用方
    /// 持有，disconnect 时 `cancel()`。
    ///
    /// 连续 [`PERMANENT_FAILURE_THRESHOLD`] 轮永久性 SFTP 错误后
    /// watcher 自身退出 + 触发 `handle.dead_signal()` 通知调用方（典型：
    /// cdt-api 的 monitor task → `ssh_mgr.disconnect`），避免在已死 SFTP
    /// channel 上空转浪费 RTT、active context 状态长期撒谎。
    pub fn spawn(
        client: Arc<dyn SftpClient>,
        projects_root: PathBuf,
        sender: broadcast::Sender<FileChangeEvent>,
        cancel_token: CancelToken,
    ) -> RemoteWatcherHandle {
        let cancel_for_handle = cancel_token.clone();
        let dead_signal = Arc::new(Notify::new());
        let dead_signal_for_loop = Arc::clone(&dead_signal);
        let join_handle = tokio::spawn(run_polling_loop(
            client,
            projects_root,
            sender,
            cancel_token,
            dead_signal_for_loop,
        ));
        RemoteWatcherHandle {
            cancel_token: cancel_for_handle,
            join_handle,
            dead_signal,
        }
    }
}

/// 单轮 polling 失败的语义分类——给 [`run_polling_loop`] 双 counter 演化用。
///
/// 修 GitHub issue #231：旧设计单一 `is_permanent_sftp_failure -> bool` 让纯
/// `Transient("timeout")` 永远不进 counter；新设计三态分类 + 独立 timeout
/// counter（高阈值 6 ≈ 18s）覆盖"sshd hang 但 TCP 未断"场景。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PollFailureKind {
    /// 永久 transport 错误：错误消息含 `session closed` / `eof` / `broken pipe` /
    /// `epipe` / `connection reset` / `econnreset` 任一关键字。不区分 `Other` 与
    /// `Transient` 来源——`provider::is_transient_io_reason` 把 `broken pipe` /
    /// `connection reset` / `epipe` 都归 `Transient`，`with_retry` 3 次后仍是
    /// transport-dead 即视同 channel 真死。
    Permanent,
    /// timeout 类瞬时错误：错误消息含 `timeout` / `etimedout` / `timed out` /
    /// `eagain` / `would block` 任一关键字。来自 `provider::is_transient_io_reason`
    /// 列表减去 transport-dead 子集；`would block` 即 `std::io::ErrorKind::WouldBlock`，
    /// 与 EAGAIN 同源——不纳入 timeout 类会让"反复 `WouldBlock`"序列只能落
    /// `OtherTransient` 重置计数永远不触发 `dead_signal`。
    Timeout,
    /// 其它非 channel-related 错误：`NoSuchFile` / `PermissionDenied` /
    /// `Status::Failure` 的 `error_message` 等不带 transport-dead / timeout
    /// 关键字的失败——本轮 silent skip，**reset** 两个 counter（视同 channel
    /// 真活着的强证据）。
    OtherTransient,
}

/// 单轮 polling 的结果——给 [`run_polling_loop`] 双 counter 演化用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PollOutcome {
    /// scan 成功（无论是否有事件 emit）—— reset 两个 counter。
    Ok,
    /// 永久错误（[`PollFailureKind::Permanent`]）—— bump `consecutive_permanent`，
    /// 达 [`PERMANENT_FAILURE_THRESHOLD`] 时触发 `dead_signal`。
    Permanent,
    /// timeout 类错误（[`PollFailureKind::Timeout`]）—— bump `consecutive_timeout`，
    /// 达 [`TIMEOUT_FAILURE_THRESHOLD`] 时触发 `dead_signal`。
    Timeout,
    /// 其它非 channel-related 错误（[`PollFailureKind::OtherTransient`]）——
    /// **reset** 两个 counter（与 `Ok` 等价，视同 channel 真活着的强证据）。
    OtherTransient,
}

/// 把 SFTP 错误分到 [`PollFailureKind`] 三态。
///
/// 关键字小写匹配，按以下优先级（同一字符串可能同时含 transport-dead 与 timeout
/// 关键字时，transport-dead 优先——permanent 比 timeout 更"严重"）：
///
/// 1. transport-dead 任一关键字 → [`PollFailureKind::Permanent`]
/// 2. timeout-class 任一关键字 → [`PollFailureKind::Timeout`]
/// 3. 其它（含 `NoSuchFile` / `PermissionDenied` / 不带关键字的 `Status::Failure`）
///    → [`PollFailureKind::OtherTransient`]
fn classify_failure(err: &SftpClientError) -> PollFailureKind {
    let s = err.to_string().to_ascii_lowercase();
    if s.contains("session closed")
        || s.contains("eof")
        || s.contains("broken pipe")
        || s.contains("epipe")
        || s.contains("connection reset")
        || s.contains("econnreset")
    {
        return PollFailureKind::Permanent;
    }
    if s.contains("timeout")
        || s.contains("etimedout")
        || s.contains("timed out")
        || s.contains("eagain")
        || s.contains("would block")
    {
        return PollFailureKind::Timeout;
    }
    PollFailureKind::OtherTransient
}

async fn run_polling_loop(
    client: Arc<dyn SftpClient>,
    projects_root: PathBuf,
    sender: broadcast::Sender<FileChangeEvent>,
    cancel_token: CancelToken,
    dead_signal: Arc<Notify>,
) {
    let mut warned_missing_mtime: BTreeSet<PathBuf> = BTreeSet::new();
    let mut consecutive_permanent: u32 = 0;
    let mut consecutive_timeout: u32 = 0;

    // Eager 第一次 scan 建 baseline——spec "First poll establishes baseline"。
    // 顶层 `read_dir` 失败时 baseline 取空（首次连接刚好远端 home 临时不可读时
    // 也不让 watcher 死掉；下一轮 catch-up 会再尝试）。baseline 失败也按
    // classify_failure 三态 bump 对应 counter—— baseline 一次跑不可能达阈值，
    // 但让"connect 后立刻 timeout"序列首轮就计入提前推进 dead 检测。
    let mut baseline = match scan_once(&client, &projects_root, &mut warned_missing_mtime).await {
        Ok(b) => b,
        Err(e) => {
            match classify_failure(&e) {
                PollFailureKind::Permanent => {
                    consecutive_permanent = consecutive_permanent.saturating_add(1);
                }
                PollFailureKind::Timeout => {
                    consecutive_timeout = consecutive_timeout.saturating_add(1);
                }
                PollFailureKind::OtherTransient => {
                    // baseline 失败若是非 channel-related，**不 reset** 已有 counter
                    // （baseline 阶段两 counter 都还是 0，reset 也是 no-op；保持
                    // 与 run_one_pass `OtherTransient` 同语义即可）
                }
            }
            tracing::warn!(
                target: "cdt_watch::ssh_polling",
                error = %e,
                "initial baseline scan failed; starting with empty baseline",
            );
            BTreeMap::new()
        }
    };

    let now = Instant::now();
    let mut poll_interval = interval_at(now + POLL_INTERVAL, POLL_INTERVAL);
    poll_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut catch_up_interval = interval_at(now + CATCH_UP_INTERVAL, CATCH_UP_INTERVAL);
    catch_up_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;
            () = cancel_token.cancelled() => break,
            _ = poll_interval.tick() => {
                let outcome = run_one_pass(&client, &projects_root, &sender, &mut baseline, &mut warned_missing_mtime, &cancel_token).await;
                if update_counters(outcome, &mut consecutive_permanent, &mut consecutive_timeout) {
                    tracing::warn!(
                        target: "cdt_watch::ssh_polling",
                        consecutive_permanent,
                        consecutive_timeout,
                        "SFTP channel appears dead; signaling dead_signal and exiting watcher",
                    );
                    // notify_one 存 permit——后注册的 monitor task 仍能消费，
                    // 与 notify_waiters 仅唤醒"已在等待"的 waiters 不同（后者
                    // 在 monitor 还没 .notified().await 时发出会丢失信号）。
                    dead_signal.notify_one();
                    break;
                }
            }
            _ = catch_up_interval.tick() => {
                // 30s catch-up 与 3s poll 算法相同——兜底 SFTP 偶发漏事件
                // （spec D5：catch-up 同样按 size + mtime 双维度比对）。
                let outcome = run_one_pass(&client, &projects_root, &sender, &mut baseline, &mut warned_missing_mtime, &cancel_token).await;
                if update_counters(outcome, &mut consecutive_permanent, &mut consecutive_timeout) {
                    tracing::warn!(
                        target: "cdt_watch::ssh_polling",
                        consecutive_permanent,
                        consecutive_timeout,
                        "SFTP channel appears dead; signaling dead_signal and exiting watcher",
                    );
                    dead_signal.notify_one();
                    break;
                }
            }
        }
    }
}

/// 按 [`PollOutcome`] 演化两个独立 counter，返 `true` 表示任一 counter 达
/// 自己阈值（caller 据此触发 `dead_signal` + 退 loop）。
///
/// 演化规则（codex 二审收紧 reset 规则，避免攻击序列推迟 `dead_signal`）：
/// - `Ok` / `OtherTransient`：两 counter 都 reset 为 0（唯一 reset 入口；
///   只有 channel 真活着的强证据才清零）
/// - `Permanent`：仅 `consecutive_permanent += 1`，**不动** `consecutive_timeout`
/// - `Timeout`：仅 `consecutive_timeout += 1`，**不动** `consecutive_permanent`
///
/// 早期"互斥重置"规则被 `5T → 1P → 5T → 1P → ...` 攻击序列利用让 timeout 永远
/// reset 不到 6；新规则下 dead-向量单调累积，攻击序列只能拖延无法阻止。
fn update_counters(
    outcome: PollOutcome,
    consecutive_permanent: &mut u32,
    consecutive_timeout: &mut u32,
) -> bool {
    match outcome {
        PollOutcome::Ok | PollOutcome::OtherTransient => {
            *consecutive_permanent = 0;
            *consecutive_timeout = 0;
        }
        PollOutcome::Permanent => {
            *consecutive_permanent = consecutive_permanent.saturating_add(1);
        }
        PollOutcome::Timeout => {
            *consecutive_timeout = consecutive_timeout.saturating_add(1);
        }
    }
    *consecutive_permanent >= PERMANENT_FAILURE_THRESHOLD
        || *consecutive_timeout >= TIMEOUT_FAILURE_THRESHOLD
}

/// 执行单轮 scan + diff + emit + baseline 更新；瞬时错误跳过本轮（不传播）。
/// 返回 [`PollOutcome`] 给上层 loop 累计永久错误次数判断 channel 是否已死。
async fn run_one_pass(
    client: &Arc<dyn SftpClient>,
    projects_root: &Path,
    sender: &broadcast::Sender<FileChangeEvent>,
    baseline: &mut BTreeMap<PathBuf, FileFingerprint>,
    warned_missing_mtime: &mut BTreeSet<PathBuf>,
    cancel_token: &CancelToken,
) -> PollOutcome {
    if cancel_token.is_cancelled() {
        return PollOutcome::Ok;
    }
    let current = match scan_once(client, projects_root, warned_missing_mtime).await {
        Ok(c) => c,
        Err(e) => {
            let kind = classify_failure(&e);
            tracing::warn!(
                target: "cdt_watch::ssh_polling",
                error = %e,
                kind = ?kind,
                "polling scan failed (skipping this round)",
            );
            return match kind {
                PollFailureKind::Permanent => PollOutcome::Permanent,
                PollFailureKind::Timeout => PollOutcome::Timeout,
                PollFailureKind::OtherTransient => PollOutcome::OtherTransient,
            };
        }
    };

    for (path, cur_fp) in &current {
        let changed = match baseline.get(path) {
            None => true,
            Some(old_fp) => old_fp.size != cur_fp.size || old_fp.mtime != cur_fp.mtime,
        };
        if changed {
            if let Some(event) = build_change_event(projects_root, path, false) {
                let _ = sender.send(event);
            }
        }
    }
    let removed: Vec<PathBuf> = baseline
        .keys()
        .filter(|p| !current.contains_key(*p))
        .cloned()
        .collect();
    for path in removed {
        if let Some(event) = build_change_event(projects_root, &path, true) {
            let _ = sender.send(event);
        }
    }

    *baseline = current;
    PollOutcome::Ok
}

/// 从 `<projects_root>` 跑一轮全量 scan：
/// 1. 顶层 `read_dir(projects_root)` 拿 `project_id` 目录列表
/// 2. 每个 `project_id` 目录 `read_dir` 拿 `.jsonl` 文件 + metadata
/// 3. 不递归进 `subagents/` 等更深目录——v1 仅扫主 session jsonl。
async fn scan_once(
    client: &Arc<dyn SftpClient>,
    projects_root: &Path,
    warned_missing_mtime: &mut BTreeSet<PathBuf>,
) -> Result<BTreeMap<PathBuf, FileFingerprint>, SftpClientError> {
    let mut current: BTreeMap<PathBuf, FileFingerprint> = BTreeMap::new();
    let projects_root_str = posix_path_str(projects_root);

    let project_entries = client.read_dir(&projects_root_str).await?;
    for proj in project_entries {
        if !matches!(proj.kind, EntryKind::Dir) {
            continue;
        }
        let proj_dir = projects_root.join(&proj.name);
        let proj_dir_str = posix_join(&projects_root_str, &proj.name);
        let session_entries = match client.read_dir(&proj_dir_str).await {
            Ok(entries) => entries,
            Err(SftpClientError::NoSuchFile | SftpClientError::PermissionDenied) => continue,
            Err(err) => {
                // sub-project read_dir 错误按 classify_failure 三态分流：
                // - Permanent → escalate 整个 scan_once 返 Err，让外层
                //   counter 累计 + dead_signal 触发自愈（修 issue #231 #2）
                // - Timeout / OtherTransient → silent skip 该 project，下轮
                //   catch-up 重试（保留容错避免单 project 临时不可读误杀）
                match classify_failure(&err) {
                    PollFailureKind::Permanent => {
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
        for session in session_entries {
            if !matches!(session.kind, EntryKind::File) {
                continue;
            }
            if !session.name.to_ascii_lowercase().ends_with(".jsonl") {
                continue;
            }
            let path = proj_dir.join(&session.name);
            let fp = if let Some(m) = session.metadata.as_ref() {
                fingerprint_from_meta(m, session.mtime_missing, &path, warned_missing_mtime)
            } else {
                if warned_missing_mtime.insert(path.clone()) {
                    tracing::debug!(
                        target: "cdt_watch::ssh_polling",
                        path = %path.display(),
                        "metadata missing; falling back to size-only fingerprint",
                    );
                }
                FileFingerprint {
                    size: 0,
                    mtime: None,
                }
            };
            current.insert(path, fp);
        }
    }
    Ok(current)
}

fn fingerprint_from_meta(
    meta: &cdt_discover::FsMetadata,
    mtime_missing: bool,
    path: &Path,
    warned_missing_mtime: &mut BTreeSet<PathBuf>,
) -> FileFingerprint {
    if mtime_missing {
        if warned_missing_mtime.insert(path.to_path_buf()) {
            tracing::debug!(
                target: "cdt_watch::ssh_polling",
                path = %path.display(),
                "mtime missing; falling back to size-only fingerprint",
            );
        }
        FileFingerprint {
            size: meta.size,
            mtime: None,
        }
    } else {
        FileFingerprint {
            size: meta.size,
            mtime: Some(meta.mtime),
        }
    }
}

fn build_change_event(projects_root: &Path, path: &Path, deleted: bool) -> Option<FileChangeEvent> {
    let rel = path.strip_prefix(projects_root).ok()?;
    let mut comps = rel.components();
    let project_comp = comps.next()?;
    let session_comp = comps.next()?;
    if comps.next().is_some() {
        return None;
    }
    let project_id = project_comp.as_os_str().to_string_lossy().into_owned();
    let session_file = session_comp.as_os_str().to_string_lossy();
    let session_id = Path::new(session_file.as_ref())
        .file_stem()?
        .to_string_lossy()
        .into_owned();
    Some(FileChangeEvent {
        project_id,
        session_id,
        deleted,
        project_list_changed: false,
    })
}

/// 把 `Path` 渲染为 POSIX 形式（Windows 上替换 `\` 为 `/`）—— SFTP 协议
/// 强制 POSIX 路径。
fn posix_path_str(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn posix_join(parent: &str, child: &str) -> String {
    if parent.ends_with('/') {
        format!("{parent}{child}")
    } else {
        format!("{parent}/{child}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::RemoteEntry;
    use async_trait::async_trait;
    use cdt_discover::{EntryKind, FsMetadata};
    use std::sync::Mutex;
    use tokio::sync::Mutex as TokioMutex;

    /// Fake SFTP client：每次 `read_dir(projects_root)` 拉队列首；耗尽后保持
    /// 最后一次返回（让稳态轮次都基于最后一次快照 diff）。
    struct FakeSftpClient {
        scripted_dirs: TokioMutex<Vec<Result<DirSnapshot, SftpClientError>>>,
        last_snapshot: Mutex<Option<DirSnapshot>>,
        projects_root: String,
    }

    /// 单次 `scan_once` 应看到的远端状态：`projects_root` 下若干 `project_id` 目录，
    /// 每个目录下若干 session jsonl 文件 + metadata。
    #[derive(Clone, Debug)]
    struct DirSnapshot {
        /// `project_id` → 该 project 下的 session entries。
        projects: Vec<(String, Vec<(String, FsMetadata)>)>,
    }

    impl FakeSftpClient {
        fn arc(
            projects_root: &str,
            scripted: Vec<Result<DirSnapshot, SftpClientError>>,
        ) -> Arc<Self> {
            Arc::new(Self {
                scripted_dirs: TokioMutex::new(scripted),
                last_snapshot: Mutex::new(None),
                projects_root: projects_root.to_owned(),
            })
        }

        async fn next_snapshot(&self) -> Result<DirSnapshot, SftpClientError> {
            let mut q = self.scripted_dirs.lock().await;
            if q.is_empty() {
                return self
                    .last_snapshot
                    .lock()
                    .unwrap()
                    .clone()
                    .ok_or_else(|| SftpClientError::Other("no scripted snapshot".into()));
            }
            let r = q.remove(0);
            if let Ok(snap) = &r {
                *self.last_snapshot.lock().unwrap() = Some(snap.clone());
            }
            r
        }

        fn lookup_project<'a>(
            &self,
            snap: &'a DirSnapshot,
            path: &str,
        ) -> Option<&'a Vec<(String, FsMetadata)>> {
            let prefix = format!("{}/", self.projects_root);
            let rest = path.strip_prefix(&prefix)?;
            snap.projects
                .iter()
                .find(|(n, _)| n == rest)
                .map(|(_, files)| files)
        }
    }

    #[async_trait]
    impl SftpClient for FakeSftpClient {
        async fn metadata(&self, _path: &str) -> Result<FsMetadata, SftpClientError> {
            Err(SftpClientError::Other(
                "metadata not used in polling".into(),
            ))
        }
        async fn try_exists(&self, _path: &str) -> Result<bool, SftpClientError> {
            Err(SftpClientError::Other(
                "try_exists not used in polling".into(),
            ))
        }
        async fn read(&self, _path: &str) -> Result<Vec<u8>, SftpClientError> {
            Err(SftpClientError::Other("read not used in polling".into()))
        }
        async fn read_lines_head(
            &self,
            _path: &str,
            _max: usize,
        ) -> Result<Vec<String>, SftpClientError> {
            Err(SftpClientError::Other("read_lines_head not used".into()))
        }
        async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
            // 顶层 projects_root → 列 project_id 目录
            if path == self.projects_root {
                let snap = self.next_snapshot().await?;
                return Ok(snap
                    .projects
                    .iter()
                    .map(|(name, _)| RemoteEntry {
                        name: name.clone(),
                        kind: EntryKind::Dir,
                        metadata: None,
                        mtime_missing: false,
                    })
                    .collect());
            }
            // 子目录 <projects_root>/<project_id> → 列 session jsonl
            let snap =
                self.last_snapshot.lock().unwrap().clone().ok_or_else(|| {
                    SftpClientError::Other("no snapshot for child read_dir".into())
                })?;
            let files = self
                .lookup_project(&snap, path)
                .ok_or_else(|| SftpClientError::NoSuchFile)?;
            Ok(files
                .iter()
                .map(|(name, meta)| RemoteEntry {
                    name: name.clone(),
                    kind: EntryKind::File,
                    metadata: Some(*meta),
                    mtime_missing: meta.mtime == SystemTime::UNIX_EPOCH,
                })
                .collect())
        }
        async fn write(&self, _path: &str, _data: &[u8]) -> Result<(), SftpClientError> {
            Err(SftpClientError::Other(
                "write not used in polling fake".into(),
            ))
        }
        async fn mkdir(&self, _path: &str) -> Result<(), SftpClientError> {
            Err(SftpClientError::Other(
                "mkdir not used in polling fake".into(),
            ))
        }
        async fn remove(&self, _path: &str) -> Result<(), SftpClientError> {
            Err(SftpClientError::Other(
                "remove not used in polling fake".into(),
            ))
        }
        async fn rename(&self, _src: &str, _dst: &str) -> Result<(), SftpClientError> {
            Err(SftpClientError::Other(
                "rename not used in polling fake".into(),
            ))
        }
    }

    fn meta(size: u64, mtime: SystemTime) -> FsMetadata {
        FsMetadata {
            size,
            mtime,
            identity: None,
        }
    }

    fn snap_one_project(name: &str, files: Vec<(&str, FsMetadata)>) -> DirSnapshot {
        DirSnapshot {
            projects: vec![(
                name.to_owned(),
                files.into_iter().map(|(n, m)| (n.to_owned(), m)).collect(),
            )],
        }
    }

    fn projects_root() -> PathBuf {
        PathBuf::from("/remote/home/.claude/projects")
    }

    fn projects_root_str() -> &'static str {
        "/remote/home/.claude/projects"
    }

    #[tokio::test(start_paused = true)]
    async fn first_scan_establishes_baseline_without_events() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let snap = snap_one_project("proj-A", vec![("sess1.jsonl", meta(100, now))]);
        let client = FakeSftpClient::arc(projects_root_str(), vec![Ok(snap)]);
        let (tx, mut rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());

        // 让 spawned task 跑到 eager baseline scan 完成 + 进入 select! 等 tick。
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // 不应有任何事件
        let r = rx.try_recv();
        assert!(
            matches!(r, Err(broadcast::error::TryRecvError::Empty)),
            "first scan must not emit events: {r:?}"
        );

        cancel.cancel();
        handle.join().await;
    }

    #[tokio::test(start_paused = true)]
    async fn second_poll_emits_new_file() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let snap1 = snap_one_project("proj-A", vec![("a.jsonl", meta(100, now))]);
        let snap2 = snap_one_project(
            "proj-A",
            vec![
                ("a.jsonl", meta(100, now)),
                ("b.jsonl", meta(50, now + Duration::from_secs(5))),
            ],
        );
        let client = FakeSftpClient::arc(projects_root_str(), vec![Ok(snap1), Ok(snap2)]);
        let (tx, mut rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle =
            RemotePollingWatcher::spawn(client, projects_root(), tx.clone(), cancel.clone());

        // 让 baseline 建好
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // advance 到第一个 poll tick (3s)
        tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        let event = rx.try_recv().expect("should emit event for new file");
        assert_eq!(event.project_id, "proj-A");
        assert_eq!(event.session_id, "b");
        assert!(!event.deleted);
        assert!(!event.project_list_changed);
        // 新文件后无更多事件（只有 b.jsonl 是新的，a.jsonl 没变）
        assert!(rx.try_recv().is_err());

        cancel.cancel();
        handle.join().await;
    }

    #[tokio::test(start_paused = true)]
    async fn second_poll_emits_size_change() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let snap1 = snap_one_project("proj-A", vec![("a.jsonl", meta(100, now))]);
        // 同 mtime（在 mtime 精度内），size 变化应触发 emit
        let snap2 = snap_one_project("proj-A", vec![("a.jsonl", meta(2048, now))]);
        let client = FakeSftpClient::arc(projects_root_str(), vec![Ok(snap1), Ok(snap2)]);
        let (tx, mut rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());

        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        let event = rx.try_recv().expect("size change should emit");
        assert_eq!(event.session_id, "a");
        assert!(!event.deleted);

        cancel.cancel();
        handle.join().await;
    }

    #[tokio::test(start_paused = true)]
    async fn second_poll_emits_mtime_change_when_size_unchanged() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let later = now + Duration::from_secs(10);
        let snap1 = snap_one_project("proj-A", vec![("a.jsonl", meta(1024, now))]);
        let snap2 = snap_one_project("proj-A", vec![("a.jsonl", meta(1024, later))]);
        let client = FakeSftpClient::arc(projects_root_str(), vec![Ok(snap1), Ok(snap2)]);
        let (tx, mut rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());

        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        let event = rx
            .try_recv()
            .expect("size-unchanged + mtime-changed should emit (双维度 diff)");
        assert_eq!(event.session_id, "a");

        cancel.cancel();
        handle.join().await;
    }

    #[tokio::test(start_paused = true)]
    async fn second_poll_emits_deletion() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let snap1 = snap_one_project("proj-A", vec![("gone.jsonl", meta(50, now))]);
        let snap2 = snap_one_project("proj-A", vec![]);
        let client = FakeSftpClient::arc(projects_root_str(), vec![Ok(snap1), Ok(snap2)]);
        let (tx, mut rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());

        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        let event = rx.try_recv().expect("removal should emit deleted=true");
        assert_eq!(event.session_id, "gone");
        assert!(event.deleted);

        cancel.cancel();
        handle.join().await;
    }

    #[tokio::test(start_paused = true)]
    async fn mtime_missing_size_change_still_detected() {
        // 远端 SFTP 不返回 mtime 时——fingerprint 退化为 size-only，size 变化仍触发 emit。
        let snap1 = snap_one_project(
            "proj-A",
            vec![("a.jsonl", meta(100, SystemTime::UNIX_EPOCH))],
        );
        let snap2 = snap_one_project(
            "proj-A",
            vec![("a.jsonl", meta(200, SystemTime::UNIX_EPOCH))],
        );
        let client = FakeSftpClient::arc(projects_root_str(), vec![Ok(snap1), Ok(snap2)]);
        let (tx, mut rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());

        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        let event = rx
            .try_recv()
            .expect("mtime=None + size change should still emit");
        assert_eq!(event.session_id, "a");

        cancel.cancel();
        handle.join().await;
    }

    #[tokio::test(start_paused = true)]
    async fn catch_up_timer_fires_at_30s() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let stable = snap_one_project("proj-A", vec![("a.jsonl", meta(100, now))]);
        let changed = snap_one_project("proj-A", vec![("a.jsonl", meta(500, now))]);
        let client = FakeSftpClient::arc(
            projects_root_str(),
            vec![Ok(stable.clone()), Ok(stable), Ok(changed)],
        );
        let (tx, mut rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());

        // baseline 跑完
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // 3s poll 消耗第二个 stable snapshot，不应发事件。
        tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        assert!(rx.try_recv().is_err());

        // 30s catch-up 消耗 changed snapshot，必须发 size-change 事件。
        tokio::time::advance(
            CATCH_UP_INTERVAL.checked_sub(POLL_INTERVAL).unwrap() + Duration::from_millis(50),
        )
        .await;
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }

        let event = rx
            .try_recv()
            .expect("catch-up should emit the scripted size change");
        assert_eq!(event.session_id, "a");
        assert!(!event.deleted);

        cancel.cancel();
        handle.join().await;
    }

    #[tokio::test(start_paused = true)]
    async fn transient_error_does_not_stop_watcher() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let snap1 = snap_one_project("proj-A", vec![("a.jsonl", meta(100, now))]);
        let snap3 = snap_one_project("proj-A", vec![("a.jsonl", meta(999, now))]);
        let client = FakeSftpClient::arc(
            projects_root_str(),
            vec![
                Ok(snap1),
                Err(SftpClientError::Transient("ETIMEDOUT".into())),
                Ok(snap3),
            ],
        );
        let (tx, mut rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());

        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // 第 1 个 3s poll 拿瞬时错误——baseline 不变，无事件
        tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        assert!(matches!(
            rx.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));

        // 第 2 个 3s poll 拿到 snap3 → size 变化 emit
        tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        let event = rx.try_recv().expect("post-transient should emit");
        assert_eq!(event.session_id, "a");

        cancel.cancel();
        handle.join().await;
    }

    /// Spec `ssh-remote-context::Polling watcher exits promptly on
    /// cancellation` Scenario "cancel 在 sleep 阶段触发时 watcher 立即退出"。
    ///
    /// 用 `start_paused` 维度断言：watcher 进入 `poll_interval.tick()` 等待
    /// 后调 `cancel.cancel()`，cancel-and-join 在 100ms 内完成——**不**通过
    /// `tokio::time::advance` 推进 `POLL_INTERVAL` 让 timer 自然触发，验证
    /// `tokio::select!` 内 `cancel_token.cancelled()` 分支真正抢占了 sleep。
    #[tokio::test(start_paused = true)]
    async fn cancel_during_long_poll() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let snap = snap_one_project("proj-A", vec![("sess1.jsonl", meta(100, now))]);
        let client = FakeSftpClient::arc(projects_root_str(), vec![Ok(snap)]);
        let (tx, _rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());

        // 让 watcher 跑完 eager baseline scan + 进入 select! 等 poll_interval.tick()
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // 关键：不 advance 时钟，直接 cancel——必须让 cancel_token.cancelled()
        // 分支在 select! 内立即胜出（验证 cancel-aware long poll 行为）
        cancel.cancel();

        // paused-time 100ms timeout 足够让 cancel-and-join 完成；如果实现
        // 错把 sleep 排在 cancel-token 前面，handle.join() 会等满 POLL_INTERVAL=3s
        // 才返回，触发 timeout panic
        tokio::time::timeout(Duration::from_millis(100), handle.cancel_and_join())
            .await
            .expect("cancel-and-join SHALL 在 100ms paused-time 内完成（cancel-aware long poll）");
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_token_stops_watcher_within_deadline() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let snap = snap_one_project("proj-A", vec![("a.jsonl", meta(100, now))]);
        let client = FakeSftpClient::arc(projects_root_str(), vec![Ok(snap)]);
        let (tx, _rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());

        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        cancel.cancel();
        // cancel_and_join 内部走 CANCEL_DEADLINE=1s timeout——应远早于此完成
        let started = Instant::now();
        handle.cancel_and_join().await;
        let elapsed = started.elapsed();
        assert!(
            elapsed <= CANCEL_DEADLINE + Duration::from_millis(100),
            "watcher should exit within {CANCEL_DEADLINE:?}, took {elapsed:?}",
        );
    }

    /// 连续 `PERMANENT_FAILURE_THRESHOLD` 轮永久性 SFTP 错误后，watcher SHALL
    /// 触发 `dead_signal` 并退出 loop。模拟 docker openssh idle timeout 90s
    /// 后 SFTP 报 `Other("session closed")` 永久死亡（详 followups.md SSH/SFTP
    /// keepalive 条目附录的真实复现 log）。
    #[tokio::test(start_paused = true)]
    async fn permanent_failures_trigger_dead_signal_and_exit() {
        // 顶层 read_dir 连续报 "session closed" Other error → 永久错误
        let client = FakeSftpClient::arc(
            projects_root_str(),
            vec![
                // 第 1 次（eager baseline）
                Err(SftpClientError::Other("sftp error: session closed".into())),
                // 第 2 次 (poll tick)
                Err(SftpClientError::Other("sftp error: session closed".into())),
                // 第 3 次 (poll tick) — 达到 PERMANENT_FAILURE_THRESHOLD=3
                Err(SftpClientError::Other("sftp error: session closed".into())),
            ],
        );
        let (tx, _rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());
        let dead = handle.dead_signal();

        // eager baseline scan = 1 次永久错误
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // 2 个 poll tick 各贡献一次永久错误 = 累计 3 次 ≥ 阈值 → dead_signal 触发
        tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // dead_signal SHALL 已经被 notify
        tokio::time::timeout(Duration::from_millis(100), dead.notified())
            .await
            .expect("dead_signal SHALL fire after PERMANENT_FAILURE_THRESHOLD consecutive permanent errors");

        // watcher 自己退出——join 立即返回，无需 cancel
        tokio::time::timeout(Duration::from_millis(100), handle.join())
            .await
            .expect("watcher SHALL exit loop after dead_signal fires");
    }

    /// 永久错误 counter SHALL 在中间出现成功 / 瞬时错误时**重置**——避免长期
    /// 累积导致单次偶发永久错误也触发自愈。
    #[tokio::test(start_paused = true)]
    async fn permanent_counter_resets_on_intervening_success() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let snap = snap_one_project("proj-A", vec![("a.jsonl", meta(100, now))]);
        let client = FakeSftpClient::arc(
            projects_root_str(),
            vec![
                Ok(snap.clone()),                                     // baseline 成功
                Err(SftpClientError::Other("session closed".into())), // permanent #1
                Err(SftpClientError::Other("session closed".into())), // permanent #2
                Ok(snap.clone()), // 中间成功 → counter reset 为 0
                Err(SftpClientError::Other("session closed".into())), // permanent #1 (重新计数)
            ],
        );
        let (tx, _rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());
        let dead = handle.dead_signal();

        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // 4 个 poll tick：err / err / ok / err；最后状态 counter=1 < 阈值=3
        for _ in 0..4 {
            tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
        }

        // dead_signal SHALL NOT 触发
        assert!(
            tokio::time::timeout(Duration::from_millis(50), dead.notified())
                .await
                .is_err(),
            "dead_signal MUST NOT fire when intervening success resets counter",
        );

        cancel.cancel();
        handle.cancel_and_join().await;
    }

    /// 修 GitHub issue #231：旧测试断言"5 轮 timeout/eagain 不触发"——前提是
    /// timeout 永远不触发；新行为下 timeout 走独立 counter 阈值 6，5 轮 < 6
    /// 仍然不触发，但语义从"永远不触发"变为"低于阈值不触发"。本测试覆盖
    /// timeout 类 5 轮（< `TIMEOUT_FAILURE_THRESHOLD = 6`）不触发；6+ 轮触发
    /// 由 `timeout_threshold_triggers_dead_signal_at_6_consecutive` 单独覆盖。
    #[tokio::test(start_paused = true)]
    async fn timeout_below_threshold_does_not_trigger() {
        let client = FakeSftpClient::arc(
            projects_root_str(),
            vec![
                // baseline = 1 个 timeout（counter=1）
                Err(SftpClientError::Transient("ETIMEDOUT".into())),
                // 4 个 poll tick 各贡献 1 个 timeout-class（counter=2..5）
                Err(SftpClientError::Transient("timeout".into())),
                Err(SftpClientError::Transient("EAGAIN".into())),
                Err(SftpClientError::Transient("would block".into())),
                Err(SftpClientError::Transient("timed out".into())),
            ],
        );
        let (tx, _rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());
        let dead = handle.dead_signal();

        // baseline scan
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // 4 个 poll tick：advance + yield + yield 严格顺序（避免 flaky）
        for _ in 0..4 {
            tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
        }

        // 共 5 轮 timeout < 6 → dead_signal SHALL NOT fire
        assert!(
            tokio::time::timeout(Duration::from_millis(50), dead.notified())
                .await
                .is_err(),
            "dead_signal MUST NOT fire on 5 < 6 timeout-class errors",
        );

        cancel.cancel();
        handle.cancel_and_join().await;
    }

    /// 修 GitHub issue #231 主诉求：连续 6 轮 timeout 类错误（典型 `pkill -STOP
    /// sshd` 场景：SFTP 协议层 hang 但 TCP 未断）SHALL 触发 `dead_signal`。
    ///
    /// 严格驱动顺序：每轮 `advance(POLL_INTERVAL + 50ms) + yield * 2`，避免 `select!`
    /// tick 与 fake snapshot 消耗 race（参考 `crates/CLAUDE.md` `tokio::time::pause`
    /// 测试基础设施陷阱）。
    #[tokio::test(start_paused = true)]
    async fn timeout_threshold_triggers_dead_signal_at_6_consecutive() {
        let client = FakeSftpClient::arc(
            projects_root_str(),
            vec![
                // baseline (1)
                Err(SftpClientError::Transient("ETIMEDOUT".into())),
                // 5 个 poll tick (2..6)，第 6 个 tick 后 consecutive_timeout=6 ≥ 阈值 → dead
                Err(SftpClientError::Transient("timeout".into())),
                Err(SftpClientError::Transient("EAGAIN".into())),
                Err(SftpClientError::Transient("would block".into())),
                Err(SftpClientError::Transient("timed out".into())),
                Err(SftpClientError::Transient("ETIMEDOUT".into())),
            ],
        );
        let (tx, _rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());
        let dead = handle.dead_signal();

        // baseline scan = 1 个 timeout (counter=1)
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // 5 个 poll tick 各贡献 1 个 timeout = 累计 6 ≥ TIMEOUT_FAILURE_THRESHOLD → dead_signal 触发
        for _ in 0..5 {
            tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
        }

        tokio::time::timeout(Duration::from_millis(100), dead.notified())
            .await
            .expect("dead_signal SHALL fire after TIMEOUT_FAILURE_THRESHOLD=6 consecutive timeout errors");

        // watcher 自己退出
        tokio::time::timeout(Duration::from_millis(100), handle.join())
            .await
            .expect("watcher SHALL exit loop after timeout dead_signal fires");
    }

    /// timeout counter SHALL 在中间出现成功 / `OtherTransient` 时**重置**为 0——
    /// 避免长期累积导致单次偶发 timeout 也触发自愈。`5 timeout + 1 ok + 5 timeout`
    /// 序列任一时刻 counter ≤ 5 < 6 → 不触发。
    #[tokio::test(start_paused = true)]
    async fn timeout_counter_resets_on_intervening_success() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let snap = snap_one_project("proj-A", vec![("a.jsonl", meta(100, now))]);
        let mut scripted: Vec<Result<DirSnapshot, SftpClientError>> = Vec::new();
        // baseline 成功（counter=0）
        scripted.push(Ok(snap.clone()));
        // 5 个 timeout（counter=1..5）
        for _ in 0..5 {
            scripted.push(Err(SftpClientError::Transient("timeout".into())));
        }
        // 1 个 ok（counter reset 0）
        scripted.push(Ok(snap.clone()));
        // 5 个 timeout 再来（counter=1..5）
        for _ in 0..5 {
            scripted.push(Err(SftpClientError::Transient("timeout".into())));
        }
        let client = FakeSftpClient::arc(projects_root_str(), scripted);
        let (tx, _rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());
        let dead = handle.dead_signal();

        // baseline
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // 11 个 poll tick：5 timeout + 1 ok + 5 timeout
        for _ in 0..11 {
            tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
        }

        // counter 任一时刻 ≤ 5 < 6 → SHALL NOT fire
        assert!(
            tokio::time::timeout(Duration::from_millis(50), dead.notified())
                .await
                .is_err(),
            "dead_signal MUST NOT fire when intervening Ok resets timeout counter",
        );

        cancel.cancel();
        handle.cancel_and_join().await;
    }

    /// codex 二审收紧 reset 规则的回归保障：`5 timeout + 1 permanent + 1 timeout`
    /// SHALL 触发 `dead_signal`（`consecutive_timeout = 6` 在第 7 轮，`Permanent`
    /// **不动** timeout counter——避免攻击序列 `5T → 1P → 5T → 1P → ...` 永远
    /// 推迟 `dead_signal`）。
    #[tokio::test(start_paused = true)]
    async fn mixed_permanent_timeout_sequence_still_triggers() {
        let mut scripted: Vec<Result<DirSnapshot, SftpClientError>> = Vec::new();
        // baseline = timeout (timeout=1)
        scripted.push(Err(SftpClientError::Transient("timeout".into())));
        // 4 个 poll tick = timeout (timeout=2..5)
        for _ in 0..4 {
            scripted.push(Err(SftpClientError::Transient("timeout".into())));
        }
        // 1 个 permanent (permanent=1, timeout 不动仍 5)
        scripted.push(Err(SftpClientError::Other("session closed".into())));
        // 1 个 timeout (timeout=6 ≥ 阈值 → fire)
        scripted.push(Err(SftpClientError::Transient("timeout".into())));
        let client = FakeSftpClient::arc(projects_root_str(), scripted);
        let (tx, _rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());
        let dead = handle.dead_signal();

        // baseline + 6 poll ticks 共 7 round
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        for _ in 0..6 {
            tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
        }

        tokio::time::timeout(Duration::from_millis(100), dead.notified())
            .await
            .expect("dead_signal SHALL fire when timeout reaches 6 even after 1 permanent (codex reset rule)");
    }

    /// codex 二审收紧 reset 规则的反向回归：`2 permanent + 1 timeout + 1 permanent`
    /// SHALL 触发 `dead_signal`（`consecutive_permanent = 3` 在第 4 轮，`Timeout`
    /// **不动** permanent counter）。
    #[tokio::test(start_paused = true)]
    async fn mixed_timeout_permanent_sequence_still_triggers() {
        let scripted = vec![
            // baseline = permanent (permanent=1)
            Err(SftpClientError::Other("session closed".into())),
            // poll = permanent (permanent=2)
            Err(SftpClientError::Other("session closed".into())),
            // poll = timeout (timeout=1, permanent 不动仍 2)
            Err(SftpClientError::Transient("timeout".into())),
            // poll = permanent (permanent=3 ≥ 阈值 → fire)
            Err(SftpClientError::Other("session closed".into())),
        ];
        let client = FakeSftpClient::arc(projects_root_str(), scripted);
        let (tx, _rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());
        let dead = handle.dead_signal();

        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        for _ in 0..3 {
            tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
        }

        tokio::time::timeout(Duration::from_millis(100), dead.notified())
            .await
            .expect("dead_signal SHALL fire when permanent reaches 3 even after 1 timeout (codex reset rule)");
    }

    /// `scan_once` 子目录 `read_dir` `Permanent` 错误 SHALL escalate 到外层（修
    /// issue #231 #2：旧版 `scan_once` 内 `Other(reason)` silent continue
    /// 让 sub-project channel-dead 错误被静默吞掉、watcher 误以为 baseline
    /// 完整后下轮报"全部 session deleted"事件）。
    ///
    /// fake：顶层 `read_dir` 返 Ok 列出 1 个 sub-project，sub-project `read_dir`
    /// 返 `Other("session closed")` `Permanent` 错误。3 轮后外层 counter=3 ≥ 阈值
    /// → `dead_signal` 触发。
    #[tokio::test(start_paused = true)]
    async fn subdir_permanent_error_escalates_scan_once() {
        struct SubdirErrorFake {
            projects_root: String,
            subdir_error_count: TokioMutex<u32>,
        }

        #[async_trait]
        impl SftpClient for SubdirErrorFake {
            async fn metadata(&self, _path: &str) -> Result<FsMetadata, SftpClientError> {
                Err(SftpClientError::Other("not used".into()))
            }
            async fn try_exists(&self, _path: &str) -> Result<bool, SftpClientError> {
                Err(SftpClientError::Other("not used".into()))
            }
            async fn read(&self, _path: &str) -> Result<Vec<u8>, SftpClientError> {
                Err(SftpClientError::Other("not used".into()))
            }
            async fn read_lines_head(
                &self,
                _path: &str,
                _max: usize,
            ) -> Result<Vec<String>, SftpClientError> {
                Err(SftpClientError::Other("not used".into()))
            }
            async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
                if path == self.projects_root {
                    // 顶层成功列出 1 个 sub-project
                    return Ok(vec![RemoteEntry {
                        name: "proj-A".to_owned(),
                        kind: EntryKind::Dir,
                        metadata: None,
                        mtime_missing: false,
                    }]);
                }
                // sub-project read_dir → 永久错误 escalate
                *self.subdir_error_count.lock().await += 1;
                Err(SftpClientError::Other("sftp error: session closed".into()))
            }
            async fn write(&self, _path: &str, _data: &[u8]) -> Result<(), SftpClientError> {
                Err(SftpClientError::Other("not used".into()))
            }
            async fn mkdir(&self, _path: &str) -> Result<(), SftpClientError> {
                Err(SftpClientError::Other("not used".into()))
            }
            async fn remove(&self, _path: &str) -> Result<(), SftpClientError> {
                Err(SftpClientError::Other("not used".into()))
            }
            async fn rename(&self, _src: &str, _dst: &str) -> Result<(), SftpClientError> {
                Err(SftpClientError::Other("not used".into()))
            }
        }

        let fake = Arc::new(SubdirErrorFake {
            projects_root: projects_root_str().to_owned(),
            subdir_error_count: TokioMutex::new(0),
        });
        let counter_handle = Arc::clone(&fake);
        let (tx, _rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(fake, projects_root(), tx, cancel.clone());
        let dead = handle.dead_signal();

        // baseline scan = 1 次 sub-project permanent 错误（escalate 到 outer scan_once
        // 返 Err → counter=1）
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // 2 次 poll tick = 2 次 sub-project permanent 错误（counter=2..3）→ 第 3 轮
        // 后达 PERMANENT_FAILURE_THRESHOLD → dead_signal 触发
        for _ in 0..2 {
            tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
        }

        tokio::time::timeout(Duration::from_millis(100), dead.notified())
            .await
            .expect(
                "subdir Permanent error SHALL escalate scan_once and trigger dead_signal at 3 consecutive rounds",
            );

        let n_subdir_errors = *counter_handle.subdir_error_count.lock().await;
        assert!(
            n_subdir_errors >= 3,
            "expected at least 3 sub-project read_dir attempts, got {n_subdir_errors}",
        );
    }

    /// `OtherTransient` 错误 SHALL reset 两 counter——视同 channel 真活着的
    /// 强证据（与 `Ok` 等价）。10 轮纯 `OtherTransient` 不触发任何阈值。
    #[tokio::test(start_paused = true)]
    async fn other_transient_does_not_trigger_dead_signal() {
        let client = FakeSftpClient::arc(
            projects_root_str(),
            vec![
                // 不带 transport-dead / timeout 关键字的 Other 错误
                Err(SftpClientError::Other("unsupported sftp version".into())),
                Err(SftpClientError::Other("status failure: unknown".into())),
                Err(SftpClientError::Other("unsupported sftp version".into())),
                Err(SftpClientError::Other("status failure: unknown".into())),
                Err(SftpClientError::Other("unsupported sftp version".into())),
            ],
        );
        let (tx, _rx) = broadcast::channel::<FileChangeEvent>(16);
        let cancel = CancelToken::new();
        let handle = RemotePollingWatcher::spawn(client, projects_root(), tx, cancel.clone());
        let dead = handle.dead_signal();

        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        for _ in 0..4 {
            tokio::time::advance(POLL_INTERVAL + Duration::from_millis(50)).await;
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
        }

        assert!(
            tokio::time::timeout(Duration::from_millis(50), dead.notified())
                .await
                .is_err(),
            "OtherTransient errors SHALL NOT trigger dead_signal (视同 channel 真活着)",
        );

        cancel.cancel();
        handle.cancel_and_join().await;
    }

    /// 三态分类覆盖——修 GitHub issue #231：旧版 `is_permanent_sftp_failure -> bool`
    /// 让 `Transient("timeout")` 永远落 false 永远不计 dead 检测；新版三态 +
    /// 双独立 counter 让 timeout 走独立高阈值（6 ≈ 18s）触发自愈。
    #[test]
    fn classify_failure_classifies_three_kinds() {
        // === Permanent：transport-dead 关键字（无论 Other / Transient 来源）===
        for case in [
            SftpClientError::Other("sftp error: session closed".into()),
            SftpClientError::Other("russh: Eof".into()),
            SftpClientError::Other("Broken pipe".into()),
            SftpClientError::Other("connection reset by peer".into()),
            // Transient 路径——provider::is_transient_io_reason 把 broken pipe /
            // connection reset / epipe 归 Transient，with_retry 3 次后仍是这类
            // 错误就是 channel 真死了。
            SftpClientError::Transient("broken pipe".into()),
            SftpClientError::Transient("EPIPE".into()),
            SftpClientError::Transient("ECONNRESET while reading".into()),
        ] {
            assert_eq!(
                classify_failure(&case),
                PollFailureKind::Permanent,
                "transport-dead 关键字 SHALL classify Permanent: {case:?}",
            );
        }

        // === Timeout：timeout / etimedout / timed out / eagain / would block ===
        // 修 issue #231 主诉求：纯 timeout 现在走独立 counter 而非永远不计
        for case in [
            SftpClientError::Transient("timeout".into()),
            SftpClientError::Transient("ETIMEDOUT".into()),
            SftpClientError::Transient("timed out".into()),
            SftpClientError::Transient("EAGAIN".into()),
            SftpClientError::Transient("would block".into()),
        ] {
            assert_eq!(
                classify_failure(&case),
                PollFailureKind::Timeout,
                "timeout-class 关键字 SHALL classify Timeout: {case:?}",
            );
        }

        // === OtherTransient：不带任何关键字的 Transient / Other / NoSuchFile /
        // PermissionDenied —— 视同 channel 真活着的强证据，reset 两 counter ===
        for case in [
            SftpClientError::Other("unsupported sftp version".into()),
            SftpClientError::Transient("status failure: weird code".into()),
            SftpClientError::NoSuchFile,
            SftpClientError::PermissionDenied,
        ] {
            assert_eq!(
                classify_failure(&case),
                PollFailureKind::OtherTransient,
                "non-channel-related error SHALL classify OtherTransient: {case:?}",
            );
        }
    }

    #[tokio::test]
    async fn cancel_token_idempotent_and_visible() {
        // 单元测 CancelToken 自身：cancel 后 is_cancelled = true；cancelled() 立即 ready。
        let token = CancelToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
        // cancelled() 应立即返回
        tokio::time::timeout(Duration::from_millis(50), token.cancelled())
            .await
            .expect("already cancelled, should return immediately");
        // 多次 cancel 幂等，不 panic
        token.cancel();
        token.cancel();
    }

    #[test]
    fn build_change_event_extracts_ids() {
        let root = projects_root();
        let path = root.join("proj-A").join("sess-1.jsonl");
        let event = build_change_event(&root, &path, false).expect("should parse");
        assert_eq!(event.project_id, "proj-A");
        assert_eq!(event.session_id, "sess-1");
        assert!(!event.deleted);
        assert!(!event.project_list_changed);
    }

    #[test]
    fn build_change_event_rejects_nested() {
        let root = projects_root();
        let path = root.join("proj-A").join("subagents").join("agent-x.jsonl");
        assert!(build_change_event(&root, &path, false).is_none());
    }

    #[test]
    fn build_change_event_preserves_deleted_flag() {
        let root = projects_root();
        let path = root.join("proj-A").join("s.jsonl");
        let event = build_change_event(&root, &path, true).expect("should parse");
        assert!(event.deleted);
    }
}
