//! Built-in reducers that ship with the kernel core.
//!
//! Phase 0 ships exactly one reducer: `KernelMetaReducer`, which updates
//! the `state.kernel` slice (last tick, last hash, event count) on every
//! event regardless of type. Future phases register their own reducers
//! alongside this one.

use std::sync::Arc;

use crate::event::Event;
use crate::reducer::{Reducer, ReducerRegistry};
use crate::state::CanonicalState;
use crate::CoreResult;

/// The kernel meta reducer. Runs on every event. Updates `state.kernel`
/// to reflect the latest applied event.
#[derive(Debug, Default)]
pub struct KernelMetaReducer;

impl KernelMetaReducer {
    /// Register this reducer for an event type. Returns the reducer as
    /// an `Arc` so it can be shared across multiple event types.
    pub fn shared() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl Reducer for KernelMetaReducer {
    fn name(&self) -> &'static str {
        "kernel.meta"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        state.kernel.last_tick = event.tick;
        state.kernel.last_hash = event.hash;
        state.kernel.event_count = state.kernel.event_count.saturating_add(1);
        Ok(())
    }
}

/// Register the kernel meta reducer for a list of event types.
///
/// Convenience function used during kernel boot.
pub fn register_for_event_types(
    registry: &mut ReducerRegistry,
    event_types: &[&str],
) {
    let r = KernelMetaReducer::shared();
    for et in event_types {
        registry.register(*et, r.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Actor;
    use crate::event::{EventHash, RawEvent};
    use serde_json::json;

    #[test]
    fn kernel_meta_updates_on_every_event() {
        let r = KernelMetaReducer;
        let mut state = CanonicalState::genesis();

        let e1 = RawEvent::new("system.booted", json!({}), Actor::owner(), 0)
            .finalize(1, EventHash::GENESIS);
        r.reduce(&mut state, &e1).unwrap();
        assert_eq!(state.kernel.last_tick, 1);
        assert_eq!(state.kernel.last_hash, e1.hash);
        assert_eq!(state.kernel.event_count, 1);

        let e2 = RawEvent::new("system.snapshot_taken", json!({"id": "s1"}), Actor::owner(), 0)
            .finalize(2, e1.hash);
        r.reduce(&mut state, &e2).unwrap();
        assert_eq!(state.kernel.last_tick, 2);
        assert_eq!(state.kernel.last_hash, e2.hash);
        assert_eq!(state.kernel.event_count, 2);
    }
}
