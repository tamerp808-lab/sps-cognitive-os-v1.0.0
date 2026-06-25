//! In-memory `StoragePort` implementation.
//!
//! Used for tests and ephemeral kernels. Backed by a `BTreeMap` keyed by
//! tick. Thread-safe via `parking_lot::RwLock`.

use std::collections::BTreeMap;
use std::sync::Arc;

use parking_lot::RwLock;
use sps_core::event::{Event, EventHash, Tick};
use sps_core::snapshot::Snapshot;
use sps_core::storage::port::StoragePort;
use sps_core::{CoreError, CoreResult};

/// In-memory storage backend.
#[derive(Default)]
pub struct InMemoryStorage {
    events: RwLock<BTreeMap<Tick, Event>>,
    snapshots: RwLock<BTreeMap<Tick, Snapshot>>,
    kv: RwLock<BTreeMap<String, Vec<u8>>>,
}

impl InMemoryStorage {
    /// Create an empty in-memory store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Wrap in `Arc<dyn StoragePort>`.
    pub fn into_arc(self) -> Arc<dyn StoragePort> {
        Arc::new(self)
    }
}

impl StoragePort for InMemoryStorage {
    fn append_event(&self, event: &Event) -> CoreResult<()> {
        let mut events = self.events.write();
        // Monotonic check.
        if let Some(last) = events.keys().next_back() {
            if event.tick <= *last {
                return Err(CoreError::NonMonotonicTick {
                    prev: *last,
                    curr: event.tick,
                });
            }
        } else if event.tick == 0 {
            return Err(CoreError::NonMonotonicTick {
                prev: 0,
                curr: 0,
            });
        }
        events.insert(event.tick, event.clone());
        Ok(())
    }

    fn read_events_from(&self, from_tick: Tick, limit: usize) -> CoreResult<Vec<Event>> {
        let events = self.events.read();
        let mut out = Vec::with_capacity(limit.min(1024));
        for (_, e) in events.range(from_tick..) {
            if out.len() >= limit {
                break;
            }
            out.push(e.clone());
        }
        Ok(out)
    }

    fn read_event_by_tick(&self, tick: Tick) -> CoreResult<Option<Event>> {
        Ok(self.events.read().get(&tick).cloned())
    }

    fn last_tick(&self) -> CoreResult<Tick> {
        Ok(self.events.read().keys().next_back().copied().unwrap_or(0))
    }

    fn last_hash(&self) -> CoreResult<EventHash> {
        Ok(self
            .events
            .read()
            .values()
            .next_back()
            .map(|e| e.hash)
            .unwrap_or(EventHash::GENESIS))
    }

    fn count_events(&self) -> CoreResult<u64> {
        Ok(self.events.read().len() as u64)
    }

    fn write_snapshot(&self, snapshot: &Snapshot) -> CoreResult<()> {
        let mut snaps = self.snapshots.write();
        snaps.insert(snapshot.tick, snapshot.clone());
        Ok(())
    }

    fn read_latest_snapshot(&self) -> CoreResult<Option<Snapshot>> {
        Ok(self.snapshots.read().values().next_back().cloned())
    }

    fn write_kv(&self, key: &str, value: &[u8]) -> CoreResult<()> {
        self.kv.write().insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn read_kv(&self, key: &str) -> CoreResult<Option<Vec<u8>>> {
        Ok(self.kv.read().get(key).cloned())
    }

    fn backend_name(&self) -> &'static str {
        "memory"
    }
}
