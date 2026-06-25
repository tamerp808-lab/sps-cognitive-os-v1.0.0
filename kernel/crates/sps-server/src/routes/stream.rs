//! Streaming chat endpoint — SSE stream of tokens.

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event as SseEvent, Sse};
use axum::routing::post;
use axum::{Json, Router};
use futures_util::Stream;
use serde::Deserialize;
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new().route("/api/chat/stream", post(stream_chat))
}

#[derive(Debug, Deserialize)]
struct StreamRequest {
    /// Provider id (defaults to server's default).
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
}

async fn stream_chat(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<StreamRequest>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, (axum::http::StatusCode, String)> {
    let provider_id = req
        .provider_id
        .or_else(|| state.default_provider().map(|s| s.as_str().to_string()))
        .ok_or((
            axum::http::StatusCode::BAD_REQUEST,
            "no provider configured".to_string(),
        ))?;

    let provider = state
        .providers
        .get(&provider_id)
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            format!("provider {} not registered", provider_id),
        ))?;

    let request = sps_effects::providers::llm::LlmRequest {
        provider_id: smol_str::SmolStr::new(&provider_id),
        model: req.model.map(smol_str::SmolStr::new),
        system: req.system,
        user: req.user,
        max_tokens: None,
        temperature: None,
    };

    // The provider's `complete` method is synchronous and internally
    // creates a tokio runtime via `block_on`. Calling it from within an
    // async context panics. We use `spawn_blocking` to run it on a
    // separate thread.
    let provider_clone = provider.clone();
    let completion = tokio::task::spawn_blocking(move || provider_clone.complete(&request))
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Now chunk the completion into streaming events.
    let text = completion.text;
    let chunk_size = 6usize;
    let stream = async_stream::stream! {
        let mut token_count = 0usize;
        let mut pos = 0;
        while pos < text.len() {
            let end = (pos + chunk_size).min(text.len());
            let end = text[..end].char_indices().last().map(|(i, _)| i).unwrap_or(end);
            let chunk = &text[pos..end];
            if !chunk.is_empty() {
                token_count += 1;
                yield Ok(SseEvent::default().event("message").data(
                    json!({"type": "token", "text": chunk, "token_count": token_count}).to_string()
                ));
                tokio::time::sleep(std::time::Duration::from_millis(15)).await;
            }
            pos = end;
        }
        yield Ok(SseEvent::default().event("message").data(
            json!({"type": "done", "total_tokens": token_count, "finish_reason": "stop"}).to_string()
        ));
    };

    Ok(Sse::new(stream))
}
