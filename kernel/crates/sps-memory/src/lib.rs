//! SPS Memory Subsystem (Phase 3).
//!
//! Four memory types:
//! - **Episodic**: time-bound records of specific runs.
//! - **Semantic**: facts and concepts.
//! - **Procedural**: how-to workflows.
//! - **Conceptual**: abstract patterns.
//!
//! All four live in a single [`MemoryGraph`] projection that supports
//! typed edges (linking), search, decay, and statistics.

#![allow(clippy::module_name_repetitions)]

pub mod memory;
pub mod graph;
pub mod reducer;
pub mod stats;

pub use memory::{Memory, MemoryId, MemoryKind, MemoryRecord, MemoryStrength};
pub use graph::{MemoryGraph, MemoryLink, MemoryLinkKind};
pub use reducer::{MemoryReducer, MemoryState};
pub use stats::MemoryStats;
