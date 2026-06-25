//! Self-improvement reducer + state slice.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;
use uuid::Uuid;

use crate::analyzers::{OptimizationKind, PromptProposal, WorkflowProposal};

/// Extension key.
pub const EXTENSION_KEY: &str = "improvement";

/// Status of an improvement proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImprovementStatus {
    /// Proposed, awaiting approval.
    Proposed,
    /// Approved by owner.
    Approved,
    /// Applied.
    Applied,
    /// Rejected.
    Rejected,
    /// Reverted (was applied, then rolled back).
    Reverted,
}

/// An improvement proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImprovementProposal {
    /// Unique id.
    pub id: Uuid,
    /// Kind of optimization.
    pub kind: OptimizationKind,
    /// Human-readable description.
    pub description: String,
    /// Status.
    pub status: ImprovementStatus,
    /// Originating tick.
    pub origin_tick: u64,
    /// Optional workflow proposal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<WorkflowProposal>,
    /// Optional prompt proposal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<PromptProposal>,
    /// Subsystem affected.
    pub subsystem: SmolStr,
}

/// Improvement state slice.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImprovementState {
    /// All proposals keyed by id.
    #[serde(default)]
    pub proposals: std::collections::BTreeMap<Uuid, ImprovementProposal>,
}

impl ImprovementState {
    /// Read from canonical state. P3D: typed first, JSON fallback.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        if let Some(arc) = state.get_typed_extension::<ImprovementState>(EXTENSION_KEY) {
            return Some((*arc).clone());
        }
        state.get_extension(EXTENSION_KEY)
    }

    /// P3D: Read typed extension directly.
    pub fn from_typed_state(state: &CanonicalState) -> Option<std::sync::Arc<Self>> {
        state.get_typed_extension::<Self>(EXTENSION_KEY)
    }

    /// Save to canonical state.
    pub fn save_to(&self, state: &mut CanonicalState) -> serde_json::Result<()> {
        state.set_extension(EXTENSION_KEY, self)
    }
}

/// Reducer for improvement events.
#[derive(Debug, Default)]
pub struct ImprovementReducer;

impl ImprovementReducer {
    /// Register this reducer.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            "improvement.proposed",
            "improvement.approved",
            "improvement.applied",
            "improvement.rejected",
            "improvement.reverted",
        ] {
            registry.register(*et, r.clone());
        }
    }

    /// P3D: Register typed-extension constructor for snapshot load.
    pub fn register_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
        reg.register::<ImprovementState>(EXTENSION_KEY);
    }
}

impl Reducer for ImprovementReducer {
    fn name(&self) -> &'static str {
        "improvement"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        // P3D: Use typed extension as source of truth.
        state.with_typed_extension(EXTENSION_KEY, |is: &mut ImprovementState| {
            match event.event_type.as_str() {
                "improvement.proposed" => {
                    if let Ok(p) = serde_json::from_value::<ImprovementProposal>(event.payload.clone()) {
                        is.proposals.insert(p.id, p);
                    }
                }
                "improvement.approved"
                | "improvement.applied"
                | "improvement.rejected"
                | "improvement.reverted" => {
                    let id: Uuid = serde_json::from_value(event.payload["id"].clone())
                        .unwrap_or_default();
                    let new_status = match event.event_type.as_str() {
                        "improvement.approved" => ImprovementStatus::Approved,
                        "improvement.applied" => ImprovementStatus::Applied,
                        "improvement.rejected" => ImprovementStatus::Rejected,
                        "improvement.reverted" => ImprovementStatus::Reverted,
                        _ => ImprovementStatus::Proposed,
                    };
                    if let Some(p) = is.proposals.get_mut(&id) {
                        p.status = new_status;
                    }
                }
                _ => {}
            }
        });
        Ok(())
    }
}
