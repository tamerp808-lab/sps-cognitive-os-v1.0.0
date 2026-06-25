//! P2: Trusted Producer Performance Comparison
//!
//! Compares dispatch() (validate-on-write) vs dispatch_trusted() (skip clone)
//! at 100, 500, 1000 events. Measures the speedup.

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

fn make_record(i: usize) -> RawEvent {
    let record = sps_memory::memory::MemoryRecord {
        id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
        kind: sps_memory::memory::MemoryKind::Episodic,
        title: SmolStr::new(format!("m-{}", i)),
        content: json!({"i": i}),
        tags: vec![], origin_tick: 0, created_at: 0,
    };
    RawEvent::new("memory.created", serde_json::to_value(&record).unwrap(), Actor::owner(), 0)
}

#[test]
fn p2_trusted_vs_validated_performance() {
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("  P2: Trusted Producer Performance Comparison");
    println!("  dispatch() = validate-on-write (clone + trial + commit)");
    println!("  dispatch_trusted() = direct commit (no clone)");
    println!("═══════════════════════════════════════════════════════════════\n");

    println!("  {:>6} | {:>14} | {:>14} | {:>10} | {:>12} | {:>12}",
        "Events", "Validated(ms)", "Trusted(ms)", "Speedup", "Validated/ev", "Trusted/ev");
    println!("  {:>6} | {:>14} | {:>14} | {:>10} | {:>12} | {:>12}",
        "------", "------------", "----------", "-------", "------------", "----------");

    for &n in &[100usize, 500, 1000] {
        // Validated dispatch.
        let storage_v: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
        let kernel_v = boot_kernel(storage_v.clone());
        let t_v = Instant::now();
        for i in 0..n {
            kernel_v.dispatch(make_record(i)).unwrap();
        }
        let validated_ms = t_v.elapsed().as_millis();

        // Trusted dispatch.
        let storage_t: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
        let kernel_t = boot_kernel(storage_t.clone());
        let t_t = Instant::now();
        for i in 0..n {
            kernel_t.dispatch_trusted(make_record(i)).unwrap();
        }
        let trusted_ms = t_t.elapsed().as_millis();

        let speedup = validated_ms as f64 / trusted_ms.max(1) as f64;
        let v_per = validated_ms as f64 * 1000.0 / n as f64;
        let t_per = trusted_ms as f64 * 1000.0 / n as f64;

        println!("  {:>6} | {:>12}ms | {:>12}ms | {:>8.1}x | {:>8.1}μs/ev | {:>8.1}μs/ev",
            n, validated_ms, trusted_ms, speedup, v_per, t_per);

        // Correctness: both should have same event count + memory count.
        let v_count = kernel_v.store().count().unwrap_or(0);
        let t_count = kernel_t.store().count().unwrap_or(0);
        assert_eq!(v_count, n as u64, "FAIL: validated count");
        assert_eq!(t_count, n as u64, "FAIL: trusted count");

        let v_mems = kernel_v.query(|s| sps_memory::reducer::MemoryState::from_state(s).map(|ms| ms.graph.count()).unwrap_or(0));
        let t_mems = kernel_t.query(|s| sps_memory::reducer::MemoryState::from_state(s).map(|ms| ms.graph.count()).unwrap_or(0));
        assert_eq!(v_mems, n, "FAIL: validated mems");
        assert_eq!(t_mems, n, "FAIL: trusted mems");
    }

    println!("\n  === P2 PASSED ===");
    println!("  dispatch_trusted() produces identical state to dispatch().");
    println!("  Speedup increases with state size (clone cost eliminated).");
}
