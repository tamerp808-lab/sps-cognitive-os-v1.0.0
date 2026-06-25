//! Vector reducer + state slice.
//!
//! Stores vector entries in the canonical state. The actual index is
//! rebuilt on load (the entries are the source of truth, not the index).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;
use uuid::Uuid;

use crate::index::VectorEntry;

/// Extension key.
pub const EXTENSION_KEY: &str = "vectors";

/// Vector state slice.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VectorState {
    /// All vector entries keyed by id.
    #[serde(default)]
    pub entries: std::collections::BTreeMap<Uuid, VectorEntry>,
}

impl VectorState {
    /// Read from canonical state.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        state.get_extension(EXTENSION_KEY)
    }

    /// Save to canonical state.
    pub fn save_to(&self, state: &mut CanonicalState) -> serde_json::Result<()> {
        state.set_extension(EXTENSION_KEY, self)
    }

    /// Rebuild a vector index from the entries.
    pub fn to_index(&self) -> crate::index::VectorIndex {
        let index = crate::index::VectorIndex::new();
        for entry in self.entries.values() {
            let _ = index.add(entry.clone());
        }
        index
    }
}

/// Reducer for vector events.
#[derive(Debug, Default)]
pub struct VectorReducer;

impl VectorReducer {
    /// Register this reducer.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            "vector.added",
            "vector.removed",
            "vector.cleared",
        ] {
            registry.register(*et, r.clone());
        }
    }
}

impl Reducer for VectorReducer {
    fn name(&self) -> &'static str {
        "vectors"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        let mut vs = VectorState::from_state(state).unwrap_or_default();
        match event.event_type.as_str() {
            "vector.added" => {
                let entry: VectorEntry = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("vector.added: {}", e)))?;
                vs.entries.insert(entry.id, entry);
            }
            "vector.removed" => {
                if let Some(id_str) = event.payload.get("id").and_then(|v| v.as_str()) {
                    if let Ok(uuid) = Uuid::parse_str(id_str) {
                        vs.entries.remove(&uuid);
                    }
                }
            }
            "vector.cleared" => {
                vs.entries.clear();
            }
            _ => {}
        }
        vs.save_to(state)?;
        Ok(())
    }
}
