//! Inline code edit endpoints — apply edits from chat to files.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/inline/edit", post(apply_edit))
        .route("/api/inline/diff", post(compute_diff))
}

#[derive(Debug, Deserialize)]
struct EditRequest {
    /// File to edit (relative to workspace).
    file: String,
    /// New content for the file.
    content: String,
}

async fn apply_edit(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<EditRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let root = state.workspace_root.read().clone();
    let root = root.ok_or((axum::http::StatusCode::BAD_REQUEST, "no workspace scanned".to_string()))?;
    let ops = sps_workspace::ops::FileOps::new(root);

    // Read old content for diff.
    let old = ops.read(&req.file).ok().and_then(|c| c.text).unwrap_or_default();

    // Write new content.
    match ops.write(&req.file, &req.content) {
        Ok(bytes) => {
            // Re-index in code intel.
            let _ = state.code_index.index_file(&req.file, &req.content);
            // Compute diff.
            let diff = compute_diff_lines(&old, &req.content);
            Ok(Json(json!({
                "file": req.file,
                "bytes_written": bytes,
                "diff": diff,
            })))
        }
        Err(e) => Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct DiffRequest {
    /// Old content.
    old: String,
    /// New content.
    new: String,
}

async fn compute_diff(Json(req): Json<DiffRequest>) -> Json<serde_json::Value> {
    let diff = compute_diff_lines(&req.old, &req.new);
    Json(json!({ "diff": diff }))
}

/// Compute a simple line-level diff.
/// Returns a list of diff entries: added, removed, or context.
fn compute_diff_lines(old: &str, new: &str) -> Vec<serde_json::Value> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    // Simple LCS-based diff (O(n*m) — fine for small files).
    let n = old_lines.len();
    let m = new_lines.len();

    // Build LCS table.
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            if old_lines[i - 1] == new_lines[j - 1] {
                lcs[i][j] = lcs[i - 1][j - 1] + 1;
            } else {
                lcs[i][j] = lcs[i][j - 1].max(lcs[i - 1][j]);
            }
        }
    }

    // Backtrack to produce diff.
    let mut result = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            result.push(json!({"type": "context", "old_line": i, "new_line": j, "content": old_lines[i - 1]}));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || lcs[i][j - 1] >= lcs[i - 1][j]) {
            result.push(json!({"type": "added", "new_line": j, "content": new_lines[j - 1]}));
            j -= 1;
        } else if i > 0 {
            result.push(json!({"type": "removed", "old_line": i, "content": old_lines[i - 1]}));
            i -= 1;
        }
    }
    result.reverse();
    result
}
