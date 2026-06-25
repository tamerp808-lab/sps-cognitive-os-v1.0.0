//! Generic HTTP adapter for OpenAI-compatible providers.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_effects::providers::llm::{
    LlmCompletion, LlmProvider, LlmRequest, ProviderConfig, ProviderHealth, TokenUsage,
};

use crate::retry::{RetryConfig, RetryPolicy};

/// A generic adapter that talks to any OpenAI-compatible `/v1/chat/completions`
/// endpoint. Used directly for OpenAI, OpenRouter, Groq, DeepSeek, LM Studio.
pub struct HttpProviderAdapter {
    id: SmolStr,
    config: RwLock<Option<ProviderConfig>>,
    client: reqwest::Client,
    retry: RetryConfig,
    endpoint_path: String,
}

impl HttpProviderAdapter {
    /// Create a new adapter with the given id and endpoint path
    /// (e.g. `/v1/chat/completions` for OpenAI-compatible,
    /// `/api/chat` for Ollama).
    pub fn new(id: impl Into<SmolStr>, endpoint_path: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build reqwest client");
        Self {
            id: id.into(),
            config: RwLock::new(None),
            client,
            retry: RetryConfig::default(),
            endpoint_path: endpoint_path.into(),
        }
    }

    /// Override the retry config.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    /// Get the current configuration (for internal use).
    fn config(&self) -> Option<ProviderConfig> {
        self.config.read().clone()
    }

    /// Build the full endpoint URL.
    fn endpoint_url(&self) -> Option<String> {
        let cfg = self.config.read();
        cfg.as_ref().map(|c| format!("{}{}", c.api_url.trim_end_matches('/'), self.endpoint_path))
    }

    /// Build the authorization header value (if any).
    fn auth_header(&self) -> Option<String> {
        let cfg = self.config.read();
        cfg.as_ref().and_then(|c| c.api_key.as_ref().map(|k| format!("Bearer {}", k)))
    }
}

/// OpenAI-compatible chat request body.
#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

/// OpenAI-compatible chat response body.
#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    #[serde(default)]
    message: Option<OpenAiMessage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

impl LlmProvider for HttpProviderAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn configure(&self, config: ProviderConfig) {
        *self.config.write() = Some(config);
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, anyhow::Error> {
        let cfg = self
            .config()
            .ok_or_else(|| anyhow::anyhow!("provider not configured"))?
            .clone();
        let model = request
            .model
            .as_ref()
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| cfg.model_name.as_str().to_string());

        let mut messages = Vec::with_capacity(2);
        if let Some(system) = &request.system {
            messages.push(OpenAiMessage {
                role: "system".into(),
                content: system.clone(),
            });
        }
        messages.push(OpenAiMessage {
            role: "user".into(),
            content: request.user.clone(),
        });

        let body = OpenAiChatRequest {
            model,
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stream: false,
        };

        let url = self.endpoint_url().ok_or_else(|| anyhow::anyhow!("provider not configured"))?;
        let auth = self.auth_header();

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
                    let url = url.clone();
                    let auth = auth.clone();
                    let body = serde_json::to_value(&body).unwrap();
                    Box::pin(async move {
                        let mut req = client.post(&url).json(&body);
                        if let Some(a) = auth {
                            req = req.header("Authorization", a);
                        }
                        let resp = req.send().await?;
                        if !resp.status().is_success() {
                            let status = resp.status();
                            let text = resp.text().await.unwrap_or_default();
                            return Err(anyhow::anyhow!(
                                "HTTP {} {}: {}",
                                status.as_u16(),
                                status.canonical_reason().unwrap_or(""),
                                text
                            ));
                        }
                        let parsed: OpenAiChatResponse = resp.json().await?;
                        Ok(parsed)
                    })
                })
                .await
        })?;

        let text = result
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.map(|m| m.content))
            .unwrap_or_default();
        let usage = result
            .usage
            .map(|u| TokenUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
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
        let cfg = self.config().ok_or_else(|| anyhow::anyhow!("provider not configured"))?;
        let url = format!(
            "{}/models",
            cfg.api_url.trim_end_matches('/')
        );
        let auth = self.auth_header();
        let client = self.client.clone();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let start = std::time::Instant::now();
        let provider_id = self.id.clone();
        let result = rt.block_on(async move {
            let mut req = client.get(&url);
            if let Some(a) = auth {
                req = req.header("Authorization", a);
            }
            let resp = req.send().await?;
            if !resp.status().is_success() {
                let status = resp.status();
                return Err(anyhow::anyhow!("healthcheck failed: HTTP {}", status));
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

/// Build a shared HTTP adapter.
pub fn shared(id: &str, endpoint_path: &str) -> Arc<dyn LlmProvider> {
    Arc::new(HttpProviderAdapter::new(id, endpoint_path))
}
