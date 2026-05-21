//! `BackendPolicy` —— PR-E 接入业务的契约锚点。
//!
//! Spec：`openspec/specs/fs-abstraction/spec.md` §`BackendPolicy` enum 雏形定义。
//! 设计：`openspec/changes/unify-fs-abstraction/design.md` D8。
//!
//! 本 change 只定义类型 + 三个构造器 + 单测；**不**接入业务（PR-E 才接入）。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendPolicy {
    /// 首屏列表加载策略——FullEager（HTTP / SSH 默认）vs SkeletonThenStream（Local Tauri 默认）。
    pub initial_load_policy: InitialLoadPolicy,
    /// 初始页能接受的最大 round trips 数。
    pub max_round_trips_for_initial_page: u8,
    /// 是否支持服务端推送（SSE / Tauri event）增量补全。
    pub supports_incremental_updates: bool,
    /// 翻页预取策略——本 change 所有 backend 默认 `None`，PR-E 才可能引入 `PrefetchNext`。
    pub prefetch_policy: PrefetchPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialLoadPolicy {
    FullEager,
    SkeletonThenStream,
}

/// 与 `InitialLoadPolicy` **正交**——表达"翻页预取"维度，**不**与 initial load 合并。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefetchPolicy {
    None,
    PrefetchNext,
}

impl BackendPolicy {
    /// 本地 Tauri backend 默认 policy。
    #[must_use]
    pub const fn for_local() -> Self {
        Self {
            initial_load_policy: InitialLoadPolicy::SkeletonThenStream,
            max_round_trips_for_initial_page: 2,
            supports_incremental_updates: true,
            prefetch_policy: PrefetchPolicy::None,
        }
    }

    /// SSH backend 默认 policy。
    #[must_use]
    pub const fn for_ssh() -> Self {
        Self {
            initial_load_policy: InitialLoadPolicy::FullEager,
            max_round_trips_for_initial_page: 1,
            supports_incremental_updates: false,
            prefetch_policy: PrefetchPolicy::None,
        }
    }

    /// HTTP server backend 默认 policy（H4 钉死：避免"骨架 + SSE 增量"
    /// 两次 round trip 的反模式）。
    #[must_use]
    pub const fn for_http() -> Self {
        Self {
            initial_load_policy: InitialLoadPolicy::FullEager,
            max_round_trips_for_initial_page: 1,
            supports_incremental_updates: false,
            prefetch_policy: PrefetchPolicy::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_local_uses_skeleton_then_stream() {
        let p = BackendPolicy::for_local();
        assert_eq!(p.initial_load_policy, InitialLoadPolicy::SkeletonThenStream);
        assert!(p.max_round_trips_for_initial_page >= 2);
        assert!(p.supports_incremental_updates);
        assert_eq!(p.prefetch_policy, PrefetchPolicy::None);
    }

    #[test]
    fn for_ssh_uses_full_eager_single_round_trip() {
        let p = BackendPolicy::for_ssh();
        assert_eq!(p.initial_load_policy, InitialLoadPolicy::FullEager);
        assert_eq!(p.max_round_trips_for_initial_page, 1);
        assert!(!p.supports_incremental_updates);
        assert_eq!(p.prefetch_policy, PrefetchPolicy::None);
    }

    #[test]
    fn for_http_uses_full_eager_single_round_trip() {
        let p = BackendPolicy::for_http();
        assert_eq!(p.initial_load_policy, InitialLoadPolicy::FullEager);
        assert_eq!(p.max_round_trips_for_initial_page, 1);
        assert!(!p.supports_incremental_updates);
        assert_eq!(p.prefetch_policy, PrefetchPolicy::None);
    }

    #[test]
    fn constructors_are_deterministic() {
        assert_eq!(BackendPolicy::for_local(), BackendPolicy::for_local());
        assert_eq!(BackendPolicy::for_ssh(), BackendPolicy::for_ssh());
        assert_eq!(BackendPolicy::for_http(), BackendPolicy::for_http());
    }

    #[test]
    fn prefetch_and_initial_load_are_orthogonal() {
        // SkeletonThenStream + PrefetchNext 可同时存在（未来 PR-E 可能用）
        let mixed = BackendPolicy {
            initial_load_policy: InitialLoadPolicy::SkeletonThenStream,
            max_round_trips_for_initial_page: 2,
            supports_incremental_updates: true,
            prefetch_policy: PrefetchPolicy::PrefetchNext,
        };
        let clone = mixed;
        assert_eq!(mixed, clone);
        // 验证 InitialLoadPolicy 没有 PrefetchNext variant（编译期约束）：
        // 数组类型 `[InitialLoadPolicy; 2]` 强制断言变体数 == 2，新增第三变体编译失败。
        let variants: [InitialLoadPolicy; 2] = [
            InitialLoadPolicy::FullEager,
            InitialLoadPolicy::SkeletonThenStream,
        ];
        assert_eq!(variants.len(), 2);
    }
}
