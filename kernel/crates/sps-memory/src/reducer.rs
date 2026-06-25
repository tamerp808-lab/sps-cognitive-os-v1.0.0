//! Memory reducer + state slice.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;

use crate::graph::{MemoryGraph, MemoryLink};
use crate::memory::{MemoryId, MemoryKind, MemoryRecord};

/// Extension key under which the memory graph is stored.
pub const EXTENSION_KEY: &str = "memory";

/// Memory subsystem state slice.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MemoryState {
    /// The memory graph.
    #[serde(flatten)]
    pub graph: MemoryGraph,
}

impl MemoryState {
    /// Read from canonical state. P3D: typed-first lookup with JSON
    /// fallback.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        if let Some(arc) = state.get_typed_extension::<MemoryState>(EXTENSION_KEY) {
            return Some((*arc).clone());
        }
        state.get_extension(EXTENSION_KEY)
    }

    /// Read the typed extension directly. P3D.
    pub fn from_typed_state(state: &CanonicalState) -> Option<std::sync::Arc<Self>> {
        state.get_typed_extension::<Self>(EXTENSION_KEY)
    }

    /// Save to canonical state.
    pub fn save_to(&self, state: &mut CanonicalState) -> serde_json::Result<()> {
        state.set_extension(EXTENSION_KEY, self)
    }
}

/// Reducer for memory events.
#[derive(Debug, Default)]
pub struct MemoryReducer;

impl MemoryReducer {
    /// Register this reducer for all memory event types.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            "memory.created",
            "memory.accessed",
            "memory.promoted",
            "memory.linked",
            "memory.unlinked",
            "memory.consolidated",
            "memory.decayed",
            "memory.removed",
        ] {
            registry.register(*et, r.clone());
        }
    }

    /// P3D: Register typed-extension constructor for snapshot load.
    pub fn register_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
        reg.register::<MemoryState>(EXTENSION_KEY);
    }
}

impl Reducer for MemoryReducer {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        let mut mem_state = MemoryState::from_state(state).unwrap_or_default();
        match event.event_type.as_str() {
            "memory.created" => {
                let record: MemoryRecord = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("memory.created payload: {}", e)))?;
                let memory = record.to_memory();
                mem_state.graph.add_memory(memory);
            }
            "memory.accessed" => {
                let id: MemoryId = serde_json::from_value(event.payload["id"].clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("memory.accessed id: {}", e)))?;
                if let Some(m) = mem_state.graph.get_mut(&id) {
                    m.access_count += 1;
                    m.last_accessed_at = event.payload["at"].as_u64().unwrap_or(m.last_accessed_at);
                    m.strength = m.strength.boost(0.05);
                }
            }
            "memory.promoted" => {
                let id: MemoryId = serde_json::from_value(event.payload["id"].clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("memory.promoted id: {}", e)))?;
                let new_kind: MemoryKind = serde_json::from_value(event.payload["new_kind"].clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("memory.promoted kind: {}", e)))?;
                if let Some(link) = mem_state.graph.promote(&id, new_kind) {
                    mem_state.graph.add_link(link);
                }
            }
            "memory.linked" => {
                let link: MemoryLink = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("memory.linked: {}", e)))?;
                mem_state.graph.add_link(link);
            }
            "memory.unlinked" => {
                if let Some(link_id) = event.payload.get("link_id").and_then(|v| v.as_str()) {
                    if let Ok(uuid) = uuid::Uuid::parse_str(link_id) {
                        mem_state.graph.remove_link(uuid);
                    }
                }
            }
            "memory.consolidated" => {
                // Consolidation merges two memories into one — caller
                // specifies which survives. For Phase 3 we just remove
                // the loser.
                if let Some(loser) = event.payload.get("loser_id").and_then(|v| v.as_str()) {
                    if let Ok(uuid) = uuid::Uuid::parse_str(loser) {
                        mem_state.graph.remove_memory(&MemoryId(uuid));
                    }
                }
            }
            "memory.decayed" => {
                let factor = event.payload["factor"].as_f64().unwrap_or(0.9) as f32;
                let kind_filter: Option<MemoryKind> = event
                    .payload
                    .get("kind")
                    .and_then(|v| serde_json::from_value(v.clone()).ok());
                mem_state.graph.apply_decay(factor, kind_filter);
            }
            "memory.removed" => {
                let id: MemoryId = serde_json::from_value(event.payload["id"].clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("memory.removed id: {}", e)))?;
                mem_state.graph.remove_memory(&id);
            }
            _ => {}
        }
        mem_state.save_to(state)?;
        Ok(())
    }
}

// (No re-exports needed — types are public via crate::memory and crate::graph.)
