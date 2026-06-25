//! Execution reducer + state slice.
//!
//! Fix #5: deterministic_id_from_tick for execution record IDs.
//! Fix #7: ExecutionRecord has plan_id, agent_id, factory_run_id for
//! cross-system queries (for_plan, for_agent, for_factory_run).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;
use uuid::Uuid;

/// Extension key.
pub const EXTENSION_KEY: &str = "execution";

/// Execution outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOutcome {
    /// Success.
    Success,
    /// Failure.
    Failure,
    /// Cancelled.
    Cancelled,
}

/// A record of a single execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Unique id (Fix #5: deterministic from tick).
    pub id: Uuid,
    /// What was executed (e.g. "shell.exec", "project.generate").
    pub operation: SmolStr,
    /// Outcome.
    pub outcome: ExecutionOutcome,
    /// Tick of the originating event.
    pub origin_tick: u64,
    /// Duration in ms (display only).
    pub duration_ms: u64,
    /// Optional error message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Fix #7: associated plan id (if any).
    #[serde(default)]
    pub plan_id: Option<Uuid>,
    /// Fix #7: associated agent id (if any).
    #[serde(default)]
    pub agent_id: Option<Uuid>,
    /// Fix #7: associated factory run id (if any).
    #[serde(default)]
    pub factory_run_id: Option<Uuid>,
}

/// Execution state slice.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExecutionState {
    /// All execution records keyed by id.
    #[serde(default)]
    pub records: std::collections::BTreeMap<Uuid, ExecutionRecord>,
}

impl ExecutionState {
    /// Read from canonical state. P3D: typed-first lookup with JSON
    /// fallback.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        if let Some(arc) = state.get_typed_extension::<ExecutionState>(EXTENSION_KEY) {
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

    /// Fix #7: query all execution records associated with a plan.
    pub fn for_plan(&self, plan_id: Uuid) -> Vec<&ExecutionRecord> {
        self.records
            .values()
            .filter(|r| r.plan_id == Some(plan_id))
            .collect()
    }

    /// Fix #7: query all execution records associated with an agent.
    pub fn for_agent(&self, agent_id: Uuid) -> Vec<&ExecutionRecord> {
        self.records
            .values()
            .filter(|r| r.agent_id == Some(agent_id))
            .collect()
    }

    /// Fix #7: query all execution records associated with a factory run.
    pub fn for_factory_run(&self, run_id: Uuid) -> Vec<&ExecutionRecord> {
        self.records
            .values()
            .filter(|r| r.factory_run_id == Some(run_id))
            .collect()
    }
}

/// Reducer for execution events.
#[derive(Debug, Default)]
pub struct ExecutionReducer;

impl ExecutionReducer {
    /// Register this reducer.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            "execution.started",
            "execution.succeeded",
            "execution.failed",
            "execution.cancelled",
        ] {
            registry.register(*et, r.clone());
        }
    }

    /// P3D: Register typed-extension constructor for snapshot load.
    pub fn register_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
        reg.register::<ExecutionState>(EXTENSION_KEY);
    }
}

impl Reducer for ExecutionReducer {
    fn name(&self) -> &'static str {
        "execution"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        let mut es = ExecutionState::from_state(state).unwrap_or_default();
        let outcome = match event.event_type.as_str() {
            "execution.started" => None,
            "execution.succeeded" => Some(ExecutionOutcome::Success),
            "execution.failed" => Some(ExecutionOutcome::Failure),
            "execution.cancelled" => Some(ExecutionOutcome::Cancelled),
            _ => None,
        };
        if let Some(outcome) = outcome {
            // Fix #5: deterministic ID from tick.
            let id = deterministic_id_from_tick(event.tick);
            let plan_id = event
                .payload
                .get("plan_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let agent_id = event
                .payload
                .get("agent_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let factory_run_id = event
                .payload
                .get("factory_run_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let record = ExecutionRecord {
                id,
                operation: SmolStr::new(
                    event.payload.get("operation").and_then(|v| v.as_str()).unwrap_or("unknown"),
                ),
                outcome,
                origin_tick: event.tick,
                duration_ms: event.payload.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0),
                error: event
                    .payload
                    .get("error")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                plan_id,
                agent_id,
                factory_run_id,
            };
            es.records.insert(record.id, record);
        }
        es.save_to(state)?;
        Ok(())
    }
}

/// Fix #5: Derive a deterministic UUID from an event tick.
fn deterministic_id_from_tick(tick: u64) -> Uuid {
    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&tick.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0F) | 0x50;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    Uuid::from_bytes(bytes)
}
