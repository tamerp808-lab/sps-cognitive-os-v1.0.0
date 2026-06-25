//! P2.6: Reducer Slice Profiling
//!
//! Measures each reducer individually by dispatching events of each type
//! and comparing dispatch_trusted time. The delta between event types
//! reveals which reducer's from_state/save_to round-trip is most expensive.

use std::sync::Arc;
use std::time::Instant;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::storage::port::StoragePort;
use sps_storage_memory::InMemoryStorage;

fn boot_kernel(storage: Arc<dyn StoragePort>) -> Arc<SpsKernel> {
    SpsKernel::boot_with(storage, KernelConfig::default(), |reg| {
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
    }).unwrap().into()
}

fn avg_dispatch_time(kernel: &SpsKernel, event_type: &str, payload: serde_json::Value, samples: usize) -> f64 {
    let mut times = Vec::new();
    for _ in 0..samples {
        let raw = RawEvent::new(event_type, payload.clone(), Actor::owner(), 0);
        let t = Instant::now();
        let _ = kernel.dispatch_trusted(raw);
        times.push(t.elapsed().as_micros() as f64);
    }
    times.iter().sum::<f64>() / times.len() as f64
}

fn make_memory_payload(i: usize) -> serde_json::Value {
    let record = sps_memory::memory::MemoryRecord {
        id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
        kind: sps_memory::memory::MemoryKind::Episodic,
        title: SmolStr::new(format!("mem-{}", i)),
        content: json!({"i": i}),
        tags: vec![], origin_tick: 0, created_at: 0,
    };
    serde_json::to_value(&record).unwrap()
}

fn make_goal_payload(i: usize) -> serde_json::Value {
    let goal = sps_goals::hierarchy::Goal {
        id: sps_goals::hierarchy::GoalId(uuid::Uuid::now_v7()),
        title: SmolStr::new(format!("goal-{}", i)),
        description: "test".into(),
        priority: 5,
        status: sps_goals::hierarchy::GoalStatus::Active,
        objectives: vec![],
        dependencies: vec![],
        created_at: 0,
        origin_tick: 0,
    };
    serde_json::to_value(&goal).unwrap()
}

fn make_execution_payload(i: usize) -> serde_json::Value {
    json!({"operation": format!("op-{}", i), "duration_ms": 100})
}

fn make_reflection_payload(i: usize) -> serde_json::Value {
    let analysis = sps_reflection::analyzers::SuccessAnalyzer::analyze(
        uuid::Uuid::now_v7(),
        vec![format!("step-{}", i)],
        format!("reason-{}", i),
        true,
    );
    serde_json::to_value(&analysis).unwrap()
}

fn make_plan_payload(i: usize) -> serde_json::Value {
    let plan = sps_planner::plan::Plan {
        id: sps_planner::plan::PlanId::new(),
        goal_id: sps_goals::hierarchy::GoalId(uuid::Uuid::now_v7()),
        template: SmolStr::new("test"),
        steps: vec![],
        status: sps_planner::plan::PlanStatus::Draft,
        created_at: 0,
        origin_tick: 0,
    };
    serde_json::to_value(&plan).unwrap()
}

fn make_world_project_payload(i: usize) -> serde_json::Value {
    json!({
        "id": uuid::Uuid::now_v7().to_string(),
        "name": format!("proj-{}", i),
        "path": "/tmp/test",
        "tags": [],
        "created_at": 0,
        "origin_tick": 0,
    })
}

#[test]
fn p2_6_reducer_slice_profiling() {
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("  P2.6: Reducer Slice Profiling");
    println!("  Dispatches events of each type and measures per-event cost.");
    println!("  Higher cost = more expensive from_state/save_to round-trip.");
    println!("═══════════════════════════════════════════════════════════════\n");

    let n = 500; // events of each type
    let samples = 20; // measurement samples per type

    // Test each reducer type independently (separate kernel per type).
    let tests: Vec<(&str, fn(usize) -> serde_json::Value)> = vec![
        ("memory.created", make_memory_payload),
        ("goal.created", make_goal_payload),
        ("execution.succeeded", make_execution_payload),
        ("reflection.success_analyzed", make_reflection_payload),
        ("plan.created", make_plan_payload),
        ("world.project_added", make_world_project_payload),
    ];

    println!("  Profiling at {} events per type, {} samples per measurement.\n", n, samples);
    println!("  {:>30} | {:>12} | {:>12} | {:>12}", "Event Type", "Baseline(μs)", "After N(μs)", "Growth(x)");
    println!("  {:>30} | {:>12} | {:>12} | {:>12}", "------------------------------", "------------", "------------", "------------");

    for &(event_type, make_payload) in &tests {
        let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
        let kernel = boot_kernel(storage.clone());

        // Baseline: dispatch 1 event, measure the 2nd.
        kernel.dispatch_trusted(RawEvent::new(event_type, make_payload(0), Actor::owner(), 0)).unwrap();
        let baseline = avg_dispatch_time(&kernel, event_type, make_payload(999), 10);

        // Pre-populate with N events.
        for i in 1..n {
            kernel.dispatch_trusted(RawEvent::new(event_type, make_payload(i), Actor::owner(), 0)).unwrap();
        }

        // Measure after N events.
        let after_n = avg_dispatch_time(&kernel, event_type, make_payload(998), samples);
        let growth = after_n / baseline.max(1.0);

        println!("  {:>30} | {:>10.1}μs | {:>10.1}μs | {:>10.1}x", event_type, baseline, after_n, growth);
    }

    // Also test with MIXED state (all reducers populated).
    println!("\n  Mixed state (all reducers populated with {} events each):\n", n);
    println!("  {:>30} | {:>12} | {:>12}", "Event Type", "After Mix(μs)", "vs Baseline");
    println!("  {:>30} | {:>12} | {:>12}", "------------------------------", "------------", "------------");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Populate all slices.
    for i in 0..n {
        kernel.dispatch_trusted(RawEvent::new("memory.created", make_memory_payload(i), Actor::owner(), 0)).unwrap();
        kernel.dispatch_trusted(RawEvent::new("goal.created", make_goal_payload(i), Actor::owner(), 0)).unwrap();
        kernel.dispatch_trusted(RawEvent::new("execution.succeeded", make_execution_payload(i), Actor::owner(), 0)).unwrap();
        kernel.dispatch_trusted(RawEvent::new("reflection.success_analyzed", make_reflection_payload(i), Actor::owner(), 0)).unwrap();
        kernel.dispatch_trusted(RawEvent::new("plan.created", make_plan_payload(i), Actor::owner(), 0)).unwrap();
        kernel.dispatch_trusted(RawEvent::new("world.project_added", make_world_project_payload(i), Actor::owner(), 0)).unwrap();
    }

    for &(event_type, make_payload) in &tests {
        let t = avg_dispatch_time(&kernel, event_type, make_payload(997), samples);
        println!("  {:>30} | {:>10.1}μs | {:>10.1}x baseline", event_type, t, t / 100.0);
    }

    println!("\n  Analysis:");
    println!("  - Higher 'After N' = more expensive from_state/save_to round-trip.");
    println!("  - If MemoryReducer is highest: MemoryState BTreeMap is the bottleneck.");
    println!("  - If WorldReducer is highest: WorldGraph with multiple BTreeMaps.");
    println!("  - Growth > 1.0x means the reducer is O(n) per dispatch.");
    println!("  - Mixed state shows cross-reducer overhead (each reducer deserializes");
    println!("    its own slice, but CanonicalState::get_extension scans all keys).");

    println!("\n  === P2.6 COMPLETE ===");
}
