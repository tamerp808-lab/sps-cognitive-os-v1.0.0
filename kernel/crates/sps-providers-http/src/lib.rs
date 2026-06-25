//! Real HTTP-based LLM providers for SPS.
//!
//! Phase 12B: Multi-provider system with 12+ built-in templates + custom
//! provider support. Any OpenAI-compatible, Anthropic-compatible, or
//! Ollama-compatible API can be registered at runtime.
//!
//! Built-in providers:
//! - OpenAI, Anthropic, OpenRouter, Groq, DeepSeek
//! - Mistral, Cohere, Together AI, Fireworks AI
//! - Ollama, LM Studio, vLLM (local)
//! - Azure OpenAI (with {resource}/{deployment} URL templating)
//!
//! Custom providers: POST /api/providers with a custom template to
//! register any OpenAI-compatible API at runtime.

#![allow(clippy::module_name_repetitions)]

pub mod adapter;
pub mod retry;
pub mod openai;
pub mod anthropic;
pub mod ollama;
pub mod streaming;
pub mod templates;

pub use adapter::HttpProviderAdapter;
pub use retry::{RetryConfig, RetryPolicy};
pub use openai::{
    OpenAiAdapter, OpenRouterAdapter, GroqAdapter, DeepSeekAdapter, LmStudioAdapter,
};
pub use anthropic::AnthropicAdapter;
pub use ollama::OllamaAdapter;
pub use streaming::{StreamChunk, StreamHandler};
pub use templates::{
    AddCustomProviderRequest, ApiFormat, ProviderTemplate, builtin_templates,
    get_builtin_template, build_provider,
};
