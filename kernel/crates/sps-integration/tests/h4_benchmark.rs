//! H4: Performance Characterization Study
//!
//! NOT a pass/fail test — a benchmarking study.
//! Measures: dispatch rate, replay rate, verify rate, snapshot+tail rate
//! at scales: 100, 500, 1000, 2000 events.
//!
//! Key questions:
//! 1. Is cost linear O(n) or quadratic O(n²)?
//! 2. How much faster is snapshot+tail vs genesis replay?
//! 3. What is the per-event dispatch cost (dominated by validate-on-write clone)?

use std::sync::Arc;
use std::time::Instant;

use serde_json::json;
use smol_str::SmolStr;
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

fn make_pipeline() -> Arc<sps_core::reducer::ReducerPipeline> {
    Arc::new(sps_core::reducer::ReducerPipeline::new(Arc::new({
        let mut reg = ReducerRegistry::new();
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        reg
    })))
}

fn dispatch_memories(kernel: &SpsKernel, count: usize) {
    for i in 0..count {
        let record = sps_memory::memory::MemoryRecord {
            id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
            kind: sps_memory::memory::MemoryKind::Episodic,
            title: SmolStr::new(format!("mem-{}", i)),
            content: json!({"i": i}),
            tags: vec![],
            origin_tick: 0,
            created_at: 0,
        };
        let payload = serde_json::to_value(&record).unwrap();
        kernel.dispatch(RawEvent::new("memory.created", payload, Actor::owner(), 0)).unwrap();
    }
}

struct BenchmarkResult {
    n: usize,
    dispatch_ms: u128,
    verify_ms: u128,
    genesis_replay_ms: u128,
    snapshot_take_ms: u128,
    snapshot_tail_replay_ms: u128,
    store_count: u64,
    meta_count: u64,
    mem_count: usize,
    replay_mem_count: usize,
    hash_match: bool,
}

impl BenchmarkResult {
    fn dispatch_per_event_us(&self) -> f64 { self.dispatch_ms as f64 * 1000.0 / self.n as f64 }
    fn replay_per_event_us(&self) -> f64 { self.genesis_replay_ms as f64 * 1000.0 / self.n as f64 }
    fn verify_per_event_us(&self) -> f64 { self.verify_ms as f64 * 1000.0 / self.n as f64 }
    fn snapshot_speedup(&self) -> f64 {
        if self.snapshot_tail_replay_ms > 0 {
            self.genesis_replay_ms as f64 / self.snapshot_tail_replay_ms as f64
        } else { 0.0 }
    }
}

fn run_benchmark(n: usize) -> BenchmarkResult {
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Dispatch.
    let t0 = Instant::now();
    dispatch_memories(&kernel, n);
    let dispatch_ms = t0.elapsed().as_millis();

    // Verify chain.
    let t1 = Instant::now();
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    let verify_ms = t1.elapsed().as_millis();

    // Take snapshot at midpoint.
    let t2 = Instant::now();
    let snapshot = kernel.snapshot(0).unwrap();
    let snapshot_take_ms = t2.elapsed().as_millis();
    let snapshot_tick = snapshot.tick;

    // Dispatch more (to have a tail to replay).
    let tail_count = n / 2;
    dispatch_memories(&kernel, tail_count);
    let total = n + tail_count;

    // Genesis replay (full).
    let pipeline = make_pipeline();
    let engine = ReplayEngine::new(pipeline.clone());
    let t3 = Instant::now();
    let genesis_state = engine.replay_from_genesis(storage.as_ref()).unwrap();
    let genesis_replay_ms = t3.elapsed().as_millis();

    // Snapshot + tail replay.
    let t4 = Instant::now();
    let snap_state = engine.replay_from_snapshot(storage.as_ref(), &snapshot).unwrap();
    let snapshot_tail_replay_ms = t4.elapsed().as_millis();

    let store_count = kernel.store().count().unwrap_or(0);
    let meta_count = kernel.query(|s| s.event_count());
    let mem_count = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s).map(|ms| ms.graph.count()).unwrap_or(0)
    });
    let replay_mem_count = sps_memory::reducer::MemoryState::from_state(&genesis_state)
        .map(|ms| ms.graph.count()).unwrap_or(0);
    let hash_match = genesis_state.last_hash() == snap_state.last_hash();

    BenchmarkResult {
        n: total,
        dispatch_ms,
        verify_ms,
        genesis_replay_ms,
        snapshot_take_ms,
        snapshot_tail_replay_ms,
        store_count,
        meta_count,
        mem_count,
        replay_mem_count,
        hash_match,
    }
}

