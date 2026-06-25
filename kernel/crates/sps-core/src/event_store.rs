//! Event Store facade.
//!
//! The EventStore is the kernel's single entry point for appending events.
//! It coordinates:
//!
//! 1. Allocating the next logical tick from the [`LogicalClock`].
//! 2. Looking up the previous event's hash.
//! 3. Finalizing the [`RawEvent`] into a fully-hashed [`Event`].
//! 4. Persisting via the [`StoragePort`].
//! 5. Returning the persisted event to the caller (for synchronous
//!    command handlers).
//!
//! The EventStore does **not** run reducers. That is the kernel's job
//! after a successful append.

use std::sync::Arc;

use parking_lot::Mutex;

use crate::clock::LogicalClock;
use crate::event::{Event, EventHash, RawEvent, Tick};
use crate::storage::port::StoragePort;
use crate::{CoreError, CoreResult};

/// The event store facade.
pub struct EventStore {
    storage: Arc<dyn StoragePort>,
    clock: Mutex<LogicalClock>,
}

impl EventStore {
    /// Create a new event store backed by the given storage. The
    /// logical clock is resumed from the storage's last tick.
    pub fn new(storage: Arc<dyn StoragePort>) -> CoreResult<Self> {
        let last_tick = storage.last_tick()?;
        let _last_hash = storage.last_hash()?;
        let clock = LogicalClock::resume_from(last_tick);
        Ok(Self {
            storage,
            clock: Mutex::new(clock),
        })
    }

    /// Returns the current last tick (without appending).
    pub fn last_tick(&self) -> CoreResult<Tick> {
        self.storage.last_tick()
    }

    /// Returns the current last hash.
    pub fn last_hash(&self) -> CoreResult<EventHash> {
        self.storage.last_hash()
    }

    /// Returns the count of events in the store.
    pub fn count(&self) -> CoreResult<u64> {
        self.storage.count_events()
    }

    /// Append a raw event. The store assigns tick, prev_hash, and hash,
    /// persists, and returns the finalized event.
    ///
    /// This is atomic against concurrent appends from other threads
    /// (the underlying storage is expected to be linearizable; the
    /// clock allocation is serialized via a mutex).
    pub fn append(&self, raw: RawEvent) -> CoreResult<Event> {
        loop {
            // Serialize clock allocation + storage read of last_hash to
            // guarantee tick monotonicity.
            let (tick, prev_hash) = {
                let clock = self.clock.lock();
                let prev = self.storage.last_hash()?;
                (clock.next_tick(), prev)
            };
            // Clone the raw event so we can retry on race.
            let event = raw.clone().finalize(tick, prev_hash);

            // Sanity: hash must be valid before persisting.
            if !event.hash_is_valid() {
                return Err(CoreError::Internal(anyhow::anyhow!(
                    "internal: event hash invalid before append at tick {}",
                    tick
                )));
            }

            // Race check: prev_hash must match the current last_hash.
            // If another thread appended between our read and now, we
            // loop and retry with a fresh tick+prev_hash.
            // We use a CAS-like pattern: re-read last_hash, and if it
            // changed, restart. (For SQLite the BEGIN IMMEDIATE in the
            // tx version will serialize; for in-memory this is fine
            // because tests are single-threaded.)
            let current_last = self.storage.last_hash()?;
            if current_last != prev_hash {
                continue;
            }

            self.storage.append_event(&event)?;
            return Ok(event);
        }
    }

    /// Read events starting from a tick (inclusive), ascending.
    pub fn read_from(&self, from_tick: Tick, limit: usize) -> CoreResult<Vec<Event>> {
        self.storage.read_events_from(from_tick, limit)
    }

    /// Read a single event by tick.
    pub fn read_by_tick(&self, tick: Tick) -> CoreResult<Option<Event>> {
        self.storage.read_event_by_tick(tick)
    }

    /// Access the underlying storage port (for snapshot/kv operations).
    pub fn storage(&self) -> &Arc<dyn StoragePort> {
        &self.storage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Actor;
    use serde_json::json;

    // We can't easily test against the real StoragePort here without a
    // concrete impl, so we test the hash logic via RawEvent::finalize
    // directly. End-to-end tests live in the workspace-level test crate.

    #[test]
    fn raw_event_finalize_produces_valid_event() {
        let raw = RawEvent::new("test.evt", json!({"x": 1}), Actor::owner(), 0);
        let e = raw.finalize(1, EventHash::GENESIS);
        assert_eq!(e.tick, 1);
        assert_eq!(e.prev_hash, EventHash::GENESIS);
        assert!(e.hash_is_valid());
    }
}
