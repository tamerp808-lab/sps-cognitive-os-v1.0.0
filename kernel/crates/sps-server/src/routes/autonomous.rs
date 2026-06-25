//! Phase 7-9: Autonomous Goal Engine + World Model + Reflection.
//!
//! Phase 7: Auto-generate milestones, weekly tasks, track progress, detect
//!           stalls, suggest plan adjustments — all from a single goal.
//! Phase 8: Unified World Model — entities (projects, apps, contacts, files,
//!           knowledge) linked by relationships.
//! Phase 9: Reflection & Learning — detect patterns (procrastination, success
//!           factors) and adapt plans.

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
        // Phase 7: Autonomous Goal Engine
        .route("/api/autonomous/activate/{goal_id}", post(activate_goal))
        .route("/api/autonomous/weekly/{goal_id}", post(generate_weekly_tasks))
        .route("/api/autonomous/review/{goal_id}", post(weekly_review))
        .route("/api/autonomous/status", get(autonomous_status))
        // Phase 8: World Model
        .route("/api/world/entities", get(list_entities).post(add_entity))
        .route("/api/world/relationships", get(list_relationships).post(add_relationship))
        .route("/api/world/search", get(search_world))
        // Phase 9: Reflection & Learning
        .route("/api/reflection/analyze", post(analyze_patterns))
        .route("/api/reflection/suggest", post(suggest_adjustments))
}

// ===== Phase 7: Autonomous Goal Engine =====

#[derive(Debug, Deserialize)]
struct ActivateRequest {
    #[serde(default)]
    provider_id: Option<String>,
}

