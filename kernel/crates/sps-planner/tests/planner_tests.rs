//! Phase 7 — Planner tests.

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_goals::GoalId;
use sps_planner::plan::{Plan, PlanStatus};
use sps_planner::reducer::{PlannerReducer, PlannerState};
use sps_planner::templates::{builtin_templates, TemplateRegistry};

fn fresh_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    PlannerReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

#[test]
fn generic_workflow_template_generates_5_steps() {
    let templates = builtin_templates();
    let generic = templates.iter().find(|t| t.name == "generic.workflow").unwrap();
    let plan = generic.generate(GoalId::new(), 0, 0);
    assert_eq!(plan.step_count(), 5);
    assert_eq!(plan.template, "generic.workflow");
    // Steps should be linearly dependent (non-parallelizable).
    assert_eq!(plan.steps[0].depends_on, Vec::<u32>::new());
    assert_eq!(plan.steps[1].depends_on, vec![0]);
    assert_eq!(plan.steps[2].depends_on, vec![1]);
}

#[test]
fn research_template_has_parallel_steps() {
    let templates = builtin_templates();
    let research = templates.iter().find(|t| t.name == "research").unwrap();
    let plan = research.generate(GoalId::new(), 0, 0);
    assert_eq!(plan.step_count(), 3);
    // First two steps are parallelizable → no deps.
    assert!(plan.steps[0].parallelizable);
    assert!(plan.steps[1].parallelizable);
    assert_eq!(plan.steps[1].depends_on, Vec::<u32>::new());
}

#[test]
fn template_registry_registers_and_looks_up() {
    let reg = TemplateRegistry::new();
    let templates = builtin_templates();
    for t in templates {
        reg.register(t);
    }
    assert_eq!(reg.list().len(), 3);
    assert!(reg.get("generic.workflow").is_some());
    assert!(reg.get("research").is_some());
    assert!(reg.get("deployment").is_some());
    assert!(reg.get("nonexistent").is_none());
}

#[test]
fn plan_lifecycle_status_transitions() {
    let mut plan = Plan::new(GoalId::new(), "test");
    assert_eq!(plan.status, PlanStatus::Draft);
    plan.approve();
    assert_eq!(plan.status, PlanStatus::Approved);
    plan.start();
    assert_eq!(plan.status, PlanStatus::Executing);
    plan.complete();
    assert_eq!(plan.status, PlanStatus::Completed);
}

#[test]
fn plan_created_event_persists_plan() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let plan = builtin_templates()[0].generate(GoalId::new(), 0, 0);
    let event = RawEvent::new(
        "plan.created",
        serde_json::to_value(&plan).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let ps = PlannerState::from_state(&state).unwrap();
    assert_eq!(ps.plans.len(), 1);
}

#[test]
fn plan_status_changed_events() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let plan = builtin_templates()[0].generate(GoalId::new(), 0, 0);
    let plan_id = plan.id;
    let e1 = RawEvent::new(
        "plan.created",
        serde_json::to_value(&plan).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();

    let e2 = RawEvent::new(
        "plan.approved",
        json!({"plan_id": plan_id, "status": "approved"}),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();

    let ps = PlannerState::from_state(&state).unwrap();
    assert_eq!(ps.plans.get(&plan_id.0).unwrap().status, PlanStatus::Approved);

    let e3 = RawEvent::new(
        "plan.completed",
        json!({"plan_id": plan_id, "status": "completed"}),
        Actor::owner(),
        0,
    )
    .finalize(3, e2.hash);
    pipeline.apply(&mut state, &e3).unwrap();
    let ps = PlannerState::from_state(&state).unwrap();
    assert_eq!(ps.plans.get(&plan_id.0).unwrap().status, PlanStatus::Completed);
}

#[test]
fn planner_state_round_trips() {
    let mut state = CanonicalState::genesis();
    let mut ps = PlannerState::default();
    let plan = builtin_templates()[0].generate(GoalId::new(), 0, 0);
    ps.plans.insert(plan.id.0, plan);
    ps.save_to(&mut state).unwrap();
    let loaded = PlannerState::from_state(&state).unwrap();
    assert_eq!(loaded.plans.len(), 1);
}
