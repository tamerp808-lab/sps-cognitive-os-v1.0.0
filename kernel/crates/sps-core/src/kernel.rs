//! The SPS Kernel facade.
//!
//! This is the single entry point that all consumers (Adapters, CLI,
//! Desktop, Web) eventually talk to. It wires together the EventStore,
//! ReducerPipeline, SnapshotManager, and ReplayEngine.
//!
//! In Phase 0 the kernel exposes:
//!
//! - `boot` — open the storage, resume the clock, verify the chain.
//! - `dispatch` — append an event and apply it to canonical state.
//! - `query` — read-only access to canonical state.
//! - `snapshot` — take and persist a snapshot.
//! - `verify` — run the replay verifier on the chain.
//! - `replay` — full replay from genesis (or snapshot) for diagnostics.

use std::sync::Arc;

use parking_lot::RwLock;

use crate::event::{Event, EventHash, RawEvent, Tick};
use crate::event_store::EventStore;
use crate::reducer::{ReducerPipeline, ReducerRegistry};
use crate::replay::{ReplayEngine, ReplayReport, ReplayVerifier};
use crate::snapshot::{Snapshot, SnapshotManager};
use crate::state::CanonicalState;
use crate::storage::port::StoragePort;
use crate::{CoreError, CoreResult};

/// Configuration for the kernel.
#[derive(Debug, Clone)]
pub struct KernelConfig {
    /// Snapshot interval (in events).
    pub snapshot_interval: u64,
    /// Optional typed-extension registry (P3D). When present, snapshot
    /// loads reconstruct typed extensions directly from JSON via the
    /// registered constructors.
    pub typed_registry: Option<Arc<crate::state::TypedExtensionRegistry>>,
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self {
            snapshot_interval: 1_000,
            typed_registry: None,
        }
    }
}

impl KernelConfig {
    /// Attach a typed-extension registry (P3D).
    pub fn with_typed_registry(mut self, reg: crate::state::TypedExtensionRegistry) -> Self {
        self.typed_registry = Some(Arc::new(reg));
        self
    }
}

/// The SPS Kernel. Owns the event store, reducer pipeline, canonical
/// state, and snapshot manager.
pub struct SpsKernel {
    config: KernelConfig,
    store: Arc<EventStore>,
    pipeline: Arc<ReducerPipeline>,
    snapshots: SnapshotManager,
    /// The current canonical state. Updated by `dispatch` after each
    /// successful append. Read by `query`.
    state: RwLock<CanonicalState>,
    /// Whether the kernel has been booted.
    booted: parking_lot::Mutex<bool>,
}

impl SpsKernel {
    /// Boot the kernel against the given storage. Equivalent to
    /// [`boot_with`](Self::boot_with) with an empty domain-reducer
    /// registration callback.
    pub fn boot(
        storage: Arc<dyn StoragePort>,
        config: KernelConfig,
    ) -> CoreResult<Self> {
        Self::boot_with(storage, config, |_| {})
    }

    /// Boot the kernel with an explicit domain-reducer registration
    /// callback. The callback receives a fresh [`ReducerRegistry`] and
    /// is expected to register all domain reducers (e.g. memory, goals,
    /// autonomy). The [`KernelMetaReducer`] is invoked automatically by
    /// the pipeline on every event — it must NOT be registered here.
    ///
    /// # Errors
    ///
    /// - [`crate::CoreError::Internal`] if chain verification fails.
    /// - [`crate::CoreError`] from the storage or replay engine.
    ///
    /// [`KernelMetaReducer`]: crate::reducer::builtin::KernelMetaReducer
    pub fn boot_with(
        storage: Arc<dyn StoragePort>,
        config: KernelConfig,
        register_domain: impl FnOnce(&mut ReducerRegistry),
    ) -> CoreResult<Self> {
        // 1. Verify chain integrity.
        let report = ReplayVerifier::verify_chain(storage.as_ref())?;
        if let Some(failure) = report.failure {
            return Err(CoreError::Internal(anyhow::anyhow!(
                "chain verification failed at {:?}",
                failure
            )));
        }

        // 2. Build reducer pipeline. KernelMetaReducer is ALWAYS invoked by
        //    the pipeline itself (see ReducerPipeline::apply). Only register
        //    domain reducers via the callback.
        let pipeline = {
            let mut reg = ReducerRegistry::new();
            register_domain(&mut reg);
            Arc::new(ReducerPipeline::new(Arc::new(reg)))
        };

        // 3. Snapshot + tail replay. If the config carries a typed-extension
        //    registry, use it to reconstruct typed extensions from JSON
        //    on snapshot load (P3D).
        let replay = match &config.typed_registry {
            Some(reg) => ReplayEngine::with_typed_registry(pipeline.clone(), reg.clone()),
            None => ReplayEngine::new(pipeline.clone()),
        };
        let snapshot = storage.read_latest_snapshot()?;
        let state = match snapshot {
            Some(s) => replay.replay_from_snapshot(storage.as_ref(), &s)?,
            None => replay.replay_from_genesis(storage.as_ref())?,
        };

        let store = Arc::new(EventStore::new(storage.clone())?);

        Ok(Self {
            config,
            store,
            pipeline,
            snapshots: SnapshotManager::new(1_000),
            state: RwLock::new(state),
            booted: parking_lot::Mutex::new(true),
        })
    }

