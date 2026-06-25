//! Direct LLM completion endpoint.

use std::sync::Arc;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use smol_str::SmolStr;
use sps_effects::providers::llm::{LlmProvider, LlmRequest};

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new().route("/api/llm/complete", post(complete))
}

#[derive(Debug, Deserialize)]
struct CompleteRequest {
    /// Provider id to use (defaults to the server's default provider).
    #[serde(default)]
    provider_id: Option<String>,
    /// User message.
    user: String,
    /// Optional system prompt.
    #[serde(default)]
    system: Option<String>,
    /// Optional model override.
    #[serde(default)]
    model: Option<String>,
    /// Max tokens.
    #[serde(default)]
    max_tokens: Option<u32>,
    /// Temperature.
    #[serde(default)]
    temperature: Option<f32>,
}

async fn complete(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<CompleteRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let provider_id = req
        .provider_id
        .or_else(|| state.default_provider().map(|s| s.as_str().to_string()))
        .ok_or((
            axum::http::StatusCode::BAD_REQUEST,
            "no provider configured — register a provider first".to_string(),
        ))?;

    let provider = state
        .providers
        .get(&provider_id)
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            format!("provider {} not registered", provider_id),
        ))?;

    let request = LlmRequest {
        provider_id: SmolStr::new(&provider_id),
        model: req.model.map(SmolStr::new),
        system: req.system,
        user: req.user,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
    };

    match provider.complete(&request) {
        Ok(completion) => Ok(Json(json!({
            "text": completion.text,
            "model": completion.model,
            "usage": {
                "prompt_tokens": completion.usage.prompt_tokens,
                "completion_tokens": completion.usage.completion_tokens,
                "total_tokens": completion.usage.total_tokens,
            },
            "elapsed_ms": completion.elapsed_ms,
            "provider": provider_id,
        }))),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )),
    }
}