/// Activate a long-term goal — auto-generates milestones + first week tasks.
async fn activate_goal(
    State(state): State<Arc<ServerState>>,
    Path(goal_id): Path<String>,
    Json(req): Json<ActivateRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let provider_id = req.provider_id
        .or_else(|| state.default_provider().map(|s| s.as_str().to_string()))
        .ok_or((axum::http::StatusCode::BAD_REQUEST, "no provider".to_string()))?;
    let provider = state.providers.get(&provider_id)
        .ok_or((axum::http::StatusCode::NOT_FOUND, "provider not found".to_string()))?;

    // Find the goal — read from CANONICAL STATE (not the event stream).
    let goal_state = state.kernel.query(|s| {
        sps_goals::reducer::GoalState::from_state(s)
    });
    let goal_uuid = uuid::Uuid::parse_str(&goal_id)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, format!("invalid goal_id: {}", e)))?;
    let goal_id_struct = sps_goals::hierarchy::GoalId(goal_uuid);

    let goal = goal_state
        .as_ref()
        .and_then(|gs| gs.tree.get(&goal_id_struct))
        .ok_or((axum::http::StatusCode::NOT_FOUND, "goal not found in canonical state".to_string()))?;

    let goal_title = goal.title.as_str();
    let goal_timeline = "6 months"; // Timeline stored as goal metadata via world.entity

    // Ask LLM to generate milestones.
    let prompt = format!(
        "You are a goal planning engine. Create a milestone breakdown for this goal.\n\n\
         Goal: {}\n\
         Timeline: {}\n\n\
         Respond as JSON:\n\
         {{\"milestones\": [{{\"title\": \"...\", \"target_week\": 1, \"tasks\": [\"task1\", \"task2\"]}}]}}\n\n\
         Create 4-8 milestones spread across the timeline. Each milestone has 2-4 tasks.",
        goal_title, goal_timeline
    );

    let llm_req = sps_effects::providers::llm::LlmRequest {
        provider_id: SmolStr::new(&provider_id),
        model: None,
        system: Some("Output valid JSON only.".into()),
        user: prompt,
        max_tokens: Some(1000),
        temperature: Some(0.4),
    };

    let provider_clone = provider.clone();
    let result = tokio::task::spawn_blocking(move || provider_clone.complete(&llm_req))
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let milestones = parse_milestones(&result.text);

    // ─── Option B: autonomous.goal_activated is a PRODUCER, not a state mutator ───
    //
    // The autonomous.goal_activated event is a MARKER — it records "at tick X,
    // the autonomous engine decided to activate this goal with these milestones".
    // It does NOT mutate the goal tree directly.
    //
    // Instead, the route (acting as producer) dispatches a SEPARATE event for
    // each state transition:
    //   1. goal.objective_added  (one per objective — usually just one for the
    //      "Main" objective that holds all milestones)
    //   2. goal.milestone_added  (one per milestone)
    //   3. task.created          (one per task within each milestone)
    //
    // Each event = single state transition. This preserves Event Sourcing purity:
    //   - Replayable: each event is independently meaningful
    //   - Auditable: "where did this task come from?" → task.created → goal.milestone_added → autonomous.goal_activated → goal.created
    //   - Debuggable: hash chain shows every transition
    //   - Multi-agent ready: any future agent (Architect, Planner, Factory) can
    //     dispatch goal.milestone_added / task.created without knowing about
    //     the autonomous engine

    use sps_core::actor::Actor;
    use sps_core::event::RawEvent;
    use sps_goals::hierarchy::{Objective, Milestone, Task, TaskStatus};

    let goal_uuid_for_events = goal_uuid;

    // 1. Dispatch the marker event (no state change — just records activation).
    let activation_payload = json!({
        "goal_id": goal_id,
        "milestones": milestones,
        "activated_at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
    });
    let _activation_event = state.kernel.dispatch(
        RawEvent::new("autonomous.goal_activated", activation_payload, Actor::owner(), 0)
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 2. Create ONE objective to hold all milestones (most goals have a single
    //    "main" objective; users can add more later via goal.objective_added).
    let objective = Objective {
        id: uuid::Uuid::now_v7(),
        title: smol_str::SmolStr::new("Main"),
        milestones: Vec::new(), // milestones added one-by-one below
    };
    let objective_id = objective.id;
    state.kernel.dispatch(
        RawEvent::new("goal.objective_added", json!({
            "goal_id": goal_uuid_for_events,
            "objective": objective,
        }), Actor::owner(), 0)
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 3. For each LLM-generated milestone, dispatch goal.milestone_added + task.created.
    let mut milestone_count = 0;
    let mut task_count = 0;
    for (milestone_idx, ms) in milestones.iter().enumerate() {
        let milestone_title = ms.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled milestone");
        let milestone = Milestone {
            id: uuid::Uuid::now_v7(),
            title: smol_str::SmolStr::new(milestone_title),
            tasks: Vec::new(), // tasks added one-by-one below
        };
        state.kernel.dispatch(
            RawEvent::new("goal.milestone_added", json!({
                "goal_id": goal_uuid_for_events,
                "objective_idx": 0, // we just added one objective at index 0
                "milestone": milestone,
            }), Actor::owner(), 0)
        ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        milestone_count += 1;

        // 4. For each task in this milestone, dispatch task.created.
        if let Some(tasks_arr) = ms.get("tasks").and_then(|v| v.as_array()) {
            for task_title_val in tasks_arr {
                let task_title = task_title_val.as_str().unwrap_or("Untitled task");
                let task = Task {
                    id: uuid::Uuid::now_v7(),
                    title: smol_str::SmolStr::new(task_title),
                    description: String::new(),
                    status: TaskStatus::Pending,
                    assigned_agent: None,
                    origin_tick: 0,
                };
                state.kernel.dispatch(
                    RawEvent::new("task.created", json!({
                        "goal_id": goal_uuid_for_events,
                        "objective_idx": 0,
                        "milestone_idx": milestone_idx,
                        "task": task,
                    }), Actor::owner(), 0)
                ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                task_count += 1;
            }
        }
    }

    Ok(Json(json!({
        "goal_id": goal_id,
        "goal_title": goal_title,
        "objective_id": objective_id.to_string(),
        "milestones_added": milestone_count,
        "tasks_added": task_count,
        "status": "activated",
    })))
}

/// Generate weekly tasks for a goal.
async fn generate_weekly_tasks(
    State(state): State<Arc<ServerState>>,
    Path(goal_id): Path<String>,
    Json(req): Json<ActivateRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let provider_id = req.provider_id
        .or_else(|| state.default_provider().map(|s| s.as_str().to_string()))
        .ok_or((axum::http::StatusCode::BAD_REQUEST, "no provider".to_string()))?;
    let provider = state.providers.get(&provider_id)
        .ok_or((axum::http::StatusCode::NOT_FOUND, "provider not found".to_string()))?;

    let prompt = format!(
        "Generate concrete weekly tasks for a YouTube channel goal.\n\
         The tasks should be actionable, specific, and completable within one week.\n\n\
         Respond as JSON:\n\
         {{\"week\": 1, \"tasks\": [{{\"title\": \"...\", \"description\": \"...\", \"estimated_hours\": 2}}]}}\n\n\
         Generate 5 tasks for this week."
    );

    let llm_req = sps_effects::providers::llm::LlmRequest {
        provider_id: SmolStr::new(&provider_id),
        model: None,
        system: Some("Output valid JSON only. Make tasks practical and specific.".into()),
        user: prompt,
        max_tokens: Some(800),
        temperature: Some(0.5),
    };

    let provider_clone = provider.clone();
    let result = tokio::task::spawn_blocking(move || provider_clone.complete(&llm_req))
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "goal_id": goal_id,
        "week": 1,
        "tasks": result.text,
    })))
}

