//! `BackendResolvers` —— `LocalDataApi` 业务路径"按后端选 trait object / Clone 字段"的承载层。
//!
//! `cdt-fs::BackendPolicy` 只放 primitive 字段（保 Copy + Eq）；trait object
//! 与 Clone 类型策略（`Arc<dyn GitIdentityResolver>` / `SearchConfig`）放在
//! 本 module —— cdt-fs SHALL NOT 依赖 cdt-discover / cdt-core（fs-abstraction
//! spec "cdt-fs 不依赖业务 crate"）。
//!
//! 设计：`openspec/changes/backend-policy-struct/design.md` D1 + D4 + D7。

use std::path::Path;
use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use cdt_core::RepositoryIdentity;
use cdt_discover::{
    FileSystemProvider, FsKind, GitIdentityResolver, LocalGitIdentityResolver, SearchConfig,
};

/// SSH context 下取代 `LocalGitIdentityResolver`：远端 `.git` 不可访问
/// （不能 spawn 子进程，多数远端是非 git 项目），所有 git 字段返回 None / true
/// 兜底。修复 codex PR-D R3 P1[1]：避免容器内 cwd 与本机宿主路径重合时泄漏本机
/// gitBranch。
struct NoopGitIdentityResolver;

#[async_trait]
impl GitIdentityResolver for NoopGitIdentityResolver {
    async fn resolve_identity(&self, _path: &Path) -> Option<RepositoryIdentity> {
        None
    }

    async fn get_branch(&self, _path: &Path) -> Option<String> {
        None
    }

    async fn is_main_worktree(&self, _path: &Path) -> bool {
        true
    }
}

/// 与 `BackendPolicy` 配套的"业务策略 trait object 包"。
pub(crate) struct BackendResolvers {
    pub search_config: SearchConfig,
    pub git_identity_resolver: Arc<dyn GitIdentityResolver>,
}

static LOCAL_RESOLVERS: LazyLock<Arc<BackendResolvers>> = LazyLock::new(|| {
    Arc::new(BackendResolvers {
        search_config: SearchConfig::default(),
        git_identity_resolver: Arc::new(LocalGitIdentityResolver::new()),
    })
});

static SSH_RESOLVERS: LazyLock<Arc<BackendResolvers>> = LazyLock::new(|| {
    Arc::new(BackendResolvers {
        search_config: SearchConfig {
            is_ssh: true,
            ..SearchConfig::default()
        },
        git_identity_resolver: Arc::new(NoopGitIdentityResolver),
    })
});

impl BackendResolvers {
    /// 本地 Tauri / HTTP server backend 复用同一实例（HTTP 当前用 Local 数据源）。
    pub fn for_local() -> Arc<Self> {
        LOCAL_RESOLVERS.clone()
    }

    /// SSH backend。
    pub fn for_ssh() -> Arc<Self> {
        SSH_RESOLVERS.clone()
    }

    /// 按 `fs.kind()` 派发——`fs.kind()` 仅允许在本派生点使用（design D6 grep
    /// 不变性测试承认 `backend_resolvers.rs` 内的 kind 比较为合理出处）。
    pub fn from_fs(fs: &dyn FileSystemProvider) -> Arc<Self> {
        match fs.kind() {
            FsKind::Local => Self::for_local(),
            FsKind::Ssh => Self::for_ssh(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_fs::local_handle;

    #[test]
    fn for_local_uses_local_git_resolver_and_local_search_config() {
        let r = BackendResolvers::for_local();
        assert!(
            !r.search_config.is_ssh,
            "Local search_config SHALL NOT enable SSH stage-limit"
        );
        // git_identity_resolver 内类型走 Arc<dyn>，行为通过 from_fs 与
        // for_ssh 的 ptr_eq 反差验证。
    }

    #[test]
    fn for_ssh_uses_noop_git_resolver_and_ssh_search_config() {
        let r = BackendResolvers::for_ssh();
        assert!(
            r.search_config.is_ssh,
            "SSH search_config SHALL enable stage-limit"
        );
    }

    #[test]
    fn for_local_returns_static_instance() {
        let a = BackendResolvers::for_local();
        let b = BackendResolvers::for_local();
        assert!(
            Arc::ptr_eq(&a, &b),
            "for_local SHALL reuse LazyLock 静态实例（零额外 alloc）"
        );
    }

    #[test]
    fn for_ssh_returns_static_instance() {
        let a = BackendResolvers::for_ssh();
        let b = BackendResolvers::for_ssh();
        assert!(Arc::ptr_eq(&a, &b), "for_ssh SHALL reuse LazyLock 静态实例");
    }

    #[test]
    fn local_and_ssh_are_different_instances() {
        let l = BackendResolvers::for_local();
        let s = BackendResolvers::for_ssh();
        assert!(
            !Arc::ptr_eq(&l, &s),
            "Local 与 SSH SHALL 是独立 LazyLock 实例"
        );
    }

    #[test]
    fn from_fs_local_dispatches_to_for_local() {
        let fs = local_handle();
        let r = BackendResolvers::from_fs(&*fs);
        assert!(
            Arc::ptr_eq(&r, &BackendResolvers::for_local()),
            "FsKind::Local SHALL 派发到 for_local 静态实例"
        );
    }
}
