use crate::counter::Counter;
use crate::event::{CriticalEventChannel, EventQueue};
use crate::histogram::Histogram;
use crate::snapshot::TelemetrySnapshot;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// 全局 Registry。启动期一次性 init，hot path 只读 lookup。
pub struct Registry {
    counters: HashMap<&'static str, Counter>,
    histograms: HashMap<&'static str, Histogram>,
    events: EventQueue,
    panic_events: CriticalEventChannel,
    correctness_kinds: HashSet<&'static str>,
    /// 顶级 crate 名 → (error counter name, warn counter name) 双映射。
    /// 启动期一次性建表；tracing layer hot path 只读 O(1) lookup，零分配。
    tracing_targets: HashMap<&'static str, (&'static str, &'static str)>,
    started_at: Instant,
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(true);

/// 已注册的静态信号 name 清单（编译期 const 数组）。
///
/// hot path 调用 `counter!("name")` 时必须在此清单内，否则 silently fallback
/// 到 `telemetry.unregistered_signal_attempt`。
pub const COUNTER_NAMES: &[&str] = &[
    // Performance — metadata cache
    "metadata.cache.hit",
    "metadata.cache.miss",
    "metadata.cache.sig_mismatch",
    "metadata.cache.stat_err",
    // Performance — project scan cache invalidation（change `project-scan-cache-semantic-invalidation`）
    "project_scan_cache.invalidate.structural",
    "project_scan_cache.invalidate.content_append_skipped",
    "project_scan_cache.invalidate.lag_conservative",
    // Reliability — runtime
    "panic.recovered",
    "panic.dropped_count",
    "ipc.error",
    "watcher.respawn",
    // Reliability — SSH
    "ssh.reconnect",
    // Correctness
    "stale_update.triggered",
    "cache.signature_skew",
    "cache.signature_skew_observed_in_ui",
    "generation.mismatch",
    // tracing bridge 方向 1（按 cdt-* 顶级 crate 名 × {error, warn} 平铺）
    "cdt_core.error",
    "cdt_core.warn",
    "cdt_parse.error",
    "cdt_parse.warn",
    "cdt_analyze.error",
    "cdt_analyze.warn",
    "cdt_discover.error",
    "cdt_discover.warn",
    "cdt_watch.error",
    "cdt_watch.warn",
    "cdt_config.error",
    "cdt_config.warn",
    "cdt_ssh.error",
    "cdt_ssh.warn",
    "cdt_api.error",
    "cdt_api.warn",
    // 自观测（hardcoded 始终在白名单）
    "telemetry.unregistered_signal_attempt",
    "telemetry.unregistered_correctness_event",
    "telemetry.unregistered_tracing_target",
];

pub const HISTOGRAM_NAMES: &[&str] = &[
    "ipc.list_sessions.duration_ns",
    "ipc.get_session_detail.duration_ns",
    "ipc.list_repository_groups.duration_ns",
    "ipc.list_projects.duration_ns",
];

const CORRECTNESS_KIND_WHITELIST: &[&str] = &[
    "stale_update.triggered",
    "cache.signature_skew_observed_in_ui",
];

const TRACING_TARGET_WHITELIST: &[(&str, &str, &str)] = &[
    ("cdt_core", "cdt_core.error", "cdt_core.warn"),
    ("cdt_parse", "cdt_parse.error", "cdt_parse.warn"),
    ("cdt_analyze", "cdt_analyze.error", "cdt_analyze.warn"),
    ("cdt_discover", "cdt_discover.error", "cdt_discover.warn"),
    ("cdt_watch", "cdt_watch.error", "cdt_watch.warn"),
    ("cdt_config", "cdt_config.error", "cdt_config.warn"),
    ("cdt_ssh", "cdt_ssh.error", "cdt_ssh.warn"),
    ("cdt_api", "cdt_api.error", "cdt_api.warn"),
];

const EVENT_QUEUE_CAP: usize = 10_000;
const PANIC_CHANNEL_CAP: usize = 1000;

/// 启动期 init Registry + 一次性读 `CDT_TELEMETRY_ENABLED` env var。
///
/// 多次调用幂等（OnceLock 保证只 init 一次）。
pub fn init_registry() {
    if let Ok(v) = std::env::var("CDT_TELEMETRY_ENABLED") {
        let on = !matches!(v.as_str(), "0" | "false" | "off" | "no");
        ENABLED.store(on, Ordering::Relaxed);
    }
    let _ = registry();
}

/// hot path lookup 入口：`&'static Registry`。
///
/// 第一次调用时 init；之后零分配 / 零锁。
pub fn registry() -> &'static Registry {
    REGISTRY.get_or_init(build)
}

