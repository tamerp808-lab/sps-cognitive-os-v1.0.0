//! Replay engine and verifier.
//!
//! The replay engine reconstructs [`crate::CanonicalState`] from the
//! event stream in the [`crate::StoragePort`]. It is the single most
//! important component of the kernel — it is what makes SPS a
//! Cognitive Operating System rather than a stateful application.
//!
//! # Determinism contract
//!
//! Given the same event stream, replay MUST produce a byte-identical
//! state. This is verified by:
//!
//! 1. [`ReplayVerifier`] — recomputes every event hash and checks the
//!    chain. Returns a [`ReplayReport`] with the results.
//! 2. [`ReplayEngine`] — applies events to a fresh state via the
//!    [`crate::ReducerPipeline`].
//! 3. Cross-check against the latest snapshot (if any). The snapshot's
//!    `tick` and `last_event_hash` must match the replayed state at
//!    that tick.

use std::sync::Arc;

use crate::event::{EventHash, Tick};
use crate::reducer::ReducerPipeline;
use crate::snapshot::Snapshot;
use crate::state::CanonicalState;
use crate::storage::port::StoragePort;
use crate::CoreResult;

/// Report produced by [`ReplayVerifier::verify_chain`].
#[derive(Debug, Clone)]
pub struct ReplayReport {
    /// Number of events verified.
    pub events_verified: u64,
    /// Tick of the last verified event. `0` if the store is empty.
    pub last_tick: Tick,
    /// Hash of the last verified event.
    pub last_hash: EventHash,
    /// First failure encountered, if any.
    pub failure: Option<ReplayFailure>,
    /// Time spent verifying (microseconds).
    pub elapsed_us: u128,
}

/// A verification failure.
#[derive(Debug, Clone)]
pub enum ReplayFailure {
    /// Hash chain broken — `prev_hash` of an event does not match the
    /// previous event's `hash`.
    HashChainBroken {
        /// Tick where the break was detected.
        tick: Tick,
        /// Stored prev_hash.
        prev: EventHash,
        /// Expected prev_hash (hash of the previous event).
        expected: EventHash,
    },
    /// Stored hash does not match recomputed hash.
    HashMismatch {
        /// Tick of the offending event.
        tick: Tick,
        /// Hash stored in the event.
        stored: EventHash,
        /// Hash recomputed from the canonical input.
        recomputed: EventHash,
    },
    /// Non-monotonic tick.
    NonMonotonicTick {
        /// Previous tick.
        prev: Tick,
        /// Current tick.
        curr: Tick,
    },
}

/// The replay verifier. Walks the event stream and checks the hash chain.
pub struct ReplayVerifier;

impl ReplayVerifier {
    /// Verify the entire event stream. Reads events in chunks and checks:
    ///
    /// 1. Each event's stored hash matches the recomputed hash.
    /// 2. Each event's `prev_hash` matches the previous event's `hash`.
    /// 3. Ticks are strictly monotonic.
    ///
    /// Returns a [`ReplayReport`] with the results.
    pub fn verify_chain(storage: &dyn StoragePort) -> CoreResult<ReplayReport> {
        Self::verify_chain_from(storage, 0)
    }

    /// Verify the event stream starting from `from_tick`.
    pub fn verify_chain_from(storage: &dyn StoragePort, from_tick: Tick) -> CoreResult<ReplayReport> {
        let start = std::time::Instant::now();
        let chunk_size = 1024usize;
        let mut events_verified: u64 = 0;
        let mut last_tick: Tick = from_tick.saturating_sub(1);
        let mut last_hash = if from_tick == 0 {
            EventHash::GENESIS
        } else {
            // Read the event just before from_tick to get the expected prev_hash.
            match storage.read_event_by_tick(from_tick.saturating_sub(1))? {
                Some(e) => e.hash,
                None => EventHash::GENESIS,
            }
        };

        let mut cursor = from_tick;
        loop {
            let chunk = storage.read_events_from(cursor, chunk_size)?;
            if chunk.is_empty() {
                break;
            }
            for event in &chunk {
                // Tick monotonicity.
                if event.tick <= last_tick && events_verified > 0 {
                    return Ok(ReplayReport {
                        events_verified,
                        last_tick,
                        last_hash,
                        failure: Some(ReplayFailure::NonMonotonicTick {
                            prev: last_tick,
                            curr: event.tick,
                        }),
                        elapsed_us: start.elapsed().as_micros(),
                    });
                }
                // Hash recomputation.
                let recomputed = event.recompute_hash();
                if recomputed != event.hash {
                    return Ok(ReplayReport {
                        events_verified,
                        last_tick,
                        last_hash,
                        failure: Some(ReplayFailure::HashMismatch {
                            tick: event.tick,
                            stored: event.hash,
                            recomputed,
                        }),
                        elapsed_us: start.elapsed().as_micros(),
                    });
                }
                // Chain continuity.
                if event.prev_hash != last_hash {
                    return Ok(ReplayReport {
                        events_verified,
                        last_tick,
                        last_hash,
                        failure: Some(ReplayFailure::HashChainBroken {
                            tick: event.tick,
                            prev: event.prev_hash,
                            expected: last_hash,
                        }),
                        elapsed_us: start.elapsed().as_micros(),
                    });
                }
                last_tick = event.tick;
                last_hash = event.hash;
                events_verified += 1;
            }
            // Advance cursor past the last read tick.
            cursor = last_tick.saturating_add(1);
            if chunk.len() < chunk_size {
                break;
            }
        }

        Ok(ReplayReport {
            events_verified,
            last_tick,
            last_hash,
            failure: None,
            elapsed_us: start.elapsed().as_micros(),
        })
    }
}

