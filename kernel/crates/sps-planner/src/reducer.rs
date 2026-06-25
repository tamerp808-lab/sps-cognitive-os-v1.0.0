//! Planner reducer + state slice.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;

use crate::plan::{Plan, PlanId, PlanStatus};

/// Extension key.
pub const EXTENSION_KEY: &str = "plans";

/// Planner state slice.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlannerState {
    /// All plans keyed by id.
    #[serde(default)]
    pub plans: std::collections::BTreeMap<uuid::Uuid, Plan>,
}

impl PlannerState {
    /// Read from canonical state. P3D: typed-first lookup with JSON
    /// fallback.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        if let Some(arc) = state.get_typed_extension::<PlannerState>(EXTENSION_KEY) {
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

/// Reducer for plan events.
#[derive(Debug, Default)]
pub struct PlannerReducer;

impl PlannerReducer {
    /// Register this reducer.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            "plan.created",
            "plan.optimized",
            "plan.approved",
            "plan.executing",
            "plan.completed",
            "plan.abandoned",
        ] {
            registry.register(*et, r.clone());
        }
    }

    /// P3D: Register typed-extension constructor for snapshot load.
    pub fn register_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
        reg.register::<PlannerState>(EXTENSION_KEY);
    }
}

impl Reducer for PlannerReducer {
    fn name(&self) -> &'static str {
        "planner"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        let mut ps = PlannerState::from_state(state).unwrap_or_default();
        match event.event_type.as_str() {
            "plan.created" => {
                let p: Plan = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("plan.created: {}", e)))?;
                ps.plans.insert(p.id.0, p);
            }
            "plan.optimized" | "plan.approved" | "plan.executing" | "plan.completed"
            | "plan.abandoned" => {
                let plan_id: PlanId = serde_json::from_value(event.payload["plan_id"].clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("plan id: {}", e)))?;
                let new_status: PlanStatus = serde_json::from_value(event.payload["status"].clone())
                    .unwrap_or(match event.event_type.as_str() {
                        "plan.optimized" => PlanStatus::Optimized,
                        "plan.approved" => PlanStatus::Approved,
                        "plan.executing" => PlanStatus::Executing,
                        "plan.completed" => PlanStatus::Completed,
                        "plan.abandoned" => PlanStatus::Abandoned,
                        _ => PlanStatus::Draft,
                    });
                if let Some(p) = ps.plans.get_mut(&plan_id.0) {
                    p.status = new_status;
                }
            }
            _ => {}
        }
        ps.save_to(state)?;
        Ok(())
    }
}
