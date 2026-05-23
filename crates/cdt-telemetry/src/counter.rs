use std::sync::atomic::{AtomicU64, Ordering};

/// hot-path-safe 单调计数器。
///
/// 全部写操作走 [`Ordering::Relaxed`]——不需要 happens-before 保证，仅保证
/// "原子单调增"。多线程并发增完全无竞争（atomic fetch_add 硬件指令）。
#[derive(Debug)]
pub struct Counter(AtomicU64);

impl Counter {
    #[must_use]
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    #[inline]
    pub fn inc(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn add(&self, n: u64) {
        if n > 0 {
            self.0.fetch_add(n, Ordering::Relaxed);
        }
    }

    #[inline]
    #[must_use]
    pub fn load(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Counter;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn inc_starts_at_zero() {
        let c = Counter::new();
        assert_eq!(c.load(), 0);
    }

    #[test]
    fn inc_increments_by_one() {
        let c = Counter::new();
        c.inc();
        c.inc();
        c.inc();
        assert_eq!(c.load(), 3);
    }

    #[test]
    fn add_zero_is_noop() {
        let c = Counter::new();
        c.add(0);
        assert_eq!(c.load(), 0);
    }

    #[test]
    fn concurrent_inc_is_atomic() {
        let c = Arc::new(Counter::new());
        let threads: Vec<_> = (0..16)
            .map(|_| {
                let c = Arc::clone(&c);
                thread::spawn(move || {
                    for _ in 0..10_000 {
                        c.inc();
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().expect("thread panicked");
        }
        assert_eq!(c.load(), 16 * 10_000);
    }
}
