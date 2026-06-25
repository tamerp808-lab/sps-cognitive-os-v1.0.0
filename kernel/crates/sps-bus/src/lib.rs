//! SPS Command Bus + Event Bus (Phase 2).
//!
//! The Command Bus is the inbound API: commands → intent events. The
//! Event Bus is the outbound pub/sub for surfaces and subscribers.
//!
//! Both are layered on top of the kernel core's EventStore and do not
//! modify the determinism contract — they are scheduling/dispatch
//! conveniences only.

#![allow(clippy::module_name_repetitions)]

pub mod command;
pub mod event_bus;
pub mod state_ext;

pub use command::{Command, CommandBus, CommandHandler, CommandRegistry};
pub use event_bus::{EventBus, EventSubscription, SubscriptionId};
pub use state_ext::{OwnerProfile, OwnerState, OwnerReducer};
