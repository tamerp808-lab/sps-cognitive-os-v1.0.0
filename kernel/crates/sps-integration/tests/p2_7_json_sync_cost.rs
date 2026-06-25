//! P2.7: JSON Sync Cost Isolation
//!
//! Question: Is the `save_to()` JSON sync layer at the end of each reducer
//! the biggest remaining cost in dispatch? If yes, P3D (remove JSON sync,
//! make typed_extensions the sole source of truth for snapshots/replay)
//! is justified. If no, P3D would be wasted work.
//!
//! Method:
//!   1. Build a kernel with state populated by N events of each event type.
//!   2. For each populated state slice, isolate the JSON sync cost by
//!      calling `save_to` on the typed state repeatedly and timing it.
//!      This is exactly the work that the reducer does at the end of every
//!      `reduce()` call.
//!   3. Compare per-call sync cost to per-event dispatch cost (P2.6 numbers).
//!
//! Verdict rule:
//!   - If `sync_cost / dispatch_cost >= 0.50` for the dominant slice → YES
//!   - If `sync_cost / dispatch_cost <= 0.20` for the dominant slice → NO
//!   - Otherwise: inconclusive (look at total share across all slices)

use std::sync::Arc;
use std::time::Instant;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::state::CanonicalState;
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
    })
    .unwrap()
    .into()
}

fn make_memory_payload(i: usize) -> serde_json::Value {
    let record = sps_memory::memory::MemoryRecord {
        id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
        kind: sps_memory::memory::MemoryKind::Episodic,
        title: SmolStr::new(format!("mem-{}", i)),
        content: json!({"i": i, "note": "lorem ipsum dolor sit amet"}),
        tags: vec![],
        origin_tick: 0,
        created_at: 0,
    };
    serde_json::to_value(&record).unwrap()
}

