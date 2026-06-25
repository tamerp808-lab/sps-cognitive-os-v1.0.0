//! H0: Event Count Drift Diagnostic
//!
//! Verifies store.count() == meta.event_count() after every dispatch.
//! This is the invariant that Bug #16 (KernelMetaReducer double-registration)
//! violated. After Fix #1 + #3 + #16, it should pass.

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

#[test]
fn h0_no_event_count_drift() {
    println!("\n=== H0: Event Count Drift Diagnostic ===");
    println!("  Checking store.count() == meta.event_count() after every dispatch.\n");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let mut drift_found = false;

    for i in 1..=20 {
        let record = sps_memory::memory::MemoryRecord {
            id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
            kind: sps_memory::memory::MemoryKind::Episodic,
            title: SmolStr::new(format!("event-{}", i)),
            content: json!({}),
            tags: vec![],
            origin_tick: 0,
            created_at: 0,
        };
        let payload = serde_json::to_value(&record).unwrap();
        kernel.dispatch(RawEvent::new("memory.created", payload, Actor::owner(), 0)).unwrap();

        let store_count = kernel.store().count().unwrap_or(0);
        let meta_count = kernel.query(|s| s.event_count());

        let status = if store_count == meta_count { "OK" } else { "DRIFT!" };
        println!("  dispatch {:>2}: store={:>3}, meta={:>3}  {}", i, store_count, meta_count, status);

        if store_count != meta_count {
            drift_found = true;
        }
    }

    // Now test with autonomous.* events (which were the original drift source).
    for i in 0..5 {
        kernel.dispatch(RawEvent::new(
            "autonomous.goal_activated",
            json!({"goal_id": uuid::Uuid::now_v7().to_string(), "milestones": [], "activated_at": 0}),
            Actor::owner(), 0,
        )).unwrap();

        let store_count = kernel.store().count().unwrap_or(0);
        let meta_count = kernel.query(|s| s.event_count());
        let status = if store_count == meta_count { "OK" } else { "DRIFT!" };
        println!("  autonomous #{}: store={:>3}, meta={:>3}  {}", i + 1, store_count, meta_count, status);

        if store_count != meta_count {
            drift_found = true;
        }
    }

    let final_store = kernel.store().count().unwrap_or(0);
    let final_meta = kernel.query(|s| s.event_count());

    println!("\n  Final: store={}, meta={}", final_store, final_meta);

    if drift_found {
        println!("  FAIL — drift detected!");
        panic!("H0 FAILED — store.count != meta.event_count");
    } else {
        println!("  PASS — store.count == meta.event_count for all 25 dispatches");
    }
}
