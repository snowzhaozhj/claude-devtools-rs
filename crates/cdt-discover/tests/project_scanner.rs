//! project-discovery capability 的端到端测试。
//!
//! 覆盖 `openspec/specs/project-discovery/spec.md` 里 Requirement 的主要 scenario，
//! 含 change `merge-composite-projects` 后的合并语义（同 encoded 目录恒产 1 个
//! `Project`，cwd 差异由 `Session.cwd` 暴露）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cdt_discover::{FileSystemProvider, LocalFileSystemProvider, ProjectScanner, encode_path};

async fn write_session(dir: &Path, session_id: &str, cwd: &str) {
    let line = format!(
        r#"{{"type":"user","uuid":"{session_id}","cwd":"{cwd}","timestamp":"2026-01-01T00:00:00Z","message":{{"role":"user","content":"hi"}}}}"#,
    );
    tokio::fs::write(dir.join(format!("{session_id}.jsonl")), format!("{line}\n"))
        .await
        .unwrap();
}

#[tokio::test]
async fn scan_returns_one_project_per_encoded_dir_with_distinct_cwds() {
    let root = tempfile::tempdir().unwrap();
    let projects_dir = root.path().join("projects");
    tokio::fs::create_dir_all(&projects_dir).await.unwrap();

    // 一个单 cwd 目录 → 1 个 Project（distinct_cwds 含 1 个）
    let foo_dir = projects_dir.join("-Users-alice-code-foo");
    tokio::fs::create_dir_all(&foo_dir).await.unwrap();
    write_session(&foo_dir, "a1", "/Users/alice/code/foo").await;
    write_session(&foo_dir, "a2", "/Users/alice/code/foo").await;

    // 一个多 cwd 目录 → 仍只产 1 个 Project（distinct_cwds 含 2 个），不再拆 composite
    let bar_dir = projects_dir.join("-Users-alice-code-bar");
    tokio::fs::create_dir_all(&bar_dir).await.unwrap();
    write_session(&bar_dir, "b1", "/Users/alice/code/bar").await;
    write_session(&bar_dir, "b2", "/Users/alice/code/bar-v2").await;

    // 一个空目录 → 被跳过
    tokio::fs::create_dir_all(projects_dir.join("-Users-alice-empty"))
        .await
        .unwrap();

    let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
    let mut scanner = ProjectScanner::new(fs, projects_dir.clone());
    let projects = scanner.scan().await.unwrap();

    // foo (1) + bar (1) = 2，empty 跳过
    assert_eq!(projects.len(), 2, "got: {projects:#?}");

    // 所有 project.id 都不含 `::`
    for p in &projects {
        assert!(
            !p.id.contains("::"),
            "id must not contain composite separator: {}",
            p.id
        );
    }

    let foo = projects
        .iter()
        .find(|p| p.id == "-Users-alice-code-foo")
        .expect("foo project");
    assert_eq!(foo.distinct_cwds, vec!["/Users/alice/code/foo".to_string()]);

    let bar = projects
        .iter()
        .find(|p| p.id == "-Users-alice-code-bar")
        .expect("bar project");
    let bar_cwds: BTreeSet<_> = bar.distinct_cwds.iter().cloned().collect();
    let expected: BTreeSet<_> = [
        "/Users/alice/code/bar".to_string(),
        "/Users/alice/code/bar-v2".to_string(),
    ]
    .into_iter()
    .collect();
    assert_eq!(bar_cwds, expected, "bar 必须含两个 distinct cwd");
}

#[tokio::test]
async fn scan_missing_root_returns_empty_and_no_error() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("does-not-exist");

    let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
    let mut scanner = ProjectScanner::new(fs, missing);
    let projects = scanner.scan().await.unwrap();
    assert!(projects.is_empty());
}

