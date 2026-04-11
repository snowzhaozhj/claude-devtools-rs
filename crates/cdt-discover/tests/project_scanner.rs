//! project-discovery capability 的端到端测试。
//!
//! 覆盖 `openspec/specs/project-discovery/spec.md` 里 5 条 Requirement
//! 的主要 scenario，外加 port-project-discovery change 里两条 ADDED
//! Requirement（`FileSystemProvider` 抽象、composite ID 形态）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cdt_discover::{
    FileSystemProvider, LocalFileSystemProvider, ProjectScanner, SubprojectRegistry,
};

async fn write_session(dir: &Path, session_id: &str, cwd: &str) {
    let line = format!(
        r#"{{"type":"user","uuid":"{session_id}","cwd":"{cwd}","timestamp":"2026-01-01T00:00:00Z","message":{{"role":"user","content":"hi"}}}}"#,
    );
    tokio::fs::write(dir.join(format!("{session_id}.jsonl")), format!("{line}\n"))
        .await
        .unwrap();
}

#[tokio::test]
async fn scan_splits_subprojects_and_sorts_by_recent_activity() {
    let root = tempfile::tempdir().unwrap();
    let projects_dir = root.path().join("projects");
    tokio::fs::create_dir_all(&projects_dir).await.unwrap();

    // 一个单 cwd 目录 → 单 Project
    let foo_dir = projects_dir.join("-Users-alice-code-foo");
    tokio::fs::create_dir_all(&foo_dir).await.unwrap();
    write_session(&foo_dir, "a1", "/Users/alice/code/foo").await;
    write_session(&foo_dir, "a2", "/Users/alice/code/foo").await;

    // 一个多 cwd 目录 → 两个 composite Project
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

    // foo (1) + bar 拆 2 = 3，empty 跳过
    assert_eq!(projects.len(), 3, "got: {projects:#?}");

    // bar 的两个 Project 应该是 composite ID（含 `::`），foo 的是 plain。
    let has_foo_plain = projects.iter().any(|p| p.id == "-Users-alice-code-foo");
    assert!(has_foo_plain, "foo 必须用 plain ID");

    let bar_composites: Vec<_> = projects
        .iter()
        .filter(|p| p.id.starts_with("-Users-alice-code-bar::"))
        .collect();
    assert_eq!(bar_composites.len(), 2, "bar 必须拆出 2 个 composite");

    for p in &bar_composites {
        let (_, hash) = p.id.split_once("::").unwrap();
        assert_eq!(hash.len(), 8);
        assert!(
            hash.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    // 两次 scan 的 composite ID 必须稳定
    let mut scanner2 = ProjectScanner::new(
        Arc::new(LocalFileSystemProvider::new()),
        projects_dir.clone(),
    );
    let projects2 = scanner2.scan().await.unwrap();
    let ids1: BTreeSet<_> = projects.iter().map(|p| p.id.clone()).collect();
    let ids2: BTreeSet<_> = projects2.iter().map(|p| p.id.clone()).collect();
    assert_eq!(ids1, ids2, "composite ID 必须 deterministic");
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
async fn composite_id_is_deterministic_across_registries() {
    let id1 = SubprojectRegistry::compose_id("-tmp-x", Path::new("/tmp/x/sub-a"));
    let id2 = SubprojectRegistry::compose_id("-tmp-x", Path::new("/tmp/x/sub-a"));
    assert_eq!(id1, id2);
    assert!(id1.starts_with("-tmp-x::"));
    assert_eq!(id1.len(), "-tmp-x::".len() + 8);
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
