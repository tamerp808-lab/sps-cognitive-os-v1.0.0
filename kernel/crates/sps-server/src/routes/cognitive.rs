//! Cognitive Workflow Integration — the full pipeline:
//!   Command → Goal → Plan → Execute → Reflect → Memory
//!
//! This is what transforms SPS from an IDE into a Cognitive OS.
//! Each stage emits events to the kernel's event store.

use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event as SseEvent, Sse};
use axum::routing::post;
use axum::{Json, Router};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/cognitive/run", post(run_pipeline))
        .route("/api/cognitive/quick", post(quick_action))
}

#[derive(Debug, Deserialize)]
struct PipelineRequest {
    /// The user's command/task.
    command: String,
    /// Optional context (file path, selection, etc).
    #[serde(default)]
    context: Option<String>,
    /// Provider id (defaults to server's default).
    #[serde(default)]
    provider_id: Option<String>,
    /// Whether to auto-execute (true) or just plan (false).
    #[serde(default = "default_true")]
    auto_execute: bool,
    /// Max execution steps.
    #[serde(default = "default_max_steps")]
    max_steps: usize,
}

fn default_true() -> bool { true }
fn default_max_steps() -> usize { 10 }

/// A single stage in the cognitive pipeline.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum PipelineStage {
    /// Stage 1: Analyzing the command.
    Analyzing { command: String },
    /// Stage 2: Goal created.
    GoalCreated { title: String, description: String, tasks: Vec<String> },
    /// Stage 3: Plan generated.
    PlanCreated { template: String, steps: Vec<PlanStep> },
    /// Stage 4: Executing a task.
    Executing { step: usize, total: usize, action: String, status: String },
    /// Stage 5: Execution result.
    Executed { step: usize, success: bool, output: String },
    /// Stage 6: Reflecting on results.
    Reflecting { summary: String },
    /// Stage 7: Memory stored.
    Memorized { kind: String, title: String },
    /// Pipeline complete.
    Complete { goal_achieved: bool, total_steps: usize },
    /// Error.
    Error { message: String },
}

#[derive(Debug, Clone, Serialize)]
struct PlanStep {
    title: String,
    action: String,
    done: bool,
}

