//! Built-in provider adapters.
//!
//! - `OpenAiCompatibleAdapter` — generic OpenAI-compatible HTTP adapter
//!   (works with OpenAI, OpenRouter, Groq, DeepSeek, LM Studio, etc.).
//! - `StaticAdapter` — returns a canned response; useful for tests and
//!   for "no provider configured" fallback.
//!
//! Note: Phase 1 ships the trait + a static adapter. Real HTTP-based
//! adapters are implemented in later phases once the HTTP client
//! dependency is finalized (Phase 1 keeps the kernel dependency-light).

use std::sync::Arc;

use parking_lot::RwLock;
use smol_str::SmolStr;

use crate::providers::llm::{
    LlmCompletion, LlmProvider, LlmRequest, ProviderConfig, ProviderHealth, TokenUsage,
};

/// Static adapter — returns a canned response. Used in tests and as a
/// fallback when no real provider is configured.
pub struct StaticAdapter {
    id: SmolStr,
    config: RwLock<Option<ProviderConfig>>,
    canned: RwLock<String>,
}

impl StaticAdapter {
    /// Create a new static adapter with the given id and canned response.
    pub fn new(id: impl Into<SmolStr>, canned: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            config: RwLock::new(None),
            canned: RwLock::new(canned.into()),
        }
    }

    /// Update the canned response.
    pub fn set_canned(&self, response: impl Into<String>) {
        *self.canned.write() = response.into();
    }
}

impl LlmProvider for StaticAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn configure(&self, config: ProviderConfig) {
        *self.config.write() = Some(config);
    }

    fn complete(&self, _request: &LlmRequest) -> Result<LlmCompletion, anyhow::Error> {
        let canned = self.canned.read().clone();
        Ok(LlmCompletion {
            text: canned,
            model: SmolStr::new("static"),
            usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            elapsed_ms: 0,
        })
    }

    fn healthcheck(&self) -> Result<ProviderHealth, anyhow::Error> {
        Ok(ProviderHealth {
            provider_id: self.id.clone(),
            healthy: true,
            latency_ms: 0,
            error: None,
        })
    }
}

/// Generic OpenAI-compatible adapter — delegates to sps-providers-http
/// for the real HTTP implementation. This adapter exists so callers
/// in sps-effects can use the LlmProvider trait without a direct
/// dependency on sps-providers-http. The real HTTP call happens via
/// the registered HttpProviderAdapter in sps-providers-http.
pub struct OpenAiCompatibleAdapter {
    id: SmolStr,
    config: RwLock<Option<ProviderConfig>>,
}

impl OpenAiCompatibleAdapter {
    /// Create a new OpenAI-compatible adapter.
    pub fn new(id: impl Into<SmolStr>) -> Self {
        Self {
            id: id.into(),
            config: RwLock::new(None),
        }
    }
}

impl LlmProvider for OpenAiCompatibleAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn configure(&self, config: ProviderConfig) {
        *self.config.write() = Some(config);
    }

    fn complete(&self, _request: &LlmRequest) -> Result<LlmCompletion, anyhow::Error> {
        // This adapter is a trait shim. The real HTTP implementation
        // lives in sps-providers-http::HttpProviderAdapter. Callers
        // should register that adapter via ProviderRegistry instead
        // of using this shim directly.
        Err(anyhow::anyhow!(
            "OpenAiCompatibleAdapter is a trait shim; \
             register sps_providers_http::HttpProviderAdapter instead"
        ))
    }

    fn healthcheck(&self) -> Result<ProviderHealth, anyhow::Error> {
        Ok(ProviderHealth {
            provider_id: self.id.clone(),
            healthy: false,
            latency_ms: 0,
            error: Some("Trait shim — use HttpProviderAdapter for real HTTP".to_string()),
        })
    }
}

/// Convenience: build a shared static adapter.
pub fn static_shared(id: &str, canned: &str) -> Arc<dyn LlmProvider> {
    Arc::new(StaticAdapter::new(id, canned))
}