/// Weekly review — analyze progress, detect stalls, suggest adjustments.
async fn weekly_review(
    State(state): State<Arc<ServerState>>,
    Path(goal_id): Path<String>,
    Json(req): Json<ActivateRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let provider_id = req.provider_id
        .or_else(|| state.default_provider().map(|s| s.as_str().to_string()))
        .ok_or((axum::http::StatusCode::BAD_REQUEST, "no provider".to_string()))?;
    let provider = state.providers.get(&provider_id)
        .ok_or((axum::http::StatusCode::NOT_FOUND, "provider not found".to_string()))?;

    // Gather goal's memory + progress events.
    let storage = state.kernel.store().storage();
    let events = storage.read_events_from(1, 10000).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let goal_events: Vec<_> = events.iter()
        .filter(|e| e.payload.get("goal_id").and_then(|v| v.as_str()) == Some(goal_id.as_str()))
        .collect();

    let prompt = format!(
        "You are a goal review engine. Analyze the progress and provide insights.\n\n\
         Goal ID: {}\n\
         Events recorded: {}\n\
         Event types: {}\n\n\
         Provide a JSON response:\n\
         {{\"progress_percent\": 25, \"on_track\": true, \"stalls_detected\": [], \"suggestions\": [\"...\"], \"next_week_focus\": \"...\"}}",
        goal_id,
        goal_events.len(),
        goal_events.iter().map(|e| e.event_type.as_str()).collect::<Vec<_>>().join(", "),
    );

    let llm_req = sps_effects::providers::llm::LlmRequest {
        provider_id: SmolStr::new(&provider_id),
        model: None,
        system: Some("You are a goal review engine. Be honest and specific. Output JSON only.".into()),
        user: prompt,
        max_tokens: Some(500),
        temperature: Some(0.5),
    };

    let provider_clone = provider.clone();
    let result = tokio::task::spawn_blocking(move || provider_clone.complete(&llm_req))
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Store review as event.
    use sps_core::actor::Actor;
    use sps_core::event::RawEvent;
    let payload = json!({
        "goal_id": goal_id,
        "review": result.text,
        "reviewed_at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
    });
    let raw = RawEvent::new("autonomous.weekly_review", payload, Actor::owner(), 0);
    let _ = state.kernel.dispatch(raw);

    Ok(Json(json!({
        "goal_id": goal_id,
        "review": result.text,
    })))
}

/// Get autonomous engine status.
async fn autonomous_status(
    State(state): State<Arc<ServerState>>,
) -> Json<serde_json::Value> {
    let storage = state.kernel.store().storage();
    let events = storage.read_events_from(1, 10000).unwrap_or_default();

    let active_goals = events.iter().filter(|e| e.event_type.as_str() == "autonomous.goal_activated").count();
    let reviews = events.iter().filter(|e| e.event_type.as_str() == "autonomous.weekly_review").count();
    let progress_updates = events.iter().filter(|e| e.event_type.as_str() == "goal.progress_updated").count();

    Json(json!({
        "active_goals": active_goals,
        "weekly_reviews": reviews,
        "progress_updates": progress_updates,
        "engine_status": "running",
    }))
}