#[test]
fn h4_performance_characterization() {
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("  H4: Performance Characterization Study");
    println!("  NOT pass/fail — measuring dispatch, replay, snapshot performance");
    println!("═══════════════════════════════════════════════════════════════\n");

    let scales = [100, 500, 1000, 2000];
    let mut results = Vec::new();

    for &n in &scales {
        let half = n / 2;
        println!("  ── Scale: {} events (dispatch {} + snapshot at {} + dispatch {} more) ──",
            n + half, n, n, half);

        let r = run_benchmark(n);
        println!("    dispatch:       {}ms ({:.0} events/sec, {:.1}μs/event)",
            r.dispatch_ms, n as f64 / (r.dispatch_ms as f64 / 1000.0), r.dispatch_per_event_us());
        println!("    verify:         {}ms ({:.0} events/sec, {:.1}μs/event)",
            r.verify_ms, n as f64 / (r.verify_ms as f64 / 1000.0), r.verify_per_event_us());
        println!("    snapshot take:  {}ms (at tick {})", r.snapshot_take_ms, n);
        println!("    genesis replay: {}ms ({:.0} events/sec, {:.1}μs/event) [total {} events]",
            r.genesis_replay_ms, r.n as f64 / (r.genesis_replay_ms as f64 / 1000.0),
            r.replay_per_event_us(), r.n);
        println!("    snapshot+tail:  {}ms ({:.1}x faster than genesis)", r.snapshot_tail_replay_ms, r.snapshot_speedup());
        println!("    store={}, meta={}, memories={} (replayed={}), hash_match={}",
            r.store_count, r.meta_count, r.mem_count, r.replay_mem_count, r.hash_match);
        println!();

        // Correctness invariants (these DO need to pass).
        assert_eq!(r.store_count, r.n as u64, "FAIL: store count mismatch at scale {}", n);
        assert_eq!(r.meta_count, r.n as u64, "FAIL: meta count drift at scale {}", n);
        assert_eq!(r.mem_count, r.n, "FAIL: memory count mismatch at scale {}", n);
        assert_eq!(r.replay_mem_count, r.n, "FAIL: replayed memory count mismatch at scale {}", n);
        assert!(r.hash_match, "FAIL: hash mismatch at scale {}", n);
        assert!(r.hash_match, "FAIL: genesis hash != snapshot hash at scale {}", n);

        results.push(r);
    }

    // Linearity analysis.
    println!("  ═══════════════════════════════════════════════════════════");
    println!("  Linearity Analysis");
    println!("  ═══════════════════════════════════════════════════════════\n");

    print!("  {:>6} | {:>10} | {:>12} | {:>12} | {:>12} | {:>12}\n",
        "Events", "Dispatch", "Dispatch/ev", "Replay", "Replay/ev", "Snap+Tail");
    print!("  {:>6} | {:>10} | {:>12} | {:>12} | {:>12} | {:>12}\n",
        "------", "----------", "------------", "------------", "------------", "------------");

    for r in &results {
        print!("  {:>6} | {:>8}ms | {:>8.1}μs/ev | {:>8}ms | {:>8.1}μs/ev | {:>8}ms\n",
            r.n, r.dispatch_ms, r.dispatch_per_event_us(),
            r.genesis_replay_ms, r.replay_per_event_us(),
            r.snapshot_tail_replay_ms);
    }

    // Check linearity: if O(n), per-event cost should be roughly constant.
    println!("\n  Per-event cost trend (O(n) = constant, O(n²) = growing):");
    for r in &results {
        println!("    {} events: dispatch={:.1}μs/ev, replay={:.1}μs/ev",
            r.n, r.dispatch_per_event_us(), r.replay_per_event_us());
    }

    let first = &results[0];
    let last = &results[results.len() - 1];
    let dispatch_growth = last.dispatch_per_event_us() / first.dispatch_per_event_us();
    let replay_growth = last.replay_per_event_us() / first.replay_per_event_us();

    println!("\n  Growth factor (last/first per-event cost):");
    println!("    dispatch: {:.2}x (1.0 = perfectly linear)", dispatch_growth);
    println!("    replay:   {:.2}x (1.0 = perfectly linear)", replay_growth);

    if dispatch_growth < 2.0 {
        println!("    → Dispatch cost is approximately LINEAR O(n) ✓");
    } else if dispatch_growth < 5.0 {
        println!("    → Dispatch cost shows mild super-linear growth (likely state clone overhead)");
    } else {
        println!("    → Dispatch cost is super-linear — investigate state clone cost");
    }

    if replay_growth < 2.0 {
        println!("    → Replay cost is approximately LINEAR O(n) ✓");
    } else {
        println!("    → Replay cost is super-linear — investigate");
    }

    // Snapshot speedup analysis.
    println!("\n  Snapshot speedup (genesis replay / snapshot+tail replay):");
    for r in &results {
        println!("    {} events (snapshot at {}): {:.1}x faster",
            r.n, r.n / 2, r.snapshot_speedup());
    }

    println!("\n  ═══════════════════════════════════════════════════════════");
    println!("  H4 Characterization Complete");
    println!("  All correctness invariants passed at every scale.");
    println!("  Performance data collected for optimization decisions.");
    println!("  ═══════════════════════════════════════════════════════════");
}
