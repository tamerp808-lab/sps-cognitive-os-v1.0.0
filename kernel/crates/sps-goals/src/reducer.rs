//! Goal reducer + state slice.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;

use crate::hierarchy::{Goal, GoalId, GoalStatus, GoalTree, Task, TaskStatus};

/// Extension key.
pub const EXTENSION_KEY: &str = "goals";

/// Goal state slice.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GoalState {
    /// The goal tree.
    #[serde(flatten)]
    pub tree: GoalTree,
}

impl GoalState {
    /// Read from canonical state. P3D: typed-first lookup with JSON
    /// fallback.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        if let Some(arc) = state.get_typed_extension::<GoalState>(EXTENSION_KEY) {
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

/// Reducer for goal events.
#[derive(Debug, Default)]
pub struct GoalReducer;

impl GoalReducer {
    /// Register this reducer.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            "goal.created",
            "goal.updated",
            "goal.status_changed",
            "goal.completed",
            "goal.blocked",
            "goal.abandoned",
            "goal.objective_added",
            "goal.milestone_added",
            "task.created",
            "task.status_changed",
            "task.assigned",
            "task.completed",
            "task.failed",
        ] {
            registry.register(*et, r.clone());
        }
    }

    /// P3D: Register typed-extension constructor for snapshot load.
    pub fn register_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
        reg.register::<GoalState>(EXTENSION_KEY);
    }
}

impl Reducer for GoalReducer {
    fn name(&self) -> &'static str {
        "goals"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        let mut gs = GoalState::from_state(state).unwrap_or_default();
        match event.event_type.as_str() {
            "goal.created" => {
                let g: Goal = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("goal.created: {}", e)))?;
                gs.tree.add_goal(g);
            }
            "goal.status_changed" => {
                let goal_id: GoalId = serde_json::from_value(event.payload["goal_id"].clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("goal.status id: {}", e)))?;
                let status: GoalStatus = serde_json::from_value(event.payload["status"].clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("goal.status: {}", e)))?;
                if let Some(g) = gs.tree.get_mut(&goal_id) {
                    g.status = status;
                }
            }
            "task.created" => {
                let task: Task = serde_json::from_value(event.payload["task"].clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("task.created: {}", e)))?;
                let goal_id: GoalId = serde_json::from_value(event.payload["goal_id"].clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("task.created goal_id: {}", e)))?;
                let objective_idx = event.payload["objective_idx"].as_u64().unwrap_or(0) as usize;
                let milestone_idx = event.payload["milestone_idx"].as_u64().unwrap_or(0) as usize;
                if let Some(g) = gs.tree.get_mut(&goal_id) {
                    if let Some(obj) = g.objectives.get_mut(objective_idx) {
                        if let Some(mil) = obj.milestones.get_mut(milestone_idx) {
                            mil.tasks.push(task);
                        }
                    }
                }
            }
            "task.status_changed" => {
                // Caller must identify the task by id; we search for it.
                let task_id_str = event.payload["task_id"].as_str().unwrap_or("");
                let task_id = match uuid::Uuid::parse_str(task_id_str) {
                    Ok(u) => u,
                    Err(_) => {
                        gs.save_to(state)?;
                        return Ok(());
                    }
                };
                let status: TaskStatus = serde_json::from_value(event.payload["status"].clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("task.status: {}", e)))?;
                for g in gs.tree.goals.values_mut() {
                    for o in &mut g.objectives {
                        for m in &mut o.milestones {
                            for t in &mut m.tasks {
                                if t.id == task_id {
                                    t.status = status;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        gs.save_to(state)?;
        Ok(())
    }
}
