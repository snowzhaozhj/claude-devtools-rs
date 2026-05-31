//! `WorktreeGrouper::new_dyn(Arc<dyn GitIdentityResolver>)` 入口单测。
//!
//! change `backend-policy-struct` design D7 + tasks 2.4。验证：
//! 1. `Arc<dyn GitIdentityResolver>` 通过 blanket impl 满足 trait
//! 2. `WorktreeGrouper::new_dyn(arc)` 返实例可正常调 `group_by_repository`
//! 3. Semaphore 限流确保并发度 ≤ `GROUPER_CONCURRENCY_LIMIT`

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use cdt_core::{Project, RepositoryIdentity};
use cdt_discover::{GitIdentityResolver, RepoLookup, WorktreeGrouper};

struct StaticResolver {
    identity_id: &'static str,
}

#[async_trait]
impl GitIdentityResolver for StaticResolver {
    async fn resolve_identity(&self, _path: &Path) -> Option<RepositoryIdentity> {
        Some(RepositoryIdentity {
            id: self.identity_id.to_owned(),
            name: "test-repo".to_owned(),
        })
    }

    async fn get_branch(&self, _path: &Path) -> Option<String> {
        Some("main".to_owned())
    }

    async fn is_main_worktree(&self, _path: &Path) -> bool {
        true
    }
}

#[tokio::test]
async fn new_dyn_via_arc_blanket_impl_groups_projects() {
    let resolver: Arc<dyn GitIdentityResolver> = Arc::new(StaticResolver {
        identity_id: "static-repo-id",
    });
    let grouper = WorktreeGrouper::new_dyn(resolver);
    let projects = vec![Project {
        id: "p1".to_owned(),
        name: "p1".to_owned(),
        path: PathBuf::from("/tmp/p1"),
        sessions: vec![],
        most_recent_session: None,
        created_at: None,
        distinct_cwds: vec![],
    }];
    let groups = grouper.group_by_repository(projects).await;
    assert_eq!(groups.len(), 1, "single project SHALL group as one repo");
    assert_eq!(
        groups[0].identity.as_ref().map(|i| i.id.as_str()),
        Some("static-repo-id"),
        "Arc<dyn> 通过 blanket impl forward 到 inner resolver"
    );
}

struct ConcurrencyTracker {
    current: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
}

#[async_trait]
impl GitIdentityResolver for ConcurrencyTracker {
    async fn resolve_identity(&self, _path: &Path) -> Option<RepositoryIdentity> {
        None
    }
    async fn get_branch(&self, _path: &Path) -> Option<String> {
        None
    }
    async fn is_main_worktree(&self, _path: &Path) -> bool {
        true
    }
    async fn resolve_all(&self, _path: &Path) -> RepoLookup {
        let c = self.current.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(c, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        self.current.fetch_sub(1, Ordering::SeqCst);
        RepoLookup::default()
    }
}

/// Spec: `project-discovery::Grouper 并发度不超过上限`
#[tokio::test]
async fn grouper_concurrency_stays_within_limit() {
    let peak = Arc::new(AtomicUsize::new(0));
    let current = Arc::new(AtomicUsize::new(0));

    let resolver: Arc<dyn GitIdentityResolver> = Arc::new(ConcurrencyTracker {
        current: Arc::clone(&current),
        peak: Arc::clone(&peak),
    });
    let grouper = WorktreeGrouper::new_dyn(resolver);

    let projects: Vec<Project> = (0..30)
        .map(|i| Project {
            id: format!("p{i}"),
            name: format!("p{i}"),
            path: PathBuf::from(format!("/tmp/p{i}")),
            sessions: vec![],
            most_recent_session: None,
            created_at: None,
            distinct_cwds: vec![],
        })
        .collect();

    let groups = grouper.group_by_repository(projects).await;
    assert_eq!(groups.len(), 30, "30 independent projects = 30 groups");

    let observed_peak = peak.load(Ordering::SeqCst);
    assert!(
        observed_peak <= 8,
        "peak concurrency {observed_peak} SHALL be <= GROUPER_CONCURRENCY_LIMIT(8)"
    );
    assert!(
        observed_peak >= 2,
        "peak concurrency {observed_peak} should show some parallelism"
    );
}
