//! Event sink trait — allows external components (like AgentRuntime) to
//! dispatch events to the kernel without a direct dependency on `SpsKernel`.
//!
//! Fix #8: AgentRuntime uses this trait to dispatch events. SpsKernel
//! implements it. This keeps the dependency direction clean.
//!
//! P2: `dispatch_trusted` skips validate-on-write clone for performance.
//! Trusted producers (AgentRuntime, FactoryWorkflow) use it.

use crate::event::{Event, RawEvent};
use crate::CoreResult;

/// A sink for events. Anything that can accept `RawEvent`s and persist them
/// to the Event Store implements this trait.
pub trait EventSink: Send + Sync + 'static {
    /// Dispatch a raw event with validate-on-write (safe, slower).
    fn dispatch(&self, raw: RawEvent) -> CoreResult<Event>;

    /// Dispatch a trusted event — skips validate-on-write clone (fast).
    /// P2: For internal producers that construct well-formed payloads.
    fn dispatch_trusted(&self, raw: RawEvent) -> CoreResult<Event> {
        // Default: fall back to validated dispatch.
        self.dispatch(raw)
    }
}
