//! Provider endpoints — register, list, remove, healthcheck.
//!
//! Phase 12B: Full multi-provider system with 12+ built-in templates
//! + custom provider support. Any OpenAI-compatible, Anthropic-compatible,
//! or Ollama-compatible API can be registered at runtime.

use std::sync::Arc;
use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;
use sps_effects::providers::llm::{LlmProvider, ProviderConfig};
use sps_providers_http::{
    AddCustomProviderRequest, AnthropicAdapter, ApiFormat, DeepSeekAdapter, GroqAdapter,
    HttpProviderAdapter, LmStudioAdapter, OllamaAdapter, OpenAiAdapter, OpenRouterAdapter,
    build_provider, builtin_templates, get_builtin_template,
};

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/providers", get(list_providers).post(register_provider))
        .route("/api/providers/templates", get(list_templates))
        .route("/api/providers/custom", post(register_custom_provider))
        .route("/api/providers/{id}", delete(remove_provider))
        .route("/api/providers/{id}/healthcheck", post(healthcheck_provider))
        .route("/api/providers/default", post(set_default_provider))
}

#[derive(Debug, Serialize)]
struct ProviderInfo {
    id: String,
    name: String,
    kind: String,
    api_url: String,
    model_name: String,
    has_key: bool,
}

async fn list_providers(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let ids = state.providers.list();
    let infos: Vec<ProviderInfo> = ids
        .iter()
        .map(|id| {
            ProviderInfo {
                id: id.as_str().to_string(),
                name: id.as_str().to_string(),
                kind: id.as_str().to_string(),
                api_url: String::new(),
                model_name: String::new(),
                has_key: false,
            }
        })
        .collect();
    let default = state.default_provider();
    Json(json!({
        "providers": infos,
        "count": infos.len(),
        "default_provider": default,
    }))
}

/// Phase 12B: List all available provider templates (built-in).
async fn list_templates() -> Json<serde_json::Value> {
    let templates = builtin_templates();
    let template_infos: Vec<serde_json::Value> = templates
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "name": t.name,
                "default_api_url": t.default_api_url,
                "api_format": format!("{:?}", t.api_format).to_lowercase(),
                "requires_api_key": t.requires_api_key,
                "default_model": t.default_model,
                "builtin": t.builtin,
            })
        })
        .collect();
    Json(json!({
        "templates": template_infos,
        "count": template_infos.len(),
    }))
}

#[derive(Debug, Deserialize)]
struct RegisterProviderRequest {
    /// Provider kind: openai, openrouter, anthropic, ollama, groq, deepseek,
    /// lmstudio, mistral, cohere, together, fireworks, vllm, azure-openai, custom.
    kind: String,
    /// Provider id (optional — defaults to kind).
    #[serde(default)]
    id: Option<String>,
    /// Display name.
    #[serde(default)]
    name: Option<String>,
    /// API URL (e.g. https://api.openai.com/v1).
    api_url: String,
    /// API key (optional for local providers like Ollama).
    #[serde(default)]
    api_key: Option<String>,
    /// Default model name.
    model_name: String,
}

async fn register_provider(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<RegisterProviderRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let id = SmolStr::new(req.id.clone().unwrap_or_else(|| req.kind.clone()));
    let name = SmolStr::new(req.name.unwrap_or_else(|| id.as_str().to_string()));

    let config = ProviderConfig {
        id: id.clone(),
        name: name.clone(),
        api_url: req.api_url.clone(),
        api_key: req.api_key.clone(),
        model_name: SmolStr::new(req.model_name.clone()),
        metadata: Default::default(),
    };

    // Phase 12B: Use the template system for all providers.
    let provider: Arc<dyn LlmProvider> = match req.kind.as_str() {
        "openai" => Arc::new(OpenAiAdapter::new()),
        "openrouter" => Arc::new(OpenRouterAdapter::new()),
        "anthropic" => Arc::new(AnthropicAdapter::new()),
        "ollama" => Arc::new(OllamaAdapter::new()),
        "groq" => Arc::new(GroqAdapter::new()),
        "deepseek" => Arc::new(DeepSeekAdapter::new()),
        "lmstudio" => Arc::new(LmStudioAdapter::new()),
        // Phase 12B: new providers use the generic HttpProviderAdapter.
        "mistral" | "cohere" | "together" | "fireworks" | "vllm" => {
            let template = get_builtin_template(&req.kind)
                .ok_or_else(|| (axum::http::StatusCode::BAD_REQUEST, format!("no template for kind: {}", req.kind)))?;
            Arc::new(HttpProviderAdapter::new(id.as_str(), &template.endpoint_path))
        }
        // Phase 12B: custom provider with full template control.
        "custom" => Arc::new(HttpProviderAdapter::new(id.as_str(), "/v1/chat/completions")),
        other => {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                format!("unknown provider kind: {}. Use /api/providers/templates to list available kinds, or /api/providers/custom for fully custom providers.", other),
            ));
        }
    };

    provider.configure(config.clone());
    state.providers.register(config.clone(), provider);

    // Auto-set default if none set.
    if state.default_provider().is_none() {
        state.set_default_provider(id.clone());
    }

    Ok(Json(json!({
        "id": id,
        "name": name,
        "kind": req.kind,
        "api_url": config.api_url,
        "model_name": config.model_name,
        "has_key": config.api_key.is_some(),
        "is_default": state.default_provider() == Some(id.clone()),
    })))
}

/// Phase 12B: Register a fully custom provider with a complete template.
///
/// This endpoint allows users to add any OpenAI-compatible, Anthropic-
/// compatible, or Ollama-compatible API at runtime. The request includes
/// the API format, endpoint path, auth header, and extra headers.
async fn register_custom_provider(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<AddCustomProviderRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // Build the template from the request.
    let template = req.to_template();
    let config = req.to_config();

    // Build the provider using the template system.
    let provider = build_provider(&template, config.clone())
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;

    state.providers.register(config.clone(), provider);

    // Auto-set default if none set.
    if state.default_provider().is_none() {
        state.set_default_provider(config.id.clone());
    }

    Ok(Json(json!({
        "id": config.id,
        "name": config.name,
        "api_url": config.api_url,
        "model_name": config.model_name,
        "has_key": config.api_key.is_some(),
        "api_format": format!("{:?}", template.api_format).to_lowercase(),
        "is_default": state.default_provider() == Some(config.id.clone()),
        "custom": true,
    })))
}

async fn remove_provider(
    State(state): State<Arc<ServerState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let removed = state.providers.remove(&id);
    if removed {
        // Clear default if it was this provider.
        if state.default_provider().as_deref() == Some(id.as_str()) {
            *state.default_provider.write() = None;
        }
    }
    Json(json!({ "removed": removed, "id": id }))
}

async fn healthcheck_provider(
    State(state): State<Arc<ServerState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let provider = state
        .providers
        .get(&id)
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            format!("provider {} not registered", id),
        ))?;
    let health = provider
        .healthcheck()
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({
        "provider_id": health.provider_id,
        "healthy": health.healthy,
        "latency_ms": health.latency_ms,
        "error": health.error,
    })))
}

#[derive(Debug, Deserialize)]
struct SetDefaultRequest {
    id: String,
}

async fn set_default_provider(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<SetDefaultRequest>,
) -> Json<serde_json::Value> {
    if state.providers.get(&req.id).is_some() {
        state.set_default_provider(req.id.clone());
        Json(json!({ "default_provider": req.id }))
    } else {
        Json(json!({ "error": format!("provider {} not registered", req.id) }))
    }
}
