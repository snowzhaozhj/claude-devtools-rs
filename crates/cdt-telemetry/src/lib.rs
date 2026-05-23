//! cdt-telemetry: 应用健康度 Signal Bus
//!
//! 提供三类信号的统一基础设施：
//! - [`Counter`]：基于 [`std::sync::atomic::AtomicU64`]，~5 ns/op，hot path 安全
//! - [`Histogram`]：32 桶 power-of-2 ns，~30-50 ns/op，hot path 安全
//! - [`Event`]：低频路径专用（panic / SSH reconnect / race 触发）；禁止 hot path 调用
//!
//! 通过 [`init_registry`] 在进程启动时一次性注册所有静态信号 name；hot path 调用
//! [`counter!`] / [`histogram!`] / [`event!`] 宏只读 lookup Registry，不增长内部 map。
//!
//! 详见 OpenSpec change `add-telemetry-signal-bus`。

#![allow(
    clippy::module_name_repetitions,
    clippy::doc_markdown,
    clippy::missing_const_for_fn
)]

pub mod counter;
pub mod event;
pub mod histogram;
pub mod registry;
pub mod snapshot;
pub mod tracing_layer;
pub mod wrappers;

pub use counter::Counter;
pub use event::{CriticalEventChannel, Event, EventField, EventQueue};
pub use histogram::{Histogram, Timer};
pub use registry::{
    Registry, init_registry, register_correctness_event_kind, registry, take_snapshot,
    telemetry_enabled,
};
pub use snapshot::{HistogramSnapshot, TelemetryEvent, TelemetrySnapshot};
pub use tracing_layer::TelemetryLayer;
pub use wrappers::{CounterRef, HistogramRef};