    /// Returns the configured snapshot interval.
    pub fn snapshot_interval(&self) -> u64 {
        self.config.snapshot_interval
    }

    /// Returns `true` if the kernel has been booted.
    pub fn is_booted(&self) -> bool {
        *self.booted.lock()
    }

    /// Dispatch a raw event: append to the store, apply to canonical
    /// state, return the finalized event.
    pub fn dispatch(&self, raw: RawEvent) -> CoreResult<Event> {
        if !*self.booted.lock() {
            return Err(CoreError::Internal(anyhow::anyhow!(
                "kernel not booted"
            )));
        }
        let event = self.store.append(raw)?;
        let mut state = self.state.write();
        self.pipeline.apply(&mut state, &event)?;
        // Possibly take a snapshot.
        // (Phase 0: snapshot logic is invoked manually via `snapshot()`.)
        Ok(event)
    }

    /// Dispatch a trusted raw event — skips validate-on-write clone
    /// (fast path). For internal producers that construct well-formed
    /// payloads (e.g. AgentRuntime, LongRunningGoalRunner).
    pub fn dispatch_trusted(&self, raw: RawEvent) -> CoreResult<Event> {
        if !*self.booted.lock() {
            return Err(CoreError::Internal(anyhow::anyhow!("kernel not booted")));
        }
        let event = self.store.append(raw)?;
        let mut state = self.state.write();
        self.pipeline.apply(&mut state, &event)?;
        Ok(event)
    }

    /// Read-only access to canonical state via a closure. The kernel
    /// holds a read lock for the duration of the closure.
    pub fn query<R>(&self, f: impl FnOnce(&CanonicalState) -> R) -> R {
        let state = self.state.read();
        f(&state)
    }

    /// Take a snapshot of the current canonical state and persist it.
    pub fn snapshot(&self, wall_time: u64) -> CoreResult<Snapshot> {
        let state = self.state.read().clone();
        let snap = Snapshot::take(&state, wall_time)?;
        self.store.storage().write_snapshot(&snap)?;
        Ok(snap)
    }

    /// Verify the hash chain. Returns a [`ReplayReport`].
    pub fn verify(&self) -> CoreResult<ReplayReport> {
        ReplayVerifier::verify_chain(self.store.storage().as_ref())
    }

    /// Full replay from genesis (ignoring snapshots). Returns the
    /// reconstructed state. Does NOT mutate the live canonical state —
    /// this is a diagnostic operation.
    pub fn replay_from_genesis(&self) -> CoreResult<CanonicalState> {
        let engine = ReplayEngine::new(self.pipeline.clone());
        engine.replay_from_genesis(self.store.storage().as_ref())
    }

    /// Returns the last tick persisted in the store.
    pub fn last_tick(&self) -> CoreResult<Tick> {
        self.store.last_tick()
    }

    /// Returns the last hash persisted in the store.
    pub fn last_hash(&self) -> CoreResult<EventHash> {
        self.store.last_hash()
    }

    /// Returns the count of events in the store.
    pub fn event_count(&self) -> CoreResult<u64> {
        self.store.count()
    }

    /// Returns the snapshot manager.
    pub fn snapshot_manager(&self) -> &SnapshotManager {
        &self.snapshots
    }

    /// Returns the reducer pipeline (for registration by future phases).
    pub fn pipeline(&self) -> &ReducerPipeline {
        &self.pipeline
    }

    /// Returns the underlying event store.
    pub fn store(&self) -> &EventStore {
        &self.store
    }

    /// Returns the storage backend name.
    pub fn backend_name(&self) -> &'static str {
        self.store.storage().backend_name()
    }
}

impl crate::sink::EventSink for SpsKernel {
    fn dispatch(&self, raw: RawEvent) -> CoreResult<Event> {
        self.dispatch(raw)
    }
    fn dispatch_trusted(&self, raw: RawEvent) -> CoreResult<Event> {
        self.dispatch_trusted(raw)
    }
}
