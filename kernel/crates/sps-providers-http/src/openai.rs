//! OpenAI (and OpenAI-compatible: OpenRouter, Groq, DeepSeek, LM Studio) adapter.

use std::sync::Arc;

use crate::adapter::HttpProviderAdapter;
use sps_effects::providers::llm::LlmProvider;

/// OpenAI adapter (https://api.openai.com/v1).
pub struct OpenAiAdapter(HttpProviderAdapter);

impl OpenAiAdapter {
    /// Create a new OpenAI adapter.
    pub fn new() -> Self {
        Self(HttpProviderAdapter::new("openai", "/v1/chat/completions"))
    }
}

impl Default for OpenAiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for OpenAiAdapter {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn configure(&self, config: sps_effects::providers::llm::ProviderConfig) {
        self.0.configure(config);
    }
    fn complete(
        &self,
        request: &sps_effects::providers::llm::LlmRequest,
    ) -> Result<sps_effects::providers::llm::LlmCompletion, anyhow::Error> {
        self.0.complete(request)
    }
    fn healthcheck(
        &self,
    ) -> Result<sps_effects::providers::llm::ProviderHealth, anyhow::Error> {
        self.0.healthcheck()
    }
}

/// OpenRouter adapter (https://openrouter.ai/api/v1).
pub struct OpenRouterAdapter(HttpProviderAdapter);

impl OpenRouterAdapter {
    /// Create a new OpenRouter adapter.
    pub fn new() -> Self {
        Self(HttpProviderAdapter::new("openrouter", "/api/v1/chat/completions"))
    }
}

impl Default for OpenRouterAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for OpenRouterAdapter {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn configure(&self, config: sps_effects::providers::llm::ProviderConfig) {
        self.0.configure(config);
    }
    fn complete(
        &self,
        request: &sps_effects::providers::llm::LlmRequest,
    ) -> Result<sps_effects::providers::llm::LlmCompletion, anyhow::Error> {
        self.0.complete(request)
    }
    fn healthcheck(
        &self,
    ) -> Result<sps_effects::providers::llm::ProviderHealth, anyhow::Error> {
        self.0.healthcheck()
    }
}

/// Groq adapter (https://api.groq.com/openai/v1).
pub struct GroqAdapter(HttpProviderAdapter);

impl GroqAdapter {
    /// Create a new Groq adapter.
    pub fn new() -> Self {
        Self(HttpProviderAdapter::new("groq", "/openai/v1/chat/completions"))
    }
}

impl Default for GroqAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for GroqAdapter {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn configure(&self, config: sps_effects::providers::llm::ProviderConfig) {
        self.0.configure(config);
    }
    fn complete(
        &self,
        request: &sps_effects::providers::llm::LlmRequest,
    ) -> Result<sps_effects::providers::llm::LlmCompletion, anyhow::Error> {
        self.0.complete(request)
    }
    fn healthcheck(
        &self,
    ) -> Result<sps_effects::providers::llm::ProviderHealth, anyhow::Error> {
        self.0.healthcheck()
    }
}

/// DeepSeek adapter (https://api.deepseek.com/v1).
pub struct DeepSeekAdapter(HttpProviderAdapter);

impl DeepSeekAdapter {
    /// Create a new DeepSeek adapter.
    pub fn new() -> Self {
        Self(HttpProviderAdapter::new("deepseek", "/v1/chat/completions"))
    }
}

impl Default for DeepSeekAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for DeepSeekAdapter {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn configure(&self, config: sps_effects::providers::llm::ProviderConfig) {
        self.0.configure(config);
    }
    fn complete(
        &self,
        request: &sps_effects::providers::llm::LlmRequest,
    ) -> Result<sps_effects::providers::llm::LlmCompletion, anyhow::Error> {
        self.0.complete(request)
    }
    fn healthcheck(
        &self,
    ) -> Result<sps_effects::providers::llm::ProviderHealth, anyhow::Error> {
        self.0.healthcheck()
    }
}

/// LM Studio adapter (http://localhost:1234/v1).
pub struct LmStudioAdapter(HttpProviderAdapter);

impl LmStudioAdapter {
    /// Create a new LM Studio adapter.
    pub fn new() -> Self {
        Self(HttpProviderAdapter::new("lmstudio", "/v1/chat/completions"))
    }
}

impl Default for LmStudioAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for LmStudioAdapter {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn configure(&self, config: sps_effects::providers::llm::ProviderConfig) {
        self.0.configure(config);
    }
    fn complete(
        &self,
        request: &sps_effects::providers::llm::LlmRequest,
    ) -> Result<sps_effects::providers::llm::LlmCompletion, anyhow::Error> {
        self.0.complete(request)
    }
    fn healthcheck(
        &self,
    ) -> Result<sps_effects::providers::llm::ProviderHealth, anyhow::Error> {
        self.0.healthcheck()
    }
}

/// Convenience constructors.
pub fn openai_shared() -> Arc<dyn LlmProvider> {
    Arc::new(OpenAiAdapter::new())
}
pub fn openrouter_shared() -> Arc<dyn LlmProvider> {
    Arc::new(OpenRouterAdapter::new())
}
pub fn groq_shared() -> Arc<dyn LlmProvider> {
    Arc::new(GroqAdapter::new())
}
pub fn deepseek_shared() -> Arc<dyn LlmProvider> {
    Arc::new(DeepSeekAdapter::new())
}
pub fn lmstudio_shared() -> Arc<dyn LlmProvider> {
    Arc::new(LmStudioAdapter::new())
}