#[tokio::test]
async fn list_sessions_filters_non_jsonl_and_sorts_by_mtime() {
    let root = tempfile::tempdir().unwrap();
    let projects_dir = root.path().join("projects");
    let proj = projects_dir.join("-tmp-x");
    tokio::fs::create_dir_all(&proj).await.unwrap();

    // 3 个 jsonl + 1 个 txt
    write_session(&proj, "old", "/tmp/x").await;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    write_session(&proj, "mid", "/tmp/x").await;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    write_session(&proj, "new", "/tmp/x").await;
    tokio::fs::write(proj.join("readme.txt"), b"hi")
        .await
        .unwrap();

    let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
    let mut scanner = ProjectScanner::new(fs, projects_dir);
    scanner.scan().await.unwrap();

    let pinned: BTreeSet<String> = ["mid".to_string()].into_iter().collect();
    let sessions = scanner.list_sessions("-tmp-x", &pinned).await.unwrap();
    assert_eq!(sessions.len(), 3);
    assert_eq!(sessions[0].id, "new");
    assert_eq!(sessions[1].id, "mid");
    assert_eq!(sessions[2].id, "old");
    assert!(sessions[1].is_pinned);
    assert!(!sessions[0].is_pinned);
}

#[tokio::test]
async fn list_sessions_many_sessions_keep_order_across_cursor_pages() {
    let root = tempfile::tempdir().unwrap();
    let projects_dir = root.path().join("projects");
    let proj = projects_dir.join("-tmp-many");
    tokio::fs::create_dir_all(&proj).await.unwrap();

    for idx in 0..12 {
        write_session(&proj, &format!("s{idx:02}"), "/tmp/many").await;
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }

    let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
    let mut scanner = ProjectScanner::new(fs, projects_dir);
    scanner.scan().await.unwrap();

    let pinned = BTreeSet::new();
    let sessions = scanner.list_sessions("-tmp-many", &pinned).await.unwrap();
    let all_ids: Vec<_> = sessions.iter().map(|s| s.id.as_str()).collect();
    let paged_ids: Vec<_> = sessions
        .chunks(5)
        .flat_map(|chunk| chunk.iter().map(|s| s.id.as_str()))
        .collect();

    assert_eq!(sessions.len(), 12);
    assert_eq!(all_ids, paged_ids);
    assert_eq!(all_ids.first(), Some(&"s11"));
    assert_eq!(all_ids.last(), Some(&"s00"));
}

