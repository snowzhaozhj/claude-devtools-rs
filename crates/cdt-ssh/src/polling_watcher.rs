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
/// 持有 [`CancelToken`]（cancel signal）+ tokio task 的 [`JoinHandle`]——
/// connection manager 在 disconnect 时调 [`cancel`] 通知 watcher 退出，
/// 必要时 await `join_handle` 等待清理完成。
///
/// [`cancel`]: Self::cancel
#[derive(Debug)]
pub struct RemoteWatcherHandle {
    cancel_token: CancelToken,
    join_handle: JoinHandle<()>,
}

impl RemoteWatcherHandle {
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
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
    pub fn spawn(
        client: Arc<dyn SftpClient>,
        projects_root: PathBuf,
        sender: broadcast::Sender<FileChangeEvent>,
        cancel_token: CancelToken,
    ) -> RemoteWatcherHandle {
        let cancel_for_handle = cancel_token.clone();
        let join_handle = tokio::spawn(run_polling_loop(
            client,
            projects_root,
            sender,
            cancel_token,
        ));
        RemoteWatcherHandle {
            cancel_token: cancel_for_handle,
            join_handle,
        }
    }
}

async fn run_polling_loop(
    client: Arc<dyn SftpClient>,
    projects_root: PathBuf,
    sender: broadcast::Sender<FileChangeEvent>,
    cancel_token: CancelToken,
) {
    let mut warned_missing_mtime: BTreeSet<PathBuf> = BTreeSet::new();

    // Eager 第一次 scan 建 baseline——spec "First poll establishes baseline"。
    // 顶层 `read_dir` 失败时 baseline 取空（首次连接刚好远端 home 临时不可读时
    // 也不让 watcher 死掉；下一轮 catch-up 会再尝试）。
    let mut baseline = match scan_once(&client, &projects_root, &mut warned_missing_mtime).await {
        Ok(b) => b,
        Err(e) => {
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
                run_one_pass(&client, &projects_root, &sender, &mut baseline, &mut warned_missing_mtime, &cancel_token).await;
            }
            _ = catch_up_interval.tick() => {
                // 30s catch-up 与 3s poll 算法相同——兜底 SFTP 偶发漏事件
                // （spec D5：catch-up 同样按 size + mtime 双维度比对）。
                run_one_pass(&client, &projects_root, &sender, &mut baseline, &mut warned_missing_mtime, &cancel_token).await;
            }
        }
    }
}

/// 执行单轮 scan + diff + emit + baseline 更新；瞬时错误跳过本轮（不传播）。
async fn run_one_pass(
    client: &Arc<dyn SftpClient>,
    projects_root: &Path,
    sender: &broadcast::Sender<FileChangeEvent>,
    baseline: &mut BTreeMap<PathBuf, FileFingerprint>,
    warned_missing_mtime: &mut BTreeSet<PathBuf>,
    cancel_token: &CancelToken,
) {
    if cancel_token.is_cancelled() {
        return;
    }
    let current = match scan_once(client, projects_root, warned_missing_mtime).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                target: "cdt_watch::ssh_polling",
                error = %e,
                "polling scan failed (skipping this round)",
            );
            return;
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
            Err(SftpClientError::Transient(reason)) => {
                tracing::warn!(
                    target: "cdt_watch::ssh_polling",
                    project = %proj.name,
                    %reason,
                    "transient sftp error reading project dir; skipping",
                );
                continue;
            }
            Err(SftpClientError::Other(reason)) => {
                tracing::warn!(
                    target: "cdt_watch::ssh_polling",
                    project = %proj.name,
                    %reason,
                    "permanent sftp error reading project dir; skipping",
                );
                continue;
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
