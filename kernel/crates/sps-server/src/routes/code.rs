//! Code intelligence endpoints — symbol search, file index, references.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/code/search", get(search_symbols))
        .route("/api/code/symbols/{file}", get(symbols_in_file))
        .route("/api/code/references/{name}", get(find_references))
        .route("/api/code/definition/{name}", get(go_to_definition))
        .route("/api/code/stats", get(code_stats))
        .route("/api/code/files", get(list_files))
        .route("/api/code/index", post(index_file))
        .route("/api/code/source/{*path}", get(get_source))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    kind: Option<String>,
}

fn default_limit() -> usize { 50 }

async fn search_symbols(
    State(state): State<Arc<ServerState>>,
    Query(q): Query<SearchQuery>,
) -> Json<serde_json::Value> {
    let kind = q.kind.as_deref().and_then(|k| match k {
        "function" => Some(sps_code_intel::symbol::SymbolKind::Function),
        "class" => Some(sps_code_intel::symbol::SymbolKind::Class),
        "struct" => Some(sps_code_intel::symbol::SymbolKind::Struct),
        "enum" => Some(sps_code_intel::symbol::SymbolKind::Enum),
        "interface" => Some(sps_code_intel::symbol::SymbolKind::Interface),
        "trait" => Some(sps_code_intel::symbol::SymbolKind::Trait),
        "constant" => Some(sps_code_intel::symbol::SymbolKind::Constant),
        "module" => Some(sps_code_intel::symbol::SymbolKind::Module),
        _ => None,
    });
    let results = state.code_index.search_filtered(&q.q, q.limit, kind, None);
    let json_results: Vec<_> = results.iter().map(|r| {
        json!({
            "symbol": symbol_to_json(&r.symbol),
            "score": r.score,
            "matched_positions": r.matched_positions,
        })
    }).collect();
    Json(json!({ "results": json_results, "count": json_results.len(), "query": q.q }))
}

async fn symbols_in_file(
    State(state): State<Arc<ServerState>>,
    Path(file): Path<String>,
) -> Json<serde_json::Value> {
    let symbols = state.code_index.symbols_in_file(&file);
    let json_symbols: Vec<_> = symbols.iter().map(symbol_to_json).collect();
    Json(json!({ "file": file, "symbols": json_symbols, "count": json_symbols.len() }))
}

async fn find_references(
    State(state): State<Arc<ServerState>>,
    Path(name): Path<String>,
) -> Json<serde_json::Value> {
    let refs = state.code_index.find_references(&name);
    let json_refs: Vec<_> = refs.iter().map(|r| {
        json!({
            "file": r.file,
            "line": r.line,
            "column": r.column,
            "context": r.context,
        })
    }).collect();
    Json(json!({ "name": name, "references": json_refs, "count": json_refs.len() }))
}

async fn go_to_definition(
    State(state): State<Arc<ServerState>>,
    Path(name): Path<String>,
) -> Json<serde_json::Value> {
    let defs = state.code_index.go_to_definition(&name);
    let json_defs: Vec<_> = defs.iter().map(symbol_to_json).collect();
    Json(json!({ "name": name, "definitions": json_defs, "count": json_defs.len() }))
}

async fn code_stats(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let stats = state.code_index.stats();
    Json(json!({
        "files_indexed": stats.files_indexed,
        "total_symbols": stats.total_symbols,
        "total_imports": stats.total_imports,
        "by_kind": stats.by_kind,
        "by_language": stats.by_language,
        "by_file": stats.by_file,
    }))
}

async fn list_files(State(state): State<Arc<ServerState>>) -> Json<serde_json::Value> {
    let files = state.code_index.files();
    Json(json!({ "files": files, "count": files.len() }))
}

#[derive(Debug, Deserialize)]
struct IndexFileRequest {
    file: String,
    source: String,
}

async fn index_file(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<IndexFileRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    match state.code_index.index_file(&req.file, &req.source) {
        Ok(count) => Ok(Json(json!({ "file": req.file, "symbols_indexed": count }))),
        Err(e) => Err((axum::http::StatusCode::BAD_REQUEST, e.to_string())),
    }
}

async fn get_source(
    State(state): State<Arc<ServerState>>,
    Path(path): Path<String>,
) -> Json<serde_json::Value> {
    match state.code_index.get_source(&path) {
        Some(source) => Json(json!({ "file": path, "source": source, "lines": source.lines().count() })),
        None => Json(json!({ "error": "file not indexed" })),
    }
}

fn symbol_to_json(s: &sps_code_intel::symbol::Symbol) -> serde_json::Value {
    json!({
        "id": s.id.to_string(),
        "name": s.name,
        "qualified_name": s.qualified_name,
        "kind": s.kind.as_str(),
        "icon": s.kind.icon(),
        "language": s.language,
        "location": {
            "file": s.location.file,
            "line": s.location.line,
            "column": s.location.column,
            "end_line": s.location.end_line,
        },
        "doc_comment": s.doc_comment,
        "parameters": s.parameters,
        "return_type": s.return_type,
    })
}