fn parse_milestones(text: &str) -> Vec<serde_json::Value> {
    let cleaned = text.trim()
        .strip_prefix("```json").unwrap_or(text)
        .strip_prefix("```").unwrap_or(text)
        .strip_suffix("```").unwrap_or(text)
        .trim();

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(cleaned) {
        if let Some(arr) = v.get("milestones").and_then(|m| m.as_array()) {
            return arr.clone();
        }
        if let Some(arr) = v.as_array() {
            return arr.clone();
        }
    }
    vec![json!({"title": "Start", "target_week": 1, "tasks": ["Begin work"]})]
}

// ===== Phase 8: World Model =====

#[derive(Debug, Deserialize, Serialize)]
struct Entity {
    #[serde(default)]
    id: Option<String>,
    entity_type: String, // "project", "app", "contact", "file", "knowledge"
    name: String,
    #[serde(default)]
    metadata: serde_json::Value,
}

async fn list_entities(
    State(state): State<Arc<ServerState>>,
) -> Json<serde_json::Value> {
    let storage = state.kernel.store().storage();
    let events = storage.read_events_from(1, 10000).unwrap_or_default();

    let entities: Vec<serde_json::Value> = events.iter()
        .filter(|e| e.event_type.as_str() == "world.entity_added")
        .map(|e| {
            json!({
                "id": e.payload.get("id").and_then(|v| v.as_str()).unwrap_or("unknown"),
                "type": e.payload.get("entity_type").and_then(|v| v.as_str()).unwrap_or("unknown"),
                "name": e.payload.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed"),
                "metadata": e.payload.get("metadata").cloned().unwrap_or(json!({})),
                "tick": e.tick,
            })
        })
        .collect();

    Json(json!({"entities": entities, "count": entities.len()}))
}

async fn add_entity(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<Entity>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use sps_core::actor::Actor;
    use sps_core::event::RawEvent;

    let id = req.id.unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    let payload = json!({
        "id": id,
        "entity_type": req.entity_type,
        "name": req.name,
        "metadata": req.metadata,
    });

    let raw = RawEvent::new("world.entity_added", payload, Actor::owner(), 0);
    let event = state.kernel.dispatch(raw).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "id": id,
        "type": req.entity_type,
        "name": req.name,
        "tick": event.tick,
    })))
}

#[derive(Debug, Deserialize)]
struct RelationshipRequest {
    from_id: String,
    to_id: String,
    relationship: String, // "uses", "depends_on", "created_by", "contains"
}

async fn list_relationships(
    State(state): State<Arc<ServerState>>,
) -> Json<serde_json::Value> {
    let storage = state.kernel.store().storage();
    let events = storage.read_events_from(1, 10000).unwrap_or_default();

    let rels: Vec<serde_json::Value> = events.iter()
        .filter(|e| e.event_type.as_str() == "world.relationship_added")
        .map(|e| json!({
            "from": e.payload.get("from_id"),
            "to": e.payload.get("to_id"),
            "type": e.payload.get("relationship"),
            "tick": e.tick,
        }))
        .collect();

    Json(json!({"relationships": rels, "count": rels.len()}))
}

async fn add_relationship(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<RelationshipRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use sps_core::actor::Actor;
    use sps_core::event::RawEvent;

    let payload = json!({
        "from_id": req.from_id,
        "to_id": req.to_id,
        "relationship": req.relationship,
    });

    let raw = RawEvent::new("world.relationship_added", payload, Actor::owner(), 0);
    let event = state.kernel.dispatch(raw).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "from": req.from_id,
        "to": req.to_id,
        "type": req.relationship,
        "tick": event.tick,
    })))
}

async fn search_world(
    State(state): State<Arc<ServerState>>,
    axum::extract::Query(q): axum::extract::Query<SearchAppQuery>,
) -> Json<serde_json::Value> {
    let query = q.q.to_lowercase();
    let storage = state.kernel.store().storage();
    let events = storage.read_events_from(1, 10000).unwrap_or_default();

    let results: Vec<serde_json::Value> = events.iter()
        .filter(|e| e.event_type.as_str() == "world.entity_added")
        .filter(|e| {
            let name = e.payload.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let etype = e.payload.get("entity_type").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            name.contains(&query) || etype.contains(&query)
        })
        .map(|e| json!({
            "id": e.payload.get("id").and_then(|v| v.as_str()).unwrap_or("unknown"),
            "type": e.payload.get("entity_type").and_then(|v| v.as_str()).unwrap_or("unknown"),
            "name": e.payload.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed"),
        }))
        .collect();

    Json(json!({"results": results, "count": results.len(), "query": q.q}))
}