/// 是否启用 telemetry。env var `CDT_TELEMETRY_ENABLED=0/false/off/no` 关闭。
#[inline]
#[must_use]
pub fn telemetry_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// 注册一个新的 correctness event kind 到白名单。
///
/// 仅供测试 / Phase 2+ 扩展使用；启动后调用无效（Registry 已冻结）。
/// Phase 1 通过 const `CORRECTNESS_KIND_WHITELIST` 已注册。
pub fn register_correctness_event_kind(_kind: &'static str) {
    // 编译期白名单冻结，本函数留作 Phase 2 扩展点；目前 no-op。
}

fn build() -> Registry {
    let mut counters = HashMap::with_capacity(COUNTER_NAMES.len());
    for &name in COUNTER_NAMES {
        counters.insert(name, Counter::new());
    }
    let mut histograms = HashMap::with_capacity(HISTOGRAM_NAMES.len());
    for &name in HISTOGRAM_NAMES {
        histograms.insert(name, Histogram::new());
    }
    let mut tracing_targets: HashMap<&'static str, (&'static str, &'static str)> =
        HashMap::with_capacity(TRACING_TARGET_WHITELIST.len());
    for &(target, error, warn) in TRACING_TARGET_WHITELIST {
        tracing_targets.insert(target, (error, warn));
    }
    let mut correctness_kinds = HashSet::with_capacity(CORRECTNESS_KIND_WHITELIST.len());
    for &kind in CORRECTNESS_KIND_WHITELIST {
        correctness_kinds.insert(kind);
    }
    Registry {
        counters,
        histograms,
        events: EventQueue::new(EVENT_QUEUE_CAP),
        panic_events: CriticalEventChannel::new(PANIC_CHANNEL_CAP),
        correctness_kinds,
        tracing_targets,
        started_at: Instant::now(),
    }
}

impl Registry {
    /// hot path lookup：返回 counter 引用，未注册时返回 unregistered fallback。
    ///
    /// 调用方通常持有 `&'static Registry`（来自 [`registry()`]），此时返回值
    /// 自动推断为 `&'static Counter`，可被 `OnceLock<&'static Counter>` 缓存。
    #[must_use]
    pub fn counter(&self, name: &'static str) -> &Counter {
        if let Some(c) = self.counters.get(name) {
            return c;
        }
        self.counters
            .get("telemetry.unregistered_signal_attempt")
            .expect("'telemetry.unregistered_signal_attempt' MUST be registered")
    }

    /// hot path lookup：histogram 引用。未注册时 panic（histogram name 是显式声明的，不应有 typo）。
    #[must_use]
    pub fn histogram(&self, name: &'static str) -> &Histogram {
        self.histograms
            .get(name)
            .unwrap_or_else(|| panic!("histogram '{name}' is not registered"))
    }

    /// 直接读 counter 值（snapshot 用）。未注册返回 0。
    #[must_use]
    pub fn counter_value(&self, name: &str) -> u64 {
        self.counters.get(name).map_or(0, Counter::load)
    }

    pub fn events(&self) -> &EventQueue {
        &self.events
    }

    pub fn panic_events(&self) -> &CriticalEventChannel {
        &self.panic_events
    }

    /// 校验 correctness kind 是否在白名单。命中返回 `true`，未命中 inc unregistered counter 后返回 `false`。
    pub fn check_correctness_kind(&self, kind: &str) -> bool {
        if self.correctness_kinds.contains(kind) {
            true
        } else {
            self.counter("telemetry.unregistered_correctness_event")
                .inc();
            false
        }
    }

