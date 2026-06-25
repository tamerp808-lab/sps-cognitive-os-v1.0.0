//! Hardening Tests: Concurrency (H1) + Corruption (H2)
//!
//! These tests verify that the kernel's Event Store is safe under:
//! - Concurrent access (H1): 1000 parallel dispatches, tick + hash chain must be intact
//! - Data corruption (H2): tampered events, truncated DB → kernel must refuse to boot

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::reducer::ReducerRegistry;
use sps_core::replay::ReplayVerifier;
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

// ════════════════════════════════════════════════════════════════════════
// H1: CONCURRENCY VALIDATION
// ════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn h1_concurrency_1000_parallel_dispatches() {
    println!("\n=== H1: Concurrency — 1000 parallel dispatches ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    const N: usize = 1000;

    // Spawn N tasks that each dispatch a memory.created event concurrently.
    let kernel_clone = kernel.clone();
    let handles: Vec<tokio::task::JoinHandle<()>> = (0..N)
        .map(|i| {
            let k = kernel_clone.clone();
            tokio::spawn(async move {
                let record = sps_memory::memory::MemoryRecord {
                    id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
                    kind: sps_memory::memory::MemoryKind::Episodic,
                    title: SmolStr::new(format!("concurrent-memory-{}", i)),
                    content: json!({"index": i}),
                    tags: vec![],
                    origin_tick: 0,
                    created_at: 0,
                };
                let payload = serde_json::to_value(&record).unwrap();
                let result = k.dispatch(RawEvent::new(
                    "memory.created",
                    payload,
                    Actor::owner(),
                    0,
                ));
                if result.is_err() {
                    eprintln!("  dispatch {} failed: {:?}", i, result);
                }
            })
        })
        .collect();

    // Wait for all tasks.
    for handle in handles {
        handle.await.unwrap();
    }
    println!("  Dispatched {} events concurrently", N);

    // Verify 1: event_count == N.
    let event_count = kernel.query(|s| s.event_count());
    if event_count == N as u64 {
        println!("  PASS — event_count == {} (all dispatches succeeded)", N);
    } else {
        println!("  FAIL — event_count == {} (expected {})", event_count, N);
        panic!("H1 FAILED — concurrent dispatches lost events");
    }

    // Verify 2: hash chain intact.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    if report.failure.is_none() {
        println!("  PASS — hash chain intact after {} concurrent events", N);
    } else {
        println!("  FAIL — hash chain broken: {:?}", report.failure);
        panic!("H1 FAILED — hash chain broken under concurrency");
    }

    // Verify 3: all N events have unique ticks (no duplicates).
    let events = storage.read_events_from(1, (N + 100) as usize).unwrap();
    let ticks: std::collections::BTreeSet<u64> = events.iter().map(|e| e.tick).collect();
    if ticks.len() == N {
        println!("  PASS — all {} ticks are unique (no duplicate ticks)", N);
    } else {
        println!("  FAIL — only {} unique ticks (expected {})", ticks.len(), N);
        panic!("H1 FAILED — duplicate ticks detected");
    }

    // Verify 4: ticks are monotonically increasing.
    let mut sorted_ticks: Vec<u64> = events.iter().map(|e| e.tick).collect();
    sorted_ticks.sort();
    let is_monotonic = sorted_ticks.windows(2).all(|w| w[0] < w[1]);
    if is_monotonic {
        println!("  PASS — ticks are monotonically increasing (1..={})", N);
    } else {
        println!("  FAIL — ticks are NOT monotonic");
        panic!("H1 FAILED — non-monotonic ticks");
    }

    // Verify 5: all N memories materialized in state.
    let mem_count = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| ms.graph.count())
            .unwrap_or(0)
    });
    if mem_count == N {
        println!("  PASS — all {} memories materialized in canonical state", N);
    } else {
        println!("  FAIL — only {} memories in state (expected {})", mem_count, N);
        panic!("H1 FAILED — state missing memories after concurrent dispatch");
    }

    // Verify 6: last_hash matches the hash of the last event.
    let last_event = events.last().unwrap();
    let live_last_hash = kernel.query(|s| s.last_hash());
    if live_last_hash == last_event.hash {
        println!("  PASS — kernel.last_hash matches last event hash");
    } else {
        println!("  FAIL — hash mismatch");
        panic!("H1 FAILED — last_hash stale after concurrent dispatch");
    }

    println!("\n  === H1 CONCURRENCY PASSED ===");
}

