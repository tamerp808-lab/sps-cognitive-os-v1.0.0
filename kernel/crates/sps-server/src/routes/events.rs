//! Events endpoints (list + SSE stream).

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event as SseEvent, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::Stream;
use serde::Deserialize;
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/events", get(list_events).post(dispatch_event))
        .route("/api/events/stream", get(stream_events))
        .route("/api/verify", get(verify_chain))
        .route("/api/snapshot", post(take_snapshot))
}

#[derive(Debug, Deserialize)]
struct ListEventsQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    from: u64,
}

fn default_limit() -> usize {
    100
}

async fn list_events(
    State(state): State<Arc<ServerState>>,
    axum::extract::Query(q): axum::extract::Query<ListEventsQuery>,
) -> Json<serde_json::Value> {
    let events = state
        .kernel
        .store()
        .read_from(q.from, q.limit)
        .unwrap_or_default();
    let json_events: Vec<serde_json::Value> = events
        .iter()
        .map(|e| {
            json!({
                "tick": e.tick,
                "type": e.event_type.as_str(),
                "hash": e.hash.to_hex(),
                "prev_hash": e.prev_hash.to_hex(),
                "payload": e.payload,
                "wall_time": e.wall_time,
                "actor": {
                    "kind": serde_json::to_string(&e.actor.kind).unwrap_or_default(),
                    "id": e.actor.id.as_str(),
                },
            })
        })
        .collect();
    Json(json!({ "events": json_events, "count": json_events.len() }))
}

#[derive(Debug, Deserialize)]
struct DispatchRequest {
    event_type: String,
    payload: serde_json::Value,
}

async fn dispatch_event(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<DispatchRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use sps_core::actor::Actor;
    use sps_core::event::RawEvent;
    let wall_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let raw = RawEvent::new(req.event_type, req.payload, Actor::owner(), wall_time);
    match state.kernel.dispatch(raw) {
        Ok(event) => Ok(Json(json!({
            "tick": event.tick,
            "hash": event.hash.to_hex(),
            "type": event.event_type.as_str(),
        }))),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )),
    }
}

async fn stream_events(
    State(state): State<Arc<ServerState>>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let storage = state.kernel.store().storage().clone();
    let initial_tick = state.kernel.last_tick().unwrap_or(0);
    let last_tick = std::sync::Arc::new(parking_lot::Mutex::new(initial_tick));

    let stream = async_stream::stream! {
        loop {
            let current = *last_tick.lock();
            let from = current + 1;
            match storage.read_events_from(from, 100) {
                Ok(events) if !events.is_empty() => {
                    if let Some(last) = events.last() {
                        *last_tick.lock() = last.tick;
                    }
                    for e in &events {
                        let json = json!({
                            "tick": e.tick,
                            "type": e.event_type.as_str(),
                            "hash": e.hash.to_hex(),
                            "payload": e.payload,
                            "wall_time": e.wall_time,
                        });
                        yield Ok(SseEvent::default().event("event").data(json.to_string()));
                    }
                }
                _ => {
                    // No new events — send a heartbeat.
                    yield Ok(SseEvent::default().event("ping").data("{}"));
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }
    };

    Sse::new(stream)
}

async fn verify_chain(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let report = state.kernel.verify().unwrap_or_else(|e| {
        sps_core::replay::ReplayReport {
            events_verified: 0,
            last_tick: 0,
            last_hash: sps_core::event::EventHash::GENESIS,
            failure: Some(sps_core::replay::ReplayFailure::HashMismatch {
                tick: 0,
                stored: sps_core::event::EventHash::GENESIS,
                recomputed: sps_core::event::EventHash::GENESIS,
            }),
            elapsed_us: 0,
        }
    });
    Json(json!({
        "events_verified": report.events_verified,
        "last_tick": report.last_tick,
        "last_hash": report.last_hash.to_hex(),
        "failure": report.failure.is_some(),
        "failure_detail": format!("{:?}", report.failure),
        "elapsed_us": report.elapsed_us,
    }))
}

async fn take_snapshot(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let wall_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    match state.kernel.snapshot(wall_time) {
        Ok(snap) => Json(json!({
            "tick": snap.tick,
            "state_hash": hex::encode(snap.state_hash),
            "wall_time": snap.wall_time,
        })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
