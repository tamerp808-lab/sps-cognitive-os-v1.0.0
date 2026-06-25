//! Sprint Hardening: H1 Concurrency + H2 Corruption + H3 Snapshot + H4 Scale + Full Cognitive Loop

use std::sync::Arc;
use std::time::Instant;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::reducer::ReducerRegistry;
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::sink::EventSink;
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

fn make_pipeline() -> Arc<sps_core::reducer::ReducerPipeline> {
    Arc::new(sps_core::reducer::ReducerPipeline::new(Arc::new({
        let mut reg = ReducerRegistry::new();
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        sps_goals::reducer::GoalReducer::register(&mut reg);
        sps_planner::reducer::PlannerReducer::register(&mut reg);
        sps_execution::reducer::ExecutionReducer::register(&mut reg);
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        sps_autonomy::reducer::AutonomyReducer::register(&mut reg);
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
            tags: vec![], origin_tick: 0, created_at: 0,
        };
        kernel.dispatch(RawEvent::new("memory.created", serde_json::to_value(&record).unwrap(), Actor::owner(), 0)).unwrap();
    }
}

// ════════════════════════════════════════════════════════════════════════
// H1: CONCURRENCY
// ════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn h1_concurrency_1000_parallel() {
    println!("\n=== H1: Concurrency — 1000 parallel dispatches ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    const N: usize = 1000;

    let kernel_clone = kernel.clone();
    let handles: Vec<tokio::task::JoinHandle<()>> = (0..N).map(|i| {
        let k = kernel_clone.clone();
        tokio::spawn(async move {
            let record = sps_memory::memory::MemoryRecord {
                id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
                kind: sps_memory::memory::MemoryKind::Episodic,
                title: SmolStr::new(format!("c-{}", i)),
                content: json!({"i": i}),
                tags: vec![], origin_tick: 0, created_at: 0,
            };
            let _ = k.dispatch(RawEvent::new("memory.created", serde_json::to_value(&record).unwrap(), Actor::owner(), 0));
        })
    }).collect();

    for h in handles { h.await.unwrap(); }
    println!("  Dispatched {} events concurrently", N);

    let event_count = kernel.query(|s| s.event_count());
    assert_eq!(event_count, N as u64, "FAIL: event_count={} expected {}", event_count, N);
    println!("  PASS — event_count == {}", N);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken");
    assert_eq!(report.events_verified, N as u64);
    println!("  PASS — hash chain intact ({} events)", N);

    let events = storage.read_events_from(1, N + 100).unwrap();
    let ticks: std::collections::BTreeSet<u64> = events.iter().map(|e| e.tick).collect();
    assert_eq!(ticks.len(), N, "FAIL: duplicate ticks");
    println!("  PASS — all {} ticks unique", N);

    let mem_count = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s).map(|ms| ms.graph.count()).unwrap_or(0)
    });
    assert_eq!(mem_count, N, "FAIL: memories={}", mem_count);
    println!("  PASS — all {} memories materialized", N);

    println!("\n  === H1 PASSED ===");
}

// ════════════════════════════════════════════════════════════════════════
// H2: CORRUPTION
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h2a_tampered_hash_detected() {
    println!("\n=== H2a: Tampered hash → kernel refuses boot ===");
    let db_path = std::env::temp_dir().join(format!("sps_h2a_{}.db", uuid::Uuid::now_v7()));

    {
        let storage: Arc<dyn StoragePort> = Arc::new(sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap());
        let kernel = boot_kernel(storage.clone());
        dispatch_memories(&kernel, 5);
        drop(kernel);
    }

    // Tamper: modify hash inside event_json for tick 3.
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let table_name: String = conn
            .query_row("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE '%event%'", [], |row| row.get(0))
            .unwrap_or_else(|_| "events".to_string());
        let original_json: String = conn
            .query_row(&format!("SELECT event_json FROM {} WHERE tick = 3", table_name), [], |row| row.get(0))
            .unwrap();
        let tampered_json = original_json.replace("\"hash\":\"", "\"hash\":\"deadbeef");
        conn.execute(&format!("UPDATE {} SET event_json = ? WHERE tick = 3", table_name), rusqlite::params![tampered_json]).unwrap();
        println!("  Tampered hash in event_json for tick 3");
    }

    let storage: Arc<dyn StoragePort> = Arc::new(sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap());
    let result = SpsKernel::boot_with(storage, KernelConfig::default(), |_| {});
    assert!(result.is_err(), "FAIL: kernel booted with tampered hash");
    println!("  PASS — kernel refused boot (tampered hash detected)");

    std::fs::remove_file(&db_path).ok();
}

