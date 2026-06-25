//! Reducer registry.

use std::sync::Arc;

use crate::event_type::EventType;
use crate::reducer::Reducer;
use crate::CoreError;

/// A registered reducer slot — the reducer plus its registration order.
pub struct ReducerSlot {
    /// Order of registration (lower runs first within an event type).
    pub order: u32,
    /// The reducer.
    pub reducer: Arc<dyn Reducer>,
}

impl std::ops::Deref for ReducerSlot {
    type Target = dyn Reducer;
    fn deref(&self) -> &Self::Target {
        self.reducer.as_ref()
    }
}

/// Registry of reducers keyed by event type.
#[derive(Default)]
pub struct ReducerRegistry {
    slots: std::collections::HashMap<EventType, Vec<ReducerSlot>>,
    next_order: u32,
}

impl ReducerRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a reducer for an event type. Multiple reducers may be
    /// registered for the same event type; they run in registration order.
    pub fn register(
        &mut self,
        event_type: impl Into<EventType>,
        reducer: Arc<dyn Reducer>,
    ) -> &mut Self {
        let entry = self.slots.entry(event_type.into()).or_default();
        let order = self.next_order;
        self.next_order = self.next_order.saturating_add(1);
        entry.push(ReducerSlot { order, reducer });
        // Keep sorted by order so the pipeline can iterate directly.
        entry.sort_by_key(|s| s.order);
        self
    }

    /// Get the reducers registered for an event type, in run order.
    pub fn get(&self, event_type: &EventType) -> &[ReducerSlot] {
        self.slots
            .get(event_type)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns `true` if at least one reducer is registered for the event type.
    pub fn has(&self, event_type: &EventType) -> bool {
        self.slots
            .get(event_type)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }

    /// Number of registered event types.
    pub fn event_type_count(&self) -> usize {
        self.slots.len()
    }
}

/// Convert a missing-reducer condition into a [`CoreError`].
pub fn require_reducer(
    registry: &ReducerRegistry,
    event_type: &EventType,
) -> Result<(), CoreError> {
    if registry.has(event_type) {
        Ok(())
    } else {
        Err(CoreError::UnknownEventType(event_type.as_str().to_string()))
    }
}
