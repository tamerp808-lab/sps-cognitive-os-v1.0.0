//! Goal endpoints.

use std::sync::Arc;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/goals", get(list_goals))
        .route("/api/goals/{id}/verify", get(verify_goal))
}

async fn list_goals(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let goals = state.kernel.query(|s| {
        sps_goals::reducer::GoalState::from_state(s)
            .map(|gs| {
                gs.tree
                    .goals
                    .values()
                    .map(|g| {
                        let tasks: u32 = g
                            .objectives
                            .iter()
                            .flat_map(|o| &o.milestones)
                            .map(|m| m.tasks.len() as u32)
                            .sum();
                        json!({
                            "id": g.id.to_string(),
                            "title": g.title.as_str(),
                            "description": g.description,
                            "status": format!("{:?}", g.status).to_lowercase(),
                            "priority": g.priority,
                            "tasks_total": tasks,
                            "objectives_count": g.objectives.len(),
                            "created_at": g.created_at,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    Json(json!({ "goals": goals, "count": goals.len() }))
}

async fn verify_goal(
    State(state): State<Arc<ServerState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(e) => {
            return Json(json!({ "error": format!("invalid goal id: {}", e) }));
        }
    };
    let result = state.kernel.query(|s| {
        sps_goals::reducer::GoalState::from_state(s)
            .and_then(|gs| {
                let goal_id = sps_goals::hierarchy::GoalId(uuid);
                let tree = gs.tree;
                let result = tree.verify(&goal_id);
                Some(json!({
                    "goal_id": id,
                    "verified": result.verified,
                    "tasks_total": result.tasks_total,
                    "tasks_completed": result.tasks_completed,
                    "reason": result.reason,
                }))
            })
            .unwrap_or(json!({ "error": "goal not found" }))
    });
    Json(result)
}