    /// 增 correctness counter（按白名单后调用）。`kind` 必须是 `&'static str`（白名单常量）。
    pub fn add_correctness_count(&self, kind: &'static str, count: u64) {
        if self.correctness_kinds.contains(kind) {
            self.counter(kind).add(count);
        } else {
            self.counter("telemetry.unregistered_correctness_event")
                .add(count.max(1));
        }
    }

    /// tracing bridge 用：按顶级 crate 名查 counter name。返回 None 表示未在白名单。
    ///
    /// hot path：单次 hashmap O(1) lookup + 一次按 level 选 tuple 字段，零分配 / 零字符串扫描。
    #[must_use]
    pub fn tracing_counter_name_for(
        &self,
        target: &str,
        level: tracing::Level,
    ) -> Option<&'static str> {
        let crate_name = target.split("::").next().unwrap_or(target);
        let (error_name, warn_name) = self.tracing_targets.get(crate_name)?;
        match level {
            tracing::Level::ERROR => Some(*error_name),
            tracing::Level::WARN => Some(*warn_name),
            _ => None,
        }
    }

    #[must_use]
    pub fn started_at(&self) -> Instant {
        self.started_at
    }

    /// 收集当前 snapshot。线性扫所有 counter / histogram / 最近 events。
    #[must_use]
    pub fn snapshot(&self, recent_events_n: usize) -> TelemetrySnapshot {
        TelemetrySnapshot::collect(self, recent_events_n)
    }
}

/// 对外简化入口：当前快照 + 最近 100 条 events。
#[must_use]
pub fn take_snapshot() -> TelemetrySnapshot {
    registry().snapshot(100)
}

#[cfg(test)]
mod tests {
    use super::{COUNTER_NAMES, HISTOGRAM_NAMES, init_registry, registry, telemetry_enabled};

    // 注意：全局 Registry 是 OnceLock 跨 tests 共享；某些 tracing layer 测试会 inc 共享 counter。
    // 初始化为 0 的断言用 build() 直接构造一个独立 Registry instance。
    #[test]
    fn build_creates_all_static_counters_at_zero() {
        let r = super::build();
        for &name in COUNTER_NAMES {
            let c = r.counters.get(name);
            assert!(c.is_some(), "counter {name} not registered");
            assert_eq!(
                c.unwrap().load(),
                0,
                "counter {name} should be 0 in fresh registry"
            );
        }
    }

    #[test]
    fn build_creates_all_static_histograms_empty() {
        let r = super::build();
        for &name in HISTOGRAM_NAMES {
            let h = r.histograms.get(name);
            assert!(h.is_some(), "histogram {name} not registered");
            assert_eq!(h.unwrap().snapshot_buckets().iter().sum::<u64>(), 0);
        }
    }

    #[test]
    fn global_registry_is_initialized() {
        init_registry();
        let r = registry();
        // 不假设值，但 lookup 必须不 panic；fallback 路径应在 counter() 内
        for &name in COUNTER_NAMES {
            // counter() 永不返回 None；返回值能 load
            let _ = r.counter(name).load();
        }
    }

    #[test]
    fn unregistered_name_falls_back_to_unregistered_counter() {
        init_registry();
        let r = registry();
        let before = r.counter_value("telemetry.unregistered_signal_attempt");
        let c = r.counter("nonexistent.signal");
        c.inc();
        let after = r.counter_value("telemetry.unregistered_signal_attempt");
        assert_eq!(after, before + 1);
    }

    #[test]
    fn correctness_kind_whitelist() {
        init_registry();
        let r = registry();
        assert!(r.check_correctness_kind("stale_update.triggered"));
        assert!(r.check_correctness_kind("cache.signature_skew_observed_in_ui"));
        let before = r.counter_value("telemetry.unregistered_correctness_event");
        assert!(!r.check_correctness_kind("fake.event"));
        let after = r.counter_value("telemetry.unregistered_correctness_event");
        assert_eq!(after, before + 1);
    }

    #[test]
    fn telemetry_enabled_defaults_true() {
        init_registry();
        // env var 未设置时默认 true
        if std::env::var("CDT_TELEMETRY_ENABLED").is_err() {
            assert!(telemetry_enabled());
        }
    }
}
