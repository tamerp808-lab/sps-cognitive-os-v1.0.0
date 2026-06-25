//! Effect Manager — the central coordinator for effect execution.
//!
//! # Lifecycle
//!
//! 1. A reducer (or agent, or planner) emits an `effect.intent` event
//!    containing an [`crate::EffectIntent`] payload.
//! 2. The Effect Manager subscribes to `effect.intent` events (via the
//!    Event Bus in Phase 2; for Phase 1, callers invoke
//!    [`EffectManager::execute`] directly).
//! 3. The Effect Manager looks up the executor for the effect type in
//!    the [`crate::EffectRegistry`].
//! 4. The executor runs (this is the ONLY non-deterministic step).
//! 5. The Effect Manager records the result as an `effect.executed`
//!    event (success) or `effect.failed` event (error) in the Event Store.
//! 6. On replay, the executor is bypassed — the recorded result is
//!    replayed verbatim.

use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::json;
use smol_str::SmolStr;

use sps_core::actor::Actor;
use sps_core::event::{Event, RawEvent};
use sps_core::event_store::EventStore;
use sps_core::{CoreError, CoreResult};

use crate::effect::{EffectError, EffectIntent, EffectResult, EffectType};
use crate::providers::registry::ProviderRegistry;
use crate::registry::EffectRegistry;

/// The Effect Manager. Coordinates executor lookup, execution, and
/// result recording.
pub struct EffectManager {
    /// Effect executor registry.
    executors: Arc<EffectRegistry>,
    /// LLM provider registry (for `LlmComplete` effects).
    providers: Arc<ProviderRegistry>,
    /// The event store (for recording executed/failed events).
    store: Arc<EventStore>,
    /// Default provider id to use when an LLM intent doesn't specify one.
    default_provider: RwLock<Option<SmolStr>>,
}

impl EffectManager {
    /// Create a new Effect Manager.
    pub fn new(
        executors: Arc<EffectRegistry>,
        providers: Arc<ProviderRegistry>,
        store: Arc<EventStore>,
    ) -> Self {
        Self {
            executors,
            providers,
            store,
            default_provider: RwLock::new(None),
        }
    }

    /// Set the default LLM provider id.
    pub fn set_default_provider(&self, id: impl Into<SmolStr>) {
        *self.default_provider.write() = Some(id.into());
    }

    /// Execute an effect intent that was already recorded as an
    /// `effect.intent` event at the given tick. Records the result
    /// (success or failure) as a new event.
    ///
    /// This is the ONLY non-deterministic entry point in the kernel.
    pub fn execute_recorded(
        &self,
        intent: &EffectIntent,
        intent_tick: u64,
        actor: &Actor,
        wall_time: u64,
    ) -> CoreResult<Event> {
        let result = self.run_executor(intent, intent_tick);

        let (event_type, payload) = match result {
            Ok(r) => (
                "effect.executed",
                json!({
                    "intent_tick": r.intent_tick,
                    "effect_type": intent.effect_type.as_str(),
                    "output": r.output,
                    "elapsed_ms": r.elapsed_ms,
                }),
            ),
            Err(e) => (
                "effect.failed",
                json!({
                    "intent_tick": intent_tick,
                    "effect_type": intent.effect_type.as_str(),
                    "error": e.to_value(),
                }),
            ),
        };

        let raw = RawEvent::new(event_type, payload, actor.clone(), wall_time)
            .with_causation(intent_tick);
        self.store.append(raw)
    }

    /// Run the executor for the intent. Internal — does NOT record
    /// events. Used by [`Self::execute_recorded`] and by tests.
    pub fn run_executor(
        &self,
        intent: &EffectIntent,
        intent_tick: u64,
    ) -> Result<EffectResult, EffectError> {
        // Special-case LLM: route through the provider registry.
        if intent.effect_type == EffectType::LlmComplete {
            return self.run_llm(intent, intent_tick);
        }
        let executor = self
            .executors
            .get(intent.effect_type.as_str())
            .ok_or_else(|| EffectError::NoExecutor(intent.effect_type.as_str().to_string()))?;
        executor.execute(intent, intent_tick)
    }

    fn run_llm(&self, intent: &EffectIntent, intent_tick: u64) -> Result<EffectResult, EffectError> {
        let start = std::time::Instant::now();
        // The intent input is an LlmRequest serialized as JSON.
        let request: crate::providers::llm::LlmRequest =
            serde_json::from_value(intent.input.clone()).map_err(|e| EffectError::ExecutorFailed {
                message: format!("invalid llm.complete input: {}", e),
                details: None,
            })?;

        let provider_id = request.provider_id.clone();
        let provider = self
            .providers
            .get(&provider_id)
            .ok_or(EffectError::NoProvider)?;

        let completion = provider.complete(&request).map_err(|e| EffectError::ExecutorFailed {
            message: format!("llm.complete via {}: {}", provider_id, e),
            details: None,
        })?;

        let output = serde_json::to_value(&completion).map_err(|e| EffectError::ExecutorFailed {
            message: format!("llm.complete serialization: {}", e),
            details: None,
        })?;

        Ok(EffectResult {
            intent_tick,
            output,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Convenience: build and execute a single-shot intent. Records both
    /// the intent and the result events.
    pub fn dispatch(
        &self,
        effect_type: EffectType,
        input: serde_json::Value,
        actor: &Actor,
        wall_time: u64,
    ) -> CoreResult<(Event, Event)> {
        let intent = EffectIntent::new(effect_type, input);
        let intent_payload = json!({
            "effect_type": intent.effect_type.as_str(),
            "input": intent.input,
        });
        let intent_event = self.store.append(RawEvent::new(
            "effect.intent",
            intent_payload,
            actor.clone(),
            wall_time,
        ))?;
        let result_event =
            self.execute_recorded(&intent, intent_event.tick, actor, wall_time)?;
        Ok((intent_event, result_event))
    }

    /// Reference to the executor registry (for registration by future phases).
    pub fn executors(&self) -> &Arc<EffectRegistry> {
        &self.executors
    }

    /// Reference to the provider registry.
    pub fn providers(&self) -> &Arc<ProviderRegistry> {
        &self.providers
    }
}

/// Helper: convert an EffectError to a CoreError (for storage).
pub fn effect_err_to_core(e: EffectError) -> CoreError {
    CoreError::Internal(anyhow::anyhow!("effect error: {}", e))
}
