//! Provider instrumentation：`InstrumentedFs` wrapper + `FsOpCounter` + `with_fs_counter`。
//!
//! Spec：`openspec/specs/fs-abstraction/spec.md` §`Provider instrumentation 入口可观测 fs op 次数`。
//! 设计：`openspec/changes/unify-fs-abstraction/design.md` D9。
//!
//! 钉死注入机制：`InstrumentedFs<P>` 在 trait 调用边界自动计数；provider impl
//! 自身（Local / SSH / fake）SHALL NOT 含 counter 调用——计数发生在 wrapper 层。
//!
//! 本 change 提供基础设施 + 单测；PR-B/C/D 起业务路径才接入。

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::dir_entry::DirEntry;
use crate::error::FsError;
use crate::kind::FsKind;
use crate::metadata::FsMetadata;
use crate::provider::FileSystemProvider;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FsOpCounts {
    pub stat: u32,
    pub read_to_string: u32,
    pub read_dir: u32,
    pub read_dir_with_metadata: u32,
    pub read_lines_head: u32,
    pub open_read: u32,
    pub stat_many: u32,
    pub exists: u32,
    pub write_atomic: u32,
    pub create_dir_all: u32,
    pub remove_file: u32,
}

/// 通过 `tokio::task_local!` 注入当前任务上下文的 fs op counter。
///
/// `with_fs_counter` 包住一段 async 代码，`InstrumentedFs` 在 trait 边界
/// 自动 `try_record` 到 counter。
#[derive(Debug, Clone)]
pub struct FsOpCounter {
    counts: Arc<Mutex<FsOpCounts>>,
}

impl FsOpCounter {
    fn new() -> Self {
        Self {
            counts: Arc::new(Mutex::new(FsOpCounts::default())),
        }
    }

    /// 取当前任务上下文 counter（若存在），调用方在 `with_fs_counter` 外
    /// 调用本方法返 `None`，对应"未启用 instrumentation"语义。
    #[must_use]
    pub fn current() -> Option<Self> {
        CURRENT_FS_COUNTER.try_with(Clone::clone).ok()
    }

    fn snapshot(&self) -> FsOpCounts {
        *self.counts.lock().expect("fs op counter mutex poisoned")
    }

    fn record_stat(&self) {
        self.counts.lock().expect("counter poisoned").stat += 1;
    }
    fn record_read_to_string(&self) {
        self.counts.lock().expect("counter poisoned").read_to_string += 1;
    }
    fn record_read_dir(&self) {
        self.counts.lock().expect("counter poisoned").read_dir += 1;
    }
    fn record_read_dir_with_metadata(&self) {
        self.counts
            .lock()
            .expect("counter poisoned")
            .read_dir_with_metadata += 1;
    }
    fn record_read_lines_head(&self) {
        self.counts
            .lock()
            .expect("counter poisoned")
            .read_lines_head += 1;
    }
    fn record_open_read(&self) {
        self.counts.lock().expect("counter poisoned").open_read += 1;
    }
    fn record_stat_many(&self) {
        self.counts.lock().expect("counter poisoned").stat_many += 1;
    }
    fn record_exists(&self) {
        self.counts.lock().expect("counter poisoned").exists += 1;
    }
    fn record_write_atomic(&self) {
        self.counts.lock().expect("counter poisoned").write_atomic += 1;
    }
    fn record_create_dir_all(&self) {
        self.counts.lock().expect("counter poisoned").create_dir_all += 1;
    }
    fn record_remove_file(&self) {
        self.counts.lock().expect("counter poisoned").remove_file += 1;
    }
}

tokio::task_local! {
    static CURRENT_FS_COUNTER: FsOpCounter;
}

