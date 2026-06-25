//! `sps provider` — manage LLM providers.

use anyhow::{anyhow, Result};
use smol_str::SmolStr;
use sps_effects::providers::llm::{LlmProvider, ProviderConfig};
use sps_providers_http::{
    AnthropicAdapter, DeepSeekAdapter, GroqAdapter, HttpProviderAdapter, LmStudioAdapter,
    OllamaAdapter, OpenAiAdapter, OpenRouterAdapter,
};
use std::sync::Arc;

/// Build a provider from kind + config.
pub fn build_provider(
    kind: &str,
    api_url: String,
    api_key: Option<String>,
    model: &str,
) -> Result<Arc<dyn LlmProvider>> {
    let id = SmolStr::new(kind);
    let config = ProviderConfig {
        id: id.clone(),
        name: id.clone(),
        api_url,
        api_key,
        model_name: model.into(),
        metadata: Default::default(),
    };
    let provider: Arc<dyn LlmProvider> = match kind {
        "openai" => Arc::new(OpenAiAdapter::new()),
        "openrouter" => Arc::new(OpenRouterAdapter::new()),
        "anthropic" => Arc::new(AnthropicAdapter::new()),
        "ollama" => Arc::new(OllamaAdapter::new()),
        "groq" => Arc::new(GroqAdapter::new()),
        "deepseek" => Arc::new(DeepSeekAdapter::new()),
        "lmstudio" => Arc::new(LmStudioAdapter::new()),
        "custom" => Arc::new(HttpProviderAdapter::new("custom", "/v1/chat/completions")),
        other => return Err(anyhow!("unknown provider kind: {}", other)),
    };
    provider.configure(config);
    Ok(provider)
}

/// Run a healthcheck on a provider.
pub fn healthcheck(provider: &Arc<dyn LlmProvider>) -> Result<()> {
    let id = provider.id().to_string();
    print!("Healthchecking {}... ", id);
    match provider.healthcheck() {
        Ok(h) => {
            if h.healthy {
                println!("OK ({} ms)", h.latency_ms);
            } else {
                println!("UNHEALTHY: {:?}", h.error);
            }
            Ok(())
        }
        Err(e) => {
            println!("ERROR: {}", e);
            Err(e)
        }
    }
}

/// List known provider kinds.
pub fn list_kinds() -> Vec<&'static str> {
    vec![
        "openai",
        "openrouter",
        "anthropic",
        "ollama",
        "groq",
        "deepseek",
        "lmstudio",
        "custom",
    ]
}
