//! Reducer pipeline.
//!
//! Reducers are pure functions `(state, event) -> state`. They are
//! registered per event type. The pipeline applies all registered
//! reducers for an event in registration order.

pub mod pipeline;
pub mod registry;
pub mod builtin;

pub use pipeline::ReducerPipeline;
pub use registry::{ReducerRegistry, ReducerSlot};

use crate::event::Event;
use crate::state::CanonicalState;
use crate::CoreResult;

/// Apply an event to the canonical state, returning the new state.
///
/// Reducers must be pure: no I/O, no clocks, no randomness. Violations
/// are treated as kernel bugs.
pub trait Reducer: Send + Sync + 'static {
    /// Human-readable name (for diagnostics). Default is `"anonymous"`;
    /// implementations should override.
    fn name(&self) -> &'static str {
        "anonymous"
    }

    /// Apply the event to the state. Returning `Err` is treated as a
    /// kernel bug and aborts the pipeline.
    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()>;
}
