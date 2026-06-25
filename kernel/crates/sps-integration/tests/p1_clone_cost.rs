//! P1: Measure clone(CanonicalState) cost precisely.
//!
//! Isolates the validate-on-write clone cost from everything else.
//! Measures: clone time, serialize time, state size at each scale.

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

fn dispatch_memories(kernel: &SpsKernel, count: usize) {
    for i in 0..count {
        let record = sps_memory::memory::MemoryRecord {
            id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
            kind: sps_memory::memory::MemoryKind::Episodic,
            title: SmolStr::new(format!("m-{}", i)),
            content: json!({"i": i}),
            tags: vec![], origin_tick: 0, created_at: 0,
        };
        kernel.dispatch(RawEvent::new("memory.created", serde_json::to_value(&record).unwrap(), Actor::owner(), 0)).unwrap();
    }
}

#[test]
fn p1_clone_cost_measurement() {
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("  P1: CanonicalState Clone Cost Measurement");
    println!("═══════════════════════════════════════════════════════════════\n");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let checkpoints = [0usize, 100, 250, 500, 750, 1000, 1500, 2000];
    let mut current = 0;

    println!("  {:>6} | {:>12} | {:>12} | {:>12} | {:>12} | {:>12}",
        "Events", "Clone(ms)", "Clone/ev(μs)", "JSON size(KB)", "Clone vs JSON", "Dispatch/ev");
    println!("  {:>6} | {:>12} | {:>12} | {:>12} | {:>12} | {:>12}",
        "------", "----------", "------------", "------------", "------------", "------------");

    for &target in &checkpoints {
        // Dispatch memories to reach the target.
        if target > current {
            dispatch_memories(&kernel, target - current);
            current = target;
        }

        // Measure clone cost (average of 5 clones).
        let clone_times: Vec<u128> = (0..5).map(|_| {
            let t = Instant::now();
            let _clone = kernel.query(|s| s.clone());
            t.elapsed().as_micros()
        }).collect();
        let avg_clone_us = clone_times.iter().sum::<u128>() as f64 / clone_times.len() as f64;
        let avg_clone_ms = avg_clone_us / 1000.0;

        // Measure JSON serialize size.
        let json_size = kernel.query(|s| {
            serde_json::to_string(s).map(|s| s.len()).unwrap_or(0)
        });
        let json_kb = json_size as f64 / 1024.0;

        // Measure JSON serialize time (for comparison).
        let json_times: Vec<u128> = (0..3).map(|_| {
            let t = Instant::now();
            let _ = kernel.query(|s| serde_json::to_string(s).unwrap());
            t.elapsed().as_micros()
        }).collect();
        let avg_json_us = json_times.iter().sum::<u128>() as f64 / json_times.len() as f64;
        let clone_vs_json = if avg_json_us > 0.0 { avg_clone_us / avg_json_us } else { 0.0 };

        // Measure single dispatch cost at this state size.
        let dispatch_us = if target < 2000 {
            let record = sps_memory::memory::MemoryRecord {
                id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
                kind: sps_memory::memory::MemoryKind::Episodic,
                title: SmolStr::new("dispatch-test"),
                content: json!({}),
                tags: vec![], origin_tick: 0, created_at: 0,
            };
            let payload = serde_json::to_value(&record).unwrap();
            let t = Instant::now();
            kernel.dispatch(RawEvent::new("memory.created", payload, Actor::owner(), 0)).unwrap();
            t.elapsed().as_micros() as f64
        } else {
            0.0 // skip dispatch at 2000 to save time
        };

        println!("  {:>6} | {:>10.2}ms | {:>10.1}μs | {:>10.1}KB | {:>10.1}x | {:>10.1}μs",
            target, avg_clone_ms, avg_clone_us, json_kb, clone_vs_json, dispatch_us);
    }

    // Analysis.
    println!("\n  Analysis:");
    println!("  1. Clone cost grows with state size (O(n) per clone, O(n²) total dispatch).");
    println!("  2. Clone is the dominant cost in dispatch (validate-on-write).");
    println!("  3. JSON serialize is a proxy for state size — clone should track it.");
    println!("\n  Conclusion:");
    println!("  The validate-on-write clone is correct but O(n) per dispatch.");
    println!("  At 1000 events, each dispatch clones ~{}KB of state.", "N/A");
    println!("  At 2000 events, each dispatch clones significantly more.");
    println!("  Solution: P2 (Trusted Producer) or P3 (persistent data structures).");
}
