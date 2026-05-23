use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// 32 个 power-of-2 桶。
///
/// bucket `i` 对应 `[2^i, 2^(i+1))` ns 区间，i ∈ `[0, 31]`。
/// bucket 31 上界 ≈ 2.1 s（事实 clamp 边界）。
///
/// 32 桶 × `AtomicU64` = 256 byte / histogram。
pub const BUCKET_COUNT: usize = 32;

/// hot-path-safe 32 桶 atomic histogram，输入单位 ns。
#[derive(Debug)]
pub struct Histogram {
    buckets: [AtomicU64; BUCKET_COUNT],
}

impl Histogram {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buckets: [const { AtomicU64::new(0) }; BUCKET_COUNT],
        }
    }

    /// 把 ns 值落进 32 个 bucket 之一并 inc。
    ///
    /// - 0 ns 落入 bucket 0（特例处理 `leading_zeros(0) = 64`）。
    /// - `> 2^32 - 1` ns 的值 clamp 到 bucket 31。
    #[inline]
    pub fn observe(&self, ns: u64) {
        let bucket = bucket_index(ns);
        self.buckets[bucket].fetch_add(1, Ordering::Relaxed);
    }

    /// 返回 RAII guard，drop 时按 `Instant::elapsed()` 调 [`observe`]。
    ///
    /// [`observe`]: Self::observe
    #[inline]
    #[must_use]
    pub fn start_timer(&self) -> Timer<'_> {
        Timer {
            histogram: self,
            start: Instant::now(),
        }
    }

    #[must_use]
    pub fn snapshot_buckets(&self) -> [u64; BUCKET_COUNT] {
        let mut out = [0u64; BUCKET_COUNT];
        for (i, b) in self.buckets.iter().enumerate() {
            out[i] = b.load(Ordering::Relaxed);
        }
        out
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
#[must_use]
pub fn bucket_index(ns: u64) -> usize {
    if ns == 0 {
        0
    } else {
        let leading = ns.leading_zeros() as usize;
        // 63 - leading = floor(log2(ns))
        (63 - leading).min(BUCKET_COUNT - 1)
    }
}

/// RAII 计时 guard。drop 时把 elapsed 入 histogram。
pub struct Timer<'h> {
    histogram: &'h Histogram,
    start: Instant,
}

impl Drop for Timer<'_> {
    fn drop(&mut self) {
        let elapsed_ns = u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.histogram.observe(elapsed_ns);
    }
}

#[cfg(test)]
mod tests {
    use super::{BUCKET_COUNT, Histogram, bucket_index};

    #[test]
    fn bucket_index_zero_falls_in_bucket_0() {
        assert_eq!(bucket_index(0), 0);
    }

    #[test]
    fn bucket_index_one_falls_in_bucket_0() {
        // 1 ns: floor(log2(1)) = 0
        assert_eq!(bucket_index(1), 0);
    }

    #[test]
    fn bucket_index_powers_of_two_boundary() {
        // 2^k ns 应该落在 bucket k（floor(log2(2^k)) = k）
        for k in 0..BUCKET_COUNT {
            let lower = 1u64 << k;
            assert_eq!(bucket_index(lower), k, "bucket index for 2^{k}");
        }
    }

    #[test]
    fn bucket_index_just_below_next_power() {
        // 2^k - 1 应落在 bucket k-1（k > 0）
        for k in 1..BUCKET_COUNT {
            let upper_minus_one = (1u64 << k) - 1;
            assert_eq!(
                bucket_index(upper_minus_one),
                k - 1,
                "bucket index for 2^{k}-1",
            );
        }
    }

    #[test]
    fn bucket_index_clamps_to_31() {
        // u64::MAX 应 clamp 到 bucket 31（floor(log2(u64::MAX)) = 63 → min(31)）
        assert_eq!(bucket_index(u64::MAX), 31);
        // 2^32 也 clamp（log2 = 32 > 31）
        assert_eq!(bucket_index(1u64 << 32), 31);
        // 2^31 落在 bucket 31
        assert_eq!(bucket_index(1u64 << 31), 31);
    }

    #[test]
    fn observe_increments_correct_bucket() {
        let h = Histogram::new();
        h.observe(100); // floor(log2(100)) = 6
        h.observe(100);
        h.observe(1024); // = 2^10 → bucket 10
        let buckets = h.snapshot_buckets();
        assert_eq!(buckets[6], 2);
        assert_eq!(buckets[10], 1);
        assert_eq!(buckets.iter().sum::<u64>(), 3);
    }

    #[test]
    fn timer_records_elapsed() {
        let h = Histogram::new();
        {
            let _t = h.start_timer();
            // 让 elapsed 至少有几 ns
            std::hint::spin_loop();
        }
        let buckets = h.snapshot_buckets();
        let total: u64 = buckets.iter().sum();
        assert_eq!(total, 1);
    }
}
