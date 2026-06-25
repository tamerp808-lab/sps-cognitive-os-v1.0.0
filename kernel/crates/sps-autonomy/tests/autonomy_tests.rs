//! Phase 12 — Autonomy tests.

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_goals::GoalId;
use sps_autonomy::governor::{AutonomyGovernor, AutonomyStatus, LongRunningGoalRunner};
use sps_autonomy::reducer::{AutonomyReducer, AutonomyState};
use sps_autonomy::sandbox::{AutonomySandbox, SandboxBoundary, SandboxViolation};
use std::path::PathBuf;

fn fresh_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    AutonomyReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

// --- Governor tests ---

#[test]
fn autonomy_governor_starts_disabled() {
    let g = AutonomyGovernor::new();
    assert_eq!(g.config().status, AutonomyStatus::Disabled);
    assert!(!g.is_enabled());
}

#[test]
fn autonomy_governor_enable_disable() {
    let g = AutonomyGovernor::new();
    let prev = g.enable();
    assert_eq!(prev, AutonomyStatus::Disabled);
    assert!(g.is_enabled());

    g.disable();
    assert!(!g.is_enabled());
}

#[test]
fn autonomy_governor_pause() {
    let g = AutonomyGovernor::new();
    g.enable();
    g.pause();
    assert_eq!(g.config().status, AutonomyStatus::Paused);
    assert!(!g.is_enabled());
}

// --- Long-running goal runner tests ---

#[test]
fn long_running_runner_rejects_when_disabled() {
    let g = Arc::new(AutonomyGovernor::new());
    let runner = LongRunningGoalRunner::new(g);
    let result = runner.start(GoalId::new());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not enabled"));
}

#[test]
fn long_running_runner_starts_when_enabled() {
    let g = Arc::new(AutonomyGovernor::new());
    g.enable();
    let runner = LongRunningGoalRunner::new(g);
    let id = GoalId::new();
    runner.start(id).unwrap();
    assert_eq!(runner.active().len(), 1);
}

#[test]
fn long_running_runner_max_concurrent() {
    let g = Arc::new(AutonomyGovernor::new());
    g.enable();
    let runner = LongRunningGoalRunner::new(g.clone());
    runner.start(GoalId::new()).unwrap();
    let result = runner.start(GoalId::new());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("max concurrent"));
}

#[test]
fn long_running_runner_stop_removes_goal() {
    let g = Arc::new(AutonomyGovernor::new());
    g.enable();
    let runner = LongRunningGoalRunner::new(g);
    let id = GoalId::new();
    runner.start(id).unwrap();
    assert_eq!(runner.active().len(), 1);
    assert!(runner.stop(id));
    assert_eq!(runner.active().len(), 0);
}

// --- Sandbox tests (already covered in sandbox.rs unit tests) ---

#[test]
fn sandbox_boundary_default_is_empty() {
    let b = SandboxBoundary::default();
    assert!(b.allowed_roots.is_empty());
    assert!(b.denied_paths.is_empty());
}

#[test]
fn sandbox_check_outside_boundary() {
    let sandbox = AutonomySandbox::with_boundary(SandboxBoundary::new(vec![
        PathBuf::from("/workspace"),
    ]));
    let result = sandbox.check(&PathBuf::from("/etc/passwd"));
    assert!(matches!(result, Err(SandboxViolation::OutsideBoundary { .. })));
}

// --- Reducer tests ---

#[test]
fn autonomy_default_state_is_disabled() {
    let state = AutonomyState::default();
    assert_eq!(state.config.status, AutonomyStatus::Disabled);
}

#[test]
fn autonomy_enabled_event_sets_status() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let event = RawEvent::new("autonomy.enabled", json!({}), Actor::owner(), 0)
        .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let as_ = AutonomyState::from_state(&state).unwrap();
    assert_eq!(as_.config.status, AutonomyStatus::Enabled);
}

#[test]
fn autonomy_disabled_event_sets_status() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    // First enable.
    let e1 = RawEvent::new("autonomy.enabled", json!({}), Actor::owner(), 0)
        .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();
    // Then disable.
    let e2 = RawEvent::new("autonomy.disabled", json!({}), Actor::owner(), 0)
        .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();
    let as_ = AutonomyState::from_state(&state).unwrap();
    assert_eq!(as_.config.status, AutonomyStatus::Disabled);
}

#[test]
fn autonomy_paused_event_sets_status() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let e1 = RawEvent::new("autonomy.enabled", json!({}), Actor::owner(), 0)
        .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();
    let e2 = RawEvent::new("autonomy.paused", json!({}), Actor::owner(), 0)
        .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();
    let as_ = AutonomyState::from_state(&state).unwrap();
    assert_eq!(as_.config.status, AutonomyStatus::Paused);
}
