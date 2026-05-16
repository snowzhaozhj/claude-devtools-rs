//! Cold-start baseline：测 `ProjectScanner::scan()` 在真实 `~/.claude/projects/`
//! 下的耗时，为首屏冷启动性能优化提供量化依据。
//!
//! 跑法：
//! ```sh
//! cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture
//! ```
//!
//! 不进 CI、不算回归——纯定位工具。
use std::sync::Arc;
use std::time::Instant;

use cdt_discover::fs_provider::FileSystemProvider;
use cdt_discover::{
    LocalFileSystemProvider, LocalGitIdentityResolver, ProjectScanner, WorktreeGrouper,
};

mod perf_fixture;

// 用 default runtime（Tauri 生产也是 default multi-thread），不强制 worker_threads；
// 让 bench 数据贴近真实启动场景。
#[tokio::test(flavor = "multi_thread")]
#[ignore = "依赖本机 ~/.claude/projects 真实数据，作为冷启动性能定位工具"]
async fn measure_cold_scan() {
    let projects_dir = perf_fixture::resolve_projects_dir();
    let dir = projects_dir.path();
    if !dir.exists() {
        eprintln!("[perf] skip: {} not exists", dir.display());
        return;
    }
    let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
    let mut scanner = ProjectScanner::new(fs, dir.to_path_buf());

    let t0 = Instant::now();
    let projects = scanner.scan().await.expect("scan ok");
    let cold_scan = t0.elapsed().as_millis();
    let total_sessions: usize = projects.iter().map(|p| p.sessions.len()).sum();
    assert!(total_sessions > 0, "perf corpus SHALL contain sessions");
    let project_count = projects.len();
    println!("[perf] cold scan={cold_scan}ms projects={project_count} sessions={total_sessions}");

    let projects_clone = projects.clone();
    let grouper = WorktreeGrouper::new(LocalGitIdentityResolver::new());
    let t1 = Instant::now();
    let groups = grouper.group_by_repository(projects_clone).await;
    let cold_group = t1.elapsed().as_millis();
    let group_count = groups.len();
    println!("[perf] cold grouper={cold_group}ms groups={group_count}");
    let cold_total = cold_scan + cold_group;
    println!("[perf] cold total (list_repository_groups equivalent)={cold_total}ms");

    let t2 = Instant::now();
    let _ = scanner.scan().await.expect("scan ok 2");
    let warm_scan = t2.elapsed().as_millis();
    let projects_clone2 = scanner.scan().await.expect("scan ok 3");
    let t3 = Instant::now();
    let _ = grouper.group_by_repository(projects_clone2).await;
    let warm_group = t3.elapsed().as_millis();
    println!("[perf] warm scan={warm_scan}ms grouper={warm_group}ms");
}