fn make_goal_payload(i: usize) -> serde_json::Value {
    let goal = sps_goals::hierarchy::Goal {
        id: sps_goals::hierarchy::GoalId(uuid::Uuid::now_v7()),
        title: SmolStr::new(format!("goal-{}", i)),
        description: "test goal description".into(),
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
    json!({"operation": format!("op-{}", i), "duration_ms": 100, "agent_id": uuid::Uuid::nil().to_string()})
}

/// Snapshot the canonical state under a read lock and return a cloned copy.
/// (We need owned access to time `save_to` without holding the kernel lock.)
fn clone_canonical(kernel: &SpsKernel) -> CanonicalState {
    kernel.query(|state| state.clone())
}

/// Time how long `f` takes averaged over `iters` calls.
fn time_avg<F: FnMut()>(mut f: F, iters: usize) -> f64 {
    // warmup
    for _ in 0..(iters.min(20)) {
        f();
    }
    let mut total_us = 0u128;
    for _ in 0..iters {
        let t = Instant::now();
        f();
        total_us += t.elapsed().as_micros();
    }
    total_us as f64 / iters as f64
}

#[test]
fn p2_7_json_sync_cost_isolation() {
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("  P2.7: JSON Sync Cost Isolation");
    println!("  Question: Is save_to() the dominant remaining dispatch cost?");
    println!("  Method: build state with N events, measure isolated save_to()");
    println!("  cost on the typed slice, compare to total dispatch cost.");
    println!("═══════════════════════════════════════════════════════════════\n");

    let n_values: Vec<usize> = vec![100, 500, 1000];

    println!("  Phase 1: Per-slice sync cost as a function of state size");
    println!("  (Memory is the largest slice, Goal is second, Execution is third)\n");
    println!(
        "  {:>6} | {:>22} | {:>14} | {:>14} | {:>14}",
        "N", "Slice", "save_to(μs)", "dispatch(μs)", "sync share"
    );
    println!(
        "  {:>6} | {:>22} | {:>14} | {:>14} | {:>14}",
        "------", "----------------------", "--------------", "--------------", "--------------"
    );

    let mut results: Vec<(usize, String, f64, f64, f64)> = Vec::new();

    for &n in &n_values {
        let slices: Vec<(&str, &str, fn(usize) -> serde_json::Value)> = vec![
            ("memory", "memory.created", make_memory_payload),
            ("goal", "goal.created", make_goal_payload),
            ("execution", "execution.succeeded", make_execution_payload),
        ];

        for (slice_key, event_type, make_payload) in &slices {
            let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
            let kernel = boot_kernel(storage.clone());

            // Build state with n events.
            for i in 0..n {
                kernel
                    .dispatch_trusted(RawEvent::new(*event_type, make_payload(i), Actor::owner(), 0))
                    .unwrap();
            }

            // Snapshot current canonical state (cloned, owned).
            let canonical = clone_canonical(&kernel);

            // (a) Isolated save_to cost.
            let sync_cost_us: f64 = match *slice_key {
                "memory" => {
                    let typed: Arc<sps_memory::reducer::MemoryState> = canonical
                        .get_typed_extension(sps_memory::reducer::EXTENSION_KEY)
                        .expect("memory typed ext present");
                    time_avg(
                        || {
                            let mut scratch = CanonicalState::genesis();
                            let _ = typed.save_to(&mut scratch);
                            std::hint::black_box(&scratch);
                        },
                        30,
                    )
                }
                "goal" => {
                    let typed: Arc<sps_goals::reducer::GoalState> = canonical
                        .get_typed_extension(sps_goals::reducer::EXTENSION_KEY)
                        .expect("goal typed ext present");
                    time_avg(
                        || {
                            let mut scratch = CanonicalState::genesis();
                            let _ = typed.save_to(&mut scratch);
                            std::hint::black_box(&scratch);
                        },
                        30,
                    )
                }
                "execution" => {
                    let typed: Arc<sps_execution::reducer::ExecutionState> = canonical
                        .get_typed_extension(sps_execution::reducer::EXTENSION_KEY)
                        .expect("execution typed ext present");
                    time_avg(
                        || {
                            let mut scratch = CanonicalState::genesis();
                            let _ = typed.save_to(&mut scratch);
                            std::hint::black_box(&scratch);
                        },
                        30,
                    )
                }
                _ => continue,
            };

            // (b) Average dispatch cost for one more event (includes save_to).
            let dispatch_cost_us = time_avg(
                || {
                    let raw = RawEvent::new(*event_type, make_payload(9999), Actor::owner(), 0);
                    let _ = kernel.dispatch_trusted(raw);
                },
                30,
            );

            let share = sync_cost_us / dispatch_cost_us.max(1.0);

            println!(
                "  {:>6} | {:>22} | {:>10.1}μs   | {:>10.1}μs   | {:>10.1}%",
                n,
                slice_key,
                sync_cost_us,
                dispatch_cost_us,
                share * 100.0
            );

            results.push((n, slice_key.to_string(), sync_cost_us, dispatch_cost_us, share));
        }
        println!();
    }

    // Phase 2: Mixed state — total JSON sync across all typed slices.
    println!("  Phase 2: Mixed state — total JSON sync cost across ALL typed slices");
    println!("  (In production dispatch, only ONE reducer's save_to fires per event,");
    println!("   but the dispatch cost the *user* sees includes that one save_to.)\n");

    println!(
        "  {:>6} | {:>22} | {:>16} | {:>16} | {:>14}",
        "N", "Slice Synced", "save_to(μs)", "dispatch(μs)", "Sync Share"
    );
    println!(
        "  {:>6} | {:>22} | {:>16} | {:>16} | {:>14}",
        "------", "----------------------", "------------------", "------------------", "--------------"
    );

    for &n in &n_values {
        let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
        let kernel = boot_kernel(storage.clone());

        // Populate all slices with n events each.
        for i in 0..n {
            kernel
                .dispatch_trusted(RawEvent::new("memory.created", make_memory_payload(i), Actor::owner(), 0))
                .unwrap();
            kernel
                .dispatch_trusted(RawEvent::new("goal.created", make_goal_payload(i), Actor::owner(), 0))
                .unwrap();
            kernel
                .dispatch_trusted(RawEvent::new("execution.succeeded", make_execution_payload(i), Actor::owner(), 0))
                .unwrap();
        }

        let canonical = clone_canonical(&kernel);

        // Measure sync cost for each slice separately.
        let mem_typed: Arc<sps_memory::reducer::MemoryState> = canonical
            .get_typed_extension(sps_memory::reducer::EXTENSION_KEY)
            .unwrap();
        let goal_typed: Arc<sps_goals::reducer::GoalState> = canonical
            .get_typed_extension(sps_goals::reducer::EXTENSION_KEY)
            .unwrap();
        let exec_typed: Arc<sps_execution::reducer::ExecutionState> = canonical
            .get_typed_extension(sps_execution::reducer::EXTENSION_KEY)
            .unwrap();

        let mem_sync = time_avg(
            || {
                let mut scratch = CanonicalState::genesis();
                let _ = mem_typed.save_to(&mut scratch);
                std::hint::black_box(&scratch);
            },
            30,
        );
        let goal_sync = time_avg(
            || {
                let mut scratch = CanonicalState::genesis();
                let _ = goal_typed.save_to(&mut scratch);
                std::hint::black_box(&scratch);
            },
            30,
        );
        let exec_sync = time_avg(
            || {
                let mut scratch = CanonicalState::genesis();
                let _ = exec_typed.save_to(&mut scratch);
                std::hint::black_box(&scratch);
            },
            30,
        );

        // Dispatch cost: for memory.created (the dominant one).
        let dispatch_mem = time_avg(
            || {
                let raw = RawEvent::new("memory.created", make_memory_payload(9999), Actor::owner(), 0);
                let _ = kernel.dispatch_trusted(raw);
            },
            30,
        );

        println!(
            "  {:>6} | {:>22} | {:>12.1}μs     | {:>12.1}μs     | {:>10.1}%",
            n, "memory slice", mem_sync, dispatch_mem, (mem_sync / dispatch_mem.max(1.0)) * 100.0
        );
        println!(
            "  {:>6} | {:>22} | {:>12.1}μs     | {:>14}    | {:>14}",
            n, "goal slice", goal_sync, "(no dispatch)", ""
        );
        println!(
            "  {:>6} | {:>22} | {:>12.1}μs     | {:>14}    | {:>14}",
            n, "execution slice", exec_sync, "(no dispatch)", ""
        );
        println!();
    }

    // Verdict
    println!("  VERDICT");
    println!("  ───────────────────────────────────────────────────────────────");
    println!("  Rule:");
    println!("    sync_share >= 50% for the dominant slice → YES, P3D justified");
    println!("    sync_share <= 20% for the dominant slice → NO,  P3D skipped");
    println!("    otherwise                                → INCONCLUSIVE");
    println!();

    // Find dominant slice at N=1000.
    if let Some(dominant) = results
        .iter()
        .filter(|(n, _, _, _, _)| *n == 1000)
        .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
    {
        let (_, slice, sync_us, _disp_us, share) = dominant;
        println!(
            "  Dominant slice at N=1000: {} (sync = {:.1}μs, share = {:.1}%)",
            slice,
            sync_us,
            share * 100.0
        );
        if *share >= 0.50 {
            println!("  → VERDICT: YES — JSON sync is the dominant remaining cost. P3D is justified.");
        } else if *share <= 0.20 {
            println!("  → VERDICT: NO  — JSON sync is NOT the dominant cost. Skip P3D.");
        } else {
            println!("  → VERDICT: INCONCLUSIVE — sync is meaningful but not dominant.");
            println!("    Look at other components (reducer body, pipeline overhead) before P3D.");
        }
    }

    println!("\n  === P2.7 COMPLETE ===");
}
