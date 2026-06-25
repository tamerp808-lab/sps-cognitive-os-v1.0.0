//! World Model reducer + state slice.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;

use crate::entities::{
    AgentDescriptor, EntityId, ExternalSystem, FileNode, Project, ToolDescriptor,
};
use crate::graph::{WorldGraph, WorldRelationship};

/// Extension key under which the world model is stored.
pub const EXTENSION_KEY: &str = "world";

/// World Model state slice.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldState {
    /// The world graph.
    #[serde(flatten)]
    pub graph: WorldGraph,
}

impl WorldState {
    /// Read from canonical state. P3D: typed-first lookup with JSON
    /// fallback.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        if let Some(arc) = state.get_typed_extension::<WorldState>(EXTENSION_KEY) {
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

/// Reducer for world events.
#[derive(Debug, Default)]
pub struct WorldReducer;

impl WorldReducer {
    /// Register this reducer for all world event types.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            "world.project_added",
            "world.project_removed",
            "world.file_added",
            "world.file_removed",
            "world.file_updated",
            "world.agent_added",
            "world.tool_added",
            "world.external_system_added",
            "world.relationship_added",
            "world.entity_removed",
        ] {
            registry.register(*et, r.clone());
        }
    }

    /// P3D: Register typed-extension constructor for snapshot load.
    pub fn register_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
        reg.register::<WorldState>(EXTENSION_KEY);
    }
}

impl Reducer for WorldReducer {
    fn name(&self) -> &'static str {
        "world"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        let mut world = WorldState::from_state(state).unwrap_or_default();
        match event.event_type.as_str() {
            "world.project_added" => {
                let p: Project = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("project: {}", e)))?;
                world.graph.add_project(p);
            }
            "world.project_removed" | "world.entity_removed" => {
                if let Some(id_str) = event.payload.get("id").and_then(|v| v.as_str()) {
                    if let Ok(uuid) = uuid::Uuid::parse_str(id_str) {
                        world.graph.remove_entity(&EntityId(uuid));
                    }
                }
            }
            "world.file_added" => {
                let f: FileNode = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("file: {}", e)))?;
                world.graph.add_file(f);
            }
            "world.file_removed" => {
                if let Some(id_str) = event.payload.get("id").and_then(|v| v.as_str()) {
                    if let Ok(uuid) = uuid::Uuid::parse_str(id_str) {
                        world.graph.files.remove(&uuid);
                    }
                }
            }
            "world.file_updated" => {
                let f: FileNode = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("file update: {}", e)))?;
                world.graph.add_file(f); // add_file overwrites same id
            }
            "world.agent_added" => {
                let a: AgentDescriptor = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("agent: {}", e)))?;
                world.graph.add_agent(a);
            }
            "world.tool_added" => {
                let t: ToolDescriptor = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("tool: {}", e)))?;
                world.graph.add_tool(t);
            }
            "world.external_system_added" => {
                let s: ExternalSystem = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("external: {}", e)))?;
                world.graph.add_external_system(s);
            }
            "world.relationship_added" => {
                let r: WorldRelationship = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("rel: {}", e)))?;
                world.graph.add_relationship(r);
            }
            _ => {}
        }
        world.save_to(state)?;
        Ok(())
    }
}
