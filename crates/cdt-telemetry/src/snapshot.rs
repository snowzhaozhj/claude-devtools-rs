use crate::histogram::BUCKET_COUNT;
use crate::registry::{COUNTER_NAMES, HISTOGRAM_NAMES, Registry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 1;

/// IPC 返回的快照载荷。camelCase 序列化。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySnapshot {
    pub schema_version: u32,
    pub uptime_secs: u64,
    pub captured_at: u64,
    pub counters: BTreeMap<String, u64>,
    pub histograms: BTreeMap<String, HistogramSnapshot>,
    pub recent_events: Vec<TelemetryEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistogramSnapshot {
    pub count: u64,
    pub buckets: Vec<u64>,
    pub p50_ns: Option<u64>,
    pub p95_ns: Option<u64>,
    pub p99_ns: Option<u64>,
    pub max_bucket: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryEvent {
    pub kind: String,
    pub ts_unix_ms: u64,
    pub fields: BTreeMap<String, String>,
}

impl TelemetrySnapshot {
    pub(crate) fn collect(reg: &Registry, recent_events_n: usize) -> Self {
        let now_ms = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis()),
        )
        .unwrap_or(0);
        let uptime_secs = reg.started_at().elapsed().as_secs();

        let mut counters = BTreeMap::new();
        for &name in COUNTER_NAMES {
            counters.insert(name.to_string(), reg.counter_value(name));
        }
        // 也包含 critical channel 自身的 dropped 计数
        counters.insert(
            "panic.dropped_count".to_string(),
            reg.panic_events().dropped_count(),
        );
        counters.insert("events.dropped".to_string(), reg.events().dropped_count());

        let mut histograms = BTreeMap::new();
        for &name in HISTOGRAM_NAMES {
            let h = reg.histogram(name);
            let buckets = h.snapshot_buckets();
            histograms.insert(name.to_string(), HistogramSnapshot::from_buckets(&buckets));
        }

        let mut recent_events: Vec<TelemetryEvent> = reg
            .events()
            .snapshot(recent_events_n)
            .into_iter()
            .map(TelemetryEvent::from)
            .collect();
        // 把 critical panic events 也并进 recent_events 末尾
        let panic_events: Vec<TelemetryEvent> = reg
            .panic_events()
            .snapshot(recent_events_n)
            .into_iter()
            .map(TelemetryEvent::from)
            .collect();
        recent_events.extend(panic_events);
        recent_events.sort_by_key(|e| e.ts_unix_ms);

        Self {
            schema_version: SCHEMA_VERSION,
            uptime_secs,
            captured_at: now_ms,
            counters,
            histograms,
            recent_events,
        }
    }
}

impl HistogramSnapshot {
    fn from_buckets(buckets: &[u64; BUCKET_COUNT]) -> Self {
        let count: u64 = buckets.iter().sum();
        let max_bucket = buckets
            .iter()
            .enumerate()
            .rev()
            .find(|&(_, &v)| v > 0)
            .map(|(i, _)| u32::try_from(i).unwrap_or(0));

        let p50_ns = percentile_upper_bound(buckets, count, 50);
        let p95_ns = percentile_upper_bound(buckets, count, 95);
        let p99_ns = percentile_upper_bound(buckets, count, 99);

        Self {
            count,
            buckets: buckets.to_vec(),
            p50_ns,
            p95_ns,
            p99_ns,
            max_bucket,
        }
    }
}

/// 给定 32 个 bucket count 与 percentile（如 95），返回首个累计 ≥ count*p/100 的 bucket
/// 上界 ns（即 `2^(i+1)`），即"实际值 ≤ 此值"的保守估计。
///
/// count == 0 返回 None。
fn percentile_upper_bound(
    buckets: &[u64; BUCKET_COUNT],
    count: u64,
    percentile: u8,
) -> Option<u64> {
    if count == 0 {
        return None;
    }
    let target = count.saturating_mul(u64::from(percentile)).div_ceil(100);
    let mut acc: u64 = 0;
    for (i, &v) in buckets.iter().enumerate() {
        acc = acc.saturating_add(v);
        if acc >= target {
            // bucket i 上界 = 2^(i+1)，i=31 时 = 2^32，超 u64 不会发生（因 BUCKET_COUNT=32）
            return Some(1u64 << (i + 1));
        }
    }
    // 理论不可达（target ≤ count，循环必命中）；safety net
    Some(1u64 << BUCKET_COUNT)
}

impl From<crate::event::Event> for TelemetryEvent {
    fn from(ev: crate::event::Event) -> Self {
        let mut fields = BTreeMap::new();
        for f in ev.fields {
            match f {
                crate::event::EventField::Str(k, v) => {
                    fields.insert(k.to_string(), v);
                }
                crate::event::EventField::Int(k, v) => {
                    fields.insert(k.to_string(), v.to_string());
                }
                crate::event::EventField::UInt(k, v) => {
                    fields.insert(k.to_string(), v.to_string());
                }
            }
        }
        Self {
            kind: ev.kind.to_string(),
            ts_unix_ms: ev.ts_unix_ms,
            fields,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HistogramSnapshot, percentile_upper_bound};
    use crate::histogram::BUCKET_COUNT;

    #[test]
    fn percentile_zero_count_returns_none() {
        let buckets = [0u64; BUCKET_COUNT];
        assert!(percentile_upper_bound(&buckets, 0, 50).is_none());
    }

    #[test]
    fn percentile_single_bucket() {
        let mut buckets = [0u64; BUCKET_COUNT];
        buckets[10] = 100; // 全部观察落在 bucket 10
        let snap = HistogramSnapshot::from_buckets(&buckets);
        assert_eq!(snap.count, 100);
        assert_eq!(snap.p50_ns, Some(1u64 << 11));
        assert_eq!(snap.p95_ns, Some(1u64 << 11));
        assert_eq!(snap.p99_ns, Some(1u64 << 11));
        assert_eq!(snap.max_bucket, Some(10));
    }

    #[test]
    fn percentile_distributed() {
        let mut buckets = [0u64; BUCKET_COUNT];
        // 50 in bucket 5, 40 in bucket 10, 9 in bucket 15, 1 in bucket 20
        buckets[5] = 50;
        buckets[10] = 40;
        buckets[15] = 9;
        buckets[20] = 1;
        let snap = HistogramSnapshot::from_buckets(&buckets);
        // count = 100
        // p50 累计 50 命中 bucket 5 → upper = 2^6
        assert_eq!(snap.p50_ns, Some(1u64 << 6));
        // p95 累计 95: 50+40=90, +9=99 命中 bucket 15 → upper = 2^16
        assert_eq!(snap.p95_ns, Some(1u64 << 16));
        // p99 累计 99: 50+40+9=99 命中 bucket 15 → upper = 2^16
        assert_eq!(snap.p99_ns, Some(1u64 << 16));
        assert_eq!(snap.max_bucket, Some(20));
    }
}
