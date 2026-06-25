//! Health endpoint.

use std::sync::Arc;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new().route("/api/health", get(health))
}

async fn health(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "kernel_booted": state.kernel.is_booted(),
        "backend": state.kernel.backend_name(),
        "last_tick": state.kernel.last_tick().unwrap_or(0),
        "event_count": state.kernel.event_count().unwrap_or(0),
        "providers": state.providers.list(),
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    }))
}