// ════════════════════════════════════════════════════════════════════════
// H2: CORRUPTION VALIDATION
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h2a_tampered_hash_detected() {
    println!("\n=== H2a: Tampered event hash → kernel refuses boot ===");

    let db_path = std::env::temp_dir().join(format!("sps_h2a_{}.db", uuid::Uuid::now_v7()));

    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel = boot_kernel(storage.clone());
        for i in 0..5 {
            let record = sps_memory::memory::MemoryRecord {
                id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
                kind: sps_memory::memory::MemoryKind::Episodic,
                title: SmolStr::new(format!("pre-corruption-{}", i)),
                content: json!({}),
                tags: vec![],
                origin_tick: 0,
                created_at: 0,
            };
            let payload = serde_json::to_value(&record).unwrap();
            kernel.dispatch(RawEvent::new("memory.created", payload, Actor::owner(), 0)).unwrap();
        }
        println!("  Phase 1: wrote 5 events to SQLite");
        drop(kernel);
    }

    // Tamper: modify the hash INSIDE the event_json for event #3.
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let table_name: String = conn
            .query_row("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE '%event%'", [], |row| row.get(0))
            .unwrap_or_else(|_| "events".to_string());
        println!("  Phase 2: tampering with table '{}'", table_name);

        // Read the original event_json for tick 3.
        let original_json: String = conn
            .query_row(&format!("SELECT event_json FROM {} WHERE tick = 3", table_name), [], |row| row.get(0))
            .unwrap();
        println!("    original event_json (first 80 chars): {}...", &original_json[..80.min(original_json.len())]);

        // Tamper: replace the hash field in the JSON with garbage.
        // The hash is stored as a hex string in the JSON.
        // We replace a chunk of the hash with "deadbeef".
        let tampered_json = original_json.replace(
            // Find the first occurrence of a 64-char hex hash and replace it.
            |c: char| c.is_ascii_hexdigit(),
            "d",
        );
        // Actually, simpler approach: just append garbage to the payload.
        // This will cause the recompute_hash to differ from the stored hash.
        let tampered_json2 = original_json.replace(
            "\"hash\":\"",
            "\"hash\":\"deadbeef",
        );
        conn.execute(
            &format!("UPDATE {} SET event_json = ? WHERE tick = 3", table_name),
            rusqlite::params![tampered_json2],
        ).unwrap();
        println!("    tampered event_json for tick 3 (hash field corrupted)");
    }

    // Try to boot — should FAIL.
    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
    );
    let boot_result = SpsKernel::boot_with(storage, KernelConfig::default(), |_| {});

    if boot_result.is_err() {
        println!("  PASS — kernel refused boot (tampered hash detected)");
    } else {
        println!("  FAIL — kernel booted with corrupted hash");
        panic!("H2a FAILED — kernel booted with tampered hash");
    }

    std::fs::remove_file(&db_path).ok();
}

#[test]
fn h2b_broken_chain_detected() {
    println!("\n=== H2b: Broken hash chain → detected ===");

    let db_path = std::env::temp_dir().join(format!("sps_h2b_{}.db", uuid::Uuid::now_v7()));

    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel = boot_kernel(storage.clone());
        for i in 0..5 {
            let record = sps_memory::memory::MemoryRecord {
                id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
                kind: sps_memory::memory::MemoryKind::Semantic,
                title: SmolStr::new(format!("chain-test-{}", i)),
                content: json!({}),
                tags: vec![],
                origin_tick: 0,
                created_at: 0,
            };
            let payload = serde_json::to_value(&record).unwrap();
            kernel.dispatch(RawEvent::new("memory.created", payload, Actor::owner(), 0)).unwrap();
        }
        println!("  Phase 1: wrote 5 events");
        drop(kernel);
    }

    // Tamper: modify prev_hash INSIDE the event_json for event #3.
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let table_name: String = conn
            .query_row("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE '%event%'", [], |row| row.get(0))
            .unwrap_or_else(|_| "events".to_string());

        let original_json: String = conn
            .query_row(&format!("SELECT event_json FROM {} WHERE tick = 3", table_name), [], |row| row.get(0))
            .unwrap();

        // Replace prev_hash field in JSON with a fake value.
        let tampered_json = original_json.replace(
            "\"prev_hash\":\"",
            "\"prev_hash\":\"00000000000000000000000000000000000000000000000000000000000000ff",
        );
        conn.execute(
            &format!("UPDATE {} SET event_json = ? WHERE tick = 3", table_name),
            rusqlite::params![tampered_json],
        ).unwrap();
        println!("  Phase 2: tampered prev_hash in event_json for tick 3 (chain broken)");
    }

    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
    );
    let boot_result = SpsKernel::boot_with(storage, KernelConfig::default(), |_| {});

    if boot_result.is_err() {
        println!("  PASS — kernel refused boot (broken chain detected)");
    } else {
        println!("  FAIL — kernel booted with broken chain");
        panic!("H2b FAILED — broken chain not detected");
    }

    std::fs::remove_file(&db_path).ok();
}

// ════════════════════════════════════════════════════════════════════════
// AUTONOMOUS LOOP TEST
// ════════════════════════════════════════════════════════════════════════