#[test]
fn h2b_broken_chain_detected() {
    println!("\n=== H2b: Broken chain → detected ===");
    let db_path = std::env::temp_dir().join(format!("sps_h2b_{}.db", uuid::Uuid::now_v7()));

    {
        let storage: Arc<dyn StoragePort> = Arc::new(sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap());
        let kernel = boot_kernel(storage.clone());
        dispatch_memories(&kernel, 5);
        drop(kernel);
    }

    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let table_name: String = conn
            .query_row("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE '%event%'", [], |row| row.get(0))
            .unwrap_or_else(|_| "events".to_string());
        let original_json: String = conn
            .query_row(&format!("SELECT event_json FROM {} WHERE tick = 3", table_name), [], |row| row.get(0))
            .unwrap();
        let tampered_json = original_json.replace("\"prev_hash\":\"", "\"prev_hash\":\"00000000000000000000000000000000000000000000000000000000000000ff");
        conn.execute(&format!("UPDATE {} SET event_json = ? WHERE tick = 3", table_name), rusqlite::params![tampered_json]).unwrap();
        println!("  Tampered prev_hash in event_json for tick 3");
    }

    let storage: Arc<dyn StoragePort> = Arc::new(sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap());
    let result = SpsKernel::boot_with(storage, KernelConfig::default(), |_| {});
    assert!(result.is_err(), "FAIL: kernel booted with broken chain");
    println!("  PASS — kernel refused boot (broken chain detected)");

    std::fs::remove_file(&db_path).ok();
}

// ════════════════════════════════════════════════════════════════════════
// H3: SNAPSHOT
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h3_snapshot_replay_matches_genesis() {
    println!("\n=== H3: Snapshot + Tail == Genesis Replay ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    dispatch_memories(&kernel, 500);
    println!("  Phase 1: 500 events dispatched");

    let snapshot = kernel.snapshot(0).unwrap();
    snapshot.verify().unwrap();
    println!("  Phase 2: snapshot at tick {} (verified)", snapshot.tick);

    dispatch_memories(&kernel, 500);
    println!("  Phase 3: 500 more events (total: 1000)");

    let live_hash = kernel.query(|s| s.last_hash().clone());
    let live_count = kernel.store().count().unwrap_or(0);
    let live_mems = kernel.query(|s| sps_memory::reducer::MemoryState::from_state(s).map(|ms| ms.graph.count()).unwrap_or(0));

    let pipeline = make_pipeline();
    let engine = ReplayEngine::new(pipeline.clone());

    let t_genesis = Instant::now();
    let genesis_state = engine.replay_from_genesis(storage.as_ref()).unwrap();
    let genesis_ms = t_genesis.elapsed().as_millis();

    let t_snap = Instant::now();
    let snap_state = engine.replay_from_snapshot(storage.as_ref(), &snapshot).unwrap();
    let snap_ms = t_snap.elapsed().as_millis();

    assert_eq!(genesis_state.event_count(), live_count, "FAIL: event_count genesis");
    assert_eq!(snap_state.event_count(), live_count, "FAIL: event_count snapshot");
    assert_eq!(genesis_state.last_hash(), live_hash, "FAIL: hash genesis");
    assert_eq!(snap_state.last_hash(), live_hash, "FAIL: hash snapshot");

    let gen_mems = sps_memory::reducer::MemoryState::from_state(&genesis_state).map(|ms| ms.graph.count()).unwrap_or(0);
    let snap_mems = sps_memory::reducer::MemoryState::from_state(&snap_state).map(|ms| ms.graph.count()).unwrap_or(0);
    assert_eq!(gen_mems, live_mems, "FAIL: mem count genesis");
    assert_eq!(snap_mems, live_mems, "FAIL: mem count snapshot");

    println!("  PASS — genesis == snapshot == live (count={}, hash match, mems={})", live_count, live_mems);
    println!("  genesis={}ms, snapshot+tail={}ms ({:.1}x)", genesis_ms, snap_ms, genesis_ms as f64 / snap_ms.max(1) as f64);

    println!("\n  === H3 PASSED ===");
}

// ════════════════════════════════════════════════════════════════════════
// H4: SCALE (benchmark, not pass/fail)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h4_scale_characterization() {
    println!("\n=== H4: Scale Characterization ===");

    for &n in &[100usize, 500, 1000, 2000] {
        let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
        let kernel = boot_kernel(storage.clone());

        let t0 = Instant::now();
        dispatch_memories(&kernel, n);
        let dispatch_ms = t0.elapsed().as_millis();

        let store_count = kernel.store().count().unwrap_or(0);
        let meta_count = kernel.query(|s| s.event_count());
        assert_eq!(store_count, n as u64, "FAIL: store count at {}", n);
        assert_eq!(meta_count, n as u64, "FAIL: meta count drift at {}", n);

        let t1 = Instant::now();
        let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
        let verify_ms = t1.elapsed().as_millis();
        assert!(report.failure.is_none());

        let pipeline = make_pipeline();
        let engine = ReplayEngine::new(pipeline);
        let t2 = Instant::now();
        let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
        let replay_ms = t2.elapsed().as_millis();

        assert_eq!(replayed.event_count(), n as u64, "FAIL: replayed count at {}", n);
        assert_eq!(replayed.last_hash(), kernel.query(|s| s.last_hash()), "FAIL: hash at {}", n);

        let d_rate = n as f64 / (dispatch_ms.max(1) as f64 / 1000.0);
        let r_rate = n as f64 / (replay_ms.max(1) as f64 / 1000.0);
        println!("  {:>5} events: dispatch={}ms ({:.0}/s), verify={}ms, replay={}ms ({:.0}/s), per-ev: d={:.1}μs r={:.1}μs",
            n, dispatch_ms, d_rate, verify_ms, replay_ms, r_rate,
            dispatch_ms as f64 * 1000.0 / n as f64, replay_ms as f64 * 1000.0 / n as f64);
    }

    println!("\n  === H4 PASSED (all correctness invariants at all scales) ===");
}

