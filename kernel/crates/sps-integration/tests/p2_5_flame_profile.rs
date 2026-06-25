//! P2.5: Dispatch Flame Profile
//!
//! Measures each phase of dispatch_trusted() at different scales.
//! We can't access kernel internals directly, so we measure what we can:
//! - dispatch_trusted total (end-to-end)
//! - store.append alone (hash + persist)
//! - clone alone (for comparison)
//! - JSON serialize (state size proxy)
//!
//! apply ≈ total - append (derived)

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

fn make_raw(i: usize) -> RawEvent {
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
fn p2_5_dispatch_flame_profile() {
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("  P2.5: Dispatch Flame Profile");
    println!("  Measures: total, append, apply(=total-append), clone, json");
    println!("═══════════════════════════════════════════════════════════════\n");

    println!("  {:>6} | {:>10} | {:>10} | {:>10} | {:>10} | {:>10} | {:>10}",
        "N", "Total(μs)", "Append(μs)", "Apply(μs)", "Append%", "Clone(μs)", "JSON(μs)");
    println!("  {:>6} | {:>10} | {:>10} | {:>10} | {:>10} | {:>10} | {:>10}",
        "------", "----------", "----------", "----------", "----------", "----------", "----------");

    for &n in &[100usize, 500, 1000] {
        let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
        let kernel = boot_kernel(storage.clone());

        // Pre-populate with n events (trusted for speed).
        for i in 0..n {
            kernel.dispatch_trusted(make_raw(i)).unwrap();
        }

        // Measure dispatch_trusted total (10 samples).
        let mut total_times = Vec::new();
        for i in 0..10 {
            let raw = make_raw(n + i);
            let t = Instant::now();
            kernel.dispatch_trusted(raw).unwrap();
            total_times.push(t.elapsed().as_micros() as f64);
        }
        let avg_total: f64 = total_times.iter().sum::<f64>() / 10.0;

        // Measure store.append alone (10 samples).
        // Note: these events go into the store but aren't applied to state.
        // This is fine for timing — we just need to measure append cost.
        let mut append_times = Vec::new();
        for i in 0..10 {
            let raw = make_raw(n + 100 + i);
            let t = Instant::now();
            let _event = kernel.store().append(raw).unwrap();
            append_times.push(t.elapsed().as_micros() as f64);
        }
        let avg_append: f64 = append_times.iter().sum::<f64>() / 10.0;

        // Measure clone alone (10 samples).
        let mut clone_times = Vec::new();
        for _ in 0..10 {
            let t = Instant::now();
            let _ = kernel.query(|s| s.clone());
            clone_times.push(t.elapsed().as_micros() as f64);
        }
        let avg_clone: f64 = clone_times.iter().sum::<f64>() / 10.0;

        // Measure JSON serialize (5 samples, proxy for state size).
        let mut json_times = Vec::new();
        for _ in 0..5 {
            let t = Instant::now();
            let _ = kernel.query(|s| serde_json::to_string(s).unwrap());
            json_times.push(t.elapsed().as_micros() as f64);
        }
        let avg_json: f64 = json_times.iter().sum::<f64>() / 5.0;

        let avg_apply = avg_total - avg_append;
        let append_pct = avg_append / avg_total * 100.0;

        println!("  {:>6} | {:>8.1}μs | {:>8.1}μs | {:>8.1}μs | {:>8.1}% | {:>8.1}μs | {:>8.1}μs",
            n, avg_total, avg_append, avg_apply, append_pct, avg_clone, avg_json);
    }

    println!("\n  Analysis:");
    println!("  - Total = dispatch_trusted (append + write_lock + apply)");
    println!("  - Append = store.append (SHA-256 hash + InMemory persist)");
    println!("  - Apply = Total - Append (write lock + KernelMeta + MemoryReducer)");
    println!("  - Clone = full state clone (not used in trusted path, for reference)");
    println!("  - JSON = serialize state to string (proxy for state size)");
    println!("\n  Key question: does Append or Apply grow faster with N?");
    println!("  If Append grows: hash computation or storage is super-linear.");
    println!("  If Apply grows:  BTreeMap insertion or reducer traversal is super-linear.");

    println!("\n  === P2.5 COMPLETE ===");
}
