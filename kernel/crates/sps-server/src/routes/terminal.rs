//! Terminal endpoint — execute shell commands and return output.
//! Also provides a project-wide search & replace endpoint.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/terminal/exec", post(exec_command))
        .route("/api/search/replace", post(search_replace))
        .route("/api/search/code", post(search_code))
}

#[derive(Debug, Deserialize)]
struct ExecRequest {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    cwd: Option<String>,
}

async fn exec_command(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<ExecRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let root = state.workspace_root.read().clone();
    let cwd = req.cwd
        .map(std::path::PathBuf::from)
        .or(root)
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    let output = std::process::Command::new(&req.command)
        .args(&req.args)
        .current_dir(&cwd)
        .output()
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(Json(json!({
        "command": req.command,
        "args": req.args,
        "exit_code": exit_code,
        "stdout": stdout,
        "stderr": stderr,
        "success": output.status.success(),
    })))
}

#[derive(Debug, Deserialize)]
struct SearchReplaceRequest {
    /// Search pattern (plain text).
    query: String,
    /// Replacement text.
    replacement: String,
    /// File filter (glob-style, e.g. "*.rs"). Empty = all files.
    #[serde(default)]
    file_filter: Option<String>,
    /// Whether to use regex.
    #[serde(default)]
    use_regex: bool,
    /// Whether to actually apply the replacement (false = preview only).
    #[serde(default)]
    apply: bool,
}

async fn search_replace(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<SearchReplaceRequest>,
) -> Json<serde_json::Value> {
    let root = state.workspace_root.read().clone();
    let root = match root {
        Some(r) => r,
        None => return Json(json!({ "error": "no workspace scanned" })),
    };

    let mut matches: Vec<serde_json::Value> = Vec::new();
    let mut total_replaced = 0;

    // Walk all indexed files.
    let files = state.code_index.files();
    for file_path in &files {
        // Apply file filter.
        if let Some(ref filter) = req.file_filter {
            if !file_path.ends_with(filter.trim_start_matches('*')) {
                continue;
            }
        }

        let full_path = root.join(file_path);
        let source = match std::fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Find matches.
        let mut file_matches: Vec<(u32, String, String)> = Vec::new(); // (line, old, new)
        for (i, line) in source.lines().enumerate() {
            let contains = if req.use_regex {
                regex::Regex::new(&req.query)
                    .map(|re| re.is_match(line))
                    .unwrap_or(false)
            } else {
                line.contains(&req.query)
            };

            if contains {
                let new_line = if req.use_regex {
                    regex::Regex::new(&req.query)
                        .ok()
                        .and_then(|re| re.replace(line, &req.replacement).into_owned().into())
                        .unwrap_or_else(|| line.replace(&req.query, &req.replacement))
                } else {
                    line.replace(&req.query, &req.replacement)
                };
                file_matches.push(((i + 1) as u32, line.to_string(), new_line));
            }
        }

        if !file_matches.is_empty() {
            // Apply if requested.
            if req.apply {
                let new_source: String = source
                    .lines()
                    .enumerate()
                    .map(|(i, line)| {
                        if let Some((_, _, new)) = file_matches.iter().find(|(ln, _, _)| *ln == (i + 1) as u32) {
                            new.clone()
                        } else {
                            line.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                // Add trailing newline if original had one.
                if source.ends_with('\n') {
                    let _ = std::fs::write(&full_path, format!("{}\n", new_source));
                } else {
                    let _ = std::fs::write(&full_path, &new_source);
                }
                // Re-index.
                let _ = state.code_index.index_file(file_path, &new_source);
                total_replaced += file_matches.len();
            }

            for (line, old, new) in file_matches {
                matches.push(json!({
                    "file": file_path,
                    "line": line,
                    "old": old,
                    "new": new,
                }));
            }
        }
    }

    Json(json!({
        "query": req.query,
        "replacement": req.replacement,
        "total_matches": matches.len(),
        "total_replaced": total_replaced,
        "applied": req.apply,
        "matches": matches,
    }))
}

#[derive(Debug, Deserialize)]
struct SearchCodeRequest {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize { 100 }

async fn search_code(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<SearchCodeRequest>,
) -> Json<serde_json::Value> {
    let root = state.workspace_root.read().clone();
    let root = match root {
        Some(r) => r,
        None => return Json(json!({ "error": "no workspace scanned" })),
    };

    let mut results: Vec<serde_json::Value> = Vec::new();
    let files = state.code_index.files();
    let query_lower = req.query.to_lowercase();

    for file_path in &files {
        let full_path = root.join(file_path);
        let source = match std::fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for (i, line) in source.lines().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                results.push(json!({
                    "file": file_path,
                    "line": i + 1,
                    "content": line.trim(),
                }));
                if results.len() >= req.limit {
                    break;
                }
            }
        }
        if results.len() >= req.limit {
            break;
        }
    }

    Json(json!({
        "query": req.query,
        "results": results,
        "count": results.len(),
    }))
}
