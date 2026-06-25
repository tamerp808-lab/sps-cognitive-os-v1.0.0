//! Phase 11 — Software Factory tests.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_factory::reducer::{FactoryReducer, FactoryRunStatus, FactoryState};
use sps_factory::workflow::{
    FactoryStage, FactoryWorkflow, ProjectRequest,
};

fn fresh_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    FactoryReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

#[test]
fn factory_analyzes_rust_cli_request() {
    let req = ProjectRequest {
        description: "I want a rust CLI that says hello".into(),
        preferred_name: Some(SmolStr::new("hello-cli")),
        output_dir: None,
    };
    let spec = FactoryWorkflow::analyze_requirement(&req);
    assert_eq!(spec.name, "hello-cli");
    assert_eq!(spec.kind, "rust_cli");
    assert!(!spec.requirements.is_empty());
}

#[test]
fn factory_analyzes_nextjs_request() {
    let req = ProjectRequest {
        description: "build a next.js dashboard".into(),
        preferred_name: None,
        output_dir: None,
    };
    let spec = FactoryWorkflow::analyze_requirement(&req);
    assert_eq!(spec.kind, "nextjs");
    assert_eq!(spec.name, "sps-project"); // default
}

#[test]
fn factory_designs_rust_cli_architecture() {
    let spec = sps_factory::workflow::RequirementSpec {
        name: "test".into(),
        kind: "rust_cli".into(),
        requirements: vec!["cli".into()],
        non_functional: vec![],
    };
    let arch = FactoryWorkflow::design_architecture(&spec);
    assert!(arch.stack.contains(&SmolStr::new("rust")));
    assert!(arch.file_layout.contains(&"Cargo.toml".to_string()));
    assert!(arch.file_layout.contains(&"src/main.rs".to_string()));
}

#[test]
fn factory_generates_rust_cli_files() {
    let spec = sps_factory::workflow::RequirementSpec {
        name: "my-cli".into(),
        kind: "rust_cli".into(),
        requirements: vec!["cli".into()],
        non_functional: vec![],
    };
    let files = FactoryWorkflow::generate_code(&spec, "/tmp/my-cli");
    assert!(files.iter().any(|f| f.path == "Cargo.toml"));
    assert!(files.iter().any(|f| f.path == "src/main.rs"));
}

#[test]
fn factory_full_run_produces_files() {
    let req = ProjectRequest {
        description: "build a tauri desktop app".into(),
        preferred_name: Some(SmolStr::new("my-app")),
        output_dir: Some("/tmp/my-app".into()),
    };
    let files = FactoryWorkflow::run(req, "/tmp/my-app");
    assert!(files.iter().any(|f| f.path == "src-tauri/Cargo.toml"));
    assert!(files.iter().any(|f| f.path == "package.json"));
}

#[test]
fn factory_stage_progression() {
    let stages = FactoryStage::all();
    assert_eq!(stages.len(), 8);
    assert_eq!(stages[0], FactoryStage::RequirementAnalysis);
    assert_eq!(stages[7], FactoryStage::DeploymentPrep);
}

#[test]
fn factory_run_started_event_persists() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let id = uuid::Uuid::now_v7();
    let event = RawEvent::new(
        "factory.run_started",
        json!({"id": id, "project_name": "test-project"}),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let fs = FactoryState::from_state(&state).unwrap();
    assert_eq!(fs.runs.len(), 1);
    let run = fs.runs.get(&id).unwrap();
    assert_eq!(run.project_name, "test-project");
    assert_eq!(run.status, FactoryRunStatus::Running);
}

#[test]
fn factory_stage_completed_advances() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let id = uuid::Uuid::now_v7();
    let e1 = RawEvent::new(
        "factory.run_started",
        json!({"id": id, "project_name": "test"}),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();

    let e2 = RawEvent::new(
        "factory.stage_completed",
        json!({"id": id, "stage": "requirement_analysis"}),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();
    let fs = FactoryState::from_state(&state).unwrap();
    let run = fs.runs.get(&id).unwrap();
    assert!(run.completed_stages.contains(&FactoryStage::RequirementAnalysis));
    assert_eq!(run.current_stage, Some(FactoryStage::ArchitectureDesign));

    let e3 = RawEvent::new(
        "factory.stage_completed",
        json!({"id": id, "stage": "code_generation", "files_generated": 3u64}),
        Actor::owner(),
        0,
    )
    .finalize(3, e2.hash);
    pipeline.apply(&mut state, &e3).unwrap();
    let fs = FactoryState::from_state(&state).unwrap();
    let run = fs.runs.get(&id).unwrap();
    assert_eq!(run.files_generated, 3);
}

#[test]
fn factory_run_completed_sets_status() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let id = uuid::Uuid::now_v7();
    let e1 = RawEvent::new(
        "factory.run_started",
        json!({"id": id, "project_name": "test"}),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();
    let e2 = RawEvent::new(
        "factory.run_completed",
        json!({"id": id}),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();
    let fs = FactoryState::from_state(&state).unwrap();
    let run = fs.runs.get(&id).unwrap();
    assert_eq!(run.status, FactoryRunStatus::Completed);
    assert_eq!(run.current_stage, None);
}
