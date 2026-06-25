//! Android Companion endpoints.
//!
//! Fix #2 / E3: Thin HTTP access layer for goal lifecycle + heartbeat.
//! All business logic lives in the AutonomyReducer (kernel side). This
//! route is a pure HTTP → EventSink translation.
//!
//! Endpoints:
//!   POST /api/companion/goal/activate    → start_with_sink
//!   POST /api/companion/goal/deactivate  → stop_with_sink
//!   GET  /api/companion/active           → query AutonomyState
//!   POST /api/companion/heartbeat        → dispatch autonomous.weekly_review
//!   GET  /api/companion/status           → query AutonomyGovernor config
//!
//! Idempotency: activate is safe to retry (latest-wins on the reducer side).
//! deactivate is also safe (no-op if goal wasn't active).

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/companion/goal/activate", post(activate_goal))
        .route("/api/companion/goal/deactivate", post(deactivate_goal))
        .route("/api/companion/goal/{id}/deactivate", post(deactivate_goal_by_id))
        .route("/api/companion/active", get(list_active_goals))
        .route("/api/companion/status", get(autonomy_status))
        .route("/api/companion/heartbeat", post(heartbeat))
}

// ──────────────────────────────────────────────────────────────────────────
// Activate
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ActivateRequest {
    /// Goal UUID (must already exist in GoalState; the companion app
    /// typically lists goals via GET /api/goals first).
    goal_id: String,
    /// Optional milestones payload (free-form JSON; the kernel stores
    /// it verbatim). Useful for the companion to attach a plan.
    #[serde(default)]
    milestones: serde_json::Value,
}

async fn activate_goal(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<ActivateRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let goal_uuid = match uuid::Uuid::parse_str(&req.goal_id) {
        Ok(u) => u,
        Err(e) => {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                format!("invalid goal_id: {}", e),
            ));
        }
    };
    let goal_id = sps_goals::hierarchy::GoalId(goal_uuid);

    let wall_time = now_ms();
    let result = state.goal_runner.start_with_sink(
        goal_id,
        req.milestones.clone(),
        state.kernel.as_ref(),
        wall_time,
    );

    match result {
        Ok(()) => {
            // Read back the freshly materialized AutonomyState.
            let activation = state.kernel.query(|s| {
                sps_autonomy::reducer::AutonomyState::from_state(s)
                    .and_then(|a| a.active_goals.get(&goal_uuid).cloned())
            });
            Ok(Json(json!({
                "ok": true,
                "goal_id": req.goal_id,
                "activated_at": wall_time,
                "activation": activation,
            })))
        }
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("activation failed: {}", e),
        )),
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Deactivate (by body)
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DeactivateRequest {
    goal_id: String,
}

async fn deactivate_goal(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<DeactivateRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    deactivate_impl(&state, &req.goal_id)
}

async fn deactivate_goal_by_id(
    State(state): State<Arc<ServerState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    deactivate_impl(&state, &id)
}

fn deactivate_impl(
    state: &Arc<ServerState>,
    goal_id_str: &str,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let goal_uuid = match uuid::Uuid::parse_str(goal_id_str) {
        Ok(u) => u,
        Err(e) => {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                format!("invalid goal_id: {}", e),
            ));
        }
    };
    let goal_id = sps_goals::hierarchy::GoalId(goal_uuid);
    let wall_time = now_ms();

    match state
        .goal_runner
        .stop_with_sink(goal_id, state.kernel.as_ref(), wall_time)
    {
        Ok(()) => Ok(Json(json!({
            "ok": true,
            "goal_id": goal_id_str,
            "deactivated_at": wall_time,
        }))),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("deactivation failed: {}", e),
        )),
    }
}

// ──────────────────────────────────────────────────────────────────────────
// List active goals (from authoritative AutonomyState)
// ──────────────────────────────────────────────────────────────────────────

async fn list_active_goals(
    State(state): State<Arc<ServerState>>,
) -> Json<serde_json::Value> {
    let goals = state.kernel.query(|s| {
        sps_autonomy::reducer::AutonomyState::from_state(s)
            .map(|a| {
                a.active_goals
                    .values()
                    .map(|act| {
                        json!({
                            "goal_id": act.goal_id.to_string(),
                            "milestones": act.milestones,
                            "activated_at": act.activated_at,
                            "origin_tick": act.origin_tick,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    Json(json!({
        "active_goals": goals,
        "count": goals.len(),
    }))
}

// ──────────────────────────────────────────────────────────────────────────
// Autonomy status (config + state)
// ──────────────────────────────────────────────────────────────────────────

async fn autonomy_status(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let cfg = state.autonomy_governor.config();
    let active_count = state.kernel.query(|s| {
        sps_autonomy::reducer::AutonomyState::from_state(s)
            .map(|a| a.active_goals.len())
            .unwrap_or(0)
    });
    Json(json!({
        "status": format!("{:?}", cfg.status).to_lowercase(),
        "max_concurrent_goals": cfg.max_concurrent_goals,
        "sandbox_paths": cfg.sandbox_paths,
        "max_run_time_ms": cfg.max_run_time_ms,
        "active_count": active_count,
    }))
}

// ──────────────────────────────────────────────────────────────────────────
// Heartbeat (dispatch weekly_review event)
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct HeartbeatRequest {
    /// Goal UUID the heartbeat is reporting on.
    goal_id: String,
    /// Free-form review text (the companion app's weekly summary).
    #[serde(default)]
    review: String,
}

async fn heartbeat(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let goal_uuid = match uuid::Uuid::parse_str(&req.goal_id) {
        Ok(u) => u,
        Err(e) => {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                format!("invalid goal_id: {}", e),
            ));
        }
    };

    let wall_time = now_ms();
    let payload = json!({
        "goal_id": goal_uuid.to_string(),
        "review": req.review,
        "reviewed_at": wall_time,
    });
    let raw = sps_core::event::RawEvent::new(
        "autonomous.weekly_review",
        payload,
        sps_core::actor::Actor::system("companion"),
        wall_time,
    );

    match state.kernel.dispatch_trusted(raw) {
        Ok(event) => Ok(Json(json!({
            "ok": true,
            "tick": event.tick,
            "hash": event.hash.to_hex(),
            "reviewed_at": wall_time,
        }))),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("heartbeat dispatch failed: {}", e),
        )),
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_request_parses_uuid() {
        let req = ActivateRequest {
            goal_id: "019ef48a-1af4-7613-be19-ab0ccac6efa9".into(),
            milestones: json!({"steps": ["a", "b"]}),
        };
        assert!(uuid::Uuid::parse_str(&req.goal_id).is_ok());
    }

    #[test]
    fn activate_request_rejects_bad_uuid() {
        let req = ActivateRequest {
            goal_id: "not-a-uuid".into(),
            milestones: json!({}),
        };
        assert!(uuid::Uuid::parse_str(&req.goal_id).is_err());
    }
}
