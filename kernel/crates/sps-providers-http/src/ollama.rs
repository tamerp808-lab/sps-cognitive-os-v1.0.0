//! Ollama adapter (http://localhost:11434).
//!
//! Ollama uses its own `/api/chat` endpoint. No API key required.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_effects::providers::llm::{
    LlmCompletion, LlmProvider, LlmRequest, ProviderConfig, ProviderHealth, TokenUsage,
};

use crate::retry::{RetryConfig, RetryPolicy};

/// Ollama adapter for local LLMs.
pub struct OllamaAdapter {
    id: SmolStr,
    config: RwLock<Option<ProviderConfig>>,
    client: reqwest::Client,
    retry: RetryConfig,
}

impl OllamaAdapter {
    /// Create a new Ollama adapter. Default URL is `http://localhost:11434`.
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // local models can be slow
            .build()
            .expect("failed to build reqwest client");
        Self {
            id: "ollama".into(),
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

impl Default for OllamaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    #[serde(default)]
    message: Option<OllamaMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    total_duration: Option<u64>, // nanoseconds
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
}

impl LlmProvider for OllamaAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn configure(&self, config: ProviderConfig) {
        *self.config.write() = Some(config);
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, anyhow::Error> {
        let cfg = self.config.read().clone().ok_or_else(|| anyhow::anyhow!("ollama not configured"))?;
        let model = request
            .model
            .as_ref()
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| cfg.model_name.as_str().to_string());

        let mut messages = Vec::with_capacity(2);
        if let Some(system) = &request.system {
            messages.push(OllamaMessage {
                role: "system".into(),
                content: system.clone(),
            });
        }
        messages.push(OllamaMessage {
            role: "user".into(),
            content: request.user.clone(),
        });

        let options = Some(OllamaOptions {
            temperature: request.temperature,
            num_predict: request.max_tokens,
        });

        let body = OllamaChatRequest {
            model,
            messages,
            options,
            stream: false,
        };

        let url = format!("{}/api/chat", cfg.api_url.trim_end_matches('/'));
        let client = self.client.clone();
        let policy = RetryPolicy::new(self.retry.clone());

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let start = std::time::Instant::now();
        let url_clone = url.clone();
        let result = rt.block_on(async move {
            policy
                .run(|| {
                    let client = client.clone();
                    let url = url_clone.clone();
                    let body = serde_json::to_value(&body).unwrap();
                    Box::pin(async move {
                        let resp = client.post(&url).json(&body).send().await?;
                        if !resp.status().is_success() {
                            let status = resp.status();
                            let text = resp.text().await.unwrap_or_default();
                            return Err(anyhow::anyhow!("HTTP {}: {}", status, text));
                        }
                        let parsed: OllamaChatResponse = resp.json().await?;
                        Ok(parsed)
                    })
                })
                .await
        })?;

        let text = result
            .message
            .map(|m| m.content)
            .unwrap_or_default();
        let usage = TokenUsage {
            prompt_tokens: result.prompt_eval_count.unwrap_or(0),
            completion_tokens: result.eval_count.unwrap_or(0),
            total_tokens: result.prompt_eval_count.unwrap_or(0) + result.eval_count.unwrap_or(0),
        };

        Ok(LlmCompletion {
            text,
            model: cfg.model_name,
            usage,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    fn healthcheck(&self) -> Result<ProviderHealth, anyhow::Error> {
        let cfg = self.config.read().clone();
        let url = match cfg {
            Some(c) => format!("{}/api/tags", c.api_url.trim_end_matches('/')),
            None => {
                return Ok(ProviderHealth {
                    provider_id: self.id.clone(),
                    healthy: false,
                    latency_ms: 0,
                    error: Some("not configured".to_string()),
                });
            }
        };
        let client = self.client.clone();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let start = std::time::Instant::now();
        let provider_id = self.id.clone();
        let result = rt.block_on(async move {
            let resp = client.get(&url).send().await?;
            if !resp.status().is_success() {
                let status = resp.status();
                return Err(anyhow::anyhow!("healthcheck HTTP {}", status));
            }
            Ok(())
        });
        let latency_ms = start.elapsed().as_millis() as u64;
        match result {
            Ok(()) => Ok(ProviderHealth {
                provider_id,
                healthy: true,
                latency_ms,
                error: None,
            }),
            Err(e) => Ok(ProviderHealth {
                provider_id,
                healthy: false,
                latency_ms,
                error: Some(e.to_string()),
            }),
        }
    }
}

/// Convenience shared constructor.
pub fn shared() -> Arc<dyn LlmProvider> {
    Arc::new(OllamaAdapter::new())
}
