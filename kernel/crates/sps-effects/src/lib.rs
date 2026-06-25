//! SPS Effect System (Phase 1)
//!
//! Quarantines all non-determinism behind the Effect Manager. The kernel's
//! pure core never performs I/O — instead, reducers emit `effect.intent`
//! events describing what they want done. The Effect Manager picks those
//! events up, executes them via the appropriate executor, and records an
//! `effect.executed` (or `effect.failed`) event with the result. Reducers
//! consume the recorded result event; on replay, the executor is bypassed
//! entirely.
//!
//! # Determinism contract
//!
//! - `effect.intent` events describe what is wanted (deterministic).
//! - `effect.executed` events record the actual result (deterministic on
//!   replay — the recorded result is replayed verbatim).
//! - The executor itself is the ONLY non-deterministic step, and it runs
//!   exactly once per intent.

#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

pub mod effect;
pub mod manager;
pub mod registry;
pub mod executors;
pub mod providers;

pub use effect::{EffectIntent, EffectResult, EffectType, EffectError};
pub use manager::EffectManager;
pub use registry::{EffectExecutor, EffectRegistry};
pub use executors::{FsExecutor, ShellExecutor, GitExecutor, SearchExecutor, FactoryExecutor, FactoryExecutorConfig};
pub use providers::{LlmProvider, ProviderConfig, ProviderRegistry, LlmCompletion, LlmRequest};
