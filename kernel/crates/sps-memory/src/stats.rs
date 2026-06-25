//! Memory statistics — surfaced in the Dashboard.

use serde::{Deserialize, Serialize};

use crate::memory::MemoryKind;
use crate::graph::MemoryGraph;

/// Aggregated memory statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Total memory count.
    pub total: usize,
    /// Count by kind.
    pub by_kind: std::collections::BTreeMap<String, usize>,
    /// Total link count.
    pub links: usize,
    /// Average strength across all memories (0.0–1.0).
    pub avg_strength: f32,
    /// Memories accessed in the last 24h (caller-provided window).
    pub recently_accessed: usize,
}

impl MemoryStats {
    /// Compute stats from a memory graph.
    pub fn from_graph(graph: &MemoryGraph) -> Self {
        let total = graph.count();
        let mut by_kind = std::collections::BTreeMap::new();
        for kind in [
            MemoryKind::Episodic,
            MemoryKind::Semantic,
            MemoryKind::Procedural,
            MemoryKind::Conceptual,
        ] {
            by_kind.insert(kind.as_str().to_string(), graph.count_by_kind(kind));
        }
        let links = graph.links.len();
        let avg_strength = if total == 0 {
            0.0
        } else {
            let sum: f32 = graph.memories.values().map(|m| m.strength.0).sum();
            sum / total as f32
        };
        let recently_accessed = 0; // Phase 3: no time-window tracking
        Self {
            total,
            by_kind,
            links,
            avg_strength,
            recently_accessed,
        }
    }
}
