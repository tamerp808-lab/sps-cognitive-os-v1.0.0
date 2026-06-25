//! Phase 8 — Execution Layer tests.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_execution::analysis::CodeAnalyzer;
use sps_execution::generation::{ProjectGenerator, ProjectSpec};
use sps_execution::reducer::{ExecutionReducer, ExecutionState, ExecutionOutcome};

fn fresh_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    ExecutionReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

#[test]
fn code_analyzer_counts_rust_lines_and_functions() {
    let content = "use std::io;\n\nfn main() {\n    println!(\"hi\");\n}\n\nfn helper() {}\n";
    let a = CodeAnalyzer::analyze_file("src/main.rs", content);
    assert_eq!(a.language, "rust");
    assert_eq!(a.lines, 7);
    assert_eq!(a.imports, 1);
    assert_eq!(a.functions, 2);
}

#[test]
fn code_analyzer_counts_typescript() {
    let content = "import { foo } from 'bar';\n\nfunction baz() {\n  return 1;\n}\n\nconst arrow = () => 2;\n";
    let a = CodeAnalyzer::analyze_file("src/index.ts", content);
    assert_eq!(a.language, "typescript");
    assert_eq!(a.imports, 1);
    assert!(a.functions >= 2);
}

#[test]
fn code_analyzer_aggregates_multiple_files() {
    let files = vec![
        ("a.rs".to_string(), "fn f() {}".to_string()),
        ("b.rs".to_string(), "fn g() {}\nfn h() {}".to_string()),
    ];
    let analysis = CodeAnalyzer::analyze(&files);
    assert_eq!(analysis.files.len(), 2);
    assert_eq!(analysis.total_functions, 3);
    assert_eq!(analysis.languages.get("rust"), Some(&2));
}

#[test]
fn project_generator_rust_cli() {
    let spec = ProjectSpec {
        name: SmolStr::new("my-cli"),
        kind: SmolStr::new("rust_cli"),
        output_dir: "/tmp/my-cli".into(),
        description: Some("test".into()),
    };
    let files = ProjectGenerator::generate(&spec);
    assert!(files.iter().any(|f| f.path == "Cargo.toml"));
    assert!(files.iter().any(|f| f.path == "src/main.rs"));
    assert!(files.iter().any(|f| f.path == "README.md"));
    let main = files.iter().find(|f| f.path == "src/main.rs").unwrap();
    assert!(main.content.contains("my-cli"));
}

#[test]
fn project_generator_tauri_includes_rust_files() {
    let spec = ProjectSpec {
        name: SmolStr::new("my-app"),
        kind: SmolStr::new("tauri"),
        output_dir: "/tmp/my-app".into(),
        description: None,
    };
    let files = ProjectGenerator::generate(&spec);
    assert!(files.iter().any(|f| f.path == "src-tauri/Cargo.toml"));
    assert!(files.iter().any(|f| f.path == "src-tauri/src/main.rs"));
}

#[test]
fn execution_succeeded_event_persists_record() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let event = RawEvent::new(
        "execution.succeeded",
        json!({"operation": "shell.exec", "duration_ms": 150u64}),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let es = ExecutionState::from_state(&state).unwrap();
    assert_eq!(es.records.len(), 1);
    let r = es.records.values().next().unwrap();
    assert_eq!(r.outcome, ExecutionOutcome::Success);
    assert_eq!(r.operation, "shell.exec");
}

#[test]
fn execution_failed_event_records_error() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let event = RawEvent::new(
        "execution.failed",
        json!({"operation": "fs.write", "duration_ms": 5u64, "error": "permission denied"}),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let es = ExecutionState::from_state(&state).unwrap();
    let r = es.records.values().next().unwrap();
    assert_eq!(r.outcome, ExecutionOutcome::Failure);
    assert_eq!(r.error, Some("permission denied".to_string()));
}