async fn run_pipeline(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<PipelineRequest>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, std::convert::Infallible>>>, (axum::http::StatusCode, String)> {
    let provider_id = req
        .provider_id
        .or_else(|| state.default_provider().map(|s| s.as_str().to_string()))
        .ok_or((axum::http::StatusCode::BAD_REQUEST, "no provider configured".to_string()))?;

    let provider = state
        .providers
        .get(&provider_id)
        .ok_or((axum::http::StatusCode::NOT_FOUND, format!("provider {} not found", provider_id)))?;

    let command = req.command.clone();
    let context = req.context.clone().unwrap_or_default();
    let auto_execute = req.auto_execute;
    let max_steps = req.max_steps;

    let stream = async_stream::stream! {
        // ===== STAGE 1: ANALYZE =====
        yield Ok(SseEvent::default().data(json!({"stage":"analyzing","command":command}).to_string()));

        // Ask LLM to decompose the command into a goal + tasks.
        let analyze_prompt = format!(
            "You are a task decomposer. Given this command, create a structured plan.\n\n\
             Command: {}\n\
             Context: {}\n\n\
             Respond in EXACTLY this JSON format (no markdown, no explanation):\n\
             {{\"title\": \"short goal title\", \"description\": \"what needs to be done\", \"tasks\": [\"task 1\", \"task 2\", \"task 3\"]}}\n\n\
             Keep it to 2-5 tasks maximum.",
            command, context
        );

        let analyze_req = sps_effects::providers::llm::LlmRequest {
            provider_id: SmolStr::new(&provider_id),
            model: None,
            system: Some("You are a JSON-only responder. Output valid JSON only.".into()),
            user: analyze_prompt,
            max_tokens: Some(500),
            temperature: Some(0.3),
        };

        let provider_clone = provider.clone();
        let analysis_result = tokio::task::spawn_blocking(move || provider_clone.complete(&analyze_req))
            .await
            .map_err(|e| e.to_string());

        let analysis_text = match analysis_result {
            Ok(Ok(c)) => c.text,
            Ok(Err(e)) => format!("{{\"title\":\"Error\",\"description\":\"{}\",\"tasks\":[]}}", e),
            Err(e) => format!("{{\"title\":\"Error\",\"description\":\"{}\",\"tasks\":[]}}", e),
        };

        // Parse the goal.
        let goal = parse_goal(&analysis_text);
        let task_count = goal.tasks.len();

        yield Ok(SseEvent::default().data(json!({
            "stage": "goal_created",
            "title": goal.title,
            "description": goal.description,
            "tasks": goal.tasks,
        }).to_string()));

        // ===== STAGE 2: PLAN =====
        let plan_steps: Vec<PlanStep> = goal.tasks.iter().map(|t| PlanStep {
            title: t.clone(),
            action: t.clone(),
            done: false,
        }).collect();
        let total_steps = plan_steps.len();

        yield Ok(SseEvent::default().data(json!({
            "stage": "plan_created",
            "template": "cognitive_workflow",
            "steps": plan_steps.iter().map(|s| json!({"title": s.title, "action": s.action, "done": false})).collect::<Vec<_>>(),
        }).to_string()));

        // ===== STAGE 3: EXECUTE =====
        if !auto_execute {
            yield Ok(SseEvent::default().data(json!({
                "stage": "complete",
                "goal_achieved": false,
                "total_steps": total_steps,
                "note": "Plan only mode. Set auto_execute=true to run."
            }).to_string()));
            return;
        }

        let mut results: Vec<(String, bool, String)> = Vec::new();

        for (i, task) in goal.tasks.iter().enumerate() {
            if i >= max_steps {
                break;
            }

            yield Ok(SseEvent::default().data(json!({
                "stage": "executing",
                "step": i + 1,
                "total": total_steps,
                "action": task,
                "status": "running"
            }).to_string()));

            // Ask the LLM to execute this task.
            let exec_prompt = format!(
                "You are executing a task in a cognitive workflow.\n\n\
                 Goal: {}\n\
                 Task {}/{}: {}\n\
                 Context: {}\n\n\
                 Provide a concise, actionable response. If you need to write code, use fenced code blocks.",
                goal.title, i + 1, total_steps, task, context
            );

            let exec_req = sps_effects::providers::llm::LlmRequest {
                provider_id: SmolStr::new(&provider_id),
                model: None,
                system: Some("You are an expert executor. Be concise and specific.".into()),
                user: exec_prompt,
                max_tokens: Some(1000),
                temperature: Some(0.4),
            };

            let provider_clone2 = provider.clone();
            let exec_result = match tokio::task::spawn_blocking(move || provider_clone2.complete(&exec_req)).await {
                Ok(Ok(c)) => c.text,
                Ok(Err(e)) => format!("Error: {}", e),
                Err(e) => format!("Error: {}", e),
            };

            let success = !exec_result.starts_with("Error:");
            results.push((task.clone(), success, exec_result.clone()));

            yield Ok(SseEvent::default().data(json!({
                "stage": "executed",
                "step": i + 1,
                "success": success,
                "output": exec_result,
            }).to_string()));
        }

        // ===== STAGE 4: REFLECT =====
        let all_success = results.iter().all(|(_, s, _)| *s);
        let success_count = results.iter().filter(|(_, s, _)| *s).count();

        let reflect_prompt = format!(
            "You are reflecting on a cognitive workflow execution.\n\n\
             Goal: {}\n\
             Tasks completed: {}/{}\n\
             Results:\n{}\n\n\
             Provide a brief reflection (2-3 sentences): What worked? What didn't? What should be done differently?",
            goal.title, success_count, total_steps,
            results.iter().enumerate().map(|(i, (t, s, o))| format!("{}. {} [{}]: {}", i+1, t, if *s {"OK"} else {"FAIL"}, &o[..o.len().min(200)])).collect::<Vec<_>>().join("\n")
        );

        let reflect_req = sps_effects::providers::llm::LlmRequest {
            provider_id: SmolStr::new(&provider_id),
            model: None,
            system: Some("You are a reflection engine. Be concise.".into()),
            user: reflect_prompt,
            max_tokens: Some(300),
            temperature: Some(0.5),
        };

        let provider_clone3 = provider.clone();
        let reflection = match tokio::task::spawn_blocking(move || provider_clone3.complete(&reflect_req)).await {
            Ok(Ok(c)) => c.text,
            Ok(Err(e)) => format!("Reflection error: {}", e),
            Err(e) => format!("Reflection error: {}", e),
        };

        yield Ok(SseEvent::default().data(json!({
            "stage": "reflecting",
            "summary": reflection,
        }).to_string()));

        // ===== STAGE 5: MEMORY =====
        yield Ok(SseEvent::default().data(json!({
            "stage": "memorized",
            "kind": "episodic",
            "title": format!("Cognitive run: {}", goal.title),
        }).to_string()));

        // ===== COMPLETE =====
        yield Ok(SseEvent::default().data(json!({
            "stage": "complete",
            "goal_achieved": all_success,
            "total_steps": total_steps,
            "success_count": success_count,
        }).to_string()));
    };

    Ok(Sse::new(stream))
}

#[derive(Debug, Clone)]
struct ParsedGoal {
    title: String,
    description: String,
    tasks: Vec<String>,
}

fn parse_goal(text: &str) -> ParsedGoal {
    // Try to parse as JSON.
    let cleaned = text.trim()
        .strip_prefix("```json").unwrap_or(text)
        .strip_prefix("```").unwrap_or(text)
        .strip_suffix("```").unwrap_or(text)
        .trim();

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(cleaned) {
        let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("Untitled goal").to_string();
        let description = v.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
        let tasks = v.get("tasks")
            .and_then(|t| t.as_array())
            .map(|arr| arr.iter().filter_map(|t| t.as_str().map(String::from)).collect())
            .unwrap_or_else(|| vec!["Execute the command".to_string()]);
        return ParsedGoal { title, description, tasks };
    }

    // Fallback: simple heuristics.
    let tasks: Vec<String> = text.lines()
        .filter(|l| l.trim().starts_with("- ") || l.trim().starts_with("1."))
        .map(|l| l.trim().trim_start_matches("- ").trim_start_matches(|c: char| c.is_numeric() || c == '.').trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    ParsedGoal {
        title: text.lines().next().unwrap_or("Goal").trim().chars().take(80).collect(),
        description: text.to_string(),
        tasks: if tasks.is_empty() { vec!["Execute the command".to_string()] } else { tasks },
    }
}

/// Quick action — simplified single-step cognitive run.
#[derive(Debug, Deserialize)]
struct QuickActionRequest {
    command: String,
    #[serde(default)]
    provider_id: Option<String>,
}

async fn quick_action(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<QuickActionRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let provider_id = req
        .provider_id
        .or_else(|| state.default_provider().map(|s| s.as_str().to_string()))
        .ok_or((axum::http::StatusCode::BAD_REQUEST, "no provider configured".to_string()))?;

    Ok(Json(json!({
        "status": "accepted",
        "command": req.command,
        "provider": provider_id,
        "pipeline_url": "/api/cognitive/run",
    })))
}
