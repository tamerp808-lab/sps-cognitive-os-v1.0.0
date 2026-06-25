//! Canonical State.
//!
//! The single source of truth for everything in the kernel. All state
//! here is a pure projection of the event stream through the reducer
//! pipeline. No subsystem may hold authoritative state outside this
//! struct.
//!
//! In Phase 0 the canonical state is minimal — just enough to verify
//! the kernel mechanics. Phase 2+ adds the world model, goals, plans,
//! tasks, agents, memories, providers, etc.

pub mod canonical;
pub mod erased;
pub mod slice;

pub use canonical::CanonicalState;
pub use erased::{ErasedExtension, TypedExtensionCtor, TypedExtensionRegistry};
pub use slice::StateSlice;