/// spec：`ipc-data-api::Contract test asserts get_session_detail does not cross project boundary`。
///
/// 通过 spy `FileSystemProvider` 包装计数：调 `scanner.list_sessions("P_A", ..)`
/// 只 `read_dir` `P_A` 一个目录，**SHALL NOT** 触及兄弟 project `P_B` / `P_C`。
/// 由于 `LocalDataApi::get_session_detail` 现走 `fs.stat(single_jsonl)` 直达，
/// 此处验证 scanner 的 `list_sessions` 不内部全扫即可联防"不跨 project 边界"。
#[tokio::test]
async fn list_sessions_does_not_cross_project_boundary() {
    use cdt_discover::{DirEntry, FsError, FsKind, FsMetadata};
    use std::path::Path;
    use std::sync::Mutex;

    #[derive(Default)]
    struct CallLog {
        read_dirs: Vec<PathBuf>,
        stats: Vec<PathBuf>,
        read_lines: Vec<PathBuf>,
        read_to_strings: Vec<PathBuf>,
    }

    struct SpyFs {
        inner: cdt_discover::LocalFileSystemProvider,
        log: Arc<Mutex<CallLog>>,
    }

    #[async_trait::async_trait]
    impl FileSystemProvider for SpyFs {
        fn kind(&self) -> FsKind {
            self.inner.kind()
        }
        async fn exists(&self, path: &Path) -> bool {
            self.inner.exists(path).await
        }
        async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
            self.log.lock().unwrap().read_dirs.push(path.to_path_buf());
            self.inner.read_dir(path).await
        }
        async fn read_dir_with_metadata(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
            self.log.lock().unwrap().read_dirs.push(path.to_path_buf());
            self.inner.read_dir_with_metadata(path).await
        }
        async fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
            self.log
                .lock()
                .unwrap()
                .read_to_strings
                .push(path.to_path_buf());
            self.inner.read_to_string(path).await
        }
        async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError> {
            self.log.lock().unwrap().stats.push(path.to_path_buf());
            self.inner.stat(path).await
        }
        async fn read_lines_head(&self, path: &Path, max: usize) -> Result<Vec<String>, FsError> {
            self.log.lock().unwrap().read_lines.push(path.to_path_buf());
            self.inner.read_lines_head(path, max).await
        }
        async fn open_read(
            &self,
            path: &Path,
        ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, FsError> {
            self.inner.open_read(path).await
        }
        async fn write_atomic(&self, path: &Path, content: &[u8]) -> Result<(), FsError> {
            self.inner.write_atomic(path, content).await
        }
        async fn create_dir_all(&self, path: &Path) -> Result<(), FsError> {
            self.inner.create_dir_all(path).await
        }
        async fn remove_file(&self, path: &Path) -> Result<(), FsError> {
            self.inner.remove_file(path).await
        }
    }

    let root = tempfile::tempdir().unwrap();
    let projects_dir = root.path().join("projects");
    // 铺 3 个 project × 2 session
    for p in &["-P-A", "-P-B", "-P-C"] {
        let proj = projects_dir.join(p);
        tokio::fs::create_dir_all(&proj).await.unwrap();
        write_session(&proj, "s1", &format!("/path{p}")).await;
        write_session(&proj, "s2", &format!("/path{p}/sub")).await;
    }

    let log = Arc::new(Mutex::new(CallLog::default()));
    let spy: Arc<dyn FileSystemProvider> = Arc::new(SpyFs {
        inner: cdt_discover::LocalFileSystemProvider::new(),
        log: log.clone(),
    });
    let scanner = ProjectScanner::new(spy, projects_dir.clone());
    let pinned = BTreeSet::new();
    let sessions = scanner.list_sessions("-P-A", &pinned).await.unwrap();
    assert_eq!(sessions.len(), 2);

    let target_dir = projects_dir.join("-P-A");
    let log = log.lock().unwrap();

    // read_dir 只对 target project 目录调一次（read_dir_with_metadata 在 spy 中
    // 也归入 read_dirs）
    let unrelated_reads: Vec<&PathBuf> = log
        .read_dirs
        .iter()
        .filter(|p| !p.starts_with(&target_dir))
        .collect();
    assert!(
        unrelated_reads.is_empty(),
        "read_dir SHALL NOT 触及兄弟 project 目录，但有：{unrelated_reads:?}"
    );

    // read_lines_head（head-read 提取 cwd）SHALL 只命中 target 目录下的 jsonl
    let unrelated_heads: Vec<&PathBuf> = log
        .read_lines
        .iter()
        .filter(|p| !p.starts_with(&target_dir))
        .collect();
    assert!(
        unrelated_heads.is_empty(),
        "read_lines_head SHALL 不读取 target 之外的文件，但有：{unrelated_heads:?}"
    );

    // 不应触发任何全文件 read_to_string（fixture cwd 全在头 20 行内）
    assert!(
        log.read_to_strings.is_empty(),
        "fixture cwd 在前 20 行内 SHALL NOT 触发全文件读取：{:?}",
        log.read_to_strings
    );
}

#[tokio::test]
async fn list_sessions_includes_cwd_per_session() {
    let root = tempfile::tempdir().unwrap();
    let projects_dir = root.path().join("projects");
    let proj = projects_dir.join("-tmp-multi-cwd");
    tokio::fs::create_dir_all(&proj).await.unwrap();

    write_session(&proj, "s1", "/repo/main").await;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    write_session(&proj, "s2", "/repo/.claude/worktrees/feat-x").await;

    let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_dir);
    let pinned = BTreeSet::new();
    let sessions = scanner
        .list_sessions("-tmp-multi-cwd", &pinned)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 2);
    // 按 mtime 降序：s2 在前
    assert_eq!(sessions[0].id, "s2");
    assert_eq!(
        sessions[0].cwd.as_deref(),
        Some("/repo/.claude/worktrees/feat-x")
    );
    assert_eq!(sessions[1].id, "s1");
    assert_eq!(sessions[1].cwd.as_deref(), Some("/repo/main"));
}

