//! Phase 5 — Reasoning Engine tests.

use std::sync::Arc;

use sps_core::state::CanonicalState;
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use serde_json::json;
use uuid::Uuid;

use sps_reasoning::analyzers::{
    ConflictDetector, DependencySolver, GoalAnalyzer, PlanOptimizer, RiskAnalyzer, TaskDecomposer,
};
use sps_reasoning::reducer::{ReasoningReducer, ReasoningState, ReasoningStep};

fn fresh_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    ReasoningReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

#[test]
fn goal_analyzer_scores_short_descriptions_as_ambiguous() {
    let id = Uuid::now_v7();
    let analysis = GoalAnalyzer::analyze(id, "do the thing");
    assert!(analysis.ambiguity > 0.5);
    assert!(analysis.feasibility < 1.0);
    assert!(!analysis.suggestions.is_empty());
}

#[test]
fn goal_analyzer_scores_detailed_descriptions_as_clear() {
    let id = Uuid::now_v7();
    let analysis = GoalAnalyzer::analyze(id, "Create a Rust web server with actix-web that listens on port 8080 and responds with hello world");
    assert!(analysis.ambiguity < 0.5);
}

#[test]
fn task_decomposer_splits_by_sentences() {
    let id = Uuid::now_v7();
    let decomp = TaskDecomposer::decompose(id, "Setup project. Write code. Run tests.");
    assert_eq!(decomp.tasks.len(), 3);
    assert_eq!(decomp.dependencies.len(), 2);
}

#[test]
fn dependency_solver_topological_sort() {
    // 0 → 1, 0 → 2, 1 → 3, 2 → 3
    let deps = vec![(0, 1), (0, 2), (1, 3), (2, 3)];
    let order = DependencySolver::solve(4, &deps).unwrap();
    assert_eq!(order.len(), 4);
    // 0 must come before 1 and 2; 1 and 2 must come before 3.
    let pos: std::collections::HashMap<u32, usize> =
        order.iter().enumerate().map(|(i, &v)| (v, i)).collect();
    assert!(pos[&0] < pos[&1]);
    assert!(pos[&0] < pos[&2]);
    assert!(pos[&1] < pos[&3]);
    assert!(pos[&2] < pos[&3]);
}

#[test]
fn dependency_solver_detects_cycle() {
    let deps = vec![(0, 1), (1, 0)];
    let result = DependencySolver::solve(2, &deps);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cycle"));
}

#[test]
fn conflict_detector_finds_resource_conflicts() {
    let t1 = Uuid::now_v7();
    let t2 = Uuid::now_v7();
    let t3 = Uuid::now_v7();
    let assignments = vec![
        (t1, "file:src/main.rs".to_string()),
        (t2, "file:src/main.rs".to_string()),
        (t3, "file:src/lib.rs".to_string()),
    ];
    let conflicts = ConflictDetector::detect(&assignments);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].entities.len(), 2);
}

#[test]
fn risk_analyzer_assesses_complexity() {
    let id = Uuid::now_v7();
    let risk = RiskAnalyzer::assess(id, "deploy to production", 8);
    assert!(risk.risk_score > 0.5);
    assert!(!risk.factors.is_empty());
}

#[test]
fn plan_optimizer_parallelizes() {
    // 0 → 2, 1 → 2 (0 and 1 can run in parallel; 2 must wait).
    let deps = vec![(0, 2), (1, 2)];
    let batches = PlanOptimizer::parallelize(3, &deps).unwrap();
    assert_eq!(batches.len(), 2);
    assert!(batches[0].contains(&0));
    assert!(batches[0].contains(&1));
    assert_eq!(batches[1], vec![2]);
}

#[test]
fn reasoning_step_event_updates_state() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let step = ReasoningStep {
        id: Uuid::now_v7(),
        analyzer: "goal_analyzer".into(),
        input: "test goal".into(),
        output: json!({"feasibility": 0.8}),
        tick: 1,
    };
    let event = RawEvent::new(
        "reasoning.step",
        serde_json::to_value(&step).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let rs = ReasoningState::from_state(&state).unwrap();
    assert_eq!(rs.steps.len(), 1);
}