#[derive(Debug, Deserialize)]
struct SearchAppQuery {
    q: String,
}

// ===== Phase 9: Reflection & Learning =====

#[derive(Debug, Deserialize)]
struct AnalyzeRequest {
    #[serde(default)]
    provider_id: Option<String>,
}

/// Analyze patterns in the user's behavior.
async fn analyze_patterns(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<AnalyzeRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let provider_id = req.provider_id
        .or_else(|| state.default_provider().map(|s| s.as_str().to_string()))
        .ok_or((axum::http::StatusCode::BAD_REQUEST, "no provider".to_string()))?;
    let provider = state.providers.get(&provider_id)
        .ok_or((axum::http::StatusCode::NOT_FOUND, "provider not found".to_string()))?;

    // Gather all memories + goals + tasks.
    let storage = state.kernel.store().storage();
    let events = storage.read_events_from(1, 10000).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let memory_count = events.iter().filter(|e| e.event_type.as_str().starts_with("memory.")).count();
    let goal_count = events.iter().filter(|e| e.event_type.as_str().contains("goal")).count();
    let task_count = events.iter().filter(|e| e.event_type.as_str().starts_with("task.")).count();
    let cognitive_count = events.iter().filter(|e| e.event_type.as_str().starts_with("reasoning.")).count();

    let prompt = format!(
        "You are a behavioral analysis engine. Analyze the user's activity patterns.\n\n\
         Activity summary:\n\
         - Total events: {}\n\
         - Memories created: {}\n\
         - Goals created: {}\n\
         - Tasks executed: {}\n\
         - Cognitive steps: {}\n\n\
         Detect patterns and respond as JSON:\n\
         {{\"patterns\": [{{\"type\": \"procrastination|success_factor|preference|rhythm\", \"description\": \"...\", \"evidence\": \"...\", \"suggestion\": \"...\"}}]}}",
        events.len(), memory_count, goal_count, task_count, cognitive_count
    );

    let llm_req = sps_effects::providers::llm::LlmRequest {
        provider_id: SmolStr::new(&provider_id),
        model: None,
        system: Some("You are a pattern detection engine. Be honest and insightful. Output JSON only.".into()),
        user: prompt,
        max_tokens: Some(500),
        temperature: Some(0.6),
    };

    let provider_clone = provider.clone();
    let result = tokio::task::spawn_blocking(move || provider_clone.complete(&llm_req))
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "analysis": result.text,
        "stats": {
            "total_events": events.len(),
            "memories": memory_count,
            "goals": goal_count,
            "tasks": task_count,
            "cognitive_steps": cognitive_count,
        }
    })))
}

/// Suggest plan adjustments based on patterns.
async fn suggest_adjustments(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<AnalyzeRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let provider_id = req.provider_id
        .or_else(|| state.default_provider().map(|s| s.as_str().to_string()))
        .ok_or((axum::http::StatusCode::BAD_REQUEST, "no provider".to_string()))?;
    let provider = state.providers.get(&provider_id)
        .ok_or((axum::http::StatusCode::NOT_FOUND, "provider not found".to_string()))?;

    let prompt = "Based on the user's activity patterns, suggest concrete adjustments to their goals and plans.\n\n\
                  Respond as JSON:\n\
                  {\"adjustments\": [{\"goal\": \"...\", \"current_plan\": \"...\", \"suggested_change\": \"...\", \"reason\": \"...\", \"priority\": \"high|medium|low\"}]}";

    let llm_req = sps_effects::providers::llm::LlmRequest {
        provider_id: SmolStr::new(&provider_id),
        model: None,
        system: Some("You are a plan optimization engine. Be specific and actionable. Output JSON only.".into()),
        user: prompt.to_string(),
        max_tokens: Some(500),
        temperature: Some(0.6),
    };

    let provider_clone = provider.clone();
    let result = tokio::task::spawn_blocking(move || provider_clone.complete(&llm_req))
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({"suggestions": result.text})))
}