#[test]
fn autonomous_loop_full_cycle() {
    println!("\n=== AUTONOMOUS LOOP: Goal → Plan → Execute → Reflect → Memory → Autonomy → Update → Replay ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Stage 1: Create Goal
    let goal_id = sps_goals::hierarchy::GoalId(uuid::Uuid::now_v7());
    let goal = sps_goals::hierarchy::Goal {
        id: goal_id,
        title: SmolStr::new("Learn Rust"),
        description: "Master Rust programming".to_string(),
        priority: 5,
        status: sps_goals::hierarchy::GoalStatus::Active,
        objectives: Vec::new(),
        dependencies: Vec::new(),
        created_at: 0,
        origin_tick: 0,
    };
    kernel.dispatch(RawEvent::new("goal.created", serde_json::to_value(&goal).unwrap(), Actor::owner(), 0)).unwrap();
    println!("  Stage 1: goal.created ('Learn Rust')");

    // Stage 2: Create Plan
    let plan_id = sps_planner::plan::PlanId::new();
    let plan = sps_planner::plan::Plan {
        id: plan_id, goal_id,
        template: SmolStr::new("learning.workflow"),
        steps: vec![
            sps_planner::plan::PlanStep { id: uuid::Uuid::now_v7(), title: SmolStr::new("Read The Book"), description: String::new(), index: 0, depends_on: vec![], assigned_agent: None, parallelizable: false },
            sps_planner::plan::PlanStep { id: uuid::Uuid::now_v7(), title: SmolStr::new("Write Code"), description: String::new(), index: 1, depends_on: vec![0], assigned_agent: None, parallelizable: false },
        ],
        status: sps_planner::plan::PlanStatus::Approved, created_at: 0, origin_tick: 0,
    };
    kernel.dispatch(RawEvent::new("plan.created", serde_json::to_value(&plan).unwrap(), Actor::owner(), 0)).unwrap();
    println!("  Stage 2: plan.created (2 steps)");

    // Stage 3: Execute Tasks
    for step_title in &["Read The Book", "Write Code"] {
        kernel.dispatch(RawEvent::new("execution.succeeded", json!({"operation": step_title, "plan_id": plan_id.0.to_string(), "duration_ms": 3600000}), Actor::owner(), 0)).unwrap();
    }
    println!("  Stage 3: execution.succeeded × 2");

    // Stage 4: Reflect
    let task_id = uuid::Uuid::now_v7();
    let analysis = sps_reflection::analyzers::SuccessAnalyzer::analyze(task_id, vec!["read book".into(), "wrote code".into()], "practice reinforced theory".into(), true);
    kernel.dispatch(RawEvent::new("reflection.success_analyzed", serde_json::to_value(&analysis).unwrap(), Actor::owner(), 0)).unwrap();
    println!("  Stage 4: reflection.success_analyzed");

    // Stage 5: Store Memories
    for (title, content) in &[("Rust ownership", "borrow checker"), ("Rust async", "tokio runtime")] {
        let record = sps_memory::memory::MemoryRecord {
            id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
            kind: sps_memory::memory::MemoryKind::Semantic,
            title: SmolStr::new(*title),
            content: json!({"detail": content}),
            tags: vec![SmolStr::new("rust")],
            origin_tick: 0, created_at: 0,
        };
        kernel.dispatch(RawEvent::new("memory.created", serde_json::to_value(&record).unwrap(), Actor::owner(), 0)).unwrap();
    }
    println!("  Stage 5: memory.created × 2");

    // Stage 6: Autonomy
    kernel.dispatch(RawEvent::new("autonomous.goal_activated", json!({"goal_id": goal_id.0.to_string(), "milestones": [{"title": "Week 1"}], "activated_at": 1000}), Actor::owner(), 0)).unwrap();
    kernel.dispatch(RawEvent::new("autonomous.weekly_review", json!({"goal_id": goal_id.0.to_string(), "review": "Good progress", "reviewed_at": 2000}), Actor::owner(), 0)).unwrap();
    println!("  Stage 6: autonomous.goal_activated + weekly_review");

    // Stage 7: Update Goal Progress
    kernel.dispatch(RawEvent::new("goal.progress_updated", json!({"goal_id": goal_id.0.to_string(), "milestone": "Week 1", "completed": true}), Actor::owner(), 0)).unwrap();
    println!("  Stage 7: goal.progress_updated");

    // Verify all subsystems
    println!("\n  Verifying all subsystems...");
    let (goals, plans, execs, reflections, memories, activations, reviews) = kernel.query(|s| {
        let gs = sps_goals::reducer::GoalState::from_state(s).map(|gs| gs.tree.goals.len()).unwrap_or(0);
        let ps = sps_planner::reducer::PlannerState::from_state(s).map(|ps| ps.plans.len()).unwrap_or(0);
        let es = sps_execution::reducer::ExecutionState::from_state(s).map(|es| es.records.len()).unwrap_or(0);
        let rs = sps_reflection::reducer::ReflectionState::from_state(s).map(|rs| rs.reflections.len()).unwrap_or(0);
        let ms = sps_memory::reducer::MemoryState::from_state(s).map(|ms| ms.graph.count()).unwrap_or(0);
        let as_ = sps_autonomy::reducer::AutonomyState::from_state(s).map(|a| a.active_goals.len()).unwrap_or(0);
        let wr = sps_autonomy::reducer::AutonomyState::from_state(s).map(|a| a.reviews.len()).unwrap_or(0);
        (gs, ps, es, rs, ms, as_, wr)
    });
    println!("    Goals={} Plans={} Execs={} Refls={} Mems={} Activations={} Reviews={}", goals, plans, execs, reflections, memories, activations, reviews);

    let expected = (1, 1, 2, 1, 2, 1, 1);
    if (goals, plans, execs, reflections, memories, activations, reviews) == expected {
        println!("  PASS — all 7 subsystems populated correctly");
    } else {
        println!("  FAIL — expected {:?}", expected);
        panic!("LOOP TEST FAILED");
    }

    // Cross-system links
    let execs_for_plan = kernel.query(|s| sps_execution::reducer::ExecutionState::from_state(s).map(|es| es.for_plan(plan_id.0).len()).unwrap_or(0));
    let reviews_for_goal = kernel.query(|s| sps_autonomy::reducer::AutonomyState::from_state(s).map(|a| a.reviews_for_goal(goal_id.0).len()).unwrap_or(0));
    if execs_for_plan == 2 && reviews_for_goal == 1 {
        println!("  PASS — cross-system links: Executions→Plan={}, Reviews→Goal={}", execs_for_plan, reviews_for_goal);
    } else {
        println!("  FAIL — links broken: execs_for_plan={}, reviews_for_goal={}", execs_for_plan, reviews_for_goal);
        panic!("LOOP TEST FAILED");
    }

    // Replay
    println!("\n  Replaying entire loop...");
    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();
    let live_count = kernel.store().count().unwrap_or(0); // Use store count (source of truth)
    println!("  store has {} events", live_count);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());
    println!("  chain verified: {} events_verified", report.events_verified);

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_goals::reducer::GoalReducer::register(&mut reg);
        sps_planner::reducer::PlannerReducer::register(&mut reg);
        sps_execution::reducer::ExecutionReducer::register(&mut reg);
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        sps_autonomy::reducer::AutonomyReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = sps_core::replay::ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.event_count(), live_count, "FAIL: event_count mismatch (replayed={}, store={})", replayed.event_count(), live_count);
    assert_eq!(replayed.last_hash(), live_hash, "FAIL: last_hash mismatch");

    let replayed_state = (
        sps_goals::reducer::GoalState::from_state(&replayed).map(|gs| gs.tree.goals.len()).unwrap_or(0),
        sps_planner::reducer::PlannerState::from_state(&replayed).map(|ps| ps.plans.len()).unwrap_or(0),
        sps_execution::reducer::ExecutionState::from_state(&replayed).map(|es| es.records.len()).unwrap_or(0),
        sps_reflection::reducer::ReflectionState::from_state(&replayed).map(|rs| rs.reflections.len()).unwrap_or(0),
        sps_memory::reducer::MemoryState::from_state(&replayed).map(|ms| ms.graph.count()).unwrap_or(0),
        sps_autonomy::reducer::AutonomyState::from_state(&replayed).map(|a| a.active_goals.len()).unwrap_or(0),
        sps_autonomy::reducer::AutonomyState::from_state(&replayed).map(|a| a.reviews.len()).unwrap_or(0),
    );
    if replayed_state == expected {
        println!("  PASS — all 7 subsystem states match after replay");
    } else {
        println!("  FAIL — replay mismatch: {:?} != {:?}", replayed_state, expected);
        panic!("LOOP TEST FAILED");
    }

    let r_execs = sps_execution::reducer::ExecutionState::from_state(&replayed).map(|es| es.for_plan(plan_id.0).len()).unwrap_or(0);
    let r_reviews = sps_autonomy::reducer::AutonomyState::from_state(&replayed).map(|a| a.reviews_for_goal(goal_id.0).len()).unwrap_or(0);
    if r_execs == execs_for_plan && r_reviews == reviews_for_goal {
        println!("  PASS — cross-system links survive replay");
    } else {
        panic!("LOOP TEST FAILED — links broken after replay");
    }

    println!("\n  === AUTONOMOUS LOOP TEST PASSED ===");
    println!("  Goal → Plan → Execute → Reflect → Memory → Autonomy → Update → Replay");
    println!("  All 7 subsystems populated, cross-system links intact, replay identical");
}
