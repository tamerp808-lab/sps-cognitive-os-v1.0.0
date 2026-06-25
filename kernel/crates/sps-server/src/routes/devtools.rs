//! Dev tools endpoints — format code + run tests.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/tools/format", post(format_file))
        .route("/api/tools/format/batch", post(format_batch))
        .route("/api/tools/test", post(run_tests))
        .route("/api/tools/test/parse", post(parse_test_output))
}

#[derive(Debug, Deserialize)]
struct FormatRequest {
    file: String,
}

async fn format_file(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<FormatRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let root = state.workspace_root.read().clone();
    let root = root.ok_or((axum::http::StatusCode::BAD_REQUEST, "no workspace scanned".to_string()))?;
    let fmt = sps_tools::formatter::CodeFormatter::new(root);
    let result = fmt.format_file(&req.file);
    Ok(Json(json!({
        "file": result.file,
        "formatter": result.formatter,
        "success": result.success,
        "modified": result.modified,
        "error": result.error,
    })))
}

#[derive(Debug, Deserialize)]
struct FormatBatchRequest {
    files: Vec<String>,
}

async fn format_batch(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<FormatBatchRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let root = state.workspace_root.read().clone();
    let root = root.ok_or((axum::http::StatusCode::BAD_REQUEST, "no workspace scanned".to_string()))?;
    let fmt = sps_tools::formatter::CodeFormatter::new(root);
    let results = fmt.format_files(&req.files);
    let json_results: Vec<_> = results.iter().map(|r| json!({
        "file": r.file, "formatter": r.formatter, "success": r.success, "modified": r.modified, "error": r.error,
    })).collect();
    Ok(Json(json!({ "results": json_results, "total": json_results.len() })))
}

async fn run_tests(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let root = state.workspace_root.read().clone();
    let root = root.ok_or((axum::http::StatusCode::BAD_REQUEST, "no workspace scanned".to_string()))?;
    let runner = sps_tools::test_runner::TestRunner::new(root);
    let result = runner.run();
    Ok(Json(json!({
        "suite": result.suite,
        "success": result.success,
        "total": result.total,
        "passed": result.passed,
        "failed": result.failed,
        "exit_code": result.exit_code,
        "duration_ms": result.duration_ms,
        "cases": result.cases.iter().map(|c| json!({
            "name": c.name, "module": c.module,
            "status": format!("{:?}", c.status).to_lowercase(),
            "message": c.message,
        })).collect::<Vec<_>>(),
        "stdout": result.stdout,
        "stderr": result.stderr,
    })))
}

#[derive(Debug, Deserialize)]
struct ParseTestRequest {
    output: String,
    #[serde(default = "default_framework")]
    framework: String,
}

fn default_framework() -> String { "cargo".into() }

async fn parse_test_output(
    Json(req): Json<ParseTestRequest>,
) -> Json<serde_json::Value> {
    // Use the parser directly — it's a free function.
    // We re-implement the dispatch here since parse_* are private.
    let (cases, total, passed, failed) = match req.framework.as_str() {
        "pytest" => parse_pytest(&req.output),
        "go" => parse_go(&req.output),
        _ => parse_cargo(&req.output),
    };
    Json(json!({
        "framework": req.framework,
        "total": total,
        "passed": passed,
        "failed": failed,
        "cases": cases.iter().map(|c| json!({
            "name": c.name, "status": format!("{:?}", c.status).to_lowercase(),
        })).collect::<Vec<_>>(),
    }))
}

// Re-export parsers for the endpoint (they're private in the crate).
fn parse_cargo(stdout: &str) -> (Vec<sps_tools::test_runner::TestCase>, usize, usize, usize) {
    let mut cases = Vec::new();
    let test_re = regex::Regex::new(r"^(test (\S+) \.\.\. (ok|FAILED|ignored))").unwrap();
    for line in stdout.lines() {
        if let Some(caps) = test_re.captures(line) {
            let status = match &caps[3] {
                "ok" => sps_tools::test_runner::TestStatus::Passed,
                "FAILED" => sps_tools::test_runner::TestStatus::Failed,
                "ignored" => sps_tools::test_runner::TestStatus::Ignored,
                _ => sps_tools::test_runner::TestStatus::Error,
            };
            cases.push(sps_tools::test_runner::TestCase { name: caps[2].to_string(), module: None, status, duration_ms: None, message: None });
        }
    }
    let total = cases.len();
    let passed = cases.iter().filter(|c| c.status == sps_tools::test_runner::TestStatus::Passed).count();
    let failed = cases.iter().filter(|c| c.status == sps_tools::test_runner::TestStatus::Failed).count();
    (cases, total, passed, failed)
}

fn parse_pytest(stdout: &str) -> (Vec<sps_tools::test_runner::TestCase>, usize, usize, usize) {
    let mut cases = Vec::new();
    let re = regex::Regex::new(r"^(\S+)::(\S+)\s+(PASSED|FAILED|SKIPPED|ERROR)").unwrap();
    for line in stdout.lines() {
        if let Some(caps) = re.captures(line) {
            let status = match &caps[3] { "PASSED" => sps_tools::test_runner::TestStatus::Passed, "FAILED" => sps_tools::test_runner::TestStatus::Failed, "SKIPPED" => sps_tools::test_runner::TestStatus::Ignored, _ => sps_tools::test_runner::TestStatus::Error };
            cases.push(sps_tools::test_runner::TestCase { name: caps[2].to_string(), module: Some(caps[1].to_string()), status, duration_ms: None, message: None });
        }
    }
    let total = cases.len(); let passed = cases.iter().filter(|c| c.status == sps_tools::test_runner::TestStatus::Passed).count(); let failed = cases.iter().filter(|c| c.status == sps_tools::test_runner::TestStatus::Failed).count();
    (cases, total, passed, failed)
}

fn parse_go(stdout: &str) -> (Vec<sps_tools::test_runner::TestCase>, usize, usize, usize) {
    let mut cases = Vec::new();
    let pass_re = regex::Regex::new(r"^--- PASS:\s+(\S+)").unwrap();
    let fail_re = regex::Regex::new(r"^--- FAIL:\s+(\S+)").unwrap();
    let skip_re = regex::Regex::new(r"^--- SKIP:\s+(\S+)").unwrap();
    for line in stdout.lines() {
        if let Some(caps) = pass_re.captures(line) { cases.push(sps_tools::test_runner::TestCase { name: caps[1].to_string(), module: None, status: sps_tools::test_runner::TestStatus::Passed, duration_ms: None, message: None }); }
        else if let Some(caps) = fail_re.captures(line) { cases.push(sps_tools::test_runner::TestCase { name: caps[1].to_string(), module: None, status: sps_tools::test_runner::TestStatus::Failed, duration_ms: None, message: None }); }
        else if let Some(caps) = skip_re.captures(line) { cases.push(sps_tools::test_runner::TestCase { name: caps[1].to_string(), module: None, status: sps_tools::test_runner::TestStatus::Ignored, duration_ms: None, message: None }); }
    }
    let total = cases.len(); let passed = cases.iter().filter(|c| c.status == sps_tools::test_runner::TestStatus::Passed).count(); let failed = cases.iter().filter(|c| c.status == sps_tools::test_runner::TestStatus::Failed).count();
    (cases, total, passed, failed)
}