// ════════════════════════════════════════════════════════════════════════
// FULL COGNITIVE LOOP
// ════════════════════════════════════════════════════════════════════════

#[test]
fn full_cognitive_loop() {
    println!("\n=== FULL COGNITIVE LOOP: Goal → Plan → Execute → Reflect → Memory → Autonomy → Update → Replay ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // 1. Goal
    let goal_id = sps_goals::hierarchy::GoalId(uuid::Uuid::now_v7());
    let goal = sps_goals::hierarchy::Goal {
        id: goal_id, title: SmolStr::new("Learn Rust"),
        description: "Master Rust".to_string(), priority: 5,
        status: sps_goals::hierarchy::GoalStatus::Active,
        objectives: Vec::new(), dependencies: Vec::new(),
        created_at: 0, origin_tick: 0,
    };
    kernel.dispatch(RawEvent::new("goal.created", serde_json::to_value(&goal).unwrap(), Actor::owner(), 0)).unwrap();
    println!("  1. goal.created");

    // 2. Plan
    let plan_id = sps_planner::plan::PlanId::new();
    let plan = sps_planner::plan::Plan {
        id: plan_id, goal_id, template: SmolStr::new("learning"),
        steps: vec![], status: sps_planner::plan::PlanStatus::Approved,
        created_at: 0, origin_tick: 0,
    };
    kernel.dispatch(RawEvent::new("plan.created", serde_json::to_value(&plan).unwrap(), Actor::owner(), 0)).unwrap();
    println!("  2. plan.created");

    // 3. Execute (2 executions linked to plan)
    kernel.dispatch(RawEvent::new("execution.succeeded", json!({"operation": "Read Book", "plan_id": plan_id.0.to_string()}), Actor::owner(), 0)).unwrap();
    kernel.dispatch(RawEvent::new("execution.succeeded", json!({"operation": "Write Code", "plan_id": plan_id.0.to_string()}), Actor::owner(), 0)).unwrap();
    println!("  3. execution.succeeded × 2");

    // 4. Reflect
    let analysis = sps_reflection::analyzers::SuccessAnalyzer::analyze(
        uuid::Uuid::now_v7(), vec!["read book".into()], "practice reinforced theory".into(), true,
    );
    kernel.dispatch(RawEvent::new("reflection.success_analyzed", serde_json::to_value(&analysis).unwrap(), Actor::owner(), 0)).unwrap();
    println!("  4. reflection.success_analyzed");

    // 5. Memory (2 memories)
    for (title, detail) in &[("Rust ownership", "borrow checker"), ("Rust async", "tokio")] {
        let record = sps_memory::memory::MemoryRecord {
            id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
            kind: sps_memory::memory::MemoryKind::Semantic,
            title: SmolStr::new(*title),
            content: json!({"detail": detail}),
            tags: vec![SmolStr::new("rust")], origin_tick: 0, created_at: 0,
        };
        kernel.dispatch(RawEvent::new("memory.created", serde_json::to_value(&record).unwrap(), Actor::owner(), 0)).unwrap();
    }
    println!("  5. memory.created × 2");

    // 6. Autonomy
    kernel.dispatch(RawEvent::new("autonomous.goal_activated", json!({"goal_id": goal_id.0.to_string(), "milestones": [{"title": "Week 1"}], "activated_at": 1000}), Actor::owner(), 0)).unwrap();
    kernel.dispatch(RawEvent::new("autonomous.weekly_review", json!({"goal_id": goal_id.0.to_string(), "review": "Good progress", "reviewed_at": 2000}), Actor::owner(), 0)).unwrap();
    println!("  6. autonomous.goal_activated + weekly_review");

    // 7. Goal progress update
    kernel.dispatch(RawEvent::new("goal.progress_updated", json!({"goal_id": goal_id.0.to_string(), "milestone": "Week 1", "completed": true}), Actor::owner(), 0)).unwrap();
    println!("  7. goal.progress_updated");

    // Verify all subsystems
    let (goals, plans, execs, refls, mems, activations, reviews) = kernel.query(|s| {
        let gs = sps_goals::reducer::GoalState::from_state(s).map(|gs| gs.tree.goals.len()).unwrap_or(0);
        let ps = sps_planner::reducer::PlannerState::from_state(s).map(|ps| ps.plans.len()).unwrap_or(0);
        let es = sps_execution::reducer::ExecutionState::from_state(s).map(|es| es.records.len()).unwrap_or(0);
        let rs = sps_reflection::reducer::ReflectionState::from_state(s).map(|rs| rs.reflections.len()).unwrap_or(0);
        let ms = sps_memory::reducer::MemoryState::from_state(s).map(|ms| ms.graph.count()).unwrap_or(0);
        let as_ = sps_autonomy::reducer::AutonomyState::from_state(s).map(|a| a.active_goals.len()).unwrap_or(0);
        let wr = sps_autonomy::reducer::AutonomyState::from_state(s).map(|a| a.reviews.len()).unwrap_or(0);
        (gs, ps, es, rs, ms, as_, wr)
    });

    let expected = (1, 1, 2, 1, 2, 1, 1);
    assert_eq!((goals, plans, execs, refls, mems, activations, reviews), expected,
        "FAIL: state mismatch");
    println!("\n  PASS — all 7 subsystems populated: goals={} plans={} execs={} refls={} mems={} activations={} reviews={}",
        goals, plans, execs, refls, mems, activations, reviews);

    // Cross-system links
    let execs_for_plan = kernel.query(|s| sps_execution::reducer::ExecutionState::from_state(s).map(|es| es.for_plan(plan_id.0).len()).unwrap_or(0));
    let reviews_for_goal = kernel.query(|s| sps_autonomy::reducer::AutonomyState::from_state(s).map(|a| a.reviews_for_goal(goal_id.0).len()).unwrap_or(0));
    assert_eq!(execs_for_plan, 2, "FAIL: execs_for_plan");
    assert_eq!(reviews_for_goal, 1, "FAIL: reviews_for_goal");
    println!("  PASS — cross-links: Executions→Plan={}, Reviews→Goal={}", execs_for_plan, reviews_for_goal);

    // Replay
    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();
    let live_count = kernel.store().count().unwrap_or(0);

    let pipeline = make_pipeline();
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.event_count(), live_count, "FAIL: event_count");
    assert_eq!(replayed.last_hash(), live_hash, "FAIL: hash");

    let r_state = (
        sps_goals::reducer::GoalState::from_state(&replayed).map(|gs| gs.tree.goals.len()).unwrap_or(0),
        sps_planner::reducer::PlannerState::from_state(&replayed).map(|ps| ps.plans.len()).unwrap_or(0),
        sps_execution::reducer::ExecutionState::from_state(&replayed).map(|es| es.records.len()).unwrap_or(0),
        sps_reflection::reducer::ReflectionState::from_state(&replayed).map(|rs| rs.reflections.len()).unwrap_or(0),
        sps_memory::reducer::MemoryState::from_state(&replayed).map(|ms| ms.graph.count()).unwrap_or(0),
        sps_autonomy::reducer::AutonomyState::from_state(&replayed).map(|a| a.active_goals.len()).unwrap_or(0),
        sps_autonomy::reducer::AutonomyState::from_state(&replayed).map(|a| a.reviews.len()).unwrap_or(0),
    );
    assert_eq!(r_state, expected, "FAIL: replay state mismatch");

    let r_execs = sps_execution::reducer::ExecutionState::from_state(&replayed).map(|es| es.for_plan(plan_id.0).len()).unwrap_or(0);
    let r_reviews = sps_autonomy::reducer::AutonomyState::from_state(&replayed).map(|a| a.reviews_for_goal(goal_id.0).len()).unwrap_or(0);
    assert_eq!(r_execs, execs_for_plan, "FAIL: replay execs_for_plan");
    assert_eq!(r_reviews, reviews_for_goal, "FAIL: replay reviews_for_goal");

    println!("  PASS — replay identical ({} events, all 7 subsystems, cross-links preserved)", live_count);
    println!("\n  === FULL COGNITIVE LOOP PASSED ===");
}
