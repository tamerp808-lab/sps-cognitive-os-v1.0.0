//! Memory endpoints.

use std::sync::Arc;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/memory", get(memory_stats))
        .route("/api/memory/search", get(memory_search))
}

async fn memory_stats(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let stats = state.kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| {
                let stats = sps_memory::stats::MemoryStats::from_graph(&ms.graph);
                json!({
                    "total": stats.total,
                    "by_kind": stats.by_kind,
                    "links": stats.links,
                    "avg_strength": stats.avg_strength,
                    "recent_memories": ms.graph.memories.values().rev().take(20).map(|m| {
                        json!({
                            "id": m.id.to_string(),
                            "kind": m.kind.as_str(),
                            "title": m.title.as_str(),
                            "strength": m.strength.0,
                            "access_count": m.access_count,
                            "created_at": m.created_at,
                            "tags": m.tags,
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .unwrap_or(json!({ "total": 0 }))
    });
    Json(stats)
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    20
}

async fn memory_search(
    State(state): State<Arc<ServerState>>,
    Query(q): Query<SearchQuery>,
) -> Json<serde_json::Value> {
    let results = state.kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| {
                ms.graph
                    .search(&q.q, q.limit)
                    .into_iter()
                    .map(|m| {
                        json!({
                            "id": m.id.to_string(),
                            "kind": m.kind.as_str(),
                            "title": m.title.as_str(),
                            "content": m.content,
                            "strength": m.strength.0,
                            "access_count": m.access_count,
                            "tags": m.tags,
                            "created_at": m.created_at,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    Json(json!({ "query": q.q, "results": results, "count": results.len() }))
}
