//! State + stats endpoints.

use std::sync::Arc;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/state", get(get_state))
        .route("/api/stats", get(get_stats))
}

async fn get_state(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let json = state.kernel.query(|s| serde_json::to_value(s).unwrap_or(json!({})));
    Json(json)
}

async fn get_stats(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let last_tick = state.kernel.last_tick().unwrap_or(0);
    let event_count = state.kernel.event_count().unwrap_or(0);
    let last_hash = state
        .kernel
        .last_hash()
        .map(|h| h.to_hex())
        .unwrap_or_default();

    // Memory stats.
    let memory_stats = state.kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| {
                let stats = sps_memory::stats::MemoryStats::from_graph(&ms.graph);
                json!({
                    "total": stats.total,
                    "by_kind": stats.by_kind,
                    "links": stats.links,
                    "avg_strength": stats.avg_strength,
                })
            })
            .unwrap_or(json!({}))
    });

    // Goal stats.
    let goal_stats = state.kernel.query(|s| {
        sps_goals::reducer::GoalState::from_state(s)
            .map(|gs| {
                json!({
                    "total": gs.tree.goals.len(),
                    "active": gs.tree.active().len(),
                    "total_tasks": gs.tree.total_tasks(),
                    "completed_tasks": gs.tree.completed_tasks(),
                })
            })
            .unwrap_or(json!({}))
    });

    Json(json!({
        "kernel": {
            "backend": state.kernel.backend_name(),
            "last_tick": last_tick,
            "event_count": event_count,
            "last_hash": last_hash,
        },
        "memory": memory_stats,
        "goals": goal_stats,
        "providers": state.providers.list(),
        "default_provider": state.default_provider(),
    }))
}
