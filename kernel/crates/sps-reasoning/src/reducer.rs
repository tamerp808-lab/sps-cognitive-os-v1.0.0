//! Reasoning reducer + state slice.
//!
//! Fix #11: ReasoningStep has goal_id field, used as trace key (not step.id).
//! Fix #12: 4 reasoning event types have handlers:
//!   - reasoning.alternative_generated → Alternative
//!   - reasoning.conflict_detected → Conflict
//!   - reasoning.risk_assessed → Risk
//!   - reasoning.degraded_mode → Degradation

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;
use uuid::Uuid;

/// Extension key.
pub const EXTENSION_KEY: &str = "reasoning";

/// A single reasoning step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningStep {
    /// Step id.
    pub id: Uuid,
    /// Fix #11: Goal this step belongs to (used as trace key).
    pub goal_id: Uuid,
    /// Analyzer that produced this step.
    pub analyzer: SmolStr,
    /// Input description.
    pub input: String,
    /// Output (JSON).
    pub output: serde_json::Value,
    /// Tick when the step was recorded.
    pub tick: u64,
}

/// A trace of reasoning steps for a goal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningTrace {
    /// Goal this trace is for.
    pub goal_id: Uuid,
    /// Steps in the trace.
    pub steps: Vec<ReasoningStep>,
}

/// Fix #12: An alternative approach generated during reasoning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alternative {
    pub goal_id: Uuid,
    pub description: String,
    pub confidence: f64,
    pub origin_tick: u64,
}

/// Fix #12: A conflict detected during reasoning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conflict {
    pub entities: Vec<Uuid>,
    pub description: String,
    pub severity: f64,
    pub origin_tick: u64,
}

/// Fix #12: A risk assessed during reasoning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Risk {
    pub target_id: Uuid,
    pub risk_score: f64,
    pub factors: Vec<String>,
    pub origin_tick: u64,
}

/// Fix #12: A degraded mode entered during reasoning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Degradation {
    pub goal_id: Uuid,
    pub reason: String,
    pub fallback: String,
    pub origin_tick: u64,
}

/// Reasoning state slice.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReasoningState {
    /// All reasoning steps, keyed by id.
    #[serde(default)]
    pub steps: std::collections::BTreeMap<Uuid, ReasoningStep>,
    /// All traces, keyed by goal id (Fix #11: goal_id is the key, not step.id).
    #[serde(default)]
    pub traces: std::collections::BTreeMap<Uuid, ReasoningTrace>,
    /// Fix #12: alternatives generated.
    #[serde(default)]
    pub alternatives: Vec<Alternative>,
    /// Fix #12: conflicts detected.
    #[serde(default)]
    pub conflicts: Vec<Conflict>,
    /// Fix #12: risks assessed.
    #[serde(default)]
    pub risks: Vec<Risk>,
    /// Fix #12: degraded modes entered.
    #[serde(default)]
    pub degradations: Vec<Degradation>,
}

impl ReasoningState {
    /// Read from canonical state. P3D: typed-first lookup.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        if let Some(arc) = state.get_typed_extension::<ReasoningState>(EXTENSION_KEY) {
            return Some((*arc).clone());
        }
        state.get_extension(EXTENSION_KEY)
    }

    /// P3D: Read typed extension directly.
    pub fn from_typed_state(state: &CanonicalState) -> Option<Arc<Self>> {
        state.get_typed_extension::<Self>(EXTENSION_KEY)
    }

    /// Save to canonical state.
    pub fn save_to(&self, state: &mut CanonicalState) -> serde_json::Result<()> {
        state.set_extension(EXTENSION_KEY, self)
    }
}

/// Reducer for reasoning events.
#[derive(Debug, Default)]
pub struct ReasoningReducer;

impl ReasoningReducer {
    /// Register this reducer.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            "reasoning.step",
            "reasoning.alternative_generated",
            "reasoning.conflict_detected",
            "reasoning.risk_assessed",
            "reasoning.degraded_mode",
        ] {
            registry.register(*et, r.clone());
        }
    }

    /// P3D: Register typed-extension constructor for snapshot load.
    pub fn register_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
        reg.register::<ReasoningState>(EXTENSION_KEY);
    }
}

impl Reducer for ReasoningReducer {
    fn name(&self) -> &'static str {
        "reasoning"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        // P3D: Use typed extension as source of truth.
        state.with_typed_extension(EXTENSION_KEY, |rs: &mut ReasoningState| {
            match event.event_type.as_str() {
                "reasoning.step" => {
                    if let Ok(step) = serde_json::from_value::<ReasoningStep>(event.payload.clone()) {
                        rs.steps.insert(step.id, step.clone());
                        // Fix #11: use goal_id as trace key (not step.id).
                        let goal_id = step.goal_id;
                        let trace = rs.traces.entry(goal_id).or_insert(ReasoningTrace {
                            goal_id,
                            steps: Vec::new(),
                        });
                        trace.steps.push(step);
                    }
                }
                // Fix #12: alternative_generated
                "reasoning.alternative_generated" => {
                    if let Ok(alt) = serde_json::from_value::<Alternative>(event.payload.clone()) {
                        rs.alternatives.push(alt);
                    }
                }
                // Fix #12: conflict_detected
                "reasoning.conflict_detected" => {
                    if let Ok(conflict) = serde_json::from_value::<Conflict>(event.payload.clone()) {
                        rs.conflicts.push(conflict);
                    }
                }
                // Fix #12: risk_assessed
                "reasoning.risk_assessed" => {
                    if let Ok(risk) = serde_json::from_value::<Risk>(event.payload.clone()) {
                        rs.risks.push(risk);
                    }
                }
                // Fix #12: degraded_mode
                "reasoning.degraded_mode" => {
                    if let Ok(deg) = serde_json::from_value::<Degradation>(event.payload.clone()) {
                        rs.degradations.push(deg);
                    }
                }
                _ => {}
            }
        });
        // P3D: No per-dispatch JSON sync.
        Ok(())
    }
}
