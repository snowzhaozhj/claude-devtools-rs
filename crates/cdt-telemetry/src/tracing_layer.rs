use crate::registry::registry;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

/// `tracing-subscriber::Layer` 实现：钩 ERROR/WARN 事件按 target 顶级 crate 名归 counter。
///
/// 仅命中白名单 `cdt_*` 顶级 crate 名（cdt_core / cdt_parse / cdt_analyze /
/// cdt_discover / cdt_watch / cdt_config / cdt_ssh / cdt_api）；其他 target
/// silently 忽略，inc `telemetry.unregistered_tracing_target` counter。
pub struct TelemetryLayer;

impl TelemetryLayer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for TelemetryLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> tracing_subscriber::Layer<S> for TelemetryLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if !crate::registry::telemetry_enabled() {
            return;
        }
        let level = *event.metadata().level();
        if level != Level::ERROR && level != Level::WARN {
            return;
        }
        let target = event.metadata().target();
        let r = registry();
        if let Some(counter_name) = r.tracing_counter_name_for(target, level) {
            r.counter(counter_name).inc();
        } else {
            r.counter("telemetry.unregistered_tracing_target").inc();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TelemetryLayer;
    use crate::registry::{init_registry, registry};
    use tracing_subscriber::prelude::*;

    #[test]
    fn error_event_with_whitelisted_target_inc_counter() {
        // 注意：tracing global subscriber 只能装一次，所以这里用 with_default 单测
        init_registry();
        let r = registry();
        let before = r.counter_value("cdt_ssh.error");

        let subscriber = tracing_subscriber::registry().with(TelemetryLayer::new());
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(target: "cdt_ssh", "SFTP connection lost");
        });

        let after = r.counter_value("cdt_ssh.error");
        assert_eq!(after, before + 1, "cdt_ssh.error counter should inc by 1");
    }

    #[test]
    fn warn_event_with_whitelisted_target_inc_counter() {
        init_registry();
        let r = registry();
        let before = r.counter_value("cdt_watch.warn");

        let subscriber = tracing_subscriber::registry().with(TelemetryLayer::new());
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(target: "cdt_watch::watcher", "watcher backpressure");
        });

        let after = r.counter_value("cdt_watch.warn");
        assert_eq!(after, before + 1);
    }

    #[test]
    fn external_target_falls_back_to_unregistered() {
        init_registry();
        let r = registry();
        let before = r.counter_value("telemetry.unregistered_tracing_target");

        let subscriber = tracing_subscriber::registry().with(TelemetryLayer::new());
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(target: "tokio::io", "external crate error");
        });

        let after = r.counter_value("telemetry.unregistered_tracing_target");
        assert_eq!(after, before + 1);
    }

    #[test]
    fn info_level_does_not_inc() {
        init_registry();
        let r = registry();
        let before = r.counter_value("cdt_api.error");

        let subscriber = tracing_subscriber::registry().with(TelemetryLayer::new());
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "cdt_api", "info-level should not inc");
        });

        let after = r.counter_value("cdt_api.error");
        assert_eq!(after, before, "INFO level should not inc error counter");
    }
}
