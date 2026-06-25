//! Memory types and identifiers.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

/// Unique memory identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MemoryId(pub Uuid);

impl MemoryId {
    /// Generate a new random id.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for MemoryId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MemoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Kind of memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryKind {
    /// Time-bound record of a specific run.
    Episodic,
    /// Fact or concept.
    Semantic,
    /// How-to workflow.
    Procedural,
    /// Abstract pattern.
    Conceptual,
}

impl MemoryKind {
    /// String identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Episodic => "episodic",
            Self::Semantic => "semantic",
            Self::Procedural => "procedural",
            Self::Conceptual => "conceptual",
        }
    }

    /// Default decay TTL in days (0 = no decay).
    pub fn default_decay_days(&self) -> u64 {
        match self {
            Self::Episodic => 30,
            Self::Semantic => 0,      // never auto-decays
            Self::Procedural => 90,
            Self::Conceptual => 0,    // never auto-decays
        }
    }
}

/// Memory strength — higher is stronger. Used for decay and promotion.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MemoryStrength(pub f32);


impl Default for MemoryStrength {
    fn default() -> Self {
        Self(1.0)
    }
}

impl MemoryStrength {
    /// Create a new strength value (clamped to [0, 1]).
    pub fn new(v: f32) -> Self {
        Self(v.clamp(0.0, 1.0))
    }

    /// Boost strength by a delta (clamped).
    pub fn boost(self, delta: f32) -> Self {
        Self::new(self.0 + delta)
    }

    /// Decay strength by a factor.
    pub fn decay(self, factor: f32) -> Self {
        Self::new(self.0 * factor)
    }

    /// Is the strength below the deletion threshold?
    pub fn is_dead(&self) -> bool {
        self.0 < 0.01
    }
}

/// A single memory record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    /// Unique id.
    pub id: MemoryId,
    /// Kind of memory.
    pub kind: MemoryKind,
    /// Human-readable title.
    pub title: SmolStr,
    /// Content (free-form text or structured JSON).
    pub content: serde_json::Value,
    /// Strength (for decay and promotion).
    #[serde(default)]
    pub strength: MemoryStrength,
    /// Tags (for search).
    #[serde(default)]
    pub tags: Vec<SmolStr>,
    /// Wall time created (display only).
    pub created_at: u64,
    /// Wall time last accessed (display only).
    pub last_accessed_at: u64,
    /// Access count.
    #[serde(default)]
    pub access_count: u64,
    /// Originating tick (the event that created this memory).
    pub origin_tick: u64,
}

/// A memory creation record (used as event payload).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    /// Id of the memory.
    pub id: MemoryId,
    /// Kind.
    pub kind: MemoryKind,
    /// Title.
    pub title: SmolStr,
    /// Content.
    pub content: serde_json::Value,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<SmolStr>,
    /// Originating tick.
    pub origin_tick: u64,
    /// Wall time created.
    pub created_at: u64,
}

impl MemoryRecord {
    /// Build a Memory from this record.
    pub fn to_memory(&self) -> Memory {
        Memory {
            id: self.id,
            kind: self.kind,
            title: self.title.clone(),
            content: self.content.clone(),
            strength: MemoryStrength::default(),
            tags: self.tags.clone(),
            created_at: self.created_at,
            last_accessed_at: self.created_at,
            access_count: 0,
            origin_tick: self.origin_tick,
        }
    }
}
