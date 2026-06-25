//! Reducer pipeline — applies all reducers for an event in order.

use std::sync::Arc;

use crate::event::Event;
use crate::reducer::builtin::KernelMetaReducer;
use crate::reducer::{Reducer, ReducerRegistry};
use crate::state::CanonicalState;
use crate::{CoreError, CoreResult};

/// The reducer pipeline. Holds a reference to the registry and applies
/// events to state.
///
/// # Always-on `KernelMetaReducer`
///
/// The pipeline always invokes [`KernelMetaReducer`] first, regardless of
/// what is in the registry. This prevents double-counting when domain
/// reducers are also registered (the previous scheme required callers to
/// explicitly register `KernelMetaReducer` for every event type they
/// produced, which was error-prone — see Fix #16).
#[derive(Clone)]
pub struct ReducerPipeline {
    registry: Arc<ReducerRegistry>,
    kernel_meta: Arc<KernelMetaReducer>,
}

impl ReducerPipeline {
    /// Create a pipeline backed by the given registry.
    pub fn new(registry: Arc<ReducerRegistry>) -> Self {
        Self {
            registry,
            kernel_meta: KernelMetaReducer::shared(),
        }
    }

    /// Apply a single event to the state. The state is mutated in place.
    ///
    /// # Always-on `KernelMetaReducer`
    ///
    /// The kernel-meta reducer runs first on every event, regardless of
    /// whether any domain reducer is registered for the event type. This
    /// means `event_count`, `last_tick`, and `last_hash` are updated
    /// exactly once per applied event (Fix #16: previously callers had
    /// to register `KernelMetaReducer` explicitly per event type, which
    /// led to double-counting when domain reducers were also registered).
    ///
    /// # Errors
    ///
    /// - [`CoreError::ReducerFailed`] if any reducer returns `Err`.
    pub fn apply(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        // Kernel-meta reducer always runs first. This updates last_tick,
        // last_hash, and event_count exactly once per applied event.
        // It is NOT registered in the domain registry — registering it
        // there would cause double-counting (Fix #16).
        self.kernel_meta
            .reduce(state, event)
            .map_err(|e| CoreError::ReducerFailed {
                reducer: "kernel.meta",
                tick: event.tick,
                event_type: event.event_type.as_str().to_string(),
                source: e.into(),
            })?;

        // Domain reducers run after kernel-meta.
        let slots = self.registry.get(&event.event_type);
        for slot in slots {
            let reducer: &dyn Reducer = slot.reducer.as_ref();
            let name = reducer.name();
            reducer
                .reduce(state, event)
                .map_err(|e| CoreError::ReducerFailed {
                    reducer: name,
                    tick: event.tick,
                    event_type: event.event_type.as_str().to_string(),
                    source: e.into(),
                })?;
        }
        Ok(())
    }

    /// Reference to the underlying registry.
    pub fn registry(&self) -> &ReducerRegistry {
        &self.registry
    }
}
