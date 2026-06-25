//! Conversation endpoints — create, list, message history.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/conversations", get(list_conversations).post(create_conversation))
        .route("/api/conversations/{id}", get(get_conversation).delete(delete_conversation))
        .route("/api/conversations/{id}/messages", post(send_message))
}

#[derive(Debug, Deserialize)]
struct CreateRequest {
    provider_id: String,
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Serialize)]
struct ConversationInfo {
    id: String,
    title: String,
    provider_id: String,
    model: Option<String>,
    message_count: usize,
    created_at: u64,
    updated_at: u64,
}

async fn list_conversations(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let conversations = state.conversations.read();
    let mut convs: Vec<ConversationInfo> = conversations
        .values()
        .map(|(_, c)| ConversationInfo {
            id: c.id.to_string(),
            title: c.title.as_str().to_string(),
            provider_id: c.provider_id.as_str().to_string(),
            model: c.model.as_ref().map(|m| m.as_str().to_string()),
            message_count: c.messages.len(),
            created_at: c.created_at,
            updated_at: c.updated_at,
        })
        .collect();
    convs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Json(json!({ "conversations": convs, "count": convs.len() }))
}

async fn create_conversation(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<CreateRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let engine = sps_llm::conversation::ConversationEngine::new();
    let id = engine.create(
        SmolStr::new(&req.provider_id),
        req.system_prompt.unwrap_or_else(|| "You are a helpful AI assistant.".to_string()),
    );
    // Store the engine in the server state. Since ConversationEngine
    // doesn't implement Clone, we create a new one per request and
    // store it. This is a simplification — production would share one
    // engine across requests.
    let conv = engine.get(&id).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Store the engine in a global map keyed by conversation id.
    // For simplicity, we store the conversation directly in the server state.
    state.conversations.write().insert(id, (engine, conv.clone()));
    Ok(Json(json!({
        "id": id.to_string(),
        "title": conv.title,
        "provider_id": conv.provider_id,
        "message_count": conv.messages.len(),
    })))
}

async fn get_conversation(
    State(state): State<Arc<ServerState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let uuid = uuid::Uuid::parse_str(&id)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
    let conv_id = sps_llm::conversation::ConversationId(uuid);
    let conversations = state.conversations.read();
    let (_, conv) = conversations
        .get(&conv_id)
        .ok_or((axum::http::StatusCode::NOT_FOUND, "conversation not found".into()))?;
    let messages: Vec<serde_json::Value> = conv
        .messages
        .iter()
        .map(|m| {
            json!({
                "role": format!("{:?}", m.role).to_lowercase(),
                "content": m.content,
                "wall_time": m.wall_time,
                "token_count": m.token_count,
            })
        })
        .collect();
    Ok(Json(json!({
        "id": conv.id.to_string(),
        "title": conv.title,
        "provider_id": conv.provider_id,
        "model": conv.model,
        "messages": messages,
        "total_tokens": conv.total_tokens(),
        "context_window": {
            "max_tokens": conv.context_window.max_tokens,
            "keep_system": conv.context_window.keep_system,
            "keep_recent": conv.context_window.keep_recent,
        },
    })))
}

async fn delete_conversation(
    State(state): State<Arc<ServerState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let uuid = uuid::Uuid::parse_str(&id)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
    let conv_id = sps_llm::conversation::ConversationId(uuid);
    let removed = state.conversations.write().remove(&conv_id).is_some();
    Ok(Json(json!({ "removed": removed, "id": id })))
}

#[derive(Debug, Deserialize)]
struct SendMessageRequest {
    content: String,
}

async fn send_message(
    State(state): State<Arc<ServerState>>,
    Path(id): Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let uuid = uuid::Uuid::parse_str(&id)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
    let conv_id = sps_llm::conversation::ConversationId(uuid);

    // Get conversation + provider.
    let (provider_id, request) = {
        let conversations = state.conversations.read();
        let (_, conv) = conversations
            .get(&conv_id)
            .ok_or((axum::http::StatusCode::NOT_FOUND, "conversation not found".into()))?;
        // Add the user message.
        let mut conv = conv.clone();
        conv.add_user(&req.content);
        conv.truncate();
        let request = conv.to_request();
        (conv.provider_id.clone(), request)
    };

    // Get the provider.
    let provider = state
        .providers
        .get(provider_id.as_str())
        .ok_or((
            axum::http::StatusCode::BAD_REQUEST,
            format!("provider {} not registered", provider_id),
        ))?;

    // Call the LLM.
    let completion = provider
        .complete(&request)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Save the assistant response.
    {
        let mut conversations = state.conversations.write();
        if let Some((engine, conv)) = conversations.get_mut(&conv_id) {
            // Re-add user message + assistant message via the engine.
            // Since we cloned conv above, we need to update the stored one.
            conv.add_user(&req.content);
            conv.add_assistant(&completion.text);
            conv.truncate();
            let _ = engine; // engine reference (unused — we update conv directly)
        }
    }

    Ok(Json(json!({
        "assistant_message": completion.text,
        "model": completion.model,
        "usage": {
            "prompt_tokens": completion.usage.prompt_tokens,
            "completion_tokens": completion.usage.completion_tokens,
            "total_tokens": completion.usage.total_tokens,
        },
        "elapsed_ms": completion.elapsed_ms,
    })))
}
