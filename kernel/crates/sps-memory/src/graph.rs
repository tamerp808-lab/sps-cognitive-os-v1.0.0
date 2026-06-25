//! Memory graph — stores memories and typed links between them.

use std::collections::BTreeMap;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::memory::{Memory, MemoryId, MemoryKind, MemoryStrength};

/// Kind of link between two memories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLinkKind {
    /// Memory A caused memory B.
    Caused,
    /// Memory A is a generalization of memory B.
    Generalizes,
    /// Memory A is related to memory B (untyped).
    Related,
    /// Memory A is a part of memory B.
    PartOf,
    /// Memory A was promoted from memory B (episodic → semantic).
    PromotedFrom,
}

/// A typed link between two memories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryLink {
    /// Source memory id.
    pub from: MemoryId,
    /// Target memory id.
    pub to: MemoryId,
    /// Kind of link.
    pub kind: MemoryLinkKind,
    /// Optional weight (0.0–1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
}

/// Link id.
pub type LinkId = Uuid;

/// The memory graph — a projection of all memories and their links.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemoryGraph {
    /// All memories keyed by id.
    pub memories: BTreeMap<Uuid, Memory>,
    /// All links keyed by id.
    pub links: BTreeMap<LinkId, MemoryLink>,
    /// Index: from-memory-id → link ids. Not serialized (rebuilt on load).
    pub from_index: BTreeMap<Uuid, Vec<LinkId>>,
    /// Index: to-memory-id → link ids. Not serialized (rebuilt on load).
    pub to_index: BTreeMap<Uuid, Vec<LinkId>>,
}