#[tokio::test]
async fn scan_uses_cwd_beyond_head_window_for_local_files() {
    let root = tempfile::tempdir().unwrap();
    let projects_dir = root.path().join("projects");
    let proj = projects_dir.join("-Users-alice-encoded");
    tokio::fs::create_dir_all(&proj).await.unwrap();

    let mut lines = Vec::new();
    for idx in 0..25 {
        lines.push(format!(
            r#"{{"type":"user","uuid":"noise-{idx}","timestamp":"2026-01-01T00:00:00Z","message":{{"role":"user","content":"hi"}}}}"#,
        ));
    }
    lines.push(
        r#"{"type":"user","uuid":"real-cwd","cwd":"/Users/alice/real-worktree","timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"hi"}}"#
            .to_string(),
    );
    tokio::fs::write(proj.join("s1.jsonl"), format!("{}\n", lines.join("\n")))
        .await
        .unwrap();

    let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
    let mut scanner = ProjectScanner::new(fs, projects_dir);
    let projects = scanner.scan().await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(
        projects[0].path,
        PathBuf::from("/Users/alice/real-worktree")
    );
}

#[tokio::test]
async fn historical_worktree_dir_without_cwd_keeps_parent_repo_path() {
    let root = tempfile::tempdir().unwrap();
    let projects_dir = root.path().join("projects");
    let repo_cwd = "/Users/alice/claude-devtools-rs";
    let repo_id = encode_path(repo_cwd);
    let repo = projects_dir.join(&repo_id);
    tokio::fs::create_dir_all(&repo).await.unwrap();
    write_session(&repo, "main", repo_cwd).await;

    let historical_id =
        encode_path("/Users/alice/claude-devtools-rs/.claude/worktrees/rosetta-detect");
    let proj = projects_dir.join(&historical_id);
    tokio::fs::create_dir_all(&proj).await.unwrap();
    tokio::fs::write(
        proj.join("s1.jsonl"),
        "{\"type\":\"user\",\"uuid\":\"u\",\"timestamp\":\"2026-01-01T00:00:00Z\",\"message\":{\"role\":\"user\",\"content\":\"hi\"}}\n",
    )
    .await
    .unwrap();

    let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
    let mut scanner = ProjectScanner::new(fs, projects_dir);
    let projects = scanner.scan().await.unwrap();
    let historical = projects.iter().find(|p| p.id == historical_id).unwrap();
    assert_eq!(
        historical.path,
        PathBuf::from("/Users/alice/claude-devtools-rs/.claude/worktrees/rosetta-detect")
    );
}

#[tokio::test]
async fn decode_path_fallback_used_when_no_cwd_in_sessions() {
    let root = tempfile::tempdir().unwrap();
    let projects_dir = root.path().join("projects");
    let proj = projects_dir.join("-Users-alice-blank");
    tokio::fs::create_dir_all(&proj).await.unwrap();
    // 一个 JSONL，不含 cwd 字段 → resolver 会回退到 decode_path
    tokio::fs::write(
        proj.join("s1.jsonl"),
        "{\"type\":\"user\",\"uuid\":\"u\",\"timestamp\":\"2026-01-01T00:00:00Z\",\"message\":{\"role\":\"user\",\"content\":\"hi\"}}\n",
    )
    .await
    .unwrap();

    let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
    let mut scanner = ProjectScanner::new(fs, projects_dir);
    let projects = scanner.scan().await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].path, PathBuf::from("/Users/alice/blank"));
}

/// spec project-discovery §"Expose session cwd for downstream display"
/// Scenario "缺 cwd session 暴露 None"：jsonl 不含任何 cwd 字段时
/// `Session.cwd` SHALL 为 `None`，且 IPC 序列化 SHALL 省略 `cwd` 键。
#[tokio::test]
async fn list_sessions_returns_none_cwd_when_jsonl_lacks_cwd_field() {
    let root = tempfile::tempdir().unwrap();
    let projects_dir = root.path().join("projects");
    let proj = projects_dir.join("-tmp-no-cwd");
    tokio::fs::create_dir_all(&proj).await.unwrap();

    // 不含 cwd 字段的 jsonl —— 单一 user 消息，仅 type / uuid / timestamp / message。
    let line = r#"{"type":"user","uuid":"no-cwd","timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"hi"}}"#;
    tokio::fs::write(proj.join("s1.jsonl"), format!("{line}\n"))
        .await
        .unwrap();

    let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_dir);
    let pinned = BTreeSet::new();
    let sessions = scanner.list_sessions("-tmp-no-cwd", &pinned).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert!(
        sessions[0].cwd.is_none(),
        "缺 cwd session SHALL 让 Session.cwd 为 None，得到 {:?}",
        sessions[0].cwd
    );

    let json = serde_json::to_value(&sessions[0]).unwrap();
    assert!(
        json.get("cwd").is_none(),
        "Session.cwd = None 时 SHALL 省略 cwd 键，得到 {json}"
    );
}

