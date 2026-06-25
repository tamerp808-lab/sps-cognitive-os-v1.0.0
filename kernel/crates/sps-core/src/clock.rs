//! Logical clock — the only clock the kernel uses for ordering.
//!
//! The logical clock is a monotonically increasing `u64` ("tick") assigned
//! to every event. Ticks start at 1; tick 0 is reserved for the genesis
//! sentinel (used as `prev_hash` of the first real event).

use std::sync::atomic::{AtomicU64, Ordering};

use crate::Tick;

/// A Lamport-style logical clock. Each kernel process owns one. Ticks are
/// assigned at append time and persisted with the event — they are the
/// canonical ordering key for the entire event stream.
#[derive(Debug)]
pub struct LogicalClock {
    next: AtomicU64,
}

impl LogicalClock {
    /// Create a clock seeded with the given last-known tick. The next
    /// emitted tick will be `last_tick + 1`.
    pub fn resume_from(last_tick: Tick) -> Self {
        Self {
            next: AtomicU64::new(last_tick.saturating_add(1)),
        }
    }

    /// Create a clock starting at tick 1 (fresh store).
    pub fn fresh() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }

    /// Allocate the next tick. Monotonic; never returns the same value
    /// twice within a process.
    pub fn next_tick(&self) -> Tick {
        self.next.fetch_add(1, Ordering::SeqCst)
    }

    /// Current last-issued tick (for diagnostics only — do not use for
    /// ordering decisions, use the Event Store's `last_tick` instead).
    pub fn current(&self) -> Tick {
        self.next.load(Ordering::SeqCst).saturating_sub(1)
    }
}

impl Default for LogicalClock {
    fn default() -> Self {
        Self::fresh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_is_monotonic() {
        let c = LogicalClock::fresh();
        assert_eq!(c.next_tick(), 1);
        assert_eq!(c.next_tick(), 2);
        assert_eq!(c.next_tick(), 3);
        assert_eq!(c.current(), 3);
    }

    #[test]
    fn clock_resumes_correctly() {
        let c = LogicalClock::resume_from(100);
        assert_eq!(c.next_tick(), 101);
        assert_eq!(c.next_tick(), 102);
    }
}
