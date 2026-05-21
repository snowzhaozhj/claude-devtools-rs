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
