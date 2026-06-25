//! Provider abstraction (LLM providers).
//!
//! The kernel is provider-independent. Providers are plugins behind the
//! `ProviderPort` trait. The system boots with zero providers configured
//! — LLM effects simply fail with `EffectError::NoProvider` until one
//! is registered.

pub mod registry;
pub mod llm;
pub mod adapters;

pub use registry::{ProviderRegistry, ProviderEntry};
pub use llm::{LlmRequest, LlmCompletion, LlmProvider, ProviderConfig, ProviderHealth};
pub use adapters::{OpenAiCompatibleAdapter, StaticAdapter};