/// spec project-discovery §"Expose session cwd for downstream display"
/// Scenario "`cdt-core::Session` 不含 `cwd_relative_to_repo_root`"——
/// 该派生字段由 `Worktree` 持有，scanner 阶段不重走 repo 解析。
#[test]
fn cdt_core_session_does_not_have_cwd_relative_to_repo_root_field() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("cdt-core")
        .join("src")
        .join("project.rs");
    let body = std::fs::read_to_string(&path).expect("cdt-core/src/project.rs SHALL exist");

    // 简单扫描 Session struct block 内字段名——区分 Worktree（允许含该字段）。
    let session_idx = body
        .find("pub struct Session ")
        .or_else(|| body.find("pub struct Session{"))
        .expect("`pub struct Session` SHALL exist in cdt-core/src/project.rs");
    let rest = &body[session_idx..];
    // 找到该 struct 结束的右括号（粗略截到下一个 `}` 行首）。
    let end = rest.find("\n}").map_or(body.len(), |i| session_idx + i);
    let session_block = &body[session_idx..end];

    assert!(
        !session_block.contains("cwd_relative_to_repo_root"),
        "cdt-core::Session SHALL NOT contain field `cwd_relative_to_repo_root` \
         (it belongs to Worktree); spec project-discovery `Expose session cwd` Scenario \
         #5. block:\n{session_block}"
    );
}

// =============================================================================
// SSH 模式 channel-dead fail-fast 回归（修 GitHub issue #231 #2）
// =============================================================================
//
// 旧版 `scan` SSH 分支单 project read_dir 报 `session closed` 时只 warn +
// continue，错误信号不传导到 IPC caller，用户 sidebar 看到不完整列表。
// 新版按 `FsError::is_likely_channel_dead()` fail-fast，channel-dead 立即
// abort 整轮 scan 返 Err。
//
// 这些测试用一个 fake SSH provider（`kind() == FsKind::Ssh`）注入特定错误
// 到第一个 sub-project 的 read_dir，验证 fail-fast 路径。

mod ssh_channel_dead_fail_fast {
    use super::*;
    use cdt_discover::{DirEntry, FsError, FsKind, FsMetadata};
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Mutex;

    /// 注入 per-path 错误的 fake SSH provider；命中错误返 Err，否则委托给
    /// 内部 `LocalFileSystemProvider` 走真实 fs。`kind()` 强制返 SSH。
    struct FakeSshFs {
        local: cdt_discover::LocalFileSystemProvider,
        /// path string → 错误工厂函数（多次调用每次返新错误）
        errors: Mutex<HashMap<PathBuf, Box<dyn Fn() -> FsError + Send + Sync>>>,
    }

    impl FakeSshFs {
        fn new(local: cdt_discover::LocalFileSystemProvider) -> Self {
            Self {
                local,
                errors: Mutex::new(HashMap::new()),
            }
        }

        fn inject_error<F>(&self, path: PathBuf, factory: F)
        where
            F: Fn() -> FsError + Send + Sync + 'static,
        {
            self.errors.lock().unwrap().insert(path, Box::new(factory));
        }

        fn maybe_inject(&self, path: &Path) -> Option<FsError> {
            self.errors.lock().unwrap().get(path).map(|f| f())
        }
    }