/// 包住一段 async 代码统计内部 fs op 次数——结束 emit tracing event + 返计数。
///
/// **嵌套语义（codex 二审 M3 钉死）**：`with_fs_counter` 内部用
/// `tokio::task_local!::scope` 注入 counter，嵌套调用 SHALL 表现为
/// **独立计数**——内层 scope 创建新 counter 完全覆盖外层 task-local 槽位，
/// 内层 fs op **不**回传到外层 counter。
///
/// 设计原因：业务路径常用模式是"per-IPC-command counter"（外层）+
/// "测试 / 子任务 fs op 上限断言"（内层），独立计数语义让测试断言不被外层污染；
/// 若未来需要"嵌套聚合"（rare），调用方应该手工合并 inner snapshot 到 outer。
///
/// 未包 `InstrumentedFs` 的 provider 调 trait 方法时 counter 不递增（向后兼容）。
pub async fn with_fs_counter<F, Fut>(f: F) -> (Fut::Output, FsOpCounts)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future,
{
    let counter = FsOpCounter::new();
    let result = CURRENT_FS_COUNTER.scope(counter.clone(), f()).await;
    let snapshot = counter.snapshot();
    tracing::info!(
        target: "cdt_fs::ops",
        stat = snapshot.stat,
        read_to_string = snapshot.read_to_string,
        read_dir = snapshot.read_dir,
        read_dir_with_metadata = snapshot.read_dir_with_metadata,
        read_lines_head = snapshot.read_lines_head,
        open_read = snapshot.open_read,
        stat_many = snapshot.stat_many,
        exists = snapshot.exists,
        write_atomic = snapshot.write_atomic,
        create_dir_all = snapshot.create_dir_all,
        remove_file = snapshot.remove_file,
        "fs ops summary"
    );
    (result, snapshot)
}

/// `InstrumentedFs<P>` —— 在 trait 调用边界自动计数的 wrapper provider。
///
/// 包裹任意 `P: FileSystemProvider` 后实现 `FileSystemProvider`，每个 trait 方法
/// 内部先调 `FsOpCounter::current()` 拿当前 task 上下文 counter 计数，再 delegate
/// 到 `inner`。未包 wrapper 的 fs handle 调 trait 方法不计数（向后兼容）。
pub struct InstrumentedFs<P> {
    inner: P,
}

impl<P: FileSystemProvider> InstrumentedFs<P> {
    pub fn new(inner: P) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<P: FileSystemProvider> FileSystemProvider for InstrumentedFs<P> {
    fn kind(&self) -> FsKind {
        self.inner.kind()
    }

    async fn exists(&self, path: &Path) -> bool {
        if let Some(c) = FsOpCounter::current() {
            c.record_exists();
        }
        self.inner.exists(path).await
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
        if let Some(c) = FsOpCounter::current() {
            c.record_read_dir();
        }
        self.inner.read_dir(path).await
    }

    async fn read_dir_with_metadata(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
        if let Some(c) = FsOpCounter::current() {
            c.record_read_dir_with_metadata();
        }
        self.inner.read_dir_with_metadata(path).await
    }

    async fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        if let Some(c) = FsOpCounter::current() {
            c.record_read_to_string();
        }
        self.inner.read_to_string(path).await
    }

    async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError> {
        if let Some(c) = FsOpCounter::current() {
            c.record_stat();
        }
        self.inner.stat(path).await
    }

    async fn read_lines_head(&self, path: &Path, max: usize) -> Result<Vec<String>, FsError> {
        if let Some(c) = FsOpCounter::current() {
            c.record_read_lines_head();
        }
        self.inner.read_lines_head(path, max).await
    }