impl MemoryGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a memory.
    pub fn add_memory(&mut self, memory: Memory) {
        self.memories.insert(memory.id.0, memory);
    }

    /// Get a memory by id.
    pub fn get(&self, id: &MemoryId) -> Option<&Memory> {
        self.memories.get(&id.0)
    }

    /// Get a mutable reference to a memory.
    pub fn get_mut(&mut self, id: &MemoryId) -> Option<&mut Memory> {
        self.memories.get_mut(&id.0)
    }

    /// Remove a memory (and any links pointing to it).
    pub fn remove_memory(&mut self, id: &MemoryId) -> Option<Memory> {
        let removed = self.memories.remove(&id.0);
        if removed.is_some() {
            self.links.retain(|_, l| l.from != *id && l.to != *id);
            self.rebuild_indexes();
        }
        removed
    }

    /// Add a link between two memories.
    pub fn add_link(&mut self, link: MemoryLink) -> LinkId {
        let id = Uuid::now_v7();
        self.links.insert(id, link.clone());
        self.from_index.entry(link.from.0).or_default().push(id);
        self.to_index.entry(link.to.0).or_default().push(id);
        id
    }

    /// Remove a link.
    pub fn remove_link(&mut self, id: LinkId) -> Option<MemoryLink> {
        let removed = self.links.remove(&id);
        if removed.is_some() {
            self.rebuild_indexes();
        }
        removed
    }

    /// Get all links from a memory.
    pub fn links_from(&self, id: &MemoryId) -> Vec<&MemoryLink> {
        self.from_index
            .get(&id.0)
            .into_iter()
            .flatten()
            .filter_map(|lid| self.links.get(lid))
            .collect()
    }

    /// Get all links to a memory.
    pub fn links_to(&self, id: &MemoryId) -> Vec<&MemoryLink> {
        self.to_index
            .get(&id.0)
            .into_iter()
            .flatten()
            .filter_map(|lid| self.links.get(lid))
            .collect()
    }

    /// Search memories by keyword (title or tags contains, case-insensitive).
    pub fn search(&self, query: &str, limit: usize) -> Vec<&Memory> {
        let q = query.to_lowercase();
        self.memories
            .values()
            .filter(|m| {
                let title_match = m.title.as_str().to_lowercase().contains(&q);
                let tag_match = m.tags.iter().any(|t| t.as_str().to_lowercase().contains(&q));
                let content_match = m
                    .content
                    .to_string()
                    .to_lowercase()
                    .contains(&q);
                title_match || tag_match || content_match
            })
            .take(limit)
            .collect()
    }

    /// Filter memories by kind.
    pub fn by_kind(&self, kind: MemoryKind) -> Vec<&Memory> {
        self.memories.values().filter(|m| m.kind == kind).collect()
    }

    /// Count of memories by kind.
    pub fn count_by_kind(&self, kind: MemoryKind) -> usize {
        self.by_kind(kind).len()
    }

    /// Total memory count.
    pub fn count(&self) -> usize {
        self.memories.len()
    }

    /// Rebuild the from/to indexes from the links map.
    fn rebuild_indexes(&mut self) {
        self.from_index.clear();
        self.to_index.clear();
        for (lid, link) in &self.links {
            self.from_index.entry(link.from.0).or_default().push(*lid);
            self.to_index.entry(link.to.0).or_default().push(*lid);
        }
    }

    /// Apply a strength-decay factor to all memories of the given kind
    /// (or all if `None`). Removes memories whose strength drops below
    /// the death threshold.
    pub fn apply_decay(&mut self, factor: f32, kind: Option<MemoryKind>) -> Vec<MemoryId> {
        let mut dead = Vec::new();
        let to_decay: Vec<MemoryId> = self
            .memories
            .values()
            .filter(|m| kind.map_or(true, |k| m.kind == k))
            .map(|m| m.id)
            .collect();
        for id in to_decay {
            if let Some(m) = self.memories.get_mut(&id.0) {
                m.strength = m.strength.decay(factor);
                if m.strength.is_dead() {
                    dead.push(id);
                }
            }
        }
        for id in &dead {
            self.remove_memory(id);
        }
        dead
    }

    /// Boost a memory's strength (e.g. on access).
    pub fn boost(&mut self, id: &MemoryId, delta: f32) {
        if let Some(m) = self.memories.get_mut(&id.0) {
            m.strength = m.strength.boost(delta);
        }
    }

    /// Promote an episodic memory to semantic (or another kind).
    pub fn promote(&mut self, id: &MemoryId, new_kind: MemoryKind) -> Option<MemoryLink> {
        let m = self.memories.get_mut(&id.0)?;
        let old_kind = m.kind;
        if old_kind == new_kind {
            return None;
        }
        m.kind = new_kind;
        m.strength = MemoryStrength::new(1.0); // reset strength on promotion
        // Create a "promoted_from" self-link from the new memory to itself
        // (representing the promotion event). Caller can record the
        // original memory separately if desired.
        let link = MemoryLink {
            from: *id,
            to: *id,
            kind: crate::graph::MemoryLinkKind::PromotedFrom,
            weight: Some(1.0),
        };
        Some(link)
    }
}

/// Convenience wrapper around `RwLock<MemoryGraph>` for thread-safe access.
pub struct SharedMemoryGraph {
    inner: RwLock<MemoryGraph>,
}

impl SharedMemoryGraph {
    /// Create a new shared graph.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(MemoryGraph::new()),
        }
    }

    /// Read lock.
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, MemoryGraph> {
        self.inner.read()
    }

    /// Write lock.
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, MemoryGraph> {
        self.inner.write()
    }
}

impl Default for SharedMemoryGraph {
    fn default() -> Self {
        Self::new()
    }
}

// Make MemoryGraph rebuild indexes after deserialization.
impl<'de> Deserialize<'de> for MemoryGraph {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        #[derive(Deserialize)]
        struct Raw {
            memories: BTreeMap<Uuid, Memory>,
            links: BTreeMap<LinkId, MemoryLink>,
        }
        let raw = Raw::deserialize(d)?;
        let mut graph = MemoryGraph {
            memories: raw.memories,
            links: raw.links,
            from_index: BTreeMap::new(),
            to_index: BTreeMap::new(),
        };
        graph.rebuild_indexes();
        Ok(graph)
    }
}

// Custom serialize to skip indexes.
impl Serialize for MemoryGraph {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("MemoryGraph", 2)?;
        st.serialize_field("memories", &self.memories)?;
        st.serialize_field("links", &self.links)?;
        st.end()
    }
}

// (MemoryStrength and SmolStr are re-exported via lib.rs.)
