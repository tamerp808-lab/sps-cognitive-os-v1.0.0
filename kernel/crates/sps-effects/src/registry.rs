//! Effect executor trait and registry.

use std::sync::Arc;

use crate::effect::{EffectError, EffectIntent, EffectResult};
use sps_core::CoreResult;

/// An executor for a specific effect type. Implementations live behind
/// the Effect Manager and are the ONLY place non-determinism is allowed.
///
/// # Determinism
///
/// Executors MUST NOT be invoked during replay. The Effect Manager
/// records the result on first execution and replays it verbatim.
pub trait EffectExecutor: Send + Sync + 'static {
    /// Human-readable name (for diagnostics).
    fn name(&self) -> &'static str;

    /// Execute the effect. May perform I/O, network, shell, etc.
    fn execute(&self, intent: &EffectIntent, intent_tick: u64) -> Result<EffectResult, EffectError>;
}

/// Registry of effect executors, keyed by effect type string.
#[derive(Default)]
pub struct EffectRegistry {
    executors: parking_lot::RwLock<std::collections::HashMap<String, Arc<dyn EffectExecutor>>>,
}

impl EffectRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an executor for an effect type.
    pub fn register(&self, effect_type: &str, executor: Arc<dyn EffectExecutor>) {
        self.executors.write().insert(effect_type.to_string(), executor);
    }

    /// Look up the executor for an effect type.
    pub fn get(&self, effect_type: &str) -> Option<Arc<dyn EffectExecutor>> {
        self.executors.read().get(effect_type).cloned()
    }

    /// Number of registered executors.
    pub fn len(&self) -> usize {
        self.executors.read().len()
    }

    /// Is the registry empty?
    pub fn is_empty(&self) -> bool {
        self.executors.read().is_empty()
    }
}

/// Helper: convert a CoreResult into an executor result.
pub fn map_core_err(e: sps_core::CoreError) -> EffectError {
    EffectError::ExecutorFailed {
        message: e.to_string(),
        details: None,
    }
}

/// Helper: convert a anyhow error into an executor result.
pub fn map_anyhow_err<E: std::fmt::Display>(e: E) -> EffectError {
    EffectError::ExecutorFailed {
        message: e.to_string(),
        details: None,
    }
}

/// Convenience: convert a CoreResult<T> into Result<T, EffectError>.
pub fn lift<T>(r: CoreResult<T>) -> Result<T, EffectError> {
    r.map_err(map_core_err)
}
