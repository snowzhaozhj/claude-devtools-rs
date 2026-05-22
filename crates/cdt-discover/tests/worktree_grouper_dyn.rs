//! `WorktreeGrouper::new_dyn(Arc<dyn GitIdentityResolver>)` 入口单测。
//!
//! change `backend-policy-struct` design D7 + tasks 2.4。验证：
//! 1. `Arc<dyn GitIdentityResolver>` 通过 blanket impl 满足 trait
//! 2. `WorktreeGrouper::new_dyn(arc)` 返实例可正常调 `group_by_repository`

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use cdt_core::{Project, RepositoryIdentity};
use cdt_discover::{GitIdentityResolver, WorktreeGrouper};

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
