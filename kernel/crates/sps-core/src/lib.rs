//! SPS Kernel Core
//!
//! The deterministic, replayable foundation of the SPS Cognitive Operating
//! System. Everything in this crate is pure Rust — no I/O, no network, no
//! clocks beyond the logical clock. Non-determinism is quarantined behind
//! the `StoragePort` (which the kernel only uses for append/read, never for
//! computation) and the (Phase 1) Effect Manager.
//!
//! # Determinism contract
//!
//! Given an identical event stream, [`replay::ReplayEngine`] MUST produce
//! byte-identical [`state::CanonicalState`]. The following rules are
//! enforced:
//!
//! 1. `Event::wall_time` is **never** read by any reducer.
//! 2. `Event::wall_time` is **never** mixed into the event hash.
//! 3. `Event::correlation_id` is for display/tracing only.
//! 4. Reducers are pure functions `(state, event) -> state`.
//! 5. The hash chain is `SHA-256(prev_hash || tick_be || event_type || payload_canonical)`.
//! 6. The logical clock is monotonically increasing per stream.

#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

pub mod actor;
pub mod clock;
pub mod error;
pub mod event;
pub mod event_store;
pub mod event_type;
pub mod kernel;
pub mod reducer;
pub mod replay;
pub mod sink;
pub mod snapshot;
pub mod state;
pub mod storage;

pub use actor::{Actor, ActorKind};
pub use clock::LogicalClock;
pub use error::{CoreError, CoreResult};
pub use event::{Event, EventHash, RawEvent, Tick};
pub use event_store::EventStore;
pub use event_type::EventType;
pub use kernel::SpsKernel;
pub use reducer::{Reducer, ReducerPipeline, ReducerRegistry};
pub use replay::{ReplayEngine, ReplayReport, ReplayVerifier};
pub use sink::EventSink;
pub use snapshot::{Snapshot, SnapshotManager};
pub use state::CanonicalState;
pub use storage::port::StoragePort;

/// Version of the kernel core schema.
///
/// Bumped when the wire format of `Event` or `CanonicalState` changes in a
/// backward-incompatible way. Stored in every event and every snapshot.
pub const KERNEL_SCHEMA_VERSION: u16 = 1;
