//! Git endpoints — status, branches, blame, history.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/git/status", get(status))
        .route("/api/git/branches", get(branches))
        .route("/api/git/blame/{*path}", get(blame))
        .route("/api/git/history/{*path}", get(history))
        .route("/api/git/log", get(log))
}

async fn status(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let root = state.workspace_root.read().clone();
    match root {
        Some(r) => match sps_git::status::status(&r) {
            Ok(s) => Json(json!({
                "branch": s.branch,
                "is_clean": s.is_clean,
                "entries": s.entries.iter().map(|e| json!({
                    "file": e.file, "kind": format!("{:?}", e.kind).to_lowercase(), "staged": e.staged,
                })).collect::<Vec<_>>(),
            })),
            Err(e) => Json(json!({ "error": e.to_string() })),
        },
        None => Json(json!({ "error": "no workspace scanned" })),
    }
}

async fn branches(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let root = state.workspace_root.read().clone();
    match root {
        Some(r) => match sps_git::branches::list_branches(&r) {
            Ok(b) => Json(json!({
                "current": b.current,
                "branches": b.branches.iter().map(|br| json!({
                    "name": br.name, "is_current": br.is_current, "is_remote": br.is_remote, "last_commit": br.last_commit,
                })).collect::<Vec<_>>(),
            })),
            Err(e) => Json(json!({ "error": e.to_string() })),
        },
        None => Json(json!({ "error": "no workspace scanned" })),
    }
}

async fn blame(State(state): State<Arc<ServerState>>, Path(path): Path<String>) -> Json<serde_json::Value> {
    let root = state.workspace_root.read().clone();
    match root {
        Some(r) => match sps_git::blame::blame(&r, &path) {
            Ok(b) => Json(json!({
                "file": b.file,
                "lines": b.lines.iter().map(|l| json!({
                    "line": l.line, "commit": l.commit, "author": l.author, "date": l.date, "summary": l.summary, "content": l.content,
                })).collect::<Vec<_>>(),
            })),
            Err(e) => Json(json!({ "error": e.to_string() })),
        },
        None => Json(json!({ "error": "no workspace scanned" })),
    }
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    #[serde(default = "default_limit")]
    limit: usize,
}
fn default_limit() -> usize { 50 }

async fn history(State(state): State<Arc<ServerState>>, Path(path): Path<String>, Query(q): Query<HistoryQuery>) -> Json<serde_json::Value> {
    let root = state.workspace_root.read().clone();
    match root {
        Some(r) => match sps_git::history::file_history(&r, &path, q.limit) {
            Ok(h) => Json(json!({
                "file": h.file,
                "commits": h.commits.iter().map(|c| json!({
                    "hash": c.hash, "short_hash": c.short_hash, "author": c.author, "date": c.date, "message": c.message,
                    "files_changed": c.files_changed, "insertions": c.insertions, "deletions": c.deletions,
                })).collect::<Vec<_>>(),
            })),
            Err(e) => Json(json!({ "error": e.to_string() })),
        },
        None => Json(json!({ "error": "no workspace scanned" })),
    }
}

async fn log(State(state): State<Arc<ServerState>>, Query(q): Query<HistoryQuery>) -> Json<serde_json::Value> {
    let root = state.workspace_root.read().clone();
    match root {
        Some(r) => match sps_git::history::log(&r, q.limit) {
            Ok(commits) => Json(json!({
                "commits": commits.iter().map(|c| json!({
                    "hash": c.hash, "short_hash": c.short_hash, "author": c.author, "date": c.date, "message": c.message,
                    "files_changed": c.files_changed, "insertions": c.insertions, "deletions": c.deletions,
                })).collect::<Vec<_>>(),
            })),
            Err(e) => Json(json!({ "error": e.to_string() })),
        },
        None => Json(json!({ "error": "no workspace scanned" })),
    }
}
