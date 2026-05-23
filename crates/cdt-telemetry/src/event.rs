use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

/// 单条事件载荷。`kind` 是编译期 `&'static str`，`fields` 允许 String/数字（仅低频路径）。
#[derive(Debug, Clone)]
pub struct Event {
    pub kind: &'static str,
    pub ts_unix_ms: u64,
    pub fields: Vec<EventField>,
}

#[derive(Debug, Clone)]
pub enum EventField {
    Str(&'static str, String),
    Int(&'static str, i64),
    UInt(&'static str, u64),
}

/// 普通 Event 队列：满时 drop 最老的，永不阻塞 producer。
///
/// 实现选 `parking_lot::RwLock<VecDeque>` 而非 lock-free `crossbeam::ArrayQueue`：
/// snapshot 读取需要"取末尾 N 条不破坏 queue"语义，VecDeque 直接索引；
/// Event 路径是低频（不在 hot path），write lock 无竞争 ~50-100 ns 可接受。
/// 详见 `add-telemetry-signal-bus` design D3b。
pub struct EventQueue {
    inner: RwLock<VecDeque<Event>>,
    cap: usize,
    pub dropped: AtomicU64,
}

impl EventQueue {
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            inner: RwLock::new(VecDeque::with_capacity(cap)),
            cap,
            dropped: AtomicU64::new(0),
        }
    }

    pub fn push(&self, ev: Event) {
        let mut guard = self.inner.write();
        if guard.len() >= self.cap {
            // 满了 drop 最老
            guard.pop_front();
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        guard.push_back(ev);
    }

    /// 取末尾 N 条（最新的在末尾），按时间升序返回。
    #[must_use]
    pub fn snapshot(&self, n: usize) -> Vec<Event> {
        let guard = self.inner.read();
        let start = guard.len().saturating_sub(n);
        guard.iter().skip(start).cloned().collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }

    #[must_use]
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// Critical event channel：panic 类不能完全丢失。
///
/// 与普通 [`EventQueue`] 分离：满时移除最老 50% 而非全部 drain，
/// 保证最近 panic 一定可见；累计移除条目计入 `dropped` counter。
pub struct CriticalEventChannel {
    inner: RwLock<Vec<Event>>,
    cap: usize,
    pub dropped: AtomicU64,
}

impl CriticalEventChannel {
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            inner: RwLock::new(Vec::with_capacity(cap)),
            cap,
            dropped: AtomicU64::new(0),
        }
    }

    pub fn push(&self, ev: Event) {
        let mut guard = self.inner.write();
        if guard.len() >= self.cap {
            // 满时移除最老 50%
            let half = self.cap / 2;
            let removed = guard.drain(..half).count() as u64;
            self.dropped.fetch_add(removed, Ordering::Relaxed);
        }
        guard.push(ev);
    }

    #[must_use]
    pub fn snapshot(&self, n: usize) -> Vec<Event> {
        let guard = self.inner.read();
        let start = guard.len().saturating_sub(n);
        guard[start..].to_vec()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }

    #[must_use]
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::{CriticalEventChannel, Event, EventField, EventQueue};

    fn ev(kind: &'static str, ts: u64) -> Event {
        Event {
            kind,
            ts_unix_ms: ts,
            fields: vec![EventField::UInt("seq", ts)],
        }
    }

    #[test]
    fn event_queue_push_under_cap() {
        let q = EventQueue::new(10);
        for i in 0..5 {
            q.push(ev("test", i));
        }
        assert_eq!(q.len(), 5);
        assert_eq!(q.dropped_count(), 0);
    }

    #[test]
    fn event_queue_drops_oldest_when_full() {
        let q = EventQueue::new(3);
        for i in 0..5 {
            q.push(ev("test", i));
        }
        assert_eq!(q.len(), 3);
        assert_eq!(q.dropped_count(), 2);
        let snap = q.snapshot(3);
        assert_eq!(
            snap.iter().map(|e| e.ts_unix_ms).collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
    }

    #[test]
    fn event_queue_snapshot_returns_recent_n() {
        let q = EventQueue::new(10);
        for i in 0..10 {
            q.push(ev("test", i));
        }
        let snap = q.snapshot(3);
        assert_eq!(
            snap.iter().map(|e| e.ts_unix_ms).collect::<Vec<_>>(),
            vec![7, 8, 9]
        );
    }

    #[test]
    fn critical_channel_half_compress_when_full() {
        let ch = CriticalEventChannel::new(4);
        // 先填满
        for i in 0..4 {
            ch.push(ev("panic", i));
        }
        assert_eq!(ch.len(), 4);
        // 第 5 条触发半压缩：移除最老 50% (= 2 条)，新条入队
        ch.push(ev("panic", 4));
        assert_eq!(ch.len(), 3); // 4 - 2 + 1
        assert_eq!(ch.dropped_count(), 2);
        let snap = ch.snapshot(3);
        assert_eq!(
            snap.iter().map(|e| e.ts_unix_ms).collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
    }
}
