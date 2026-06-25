//! Phase 12B: Provider templates + custom provider support.
//!
//! A provider template describes how to construct an LlmProvider from a
//! ProviderConfig. Built-in templates cover 12+ providers; custom templates
//! allow users to add arbitrary OpenAI-compatible or Anthropic-compatible
//! providers at runtime.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::sync::Arc;

use sps_effects::providers::llm::{LlmProvider, ProviderConfig};

/// The API format a provider speaks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFormat {
    /// OpenAI-compatible (POST /v1/chat/completions with messages array).
    OpenAi,
    /// Anthropic-compatible (POST /v1/messages with messages array).
    Anthropic,
    /// Ollama-compatible (POST /api/chat with messages array).
    Ollama,
}

impl Default for ApiFormat {
    fn default() -> Self {
        Self::OpenAi
    }
}

/// A provider template — describes how to build a provider instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTemplate {
    /// Template id (e.g. "openai", "mistral", "custom-myorg").
    pub id: SmolStr,
    /// Display name.
    pub name: SmolStr,
    /// Default API URL.
    pub default_api_url: String,
    /// Default endpoint path appended to api_url.
    pub endpoint_path: String,
    /// API format.
    pub api_format: ApiFormat,
    /// Whether an API key is required.
    pub requires_api_key: bool,
    /// Auth header name (e.g. "Authorization", "x-api-key").
    pub auth_header: String,
    /// Auth header value prefix (e.g. "Bearer ", "").
    pub auth_prefix: String,
    /// Additional headers to send.
    #[serde(default)]
    pub extra_headers: std::collections::BTreeMap<String, String>,
    /// Default model name.
    pub default_model: SmolStr,
    /// Whether this is a built-in template (true) or custom (false).
    pub builtin: bool,
}

/// All built-in provider templates.
pub fn builtin_templates() -> Vec<ProviderTemplate> {
    vec![
        ProviderTemplate {
            id: "openai".into(),
            name: "OpenAI".into(),
            default_api_url: "https://api.openai.com/v1".into(),
            endpoint_path: "/chat/completions".into(),
            api_format: ApiFormat::OpenAi,
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "gpt-4o".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "anthropic".into(),
            name: "Anthropic Claude".into(),
            default_api_url: "https://api.anthropic.com".into(),
            endpoint_path: "/v1/messages".into(),
            api_format: ApiFormat::Anthropic,
            requires_api_key: true,
            auth_header: "x-api-key".into(),
            auth_prefix: "".into(),
            extra_headers: [("anthropic-version".to_string(), "2023-06-01".to_string())]
                .into_iter()
                .collect(),
            default_model: "claude-sonnet-4-20250514".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "openrouter".into(),
            name: "OpenRouter".into(),
            default_api_url: "https://openrouter.ai/api/v1".into(),
            endpoint_path: "/chat/completions".into(),
            api_format: ApiFormat::OpenAi,
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "anthropic/claude-3.5-sonnet".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "groq".into(),
            name: "Groq".into(),
            default_api_url: "https://api.groq.com/openai/v1".into(),
            endpoint_path: "/chat/completions".into(),
            api_format: ApiFormat::OpenAi,
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "llama-3.3-70b-versatile".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "deepseek".into(),
            name: "DeepSeek".into(),
            default_api_url: "https://api.deepseek.com/v1".into(),
            endpoint_path: "/chat/completions".into(),
            api_format: ApiFormat::OpenAi,
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "deepseek-chat".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "mistral".into(),
            name: "Mistral AI".into(),
            default_api_url: "https://api.mistral.ai/v1".into(),
            endpoint_path: "/chat/completions".into(),
            api_format: ApiFormat::OpenAi,
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "mistral-large-latest".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "cohere".into(),
            name: "Cohere".into(),
            default_api_url: "https://api.cohere.com/v1".into(),
            endpoint_path: "/chat".into(),
            api_format: ApiFormat::OpenAi,
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "command-r-plus".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "together".into(),
            name: "Together AI".into(),
            default_api_url: "https://api.together.xyz/v1".into(),
            endpoint_path: "/chat/completions".into(),
            api_format: ApiFormat::OpenAi,
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "meta-llama/Llama-3-70b-chat-hf".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "fireworks".into(),
            name: "Fireworks AI".into(),
            default_api_url: "https://api.fireworks.ai/inference/v1".into(),
            endpoint_path: "/chat/completions".into(),
            api_format: ApiFormat::OpenAi,
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "accounts/fireworks/models/llama-v3-70b-instruct".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "ollama".into(),
            name: "Ollama (local)".into(),
            default_api_url: "http://localhost:11434".into(),
            endpoint_path: "/api/chat".into(),
            api_format: ApiFormat::Ollama,
            requires_api_key: false,
            auth_header: "".into(),
            auth_prefix: "".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "llama3.2".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "lmstudio".into(),
            name: "LM Studio (local)".into(),
            default_api_url: "http://localhost:1234/v1".into(),
            endpoint_path: "/chat/completions".into(),
            api_format: ApiFormat::OpenAi,
            requires_api_key: false,
            auth_header: "".into(),
            auth_prefix: "".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "local-model".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "vllm".into(),
            name: "vLLM (local)".into(),
            default_api_url: "http://localhost:8000/v1".into(),
            endpoint_path: "/chat/completions".into(),
            api_format: ApiFormat::OpenAi,
            requires_api_key: false,
            auth_header: "".into(),
            auth_prefix: "".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "auto".into(),
            builtin: true,
        },
        ProviderTemplate {
            id: "azure-openai".into(),
            name: "Azure OpenAI".into(),
            default_api_url: "https://{resource}.openai.azure.com".into(),
            endpoint_path: "/openai/deployments/{deployment}/chat/completions".into(),
            api_format: ApiFormat::OpenAi,
            requires_api_key: true,
            auth_header: "api-key".into(),
            auth_prefix: "".into(),
            extra_headers: std::collections::BTreeMap::new(),
            default_model: "gpt-4o".into(),
            builtin: true,
        },
    ]
}

