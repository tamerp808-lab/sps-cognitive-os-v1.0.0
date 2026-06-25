//! Anthropic Claude adapter (https://api.anthropic.com/v1).
//!
//! Uses Anthropic's native `messages` API (not OpenAI-compatible).

use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_effects::providers::llm::{
    LlmCompletion, LlmProvider, LlmRequest, ProviderConfig, ProviderHealth, TokenUsage,
};

use crate::retry::{RetryConfig, RetryPolicy};

/// Anthropic Claude adapter.
pub struct AnthropicAdapter {
    id: SmolStr,
    config: RwLock<Option<ProviderConfig>>,
    client: reqwest::Client,
    retry: RetryConfig,
}

impl AnthropicAdapter {
    /// Create a new Anthropic adapter.
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build reqwest client");
        Self {
            id: "anthropic".into(),
            config: RwLock::new(None),
            client,
            retry: RetryConfig::default(),
        }
    }

    /// Override retry config.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }
}

impl Default for AnthropicAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    #[serde(default)]
    content: Vec<AnthropicContent>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    _type: String,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

impl LlmProvider for AnthropicAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn configure(&self, config: ProviderConfig) {
        *self.config.write() = Some(config);
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, anyhow::Error> {
        let cfg = self.config.read().clone().ok_or_else(|| anyhow::anyhow!("anthropic not configured"))?;
        let model = request
            .model
            .as_ref()
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| cfg.model_name.as_str().to_string());
        let max_tokens = request.max_tokens.unwrap_or(1024);

        let body = AnthropicRequest {
            model,
            max_tokens,
            system: request.system.clone(),
            messages: vec![AnthropicMessage {
                role: "user".into(),
                content: request.user.clone(),
            }],
            temperature: request.temperature,
        };

        let api_key = cfg
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("anthropic requires an api_key"))?
            .clone();
        let url = format!("{}/v1/messages", cfg.api_url.trim_end_matches('/'));

        let client = self.client.clone();
        let policy = RetryPolicy::new(self.retry.clone());

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let start = std::time::Instant::now();
        let result = rt.block_on(async move {
            policy
                .run(|| {
                    let client = client.clone();
                    let api_key = api_key.clone();
                    let url = url.clone();
                    let body = serde_json::to_value(&body).unwrap();
                    Box::pin(async move {
                        let resp = client
                            .post(&url)
                            .header("x-api-key", &api_key)
                            .header("anthropic-version", "2023-06-01")
                            .header("content-type", "application/json")
                            .json(&body)
                            .send()
                            .await?;
                        if !resp.status().is_success() {
                            let status = resp.status();
                            let text = resp.text().await.unwrap_or_default();
                            return Err(anyhow::anyhow!("HTTP {}: {}", status, text));
                        }
                        let parsed: AnthropicResponse = resp.json().await?;
                        Ok(parsed)
                    })
                })
                .await
        })?;

        let text = result
            .content
            .into_iter()
            .filter(|c| c._type == "text")
            .map(|c| c.text)
            .collect::<Vec<_>>()
            .join("");
        let usage = result
            .usage
            .map(|u| TokenUsage {
                prompt_tokens: u.input_tokens,
                completion_tokens: u.output_tokens,
                total_tokens: u.input_tokens + u.output_tokens,
            })
            .unwrap_or_default();

        Ok(LlmCompletion {
            text,
            model: cfg.model_name,
            usage,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    fn healthcheck(&self) -> Result<ProviderHealth, anyhow::Error> {
        // Anthropic doesn't have a free /models endpoint without auth in
        // the same way OpenAI does. We treat "configured" as "healthy".
        let cfg = self.config.read().clone();
        let healthy = cfg.is_some();
        let error = if healthy {
            None
        } else {
            Some("not configured".to_string())
        };
        Ok(ProviderHealth {
            provider_id: self.id.clone(),
            healthy,
            latency_ms: 0,
            error,
        })
    }
}

/// Convenience shared constructor.
pub fn shared() -> Arc<dyn LlmProvider> {
    Arc::new(AnthropicAdapter::new())
}
