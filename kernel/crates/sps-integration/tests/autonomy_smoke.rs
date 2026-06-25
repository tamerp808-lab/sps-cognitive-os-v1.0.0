//! Autonomy Smoke Tests — verify Fix #11a works before full validation.

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::reducer::ReducerRegistry;
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::storage::port::StoragePort;
use sps_storage_memory::InMemoryStorage;

fn boot_kernel(storage: Arc<dyn StoragePort>) -> Arc<SpsKernel> {
    let kernel = SpsKernel::boot_with(storage, KernelConfig::default(), |reg| {
        sps_bus::state_ext::OwnerReducer::register(reg);
        sps_goals::reducer::GoalReducer::register(reg);
        sps_memory::reducer::MemoryReducer::register(reg);
        sps_reflection::reducer::ReflectionReducer::register(reg);
        sps_planner::reducer::PlannerReducer::register(reg);
        sps_world::reducer::WorldReducer::register(reg);
        sps_agents::reducer::AgentReducer::register(reg);
        sps_reasoning::reducer::ReasoningReducer::register(reg);
        sps_improvement::reducer::ImprovementReducer::register(reg);
        sps_execution::reducer::ExecutionReducer::register(reg);
        sps_factory::reducer::FactoryReducer::register(reg);
        sps_autonomy::reducer::AutonomyReducer::register(reg);
        sps_vectors::reducer::VectorReducer::register(reg);
    })
    .expect("kernel boot failed");
    Arc::new(kernel)
}

#[test]
fn smoke_1_goal_activated_materializes() {
    println!("\n=== AUTONOMY SMOKE 1: autonomous.goal_activated → AutonomyState.active_goals ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let goal_id = uuid::Uuid::now_v7();
    kernel.dispatch(RawEvent::new(
        "autonomous.goal_activated",
        json!({
            "goal_id": goal_id.to_string(),
            "milestones": [{"title": "M1"}],
            "activated_at": 12345,
        }),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Dispatched autonomous.goal_activated");

    let count = kernel.query(|s| {
        sps_autonomy::reducer::AutonomyState::from_state(s)
            .map(|as_| as_.active_goals.len())
            .unwrap_or(0)
    });
    if count == 1 {
        println!("  PASS — AutonomyState.active_goals has 1 entry");
    } else {
        println!("  FAIL — expected 1, got {}", count);
        panic!("AUTONOMY SMOKE 1 FAILED");
    }
}

#[test]
fn smoke_2_weekly_review_materializes() {
    println!("\n=== AUTONOMY SMOKE 2: autonomous.weekly_review → AutonomyState.reviews ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let goal_id = uuid::Uuid::now_v7();
    kernel.dispatch(RawEvent::new(
        "autonomous.weekly_review",
        json!({
            "goal_id": goal_id.to_string(),
            "review": "Progress is on track. 3 tasks completed.",
            "reviewed_at": 67890,
        }),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Dispatched autonomous.weekly_review");

    let count = kernel.query(|s| {
        sps_autonomy::reducer::AutonomyState::from_state(s)
            .map(|as_| as_.reviews.len())
            .unwrap_or(0)
    });
    if count == 1 {
        println!("  PASS — AutonomyState.reviews has 1 entry");
    } else {
        println!("  FAIL — expected 1, got {}", count);
        panic!("AUTONOMY SMOKE 2 FAILED");
    }
}

#[test]
fn smoke_3_replay_identical() {
    println!("\n=== AUTONOMY SMOKE 3: replay → active_goals + reviews + hash identical ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let goal_id = uuid::Uuid::now_v7();
    kernel.dispatch(RawEvent::new(
        "autonomous.goal_activated",
        json!({"goal_id": goal_id.to_string(), "milestones": [], "activated_at": 1}),
        Actor::owner(), 0,
    )).unwrap();
    kernel.dispatch(RawEvent::new(
        "autonomous.weekly_review",
        json!({"goal_id": goal_id.to_string(), "review": "test review", "reviewed_at": 2}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Dispatched 1 activation + 1 review");

    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();
    let live_active = sps_autonomy::reducer::AutonomyState::from_state(&live).unwrap().active_goals.len();
    let live_reviews = sps_autonomy::reducer::AutonomyState::from_state(&live).unwrap().reviews.len();

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken");
    println!("  PASS — hash chain verified ({} events)", report.events_verified);

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_autonomy::reducer::AutonomyReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.last_hash(), live_hash, "FAIL: hash mismatch");
    println!("  PASS — hash identical");

    let replayed_as = sps_autonomy::reducer::AutonomyState::from_state(&replayed).unwrap();
    if replayed_as.active_goals.len() == live_active && replayed_as.reviews.len() == live_reviews {
        println!("  PASS — active_goals ({} == {}) + reviews ({} == {}) match",
            replayed_as.active_goals.len(), live_active,
            replayed_as.reviews.len(), live_reviews);
    } else {
        println!("  FAIL — live: {} active + {} reviews, replayed: {} active + {} reviews",
            live_active, live_reviews,
            replayed_as.active_goals.len(), replayed_as.reviews.len());
        panic!("AUTONOMY SMOKE 3 FAILED");
    }

    println!("\n  === AUTONOMY SMOKE TESTS PASSED ===");
}
