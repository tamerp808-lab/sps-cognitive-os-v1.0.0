//! Phase 10 — Self-Improvement tests.

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_improvement::analyzers::{
    BottleneckDetector, PerformanceAnalyzer, PerformanceReport, PromptOptimizer,
    WorkflowOptimizer,
};
use sps_improvement::reducer::{
    ImprovementProposal, ImprovementReducer, ImprovementState, ImprovementStatus,
};
use sps_improvement::analyzers::OptimizationKind;

fn fresh_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    ImprovementReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

fn make_report() -> PerformanceReport {
    let mut avg_latencies_ms = std::collections::BTreeMap::new();
    avg_latencies_ms.insert("effects".into(), 50.0);
    avg_latencies_ms.insert("memory".into(), 2000.0);
    avg_latencies_ms.insert("world".into(), 100.0);
    let mut failure_rates = std::collections::BTreeMap::new();
    failure_rates.insert("effects".into(), 0.01);
    failure_rates.insert("providers".into(), 0.10);
    PerformanceReport {
        avg_latencies_ms,
        failure_rates,
        total_events: 1000,
    }
}

#[test]
fn performance_analyzer_finds_slow_subsystems() {
    let report = make_report();
    let slow = PerformanceAnalyzer::analyze(&report);
    assert!(slow.contains(&"memory".to_string()));
    assert!(!slow.contains(&"effects".to_string()));
}

#[test]
fn bottleneck_detector_finds_latency_bottlenecks() {
    let report = make_report();
    let bottlenecks = BottleneckDetector::detect(&report);
    // memory (2000ms) + providers (10% failure) = 2 bottlenecks
    assert_eq!(bottlenecks.len(), 2);
    assert!(bottlenecks.iter().any(|b| b.subsystem == "memory"));
    assert!(bottlenecks.iter().any(|b| b.subsystem == "providers"));
}

#[test]
fn workflow_optimizer_proposes() {
    let p = WorkflowOptimizer::propose("generic.workflow", "skip review step", 0.2);
    assert_eq!(p.workflow, "generic.workflow");
    assert!((p.estimated_improvement - 0.2).abs() < 0.001);
}

#[test]
fn prompt_optimizer_proposes() {
    let p = PromptOptimizer::propose(
        "developer",
        "current: do the task",
        "proposed: do the task step by step",
        "step-by-step reduces errors",
    );
    assert_eq!(p.agent_archetype, "developer");
}

#[test]
fn improvement_proposed_event_persists() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let proposal = ImprovementProposal {
        id: uuid::Uuid::now_v7(),
        kind: OptimizationKind::Workflow,
        description: "skip review".into(),
        status: ImprovementStatus::Proposed,
        origin_tick: 1,
        workflow: None,
        prompt: None,
        subsystem: "planner".into(),
    };
    let id = proposal.id;
    let event = RawEvent::new(
        "improvement.proposed",
        serde_json::to_value(&proposal).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let is = ImprovementState::from_state(&state).unwrap();
    assert_eq!(is.proposals.len(), 1);
    assert_eq!(is.proposals.get(&id).unwrap().status, ImprovementStatus::Proposed);
}

#[test]
fn improvement_lifecycle_approved_applied_reverted() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();

    // Propose
    let proposal = ImprovementProposal {
        id: uuid::Uuid::now_v7(),
        kind: OptimizationKind::Workflow,
        description: "test".into(),
        status: ImprovementStatus::Proposed,
        origin_tick: 1,
        workflow: None,
        prompt: None,
        subsystem: "planner".into(),
    };
    let id = proposal.id;
    let e1 = RawEvent::new(
        "improvement.proposed",
        serde_json::to_value(&proposal).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();

    // Approve
    let e2 = RawEvent::new(
        "improvement.approved",
        json!({"id": id}),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();
    assert_eq!(
        ImprovementState::from_state(&state).unwrap().proposals.get(&id).unwrap().status,
        ImprovementStatus::Approved
    );

    // Apply
    let e3 = RawEvent::new(
        "improvement.applied",
        json!({"id": id}),
        Actor::owner(),
        0,
    )
    .finalize(3, e2.hash);
    pipeline.apply(&mut state, &e3).unwrap();
    assert_eq!(
        ImprovementState::from_state(&state).unwrap().proposals.get(&id).unwrap().status,
        ImprovementStatus::Applied
    );

    // Revert
    let e4 = RawEvent::new(
        "improvement.reverted",
        json!({"id": id}),
        Actor::owner(),
        0,
    )
    .finalize(4, e3.hash);
    pipeline.apply(&mut state, &e4).unwrap();
    assert_eq!(
        ImprovementState::from_state(&state).unwrap().proposals.get(&id).unwrap().status,
        ImprovementStatus::Reverted
    );
}
