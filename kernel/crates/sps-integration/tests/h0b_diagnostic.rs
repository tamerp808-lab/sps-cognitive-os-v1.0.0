//! H0b: Reproduce the exact Loop Test sequence to find the drift.
//!
//! H0 showed no drift with simple memory.created events.
//! This test reproduces the EXACT event sequence from the Loop Test
//! to find which specific event type causes the drift.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::kernel::{KernelConfig, SpsKernel};
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

fn check(kernel: &SpsKernel, label: &str) {
    let store = kernel.store().count().unwrap_or(0);
    let meta = kernel.query(|s| s.event_count());
    let status = if store == meta { "OK" } else { "DRIFT!" };
    println!("  {:>30}: store={:>3}, meta={:>3}  {}", label, store, meta, status);
}

#[test]
fn h0b_reproduce_loop_sequence() {
    println!("\n=== H0b: Reproduce Loop Test sequence ===\n");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Stage 1: goal.created
    let goal_id = sps_goals::hierarchy::GoalId(uuid::Uuid::now_v7());
    let goal = sps_goals::hierarchy::Goal {
        id: goal_id, title: SmolStr::new("Learn Rust"),
        description: "Master Rust".to_string(), priority: 5,
        status: sps_goals::hierarchy::GoalStatus::Active,
        objectives: Vec::new(), dependencies: Vec::new(),
        created_at: 0, origin_tick: 0,
    };
    kernel.dispatch(RawEvent::new("goal.created", serde_json::to_value(&goal).unwrap(), Actor::owner(), 0)).unwrap();
    check(&kernel, "goal.created");

    // Stage 2: plan.created
    let plan_id = sps_planner::plan::PlanId::new();
    let plan = sps_planner::plan::Plan {
        id: plan_id, goal_id, template: SmolStr::new("learning"),
        steps: vec![], status: sps_planner::plan::PlanStatus::Approved,
        created_at: 0, origin_tick: 0,
    };
    kernel.dispatch(RawEvent::new("plan.created", serde_json::to_value(&plan).unwrap(), Actor::owner(), 0)).unwrap();
    check(&kernel, "plan.created");

    // Stage 3: execution.succeeded × 2
    kernel.dispatch(RawEvent::new("execution.succeeded", json!({"operation": "Read The Book", "plan_id": plan_id.0.to_string()}), Actor::owner(), 0)).unwrap();
    check(&kernel, "execution.succeeded #1");
    kernel.dispatch(RawEvent::new("execution.succeeded", json!({"operation": "Write Code", "plan_id": plan_id.0.to_string()}), Actor::owner(), 0)).unwrap();
    check(&kernel, "execution.succeeded #2");

    // Stage 4: reflection.success_analyzed
    let analysis = sps_reflection::analyzers::SuccessAnalyzer::analyze(
        uuid::Uuid::now_v7(), vec!["read book".into()], "practice".into(), true,
    );
    kernel.dispatch(RawEvent::new("reflection.success_analyzed", serde_json::to_value(&analysis).unwrap(), Actor::owner(), 0)).unwrap();
    check(&kernel, "reflection.success_analyzed");

    // Stage 5: memory.created × 2
    for i in 0..2 {
        let record = sps_memory::memory::MemoryRecord {
            id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
            kind: sps_memory::memory::MemoryKind::Semantic,
            title: SmolStr::new(format!("mem-{}", i)),
            content: json!({}), tags: vec![], origin_tick: 0, created_at: 0,
        };
        kernel.dispatch(RawEvent::new("memory.created", serde_json::to_value(&record).unwrap(), Actor::owner(), 0)).unwrap();
        check(&kernel, &format!("memory.created #{}", i + 1));
    }

    // Stage 6: autonomous.goal_activated + autonomous.weekly_review
    kernel.dispatch(RawEvent::new("autonomous.goal_activated", json!({"goal_id": goal_id.0.to_string(), "milestones": [], "activated_at": 1000}), Actor::owner(), 0)).unwrap();
    check(&kernel, "autonomous.goal_activated");

    kernel.dispatch(RawEvent::new("autonomous.weekly_review", json!({"goal_id": goal_id.0.to_string(), "review": "Good", "reviewed_at": 2000}), Actor::owner(), 0)).unwrap();
    check(&kernel, "autonomous.weekly_review");

    // Stage 7: goal.progress_updated
    kernel.dispatch(RawEvent::new("goal.progress_updated", json!({"goal_id": goal_id.0.to_string(), "milestone": "Week 1", "completed": true}), Actor::owner(), 0)).unwrap();
    check(&kernel, "goal.progress_updated");

    println!("\n  Final: store={}, meta={}", kernel.store().count().unwrap_or(0), kernel.query(|s| s.event_count()));
}
