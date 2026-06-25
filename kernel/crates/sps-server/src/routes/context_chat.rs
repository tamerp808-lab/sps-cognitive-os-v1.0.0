//! Context-aware chat endpoint — sends current file + selection to LLM.
//! Also provides inline code edit (select code → AI modifies it).

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use smol_str::SmolStr;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/chat/context", post(chat_with_context))
        .route("/api/chat/inline-edit", post(inline_edit))
}

#[derive(Debug, Deserialize)]
struct ContextChatRequest {
    /// User's question.
    message: String,
    /// Current file path (optional).
    #[serde(default)]
    file: Option<String>,
    /// Selected code (optional).
    #[serde(default)]
    selection: Option<String>,
    /// Line number where selection starts (optional).
    #[serde(default)]
    selection_start_line: Option<u32>,
    /// Additional context files (paths).
    #[serde(default)]
    context_files: Vec<String>,
    /// Provider id (defaults to server's default).
    #[serde(default)]
    provider_id: Option<String>,
}

async fn chat_with_context(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<ContextChatRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let provider_id = req
        .provider_id
        .or_else(|| state.default_provider().map(|s| s.as_str().to_string()))
        .ok_or((
            axum::http::StatusCode::BAD_REQUEST,
            "no provider configured".to_string(),
        ))?;

    let provider = state
        .providers
        .get(&provider_id)
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            format!("provider {} not registered", provider_id),
        ))?;

    // Build context-aware system prompt.
    let mut system = String::from(
        "You are an AI coding assistant integrated into the SPS Cognitive Operating System. \
         You help with code questions, debugging, refactoring, and implementation.\n\n",
    );

    // Add current file context.
    if let Some(ref file) = req.file {
        let root = state.workspace_root.read().clone();
        if let Some(ref root) = root {
            let full_path = root.join(file);
            if let Ok(source) = std::fs::read_to_string(&full_path) {
                let lines = source.lines().count();
                // Truncate large files.
                let truncated = if source.len() > 10000 {
                    format!("{}...\n(truncated, {} total lines)", &source[..10000], lines)
                } else {
                    source
                };
                system.push_str(&format!("## Current file: `{}` ({} lines)\n```\n{}\n```\n\n", file, lines, truncated));
            }
        }
    }

    // Add selection context.
    if let Some(ref selection) = req.selection {
        let line_info = req.selection_start_line
            .map(|l| format!(" (starting at line {})", l))
            .unwrap_or_default();
        system.push_str(&format!("## Selected code{}\n```\n{}\n```\n\n", line_info, selection));
    }

    // Add additional context files.
    for ctx_file in &req.context_files {
        let root = state.workspace_root.read().clone();
        if let Some(ref root) = root {
            let full_path = root.join(ctx_file);
            if let Ok(source) = std::fs::read_to_string(&full_path) {
                let truncated = if source.len() > 5000 {
                    format!("{}...", &source[..5000])
                } else {
                    source
                };
                system.push_str(&format!("## Context file: `{}`\n```\n{}\n```\n\n", ctx_file, truncated));
            }
        }
    }

    system.push_str(
        "When suggesting code changes, use fenced code blocks with the language tag. \
         Be specific about which file and lines to change.",
    );

    let request = sps_effects::providers::llm::LlmRequest {
        provider_id: SmolStr::new(&provider_id),
        model: None,
        system: Some(system),
        user: req.message,
        max_tokens: None,
        temperature: Some(0.3), // Lower temperature for code tasks
    };

    // Use spawn_blocking to avoid runtime panic.
    let provider_clone = provider.clone();
    let completion = tokio::task::spawn_blocking(move || provider_clone.complete(&request))
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "text": completion.text,
        "model": completion.model,
        "usage": {
            "prompt_tokens": completion.usage.prompt_tokens,
            "completion_tokens": completion.usage.completion_tokens,
            "total_tokens": completion.usage.total_tokens,
        },
        "elapsed_ms": completion.elapsed_ms,
        "provider": provider_id,
    })))
}

#[derive(Debug, Deserialize)]
struct InlineEditRequest {
    /// The selected code to modify.
    code: String,
    /// What the user wants to do with it.
    instruction: String,
    /// Language of the code (for syntax context).
    #[serde(default)]
    language: Option<String>,
    /// Provider id.
    #[serde(default)]
    provider_id: Option<String>,
}

async fn inline_edit(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<InlineEditRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let provider_id = req
        .provider_id
        .or_else(|| state.default_provider().map(|s| s.as_str().to_string()))
        .ok_or((
            axum::http::StatusCode::BAD_REQUEST,
            "no provider configured".to_string(),
        ))?;

    let provider = state
        .providers
        .get(&provider_id)
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            format!("provider {} not registered", provider_id),
        ))?;

    let lang = req.language.unwrap_or_else(|| "code".to_string());
    let system = format!(
        "You are a code editing assistant. The user will give you a code snippet and an instruction. \
         Respond with ONLY the modified code in a single fenced code block. \
         Do not add explanations before or after the code block.\n\n\
         Language: {lang}",
        lang = lang
    );
    let user = format!(
        "## Instruction\n{}\n\n## Code\n```\n{}\n```\n\nReturn ONLY the modified code:",
        req.instruction, req.code
    );

    let request = sps_effects::providers::llm::LlmRequest {
        provider_id: SmolStr::new(&provider_id),
        model: None,
        system: Some(system),
        user,
        max_tokens: None,
        temperature: Some(0.2),
    };

    let provider_clone = provider.clone();
    let completion = tokio::task::spawn_blocking(move || provider_clone.complete(&request))
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Extract code from the response (strip markdown fences).
    let text = completion.text.trim();
    let new_code = {
        let mut s = text;
        if s.starts_with("```") {
            s = &s[3..];
            // Skip language tag on same line.
            if let Some(nl) = s.find('\n') {
                s = &s[nl + 1..];
            }
        }
        // Strip trailing ```.
        if let Some(pos) = s.rfind("```") {
            s = &s[..pos];
        }
        s.trim().to_string()
    };

    // Compute diff.
    let diff = compute_diff(&req.code, &new_code);

    Ok(Json(json!({
        "original": req.code,
        "modified": new_code,
        "diff": diff,
        "model": completion.model,
        "elapsed_ms": completion.elapsed_ms,
        "provider": provider_id,
    })))
}

/// Simple line-level diff (LCS-based).
fn compute_diff(old: &str, new: &str) -> Vec<serde_json::Value> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let n = old_lines.len();
    let m = new_lines.len();

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

    let mut result = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            result.push(json!({"type": "context", "old_line": i, "new_line": j, "content": old_lines[i - 1]}));
            i -= 1; j -= 1;
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
