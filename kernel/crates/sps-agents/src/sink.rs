//! Event sink re-export.
//!
//! Fix #7a: AgentRuntime uses `sps_core::sink::EventSink` (defined in
//! sps-core) to dispatch events. This keeps the dependency direction clean:
//! sps-agents → sps-core (trait), not sps-core → sps-agents.

pub use sps_core::sink::EventSink;

/// A no-op sink for headless/testing mode. Events are silently dropped.
/// This preserves backward compatibility with existing unit tests that
/// construct AgentRuntime without a kernel.
pub struct NullEventSink;

impl EventSink for NullEventSink {
    fn dispatch(&self, _raw: sps_core::event::RawEvent) -> sps_core::CoreResult<sps_core::event::Event> {
        // Return a synthetic event with tick=0 and genesis hash.
        Ok(sps_core::event::RawEvent::new(
            "noop",
            serde_json::json!({}),
            sps_core::actor::Actor::owner(),
            0,
        )
        .finalize(0, sps_core::event::EventHash::GENESIS))
    }
}