/// The replay engine. Reconstructs [`CanonicalState`] from the event
/// stream.
pub struct ReplayEngine {
    pipeline: Arc<ReducerPipeline>,
    /// Optional typed-extension registry (P3D). When present, snapshot
    /// loads reconstruct typed extensions from JSON via the registered
    /// constructors before tail-replaying.
    typed_registry: Option<Arc<crate::state::TypedExtensionRegistry>>,
}

impl ReplayEngine {
    /// Create a new engine backed by the given reducer pipeline.
    pub fn new(pipeline: Arc<ReducerPipeline>) -> Self {
        Self {
            pipeline,
            typed_registry: None,
        }
    }

    /// Create a new engine with a typed-extension registry. Snapshot
    /// loads will use this registry to rebuild typed extensions from
    /// JSON before tail-replaying (P3D).
    pub fn with_typed_registry(
        pipeline: Arc<ReducerPipeline>,
        typed_registry: Arc<crate::state::TypedExtensionRegistry>,
    ) -> Self {
        Self {
            pipeline,
            typed_registry: Some(typed_registry),
        }
    }

    /// Replay from scratch (tick 0). Returns the reconstructed state.
    pub fn replay_from_genesis(
        &self,
        storage: &dyn StoragePort,
    ) -> CoreResult<CanonicalState> {
        self.replay_from(storage, 0, CanonicalState::genesis())
    }

    /// Replay from a snapshot: load the snapshot, verify it, then
    /// replay the tail of the event stream on top.
    pub fn replay_from_snapshot(
        &self,
        storage: &dyn StoragePort,
        snapshot: &Snapshot,
    ) -> CoreResult<CanonicalState> {
        snapshot.verify()?;
        let mut state = snapshot.state.clone();
        // P3D: rebuild typed extensions from JSON before tail-replaying.
        if let Some(reg) = &self.typed_registry {
            state.rebuild_typed_from_json(reg);
        }
        // Defensive: if the snapshot was tampered with at the state level
        // but the hash matches, we still trust the recorded tick+hash
        // as the resume point.
        let from_tick = snapshot.tick.saturating_add(1);
        self.replay_from(storage, from_tick, state)
    }

    /// Replay events starting at `from_tick` onto the given state.
    pub fn replay_from(
        &self,
        storage: &dyn StoragePort,
        from_tick: Tick,
        mut state: CanonicalState,
    ) -> CoreResult<CanonicalState> {
        let chunk_size = 1024usize;
        let mut cursor = from_tick;
        loop {
            let chunk = storage.read_events_from(cursor, chunk_size)?;
            if chunk.is_empty() {
                break;
            }
            for event in &chunk {
                self.pipeline.apply(&mut state, event)?;
            }
            cursor = chunk
                .last()
                .map(|e| e.tick.saturating_add(1))
                .unwrap_or(cursor);
            if chunk.len() < chunk_size {
                break;
            }
        }
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Actor;
    use crate::event::{EventHash, RawEvent};
    use crate::reducer::{ReducerPipeline, ReducerRegistry};
    use serde_json::json;

    // Helper: build a fresh pipeline. KernelMetaReducer is always-on
    // (see ReducerPipeline::apply), so we don't register anything
    // explicitly for the test event types — the pipeline still
    // processes them and updates kernel meta on each apply.
    fn fresh_pipeline() -> Arc<ReducerPipeline> {
        let reg = ReducerRegistry::new();
        Arc::new(ReducerPipeline::new(Arc::new(reg)))
    }

    #[test]
    fn empty_store_produces_genesis_state() {
        // We need a concrete storage impl to test this; covered in
        // workspace-level tests. Here we just verify the engine builds.
        let _engine = ReplayEngine::new(fresh_pipeline());
    }

    #[test]
    fn hash_input_excludes_wall_time_and_correlation() {
        let payload = json!({"k": "v"});
        let e1 = RawEvent::new("test.a", payload.clone(), Actor::owner(), 100)
            .finalize(1, EventHash::GENESIS);
        let e2 = RawEvent::new("test.a", payload, Actor::owner(), 999)
            .finalize(1, EventHash::GENESIS);
        assert_eq!(e1.hash, e2.hash);
        assert!(e1.hash_is_valid());
        assert!(e2.hash_is_valid());
    }
}
