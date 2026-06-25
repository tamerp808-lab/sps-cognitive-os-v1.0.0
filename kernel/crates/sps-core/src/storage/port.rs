//! The `StoragePort` trait — the single persistence abstraction the
//! kernel depends on.
//!
//! Backends implement this trait; the kernel calls only these methods.
//! There is no `rusqlite` import anywhere in `sps-core`.

use crate::event::{Event, Tick};
use crate::snapshot::Snapshot;
use crate::CoreResult;

/// Atomic, transactional storage for events, snapshots, and key/value
/// metadata.
///
/// # Concurrency
///
/// Implementations must be `Send + Sync`. Internal locking is the
/// backend's responsibility. The kernel assumes that `append_event` is
/// linearizable across threads — two concurrent appends must result in
/// two events with monotonically increasing ticks, never the same tick.
pub trait StoragePort: Send + Sync {
    /// Append an event to the store. The event must already have its
    /// `tick`, `prev_hash`, and `hash` fields populated by the
    /// [`crate::EventStore`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::NonMonotonicTick`] if the event's tick
    /// is not strictly greater than the current last tick.
    fn append_event(&self, event: &Event) -> CoreResult<()>;

    /// Read up to `limit` events starting at `from_tick` (inclusive),
    /// in ascending tick order.
    fn read_events_from(&self, from_tick: Tick, limit: usize) -> CoreResult<Vec<Event>>;

    /// Read a single event by tick.
    fn read_event_by_tick(&self, tick: Tick) -> CoreResult<Option<Event>>;

    /// Return the tick of the last appended event, or `0` if the store
    /// is empty.
    fn last_tick(&self) -> CoreResult<Tick>;

    /// Return the hash of the last appended event, or
    /// [`crate::EventHash::GENESIS`] if the store is empty.
    fn last_hash(&self) -> CoreResult<crate::event::EventHash>;

    /// Count events in the store.
    fn count_events(&self) -> CoreResult<u64>;

    /// Persist a snapshot. Replaces any existing snapshot at the same
    /// tick.
    fn write_snapshot(&self, snapshot: &Snapshot) -> CoreResult<()>;

    /// Read the most recent snapshot, if any.
    fn read_latest_snapshot(&self) -> CoreResult<Option<Snapshot>>;

    /// Write a key/value pair (small metadata — config, owner profile,
    /// etc.). Replaces existing value for the same key.
    fn write_kv(&self, key: &str, value: &[u8]) -> CoreResult<()>;

    /// Read a key/value pair.
    fn read_kv(&self, key: &str) -> CoreResult<Option<Vec<u8>>>;

    /// Backend name (e.g. `"sqlite"`, `"memory"`). For diagnostics.
    fn backend_name(&self) -> &'static str;
}

