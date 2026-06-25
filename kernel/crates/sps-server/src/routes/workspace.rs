//! Workspace endpoints — file tree, read/write files, scan workspace.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/workspace/scan", post(scan_workspace))
        .route("/api/workspace/tree", get(get_tree))
        .route("/api/workspace/files/{*path}", get(read_file).put(write_file).delete(delete_file))
        .route("/api/workspace/list", get(list_dir))
}

#[derive(Debug, Deserialize)]
struct ScanRequest {
    path: String,
}

async fn scan_workspace(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<ScanRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let root = std::path::PathBuf::from(&req.path);
    if !root.exists() {
        return Err((axum::http::StatusCode::NOT_FOUND, format!("path not found: {}", req.path)));
    }
    // Store the workspace root.
    *state.workspace_root.write() = Some(root.clone());

    let scanner = sps_workspace::scanner::WorkspaceScanner::default();
    let tree = scanner.scan(&root).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Index all files into the code index.
    let code_index = state.code_index.clone();
    let files = tree.files();
    for file_node in files {
        let full_path = root.join(file_node.path.as_str());
        if let Ok(source) = std::fs::read_to_string(&full_path) {
            let _ = code_index.index_file(file_node.path.as_str(), &source);
        }
    }

    Ok(Json(json!({
        "scanned": true,
        "root": req.path,
        "files": tree.file_count(),
        "total_size": tree.total_size(),
    })))
}

async fn get_tree(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let root = state.workspace_root.read().clone();
    match root {
        Some(root) => {
            let scanner = sps_workspace::scanner::WorkspaceScanner::default();
            match scanner.scan(&root) {
                Ok(tree) => Json(json!({
                    "root": root.to_string_lossy(),
                    "tree": tree,
                })),
                Err(e) => Json(json!({ "error": e.to_string() })),
            }
        }
        None => Json(json!({ "error": "no workspace scanned" })),
    }
}

async fn read_file(
    State(state): State<Arc<ServerState>>,
    Path(path): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let root = state.workspace_root.read().clone();
    let root = root.ok_or((axum::http::StatusCode::BAD_REQUEST, "no workspace scanned".to_string()))?;
    let ops = sps_workspace::ops::FileOps::new(root);
    match ops.read(&path) {
        Ok(content) => Ok(Json(json!({
            "path": content.path,
            "text": content.text,
            "size": content.size,
            "is_text": content.is_text,
            "lines": content.lines,
        }))),
        Err(e) => Err((axum::http::StatusCode::NOT_FOUND, e.to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct WriteRequest {
    content: String,
}

async fn write_file(
    State(state): State<Arc<ServerState>>,
    Path(path): Path<String>,
    Json(req): Json<WriteRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let root = state.workspace_root.read().clone();
    let root = root.ok_or((axum::http::StatusCode::BAD_REQUEST, "no workspace scanned".to_string()))?;
    let ops = sps_workspace::ops::FileOps::new(root);
    match ops.write(&path, &req.content) {
        Ok(bytes) => {
            // Re-index the file in the code index.
            let _ = state.code_index.index_file(&path, &req.content);
            Ok(Json(json!({ "path": path, "bytes_written": bytes })))
        }
        Err(e) => Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

async fn delete_file(
    State(state): State<Arc<ServerState>>,
    Path(path): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let root = state.workspace_root.read().clone();
    let root = root.ok_or((axum::http::StatusCode::BAD_REQUEST, "no workspace scanned".to_string()))?;
    let ops = sps_workspace::ops::FileOps::new(root);
    match ops.delete(&path) {
        Ok(_) => {
            state.code_index.remove_file(&path);
            Ok(Json(json!({ "deleted": true, "path": path })))
        }
        Err(e) => Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    #[serde(default)]
    path: Option<String>,
}

async fn list_dir(
    State(state): State<Arc<ServerState>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let root = state.workspace_root.read().clone();
    let root = root.ok_or((axum::http::StatusCode::BAD_REQUEST, "no workspace scanned".to_string()))?;
    let ops = sps_workspace::ops::FileOps::new(root);
    let path = q.path.unwrap_or_default();
    match ops.list_dir(&path) {
        Ok(entries) => {
            let json_entries: Vec<_> = entries.iter().map(|e| {
                json!({
                    "name": e.name,
                    "path": e.path,
                    "is_dir": e.is_dir,
                    "size": e.size,
                })
            }).collect();
            Ok(Json(json!({ "path": path, "entries": json_entries, "count": json_entries.len() })))
        }
        Err(e) => Err((axum::http::StatusCode::NOT_FOUND, e.to_string())),
    }
}
