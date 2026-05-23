use crate::counter::Counter;
use crate::histogram::{Histogram, Timer};

/// hot-path-safe Counter 引用包装。
///
/// 所有写操作内部判断 [`telemetry_enabled`]——disabled 时 atomic load 一次后短路，
/// 不触发 fetch_add；enabled 时正常 inc。
///
/// [`telemetry_enabled`]: crate::registry::telemetry_enabled
#[derive(Copy, Clone)]
pub struct CounterRef(pub(crate) &'static Counter);

impl CounterRef {
    #[must_use]
    pub fn new(c: &'static Counter) -> Self {
        Self(c)
    }

    #[inline]
    pub fn inc(self) {
        if crate::registry::telemetry_enabled() {
            self.0.inc();
        }
    }

    #[inline]
    pub fn add(self, n: u64) {
        if crate::registry::telemetry_enabled() {
            self.0.add(n);
        }
    }

    #[inline]
    #[must_use]
    pub fn load(self) -> u64 {
        self.0.load()
    }
}

#[derive(Copy, Clone)]
pub struct HistogramRef(pub(crate) &'static Histogram);

impl HistogramRef {
    #[must_use]
    pub fn new(h: &'static Histogram) -> Self {
        Self(h)
    }

    #[inline]
    pub fn observe(self, ns: u64) {
        if crate::registry::telemetry_enabled() {
            self.0.observe(ns);
        }
    }

    /// 返回 RAII guard。disabled 时返回的 guard 在 drop 时不写入。
    #[inline]
    #[must_use]
    pub fn start_timer(self) -> Option<Timer<'static>> {
        if crate::registry::telemetry_enabled() {
            Some(self.0.start_timer())
        } else {
            None
        }
    }
}

/// hot-path-safe counter 查找。
///
/// 限制 `$name:literal`：非字面量编译期报错。per-callsite `OnceLock` 缓存
/// 避免每次调用 hashmap lookup。返回 [`CounterRef`]——hot path 调用 `.inc()`
/// 内部短路 disabled 状态。
///
/// 未在白名单的 name SHALL 在 fallback 路径 inc `telemetry.unregistered_signal_attempt`。
///
/// 用例：
/// ```ignore
/// counter!("metadata.cache.hit").inc();
/// counter!("ipc.error").add(1);
/// ```
#[macro_export]
macro_rules! counter {
    ($name:literal) => {{
        static __CDT_TELEMETRY_COUNTER: ::std::sync::OnceLock<$crate::CounterRef> =
            ::std::sync::OnceLock::new();
        *__CDT_TELEMETRY_COUNTER
            .get_or_init(|| $crate::CounterRef::new($crate::registry().counter($name)))
    }};
}

/// hot-path-safe histogram 查找。
///
/// 用例：
/// ```ignore
/// let _t = histogram!("ipc.list_sessions.duration_ns").start_timer();
/// // ... do work ...
/// // _t drop 时记录 elapsed
/// ```
#[macro_export]
macro_rules! histogram {
    ($name:literal) => {{
        static __CDT_TELEMETRY_HISTOGRAM: ::std::sync::OnceLock<$crate::HistogramRef> =
            ::std::sync::OnceLock::new();
        *__CDT_TELEMETRY_HISTOGRAM
            .get_or_init(|| $crate::HistogramRef::new($crate::registry().histogram($name)))
    }};
}

/// 低频路径专用：push 一条事件到全局 EventQueue。
///
/// **禁止 hot path 调用**——CI `scripts/check-no-hot-event.sh` 拦截。
///
/// 用例：
/// ```ignore
/// event!("ssh.sftp_death", host_hash = "abc123", ts = 1700000000);
/// event!("ssh.reconnect");  // 无字段
/// ```
#[macro_export]
macro_rules! event {
    ($kind:literal $(, $fname:ident = $fvalue:expr )* $(,)?) => {{
        if $crate::registry::telemetry_enabled() {
            let now_ms = ::std::time::SystemTime::now()
                .duration_since(::std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let ev = $crate::Event {
                kind: $kind,
                ts_unix_ms: now_ms,
                fields: vec![
                    $( $crate::__event_field!(stringify!($fname), $fvalue), )*
                ],
            };
            $crate::registry().events().push(ev);
        }
    }};
}

/// 内部 helper: 把 `name = value` 转为 EventField，根据 value 类型自动选枚举变体。
#[macro_export]
#[doc(hidden)]
macro_rules! __event_field {
    ($name:expr, $val:expr) => {
        $crate::EventField::Str($name, ::std::format!("{}", $val))
    };
}