    #[async_trait::async_trait]
    impl FileSystemProvider for FakeSshFs {
        fn kind(&self) -> FsKind {
            FsKind::Ssh
        }
        async fn exists(&self, path: &Path) -> bool {
            self.local.exists(path).await
        }
        async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
            if let Some(err) = self.maybe_inject(path) {
                return Err(err);
            }
            self.local.read_dir(path).await
        }
        async fn read_dir_with_metadata(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
            if let Some(err) = self.maybe_inject(path) {
                return Err(err);
            }
            self.local.read_dir_with_metadata(path).await
        }
        async fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
            self.local.read_to_string(path).await
        }
        async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError> {
            self.local.stat(path).await
        }
        async fn read_lines_head(&self, path: &Path, max: usize) -> Result<Vec<String>, FsError> {
            self.local.read_lines_head(path, max).await
        }
        async fn open_read(
            &self,
            path: &Path,
        ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, FsError> {
            self.local.open_read(path).await
        }
        async fn write_atomic(&self, path: &Path, content: &[u8]) -> Result<(), FsError> {
            self.local.write_atomic(path, content).await
        }
        async fn create_dir_all(&self, path: &Path) -> Result<(), FsError> {
            self.local.create_dir_all(path).await
        }
        async fn remove_file(&self, path: &Path) -> Result<(), FsError> {
            self.local.remove_file(path).await
        }
    }

    /// 铺 3 个 project 目录，每个含 1 个 session jsonl（让 `scan_project_dir`
    /// 真正进入 `read_dir_with_metadata` 调用，从而触发注入的错误）。
    async fn setup_three_projects(projects_dir: &Path) {
        for name in &["-P-A", "-P-B", "-P-C"] {
            let proj = projects_dir.join(name);
            tokio::fs::create_dir_all(&proj).await.unwrap();
            super::write_session(&proj, "s1", &format!("/path{name}")).await;
        }
    }

    /// spec `project-discovery::Scan Claude projects directory` Scenario
    /// "SSH channel-dead error aborts full scan instead of silent skip"。
    #[tokio::test]
    async fn ssh_channel_dead_aborts_scan() {
        let root = tempfile::tempdir().unwrap();
        let projects_dir = root.path().join("projects");
        setup_three_projects(&projects_dir).await;

        let fake = Arc::new(FakeSshFs::new(LocalFileSystemProvider::new()));
        // 给第一个被扫到的 project 注入 Disconnected 错误
        // dirs 是 read_dir 拿到后排序前的顺序——LocalFileSystemProvider 按
        // os 默认顺序返，3 个 fixture 里挑任一即可
        fake.inject_error(projects_dir.join("-P-A"), || FsError::Disconnected {
            path: PathBuf::from("/p"),
            reason: "channel closed".into(),
        });
        fake.inject_error(projects_dir.join("-P-B"), || FsError::Disconnected {
            path: PathBuf::from("/p"),
            reason: "channel closed".into(),
        });
        fake.inject_error(projects_dir.join("-P-C"), || FsError::Disconnected {
            path: PathBuf::from("/p"),
            reason: "channel closed".into(),
        });

        let fs: Arc<dyn FileSystemProvider> = fake;
        let mut scanner = ProjectScanner::new(fs, projects_dir);
        let result = scanner.scan().await;

        assert!(
            result.is_err(),
            "channel-dead error SHALL abort scan, got: {result:?}",
        );
    }

    /// spec `project-discovery::Scan Claude projects directory` Scenario
    /// "SSH `TransientExhausted` with transport-dead keyword aborts scan"。
    #[tokio::test]
    async fn ssh_transient_exhausted_with_transport_dead_aborts_scan() {
        let root = tempfile::tempdir().unwrap();
        let projects_dir = root.path().join("projects");
        setup_three_projects(&projects_dir).await;

        let fake = Arc::new(FakeSshFs::new(LocalFileSystemProvider::new()));
        // 任一 project 触发 channel-dead transient exhausted → abort
        for name in &["-P-A", "-P-B", "-P-C"] {
            fake.inject_error(projects_dir.join(name), || FsError::TransientExhausted {
                path: PathBuf::from("/p"),
                attempts: 3,
                last_reason: "session closed".into(),
            });
        }

        let fs: Arc<dyn FileSystemProvider> = fake;
        let mut scanner = ProjectScanner::new(fs, projects_dir);
        let result = scanner.scan().await;

        assert!(
            result.is_err(),
            "TransientExhausted with transport-dead keyword SHALL abort scan, got: {result:?}",
        );
    }

    /// spec `project-discovery::Scan Claude projects directory` Scenario
    /// "SSH per-project pure timeout `TransientExhausted` 仍 silent skip 不 abort"。
    #[tokio::test]
    async fn ssh_pure_timeout_does_not_abort() {
        let root = tempfile::tempdir().unwrap();
        let projects_dir = root.path().join("projects");
        setup_three_projects(&projects_dir).await;

        let fake = Arc::new(FakeSshFs::new(LocalFileSystemProvider::new()));
        // 仅 -P-A 注入 pure timeout（不含 transport-dead 关键字）→ silent skip
        // -P-B 与 -P-C 仍真实扫描，scan 整体应 Ok 且包含两个 project
        fake.inject_error(projects_dir.join("-P-A"), || FsError::TransientExhausted {
            path: PathBuf::from("/p"),
            attempts: 3,
            last_reason: "timeout".into(),
        });

        let fs: Arc<dyn FileSystemProvider> = fake;
        let mut scanner = ProjectScanner::new(fs, projects_dir);
        let projects = scanner
            .scan()
            .await
            .expect("pure timeout SHALL NOT abort scan");

        // -P-A 被 silent skip，-P-B / -P-C 仍出现
        let ids: Vec<&String> = projects.iter().map(|p| &p.id).collect();
        assert!(
            !ids.iter().any(|id| id.as_str() == "-P-A"),
            "P-A SHALL be silent-skipped, got ids: {ids:?}",
        );
        assert!(
            ids.iter().any(|id| id.as_str() == "-P-B"),
            "P-B SHALL be present, got ids: {ids:?}",
        );
        assert!(
            ids.iter().any(|id| id.as_str() == "-P-C"),
            "P-C SHALL be present, got ids: {ids:?}",
        );
    }

    /// spec `project-discovery::Scan Claude projects directory` Scenario
    /// "SSH per-project `NotFound` 仍 silent skip 不 abort"。
    #[tokio::test]
    async fn ssh_notfound_does_not_abort() {
        let root = tempfile::tempdir().unwrap();
        let projects_dir = root.path().join("projects");
        setup_three_projects(&projects_dir).await;

        let fake = Arc::new(FakeSshFs::new(LocalFileSystemProvider::new()));
        fake.inject_error(projects_dir.join("-P-A"), || {
            FsError::NotFound(PathBuf::from("/p"))
        });

        let fs: Arc<dyn FileSystemProvider> = fake;
        let mut scanner = ProjectScanner::new(fs, projects_dir);
        let projects = scanner.scan().await.expect("NotFound SHALL NOT abort scan");

        let ids: Vec<&String> = projects.iter().map(|p| &p.id).collect();
        assert!(
            !ids.iter().any(|id| id.as_str() == "-P-A"),
            "P-A NotFound SHALL be silent-skipped, got ids: {ids:?}",
        );
        assert!(
            ids.iter().any(|id| id.as_str() == "-P-B"),
            "P-B SHALL be present, got ids: {ids:?}",
        );
    }

    /// spec `fs-abstraction::FsError 提供错误语义元方法` Scenario `Io` `BrokenPipe`
    /// 触发 channel-dead——经由 `project_scanner` 集成路径再验一次（双层契约
    /// 锚定）。
    #[tokio::test]
    async fn ssh_io_broken_pipe_aborts_scan() {
        let root = tempfile::tempdir().unwrap();
        let projects_dir = root.path().join("projects");
        setup_three_projects(&projects_dir).await;

        let fake = Arc::new(FakeSshFs::new(LocalFileSystemProvider::new()));
        for name in &["-P-A", "-P-B", "-P-C"] {
            fake.inject_error(projects_dir.join(name), || FsError::Io {
                path: PathBuf::from("/p"),
                source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken"),
            });
        }

        let fs: Arc<dyn FileSystemProvider> = fake;
        let mut scanner = ProjectScanner::new(fs, projects_dir);
        let result = scanner.scan().await;

        assert!(
            result.is_err(),
            "Io BrokenPipe SHALL abort scan, got: {result:?}",
        );
    }
}
