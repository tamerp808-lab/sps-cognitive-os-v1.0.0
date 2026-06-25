//! LLM request/response types and the provider port trait.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// Configuration for a single provider instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Unique provider id (e.g. "openrouter", "openai", "ollama").
    pub id: SmolStr,
    /// Display name.
    pub name: SmolStr,
    /// API URL (e.g. "https://api.openai.com/v1").
    pub api_url: String,
    /// API key (optional for local providers like Ollama).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Default model name to use.
    pub model_name: SmolStr,
    /// Arbitrary metadata (timeout, max_tokens, etc.).
    #[serde(default)]
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

/// A chat completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    /// Provider id to use.
    pub provider_id: SmolStr,
    /// Model override (if None, uses provider's default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<SmolStr>,
    /// System prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// User message.
    pub user: String,
    /// Max tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// A chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCompletion {
    /// Generated text.
    pub text: String,
    /// Model that produced the response.
    pub model: SmolStr,
    /// Token usage (best-effort).
    #[serde(default)]
    pub usage: TokenUsage,
    /// Wall time elapsed (display only).
    pub elapsed_ms: u64,
}

/// Token usage stats.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Prompt tokens.
    #[serde(default)]
    pub prompt_tokens: u64,
    /// Completion tokens.
    #[serde(default)]
    pub completion_tokens: u64,
    /// Total tokens.
    #[serde(default)]
    pub total_tokens: u64,
}

/// Provider health status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    /// Provider id.
    pub provider_id: SmolStr,
    /// True if the last healthcheck succeeded.
    pub healthy: bool,
    /// Latency in ms (display only).
    pub latency_ms: u64,
    /// Optional error message if unhealthy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The provider port — every LLM provider implements this.
pub trait LlmProvider: Send + Sync + 'static {
    /// Provider id.
    fn id(&self) -> &str;

    /// Configure the provider.
    fn configure(&self, config: ProviderConfig);

    /// Perform a chat completion. This is the only non-deterministic
    /// call — it goes through the Effect Manager, never through a reducer.
    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, anyhow::Error>;

    /// Healthcheck — pings the provider.
    fn healthcheck(&self) -> Result<ProviderHealth, anyhow::Error>;
}
