//! Reflection reducer + state slice.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;
use uuid::Uuid;

use crate::analyzers::{FailureAnalysis, Pattern, SuccessAnalysis};

/// Extension key.
pub const EXTENSION_KEY: &str = "reflection";

/// A reflection record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Reflection {
    /// Success reflection.
    Success(SuccessAnalysis),
    /// Failure reflection.
    Failure(FailureAnalysis),
    /// Pattern reflection.
    Pattern(Pattern),
}

/// Reflection state slice.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReflectionState {
    /// All reflections keyed by id.
    #[serde(default)]
    pub reflections: std::collections::BTreeMap<Uuid, Reflection>,
}

impl ReflectionState {
    /// Read from canonical state. P3D: typed-first lookup with JSON
    /// fallback.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        if let Some(arc) = state.get_typed_extension::<ReflectionState>(EXTENSION_KEY) {
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

/// Reducer for reflection events.
#[derive(Debug, Default)]
pub struct ReflectionReducer;

impl ReflectionReducer {
    /// Register this reducer.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            "reflection.created",
            "reflection.pattern_extracted",
            "reflection.success_analyzed",
            "reflection.failure_analyzed",
        ] {
            registry.register(*et, r.clone());
        }
    }

    /// P3D: Register typed-extension constructor for snapshot load.
    pub fn register_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
        reg.register::<ReflectionState>(EXTENSION_KEY);
    }
}

impl Reducer for ReflectionReducer {
    fn name(&self) -> &'static str {
        "reflection"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        let mut rs = ReflectionState::from_state(state).unwrap_or_default();
        let id = Uuid::now_v7();
        match event.event_type.as_str() {
            "reflection.success_analyzed" => {
                let a: SuccessAnalysis = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("success: {}", e)))?;
                rs.reflections.insert(id, Reflection::Success(a));
            }
            "reflection.failure_analyzed" => {
                let a: FailureAnalysis = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("failure: {}", e)))?;
                rs.reflections.insert(id, Reflection::Failure(a));
            }
            "reflection.pattern_extracted" => {
                let p: Pattern = serde_json::from_value(event.payload.clone())
                    .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("pattern: {}", e)))?;
                rs.reflections.insert(id, Reflection::Pattern(p));
            }
            _ => {}
        }
        rs.save_to(state)?;
        Ok(())
    }
}
