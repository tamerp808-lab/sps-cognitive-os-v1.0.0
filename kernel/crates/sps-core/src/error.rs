//! Errors raised by the kernel core.

use thiserror::Error;

/// Top-level error type for the kernel core.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Hash chain broken — an event's `prev_hash` does not match the
    /// previous event's `hash`. The store has been tampered with or
    /// corrupted.
    #[error("hash chain broken at tick {tick}: prev_hash {prev} does not match expected {expected}")]
    HashChainBroken {
        /// Tick of the offending event.
        tick: u64,
        /// `prev_hash` stored on the event.
        prev: String,
        /// `hash` of the previous event in the store.
        expected: String,
    },

    /// An event's stored hash does not match its recomputed hash.
    #[error("hash mismatch at tick {tick}: stored {stored}, recomputed {recomputed}")]
    HashMismatch {
        /// Tick of the offending event.
        tick: u64,
        /// Hash stored in the event.
        stored: String,
        /// Hash recomputed from `prev_hash || tick || type || payload`.
        recomputed: String,
    },

    /// A non-monotonic tick was encountered.
    #[error("non-monotonic tick: previous={prev}, current={curr}")]
    NonMonotonicTick {
        /// Previously seen tick.
        prev: u64,
        /// Current tick.
        curr: u64,
    },

    /// Storage backend failure.
    #[error("storage error: {0}")]
    Storage(#[source] anyhow::Error),

    /// Reducer failure. Reducers are supposed to be pure and total; this
    /// only fires if a reducer panics or returns `Err` — both are
    /// treated as kernel bugs.
    #[error("reducer '{reducer}' failed on event tick={tick} type={event_type}: {source}")]
    ReducerFailed {
        /// Reducer identifier (typically its registered name).
        reducer: &'static str,
        /// Tick of the event that triggered the reducer.
        tick: u64,
        /// Event type.
        event_type: String,
        /// Underlying error.
        #[source]
        source: anyhow::Error,
    },

    /// Unknown event type — no reducer registered.
    #[error("no reducer registered for event type '{0}'")]
    UnknownEventType(String),

    /// Snapshot does not match replayed state.
    #[error("snapshot at tick {snapshot_tick} does not match replayed state (replayed last tick = {replayed_tick})")]
    SnapshotMismatch {
        /// Tick recorded in the snapshot.
        snapshot_tick: u64,
        /// Last tick produced by replay.
        replayed_tick: u64,
    },

    /// JSON (de)serialization failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Catch-all for unexpected internal errors.
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Convenience alias.
pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
impl PartialEq for CoreError {
    fn eq(&self, other: &Self) -> bool {
        // For test assertions — compare the display string.
        self.to_string() == other.to_string()
    }
}
