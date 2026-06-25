//! A single state slice — a typed view into the canonical state.
//!
//! In Phase 0 the only slice is `KernelMeta`. Future phases add
//! `WorldSlice`, `GoalSlice`, `MemorySlice`, etc.

use serde::{Deserialize, Serialize};

use crate::event::{EventHash, Tick};

/// Kernel metadata slice. Tracks the last applied event and snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSlice {
    /// Last applied tick. `0` if no events have been applied.
    pub last_tick: Tick,
    /// Last applied event hash. [`EventHash::GENESIS`] if no events.
    pub last_hash: EventHash,
    /// Total event count.
    pub event_count: u64,
}
