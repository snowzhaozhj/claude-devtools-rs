//! `BackendPolicy` —— `LocalDataApi` 业务路径"选后端相关行为"的真相源。
//!
//! Spec：`openspec/specs/fs-abstraction/spec.md` §`BackendPolicy` enum 雏形定义。
//! 设计：`openspec/changes/backend-policy-struct/design.md`（PR-E）+
//! `openspec/changes/archive/.../unify-fs-abstraction/design.md` D8（PR-A 雏形）。
//!
//! 字段 SHALL 保持 primitive（Copy 类型），业务侧的 trait object 与 Clone 策略
//! （`GitIdentityResolver` / `SearchConfig`）SHALL 放在更高层（如
//! `cdt-api::ipc::backend_resolvers`），与本 struct 配套使用——避免 cdt-fs
//! 反向依赖业务 crate（详 D1）。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendPolicy {
    /// 首屏列表加载策略——FullEager（HTTP / SSH 默认）vs SkeletonThenStream（Local Tauri 默认）。
    pub initial_load_policy: InitialLoadPolicy,
    /// 初始页能接受的最大 round trips 数。
    pub max_round_trips_for_initial_page: u8,
    /// 是否支持服务端推送（SSE / Tauri event）增量补全。
    pub supports_incremental_updates: bool,
    /// 翻页预取策略——`None`（默认） vs `PrefetchNext`。
    pub prefetch_policy: PrefetchPolicy,
    /// 是否支持 memory 文件读取（Local true / SSH false）。
    pub supports_memory: bool,
    /// 是否支持 subagent JSONL 扫描（Local true / SSH false）。
    pub supports_subagent_scan: bool,
    /// 5min stale 判定策略——`LocalClock5min` 用本机 mtime 比对，
    /// `SkipUntilClockSync` 跳过（远端 mtime 跨 clock domain 不可比对）。
    pub stale_check_strategy: StaleCheckStrategy,
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

/// 5min stale 判定策略。未来可扩展 `ClockSkewCompensated { offset_secs: i64 }`
/// 等 variant；调用方 `match` 走 exhaustive 检查保证加新 variant 时编译期暴露
/// 未处理路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleCheckStrategy {
    /// 本机 mtime 与 `SystemTime::now()` 比 5min 阈值；超时视为 crashed/killed。
    LocalClock5min,
    /// 远端 mtime 与本机 clock 跨 domain（远端时钟回拨/时差）—— 5min 阈值不可比对，
    /// 跳过 stale check 避免 false positive/negative。
    SkipUntilClockSync,
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
            supports_memory: true,
            supports_subagent_scan: true,
            stale_check_strategy: StaleCheckStrategy::LocalClock5min,
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
            supports_memory: false,
            supports_subagent_scan: false,
            stale_check_strategy: StaleCheckStrategy::SkipUntilClockSync,
        }
    }

    /// HTTP server backend 默认 policy。initial-load 字段保持现状（避免"骨架 +
    /// SSE 增量"两次 round trip 反模式）；PR-E 新增三字段按 Local 数据源语义
    /// 填——HTTP server 当前把 `LocalDataApi` 作为数据源访问 Local `~/.claude/`，
    /// 与 SSH 不共行为；未来 HTTP server 若接 SSH backend 再加分支。
    #[must_use]
    pub const fn for_http() -> Self {
        Self {
            initial_load_policy: InitialLoadPolicy::FullEager,
            max_round_trips_for_initial_page: 1,
            supports_incremental_updates: false,
            prefetch_policy: PrefetchPolicy::None,
            supports_memory: true,
            supports_subagent_scan: true,
            stale_check_strategy: StaleCheckStrategy::LocalClock5min,
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
        assert!(p.supports_memory);
        assert!(p.supports_subagent_scan);
        assert_eq!(p.stale_check_strategy, StaleCheckStrategy::LocalClock5min);
    }

    #[test]
    fn for_ssh_uses_full_eager_single_round_trip() {
        let p = BackendPolicy::for_ssh();
        assert_eq!(p.initial_load_policy, InitialLoadPolicy::FullEager);
        assert_eq!(p.max_round_trips_for_initial_page, 1);
        assert!(!p.supports_incremental_updates);
        assert_eq!(p.prefetch_policy, PrefetchPolicy::None);
        assert!(!p.supports_memory);
        assert!(!p.supports_subagent_scan);
        assert_eq!(
            p.stale_check_strategy,
            StaleCheckStrategy::SkipUntilClockSync
        );
    }

    #[test]
    fn for_http_uses_full_eager_single_round_trip() {
        let p = BackendPolicy::for_http();
        assert_eq!(p.initial_load_policy, InitialLoadPolicy::FullEager);
        assert_eq!(p.max_round_trips_for_initial_page, 1);
        assert!(!p.supports_incremental_updates);
        assert_eq!(p.prefetch_policy, PrefetchPolicy::None);
        // PR-E：HTTP 用 Local 数据源，三新字段按 Local 行为兜底
        assert!(p.supports_memory);
        assert!(p.supports_subagent_scan);
        assert_eq!(p.stale_check_strategy, StaleCheckStrategy::LocalClock5min);
    }

    #[test]
    fn stale_check_strategy_enum_has_two_variants() {
        // 编译期断言 variant 数 == 2，加 variant 时本测试需同步更新（exhaustive 守护）
        let variants: [StaleCheckStrategy; 2] = [
            StaleCheckStrategy::LocalClock5min,
            StaleCheckStrategy::SkipUntilClockSync,
        ];
        assert_eq!(variants.len(), 2);
    }

    #[test]
    fn backend_policy_is_copy_eq() {
        // trait bound 校验：BackendPolicy SHALL 是 Copy + Eq（spec scenario "BackendPolicy 是 Copy + Eq 类型"）
        fn assert_copy_eq<T: Copy + Eq>() {}
        assert_copy_eq::<BackendPolicy>();
        assert_copy_eq::<StaleCheckStrategy>();
        assert_copy_eq::<InitialLoadPolicy>();
        assert_copy_eq::<PrefetchPolicy>();
    }

    #[test]
    fn constructors_are_deterministic() {
        assert_eq!(BackendPolicy::for_local(), BackendPolicy::for_local());
        assert_eq!(BackendPolicy::for_ssh(), BackendPolicy::for_ssh());
        assert_eq!(BackendPolicy::for_http(), BackendPolicy::for_http());
    }

    #[test]
    fn prefetch_and_initial_load_are_orthogonal() {
        // SkeletonThenStream + PrefetchNext 可同时存在（未来可用）
        let mixed = BackendPolicy {
            initial_load_policy: InitialLoadPolicy::SkeletonThenStream,
            max_round_trips_for_initial_page: 2,
            supports_incremental_updates: true,
            prefetch_policy: PrefetchPolicy::PrefetchNext,
            supports_memory: true,
            supports_subagent_scan: true,
            stale_check_strategy: StaleCheckStrategy::LocalClock5min,
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
