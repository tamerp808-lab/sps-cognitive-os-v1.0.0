//! Snapshot manager.
//!
//! A snapshot is a serialized [`crate::CanonicalState`] tagged with the
//! tick at which it was taken. Snapshots let the replay engine skip
//! re-applying the entire event stream on boot — load the latest
//! snapshot, then replay only the tail.
//!
//! Snapshots themselves are content-addressed: their `hash` is the
//! SHA-256 of their serialized state. The replay verifier recomputes
//! this and refuses to load a snapshot whose hash doesn't match.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::event::{EventHash, Tick};
use crate::state::CanonicalState;
use crate::{CoreError, CoreResult, KERNEL_SCHEMA_VERSION};

/// A snapshot of the canonical state at a particular tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Tick of the last applied event when this snapshot was taken.
    pub tick: Tick,
    /// Hash of the last applied event.
    pub last_event_hash: EventHash,
    /// Schema version at the time of snapshot.
    pub schema_version: u16,
    /// Wall time when the snapshot was taken. Display only.
    pub wall_time: u64,
    /// Canonical state at this tick (serialized as JSON).
    pub state: CanonicalState,
    /// SHA-256 of the canonical-JSON serialization of `state`. Computed
    /// by [`Snapshot::take`] and verified on load.
    pub state_hash: [u8; 32],
}

impl Snapshot {
    /// Take a snapshot of the given state. The state's `last_tick` and
    /// `last_hash` are captured; the state is serialized to canonical
    /// JSON and hashed.
    ///
    /// P3D: typed extensions are synced into the JSON `extensions` map
    /// before serialization so they round-trip through the snapshot.
    pub fn take(state: &CanonicalState, wall_time: u64) -> CoreResult<Self> {
        let mut state_for_snapshot = state.clone();
        state_for_snapshot.sync_typed_to_json()?;

        let bytes = serialize_state_canonical(&state_for_snapshot)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let mut state_hash = [0u8; 32];
        state_hash.copy_from_slice(&hasher.finalize());

        Ok(Self {
            tick: state.last_tick(),
            last_event_hash: state.last_hash(),
            schema_version: KERNEL_SCHEMA_VERSION,
            wall_time,
            state: state_for_snapshot,
            state_hash,
        })
    }

    /// Verify that this snapshot's stored `state_hash` matches the hash
    /// recomputed from its `state`.
    pub fn verify(&self) -> CoreResult<()> {
        let bytes = serialize_state_canonical(&self.state)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let mut recomputed = [0u8; 32];
        recomputed.copy_from_slice(&hasher.finalize());
        if recomputed != self.state_hash {
            return Err(CoreError::Internal(anyhow::anyhow!(
                "snapshot state_hash mismatch: stored={}, recomputed={}",
                hex::encode(self.state_hash),
                hex::encode(recomputed)
            )));
        }
        Ok(())
    }
}

/// The snapshot manager coordinates taking and loading snapshots via a
/// [`crate::StoragePort`].
#[derive(Debug)]
pub struct SnapshotManager {
    /// Interval: take a snapshot every N events.
    pub interval: u64,
}

impl Default for SnapshotManager {
    fn default() -> Self {
        Self { interval: 1_000 }
    }
}

impl SnapshotManager {
    /// Create a manager with the given snapshot interval.
    pub fn new(interval: u64) -> Self {
        Self { interval }
    }

    /// Returns `true` if a snapshot should be taken at the current tick.
    pub fn should_snapshot(&self, current_tick: Tick, last_snapshot_tick: Tick) -> bool {
        if self.interval == 0 {
            return false;
        }
        current_tick.saturating_sub(last_snapshot_tick) >= self.interval
    }
}

/// Serialize canonical state to bytes with sorted keys (for hashing).
fn serialize_state_canonical(state: &CanonicalState) -> CoreResult<Vec<u8>> {
    // Round-trip through serde_json::Value to ensure key sorting.
    let v: serde_json::Value = serde_json::to_value(state)?;
    // Use serde_json's default serializer which sorts Map<String, Value>
    // keys when preserve_order is not enabled on this Value.
    // Our value came from `to_value` so it's a BTreeMap-backed Object.
    Ok(serde_json::to_vec(&v)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Actor;
    use crate::event::{EventHash, RawEvent};
    use crate::reducer::{ReducerPipeline, ReducerRegistry};
    use serde_json::json;

    #[test]
    fn snapshot_round_trips_and_verifies() {
        let mut state = CanonicalState::genesis();
        let e = RawEvent::new("system.booted", json!({}), Actor::owner(), 0)
            .finalize(1, EventHash::GENESIS);
        // KernelMetaReducer runs always-on in ReducerPipeline; we don't
        // need to register it explicitly.
        let reg = ReducerRegistry::new();
        let pipe = ReducerPipeline::new(std::sync::Arc::new(reg));
        pipe.apply(&mut state, &e).unwrap();

        let snap = Snapshot::take(&state, 1_000).unwrap();
        assert_eq!(snap.tick, 1);
        assert_eq!(snap.last_event_hash, e.hash);
        snap.verify().unwrap();
    }

    #[test]
    fn snapshot_detects_tampered_state() {
        let mut state = CanonicalState::genesis();
        state.kernel.event_count = 5;
        let mut snap = Snapshot::take(&state, 0).unwrap();
        // Tamper: bump count without recomputing hash.
        snap.state.kernel.event_count = 999;
        assert!(snap.verify().is_err());
    }

    #[test]
    fn snapshot_interval_check() {
        let m = SnapshotManager::new(100);
        assert!(m.should_snapshot(100, 0));
        assert!(m.should_snapshot(200, 100));
        assert!(!m.should_snapshot(50, 0));
        assert!(!m.should_snapshot(99, 0));
    }
}