/// Look up a built-in template by id.
pub fn get_builtin_template(id: &str) -> Option<ProviderTemplate> {
    builtin_templates().into_iter().find(|t| t.id == id)
}

/// Build an LlmProvider from a template + config.
///
/// The provider is configured with the given config and returned as an
/// Arc<dyn LlmProvider>. The template determines which adapter type to
/// instantiate (OpenAI-compatible, Anthropic, or Ollama).
pub fn build_provider(
    template: &ProviderTemplate,
    config: ProviderConfig,
) -> Result<Arc<dyn LlmProvider>, String> {
    let provider: Arc<dyn LlmProvider> = match template.api_format {
        ApiFormat::OpenAi => {
            // Use the generic HttpProviderAdapter for all OpenAI-compatible providers.
            Arc::new(crate::adapter::HttpProviderAdapter::new(
                config.id.as_str(),
                &template.endpoint_path,
            ))
        }
        ApiFormat::Anthropic => {
            Arc::new(crate::anthropic::AnthropicAdapter::new())
        }
        ApiFormat::Ollama => {
            Arc::new(crate::ollama::OllamaAdapter::new())
        }
    };
    provider.configure(config);
    Ok(provider)
}

/// Request to add a custom provider (via HTTP API).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddCustomProviderRequest {
    /// Provider id (unique within the registry).
    pub id: String,
    /// Display name.
    pub name: String,
    /// API URL.
    pub api_url: String,
    /// API key (optional for local providers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Model name.
    pub model_name: String,
    /// API format.
    #[serde(default)]
    pub api_format: ApiFormat,
    /// Endpoint path (e.g. "/chat/completions").
    #[serde(default = "default_endpoint_path")]
    pub endpoint_path: String,
    /// Auth header name.
    #[serde(default = "default_auth_header")]
    pub auth_header: String,
    /// Auth header value prefix.
    #[serde(default)]
    pub auth_prefix: String,
    /// Extra headers.
    #[serde(default)]
    pub extra_headers: std::collections::BTreeMap<String, String>,
}

fn default_endpoint_path() -> String {
    "/chat/completions".into()
}

fn default_auth_header() -> String {
    "Authorization".into()
}

impl AddCustomProviderRequest {
    /// Convert to a ProviderTemplate (builtin = false).
    pub fn to_template(&self) -> ProviderTemplate {
        ProviderTemplate {
            id: self.id.clone().into(),
            name: self.name.clone().into(),
            default_api_url: self.api_url.clone(),
            endpoint_path: self.endpoint_path.clone(),
            api_format: self.api_format.clone(),
            requires_api_key: self.api_key.is_some(),
            auth_header: self.auth_header.clone(),
            auth_prefix: self.auth_prefix.clone(),
            extra_headers: self.extra_headers.clone(),
            default_model: self.model_name.clone().into(),
            builtin: false,
        }
    }

    /// Convert to a ProviderConfig.
    pub fn to_config(&self) -> ProviderConfig {
        ProviderConfig {
            id: self.id.clone().into(),
            name: self.name.clone().into(),
            api_url: self.api_url.clone(),
            api_key: self.api_key.clone(),
            model_name: self.model_name.clone().into(),
            metadata: std::collections::BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_templates_has_12_providers() {
        let templates = builtin_templates();
        assert!(templates.len() >= 12, "expected >=12 builtin templates, got {}", templates.len());
        let ids: Vec<_> = templates.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"openai"));
        assert!(ids.contains(&"anthropic"));
        assert!(ids.contains(&"mistral"));
        assert!(ids.contains(&"cohere"));
        assert!(ids.contains(&"together"));
        assert!(ids.contains(&"fireworks"));
        assert!(ids.contains(&"ollama"));
        assert!(ids.contains(&"vllm"));
        assert!(ids.contains(&"azure-openai"));
    }

    #[test]
    fn get_builtin_template_finds_existing() {
        let t = get_builtin_template("openai").unwrap();
        assert_eq!(t.api_format, ApiFormat::OpenAi);
        assert!(t.requires_api_key);
    }

    #[test]
    fn get_builtin_template_returns_none_for_unknown() {
        assert!(get_builtin_template("nonexistent").is_none());
    }

    #[test]
    fn custom_provider_request_converts_to_template_and_config() {
        let req = AddCustomProviderRequest {
            id: "my-custom".into(),
            name: "My Custom Provider".into(),
            api_url: "https://my-api.example.com/v1".into(),
            api_key: Some("sk-test".into()),
            model_name: "my-model".into(),
            api_format: ApiFormat::OpenAi,
            endpoint_path: "/chat/completions".into(),
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            extra_headers: std::collections::BTreeMap::new(),
        };
        let template = req.to_template();
        assert_eq!(template.id, "my-custom");
        assert!(!template.builtin);
        assert_eq!(template.api_format, ApiFormat::OpenAi);

        let config = req.to_config();
        assert_eq!(config.id, "my-custom");
        assert_eq!(config.api_url, "https://my-api.example.com/v1");
    }
}
