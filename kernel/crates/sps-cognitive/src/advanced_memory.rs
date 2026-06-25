//! SPS Phase 15B — Advanced Memory.
//!
//! Extends the basic Memory system (Phase 3) with:
//! - Memory consolidation (episodic → semantic → procedural)
//! - Forgetting policy (decay + importance-based retention)
//! - Importance scoring
//! - Emotional memory (memory tagged with emotional context)
//! - Knowledge graph expansion (auto-link related memories)

pub mod consolidation;
pub mod forgetting;
pub mod importance;
pub mod emotional;
pub mod knowledge_graph;

pub use consolidation::MemoryConsolidator;
pub use forgetting::ForgettingPolicy;
pub use importance::ImportanceScorer;
pub use emotional::EmotionalMemory;
pub use knowledge_graph::KnowledgeGraphExpander;
