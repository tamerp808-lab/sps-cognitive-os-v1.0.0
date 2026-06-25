//! Agent endpoints.

use std::sync::Arc;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use smol_str::SmolStr;
use sps_agents::agent::AgentArchetype;
use sps_agents::runtime::AgentRuntime;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/agents", get(list_agents))
        .route("/api/agents/dispatch", post(dispatch_agent))
}

async fn list_agents(State(_state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let runtime = AgentRuntime::default();
    let agents = runtime.register_builtins();
    let agents_json: Vec<serde_json::Value> = agents
        .iter()
        .map(|id| {
            let agent = runtime.get(id).unwrap();
            json!({
                "id": agent.id.to_string(),
                "archetype": agent.archetype.to_string(),
                "name": agent.name.as_str(),
                "capabilities": {
                    "can_read_files": agent.capabilities.can_read_files,
                    "can_write_files": agent.capabilities.can_write_files,
                    "can_exec_shell": agent.capabilities.can_exec_shell,
                    "can_call_llm": agent.capabilities.can_call_llm,
                    "can_delegate": agent.capabilities.can_delegate,
                    "can_create_goals": agent.capabilities.can_create_goals,
                },
                "system_prompt": agent.system_prompt,
            })
        })
        .collect();
    Json(json!({ "agents": agents_json, "count": agents_json.len() }))
}

#[derive(Debug, Deserialize)]
struct DispatchRequest {
    archetype: String,
    title: String,
    description: String,
}

async fn dispatch_agent(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<DispatchRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let archetype: AgentArchetype = req
        .archetype
        .parse()
        .map_err(|e: String| (axum::http::StatusCode::BAD_REQUEST, e))?;
    let runtime = AgentRuntime::default();
    runtime.register_builtins();
    let wall_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let result = runtime
        .dispatch(archetype, &req.title, &req.description, 0, wall_time)
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            format!("no agent of archetype {} registered", archetype),
        ))?;
    Ok(Json(json!({
        "agent_id": result.agent_id.to_string(),
        "task_id": result.task_id.to_string(),
        "messages": result.messages.iter().map(|m| {
            json!({
                "id": m.id.to_string(),
                "from": m.from.to_string(),
                "to": m.to.map(|t| t.to_string()),
                "kind": format!("{:?}", m.kind).to_lowercase(),
                "subject": m.subject.as_str(),
                "body": m.body,
            })
        }).collect::<Vec<_>>(),
    })))
}

// Suppress unused import warning.
#[allow(dead_code)]
fn _use_smolstr(s: SmolStr) -> SmolStr {
    s
}