    async fn open_read(&self, path: &Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, FsError> {
        if let Some(c) = FsOpCounter::current() {
            c.record_open_read();
        }
        self.inner.open_read(path).await
    }

    async fn stat_many(&self, paths: &[&Path]) -> Vec<Result<FsMetadata, FsError>> {
        if let Some(c) = FsOpCounter::current() {
            c.record_stat_many();
        }
        self.inner.stat_many(paths).await
    }

    async fn write_atomic(&self, path: &Path, content: &[u8]) -> Result<(), FsError> {
        if let Some(c) = FsOpCounter::current() {
            c.record_write_atomic();
        }
        self.inner.write_atomic(path, content).await
    }

    async fn create_dir_all(&self, path: &Path) -> Result<(), FsError> {
        if let Some(c) = FsOpCounter::current() {
            c.record_create_dir_all();
        }
        self.inner.create_dir_all(path).await
    }

    async fn remove_file(&self, path: &Path) -> Result<(), FsError> {
        if let Some(c) = FsOpCounter::current() {
            c.record_remove_file();
        }
        self.inner.remove_file(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::LocalFileSystemProvider;
    use tempfile::tempdir;

    #[tokio::test]
    async fn instrumented_wrapper_counts_at_trait_boundary() {
        let dir = tempdir().unwrap();
        let p1 = dir.path().join("a");
        let p2 = dir.path().join("b");
        let p3 = dir.path().join("c");
        tokio::fs::write(&p1, b"1").await.unwrap();
        tokio::fs::write(&p2, b"22").await.unwrap();
        tokio::fs::write(&p3, b"333").await.unwrap();

        let fs = InstrumentedFs::new(LocalFileSystemProvider::new());
        let ((), counts) = with_fs_counter(|| async {
            fs.stat(&p1).await.unwrap();
            fs.stat(&p2).await.unwrap();
            fs.read_dir(dir.path()).await.unwrap();
        })
        .await;

        assert_eq!(counts.stat, 2);
        assert_eq!(counts.read_dir, 1);
        assert_eq!(counts.read_to_string, 0);
    }

    #[tokio::test]
    async fn unwrapped_provider_does_not_count() {
        let dir = tempdir().unwrap();
        let p1 = dir.path().join("a");
        tokio::fs::write(&p1, b"x").await.unwrap();

        let fs = LocalFileSystemProvider::new();
        let ((), counts) = with_fs_counter(|| async {
            fs.stat(&p1).await.unwrap();
        })
        .await;

        assert_eq!(counts.stat, 0);
    }

    #[tokio::test]
    async fn counter_does_not_leak_across_concurrent_tasks() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("a");
        tokio::fs::write(&p, b"x").await.unwrap();

        let fs1 = Arc::new(InstrumentedFs::new(LocalFileSystemProvider::new()));
        let fs2 = Arc::clone(&fs1);
        let p1 = p.clone();
        let p2 = p.clone();

        let task_a = tokio::spawn(async move {
            with_fs_counter(|| async {
                for _ in 0..5 {
                    fs1.stat(&p1).await.unwrap();
                }
            })
            .await
            .1
        });
        let task_b = tokio::spawn(async move {
            with_fs_counter(|| async {
                for _ in 0..3 {
                    fs2.stat(&p2).await.unwrap();
                }
            })
            .await
            .1
        });

        let counts_a = task_a.await.unwrap();
        let counts_b = task_b.await.unwrap();
        assert_eq!(counts_a.stat, 5);
        assert_eq!(counts_b.stat, 3);
    }

    #[tokio::test]
    async fn current_returns_none_outside_with_fs_counter() {
        assert!(FsOpCounter::current().is_none());
    }

    #[tokio::test]
    async fn nested_with_fs_counter_scopes_are_independent() {
        // codex 二审 M3 / L3：嵌套 with_fs_counter 语义钉死 = 独立计数。
        // 内层 scope 创建新 counter 覆盖外层 task-local 槽位，内层 fs op
        // 不回传到外层 counter。
        let dir = tempdir().unwrap();
        let path = dir.path().join("a");
        tokio::fs::write(&path, b"x").await.unwrap();

        let fs = InstrumentedFs::new(LocalFileSystemProvider::new());
        let ((), outer) = with_fs_counter(|| async {
            fs.stat(&path).await.unwrap();
            let ((), inner) = with_fs_counter(|| async {
                fs.stat(&path).await.unwrap();
                fs.stat(&path).await.unwrap();
            })
            .await;
            assert_eq!(inner.stat, 2, "内层 SHALL 独立计数 2 次");
            // 内层 scope 退出后外层 task-local counter 恢复
            fs.stat(&path).await.unwrap();
        })
        .await;
        // 外层 SHALL 仅含外层 scope 内调用，不含内层（独立计数）
        assert_eq!(
            outer.stat, 2,
            "外层 SHALL 仅记录自身 scope 内 2 次 stat，不聚合内层"
        );
    }
}
