//! Planner Validation Suite — 12/12 PASS required.
//!
//! Planner is the bridge between Goals (what we want) and Execution (what we do).
//! If Planner is broken, the entire Goal → Plan → Execute → Reflect pipeline breaks.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::{Event, RawEvent};
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::reducer::ReducerRegistry;
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::storage::port::StoragePort;
use sps_goals::hierarchy::{Goal, GoalId, GoalStatus};
use sps_planner::plan::{Plan, PlanId, PlanStatus, PlanStep};
use sps_planner::templates::{PlanTemplate, StepTemplate, builtin_templates};
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

fn create_goal(kernel: &SpsKernel, title: &str) -> (Event, GoalId) {
    let goal_id = GoalId(uuid::Uuid::now_v7());
    let goal = Goal {
        id: goal_id,
        title: SmolStr::new(title),
        description: String::new(),
        priority: 5,
        status: GoalStatus::Active,
        objectives: Vec::new(),
        dependencies: Vec::new(),
        created_at: 0,
        origin_tick: 0,
    };
    let payload = serde_json::to_value(&goal).unwrap();
    let event = kernel.dispatch(RawEvent::new("goal.created", payload, Actor::owner(), 0)).unwrap();
    (event, goal_id)
}

fn create_plan(kernel: &SpsKernel, goal_id: GoalId, template: &str, steps: Vec<PlanStep>) -> (Event, PlanId) {
    let plan_id = PlanId::new();
    let plan = Plan {
        id: plan_id,
        goal_id,
        template: SmolStr::new(template),
        steps,
        status: PlanStatus::Draft,
        created_at: 0,
        origin_tick: 0,
    };
    let payload = serde_json::to_value(&plan).unwrap();
    let event = kernel.dispatch(RawEvent::new("plan.created", payload, Actor::owner(), 0)).unwrap();
    (event, plan_id)
}

fn make_step(title: &str, idx: u32) -> PlanStep {
    PlanStep {
        id: uuid::Uuid::now_v7(),
        title: SmolStr::new(title),
        description: String::new(),
        index: idx,
        depends_on: if idx == 0 { Vec::new() } else { vec![idx - 1] },
        assigned_agent: None,
        parallelizable: false,
    }
}

fn plan_count(kernel: &SpsKernel) -> usize {
    kernel.query(|s| {
        sps_planner::reducer::PlannerState::from_state(s)
            .map(|ps| ps.plans.len())
            .unwrap_or(0)
    })
}

// ─── Checkpoint 1 ─────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_1_created() {
    println!("\n=== PLAN CHECKPOINT 1: plan.created updates PlannerState ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let (_, goal_id) = create_goal(&kernel, "Test goal");
    let (event, plan_id) = create_plan(&kernel, goal_id, "generic.workflow", vec![make_step("Step 1", 0)]);
    println!("  Dispatched plan.created at tick {}", event.tick);

    let count = plan_count(&kernel);
    if count == 1 {
        let ps = kernel.query(|s| sps_planner::reducer::PlannerState::from_state(s).unwrap());
        let p = ps.plans.values().next().unwrap();
        println!("  PASS — 1 plan in state: id={}..., template='{}', status={:?}, steps={}",
            &p.id.0.to_string()[..8], p.template, p.status, p.steps.len());
    } else {
        println!("  FAIL — expected 1 plan, got {}", count);
        panic!("PLAN CHECKPOINT 1 FAILED");
    }
}

// ─── Checkpoint 2 ─────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_2_status_transitions() {
    println!("\n=== PLAN CHECKPOINT 2: plan.approved → plan.executing → plan.completed ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let (_, goal_id) = create_goal(&kernel, "Test goal");
    let (_, plan_id) = create_plan(&kernel, goal_id, "test", vec![make_step("S1", 0)]);

    // approved
    kernel.dispatch(RawEvent::new(
        "plan.approved",
        json!({"plan_id": plan_id.0.to_string(), "status": "approved"}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Step 1: dispatched plan.approved");

    let status1 = kernel.query(|s| {
        sps_planner::reducer::PlannerState::from_state(s)
            .and_then(|ps| ps.plans.get(&plan_id.0).map(|p| p.status))
            .unwrap_or(PlanStatus::Draft)
    });
    assert_eq!(status1, PlanStatus::Approved, "FAIL: status not Approved after plan.approved");

    // executing
    kernel.dispatch(RawEvent::new(
        "plan.executing",
        json!({"plan_id": plan_id.0.to_string(), "status": "executing"}),
        Actor::owner(), 0,
    )).unwrap();
    let status2 = kernel.query(|s| {
        sps_planner::reducer::PlannerState::from_state(s)
            .and_then(|ps| ps.plans.get(&plan_id.0).map(|p| p.status))
            .unwrap_or(PlanStatus::Draft)
    });
    assert_eq!(status2, PlanStatus::Executing, "FAIL: status not Executing");

    // completed
    kernel.dispatch(RawEvent::new(
        "plan.completed",
        json!({"plan_id": plan_id.0.to_string(), "status": "completed"}),
        Actor::owner(), 0,
    )).unwrap();
    let status3 = kernel.query(|s| {
        sps_planner::reducer::PlannerState::from_state(s)
            .and_then(|ps| ps.plans.get(&plan_id.0).map(|p| p.status))
            .unwrap_or(PlanStatus::Draft)
    });
    if status3 == PlanStatus::Completed {
        println!("  PASS — status transitions: Approved → Executing → Completed");
    } else {
        println!("  FAIL — final status = {:?} (expected Completed)", status3);
        panic!("PLAN CHECKPOINT 2 FAILED");
    }

    // Verify steps are still intact (status change shouldn't corrupt steps).
    let step_count = kernel.query(|s| {
        sps_planner::reducer::PlannerState::from_state(s)
            .and_then(|ps| ps.plans.get(&plan_id.0).map(|p| p.steps.len()))
            .unwrap_or(0)
    });
    if step_count == 1 {
        println!("  PASS — steps intact after status changes (1 step preserved)");
    } else {
        println!("  FAIL — steps corrupted: expected 1, got {}", step_count);
        panic!("PLAN CHECKPOINT 2 FAILED");
    }
}

// ─── Checkpoint 3 ─────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_3_completed_doesnt_corrupt_steps() {
    println!("\n=== PLAN CHECKPOINT 3: plan.completed preserves steps + goal_id ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let (_, goal_id) = create_goal(&kernel, "Test goal");
    let steps = vec![make_step("S1", 0), make_step("S2", 1), make_step("S3", 2)];
    let (_, plan_id) = create_plan(&kernel, goal_id, "test", steps);

    kernel.dispatch(RawEvent::new(
        "plan.completed",
        json!({"plan_id": plan_id.0.to_string(), "status": "completed"}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Dispatched plan.completed");

    let (status, step_count, plan_goal_id) = kernel.query(|s| {
        let ps = sps_planner::reducer::PlannerState::from_state(s).unwrap();
        let p = ps.plans.get(&plan_id.0).unwrap();
        (p.status, p.steps.len(), p.goal_id)
    });

    if status == PlanStatus::Completed && step_count == 3 && plan_goal_id == goal_id {
        println!("  PASS — status=Completed, steps=3 (preserved), goal_id intact");
    } else {
        println!("  FAIL — status={:?}, steps={}, goal_match={}",
            status, step_count, plan_goal_id == goal_id);
        panic!("PLAN CHECKPOINT 3 FAILED");
    }
}

// ─── Checkpoint 4 ─────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_4_goal_plan_link() {
    println!("\n=== PLAN CHECKPOINT 4: Goal → Plan link is structural ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let (_, goal_a) = create_goal(&kernel, "Goal A");
    let (_, goal_b) = create_goal(&kernel, "Goal B");
    let (_, plan_a1) = create_plan(&kernel, goal_a, "test", vec![make_step("S1", 0)]);
    let (_, plan_a2) = create_plan(&kernel, goal_a, "test", vec![make_step("S1", 0)]);
    let (_, plan_b1) = create_plan(&kernel, goal_b, "test", vec![make_step("S1", 0)]);
    println!("  Created 2 goals + 3 plans (2 for A, 1 for B)");

    // Query plans by goal_id.
    let (a_count, b_count) = kernel.query(|s| {
        let ps = sps_planner::reducer::PlannerState::from_state(s).unwrap();
        let a = ps.plans.values().filter(|p| p.goal_id == goal_a).count();
        let b = ps.plans.values().filter(|p| p.goal_id == goal_b).count();
        (a, b)
    });

    if a_count == 2 && b_count == 1 {
        println!("  PASS — Goal A has {} plans, Goal B has {} plans (correctly attributed)",
            a_count, b_count);
        println!("  PASS — Plan.goal_id is structural, not just an event payload tag");
    } else {
        println!("  FAIL — A={}, B={} (expected 2, 1)", a_count, b_count);
        panic!("PLAN CHECKPOINT 4 FAILED");
    }
}

// ─── Checkpoint 5 ─────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_5_plan_to_execution_link() {
    println!("\n=== PLAN CHECKPOINT 5: Plan → Execution link (after Fix #6) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let (_, goal_id) = create_goal(&kernel, "Test goal");
    let (_, plan_id) = create_plan(&kernel, goal_id, "test", vec![make_step("S1", 0)]);

    // Dispatch execution events WITH plan_id in the payload (the new pattern).
    kernel.dispatch(RawEvent::new(
        "execution.started",
        json!({"operation": "execute_plan", "plan_id": plan_id.0.to_string()}),
        Actor::owner(), 0,
    )).unwrap();
    kernel.dispatch(RawEvent::new(
        "execution.succeeded",
        json!({"operation": "execute_plan", "plan_id": plan_id.0.to_string(), "duration_ms": 100}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Step 1: dispatched execution.started + execution.succeeded with plan_id in payload");

    // Check ExecutionState.
    let exec_count = kernel.query(|s| {
        sps_execution::reducer::ExecutionState::from_state(s)
            .map(|es| es.records.len())
            .unwrap_or(0)
    });
    assert_eq!(exec_count, 1, "FAIL: expected 1 execution record, got {}", exec_count);
    println!("  Step 2: PASS — 1 execution record exists");

    // Check the link: ExecutionRecord.plan_id should match.
    let linked_plan_id = kernel.query(|s| {
        let es = sps_execution::reducer::ExecutionState::from_state(s).unwrap();
        es.records.values().next().and_then(|r| r.plan_id)
    });
    if linked_plan_id == Some(plan_id.0) {
        println!("  Step 3: PASS — ExecutionRecord.plan_id matches Plan.id");
    } else {
        println!("  FAIL — ExecutionRecord.plan_id = {:?}, expected Some({})",
            linked_plan_id, plan_id.0);
        panic!("PLAN CHECKPOINT 5 FAILED");
    }

    // Use the for_plan() query helper.
    let execs_for_plan = kernel.query(|s| {
        sps_execution::reducer::ExecutionState::from_state(s)
            .map(|es| es.for_plan(plan_id.0).len())
            .unwrap_or(0)
    });
    if execs_for_plan == 1 {
        println!("  Step 4: PASS — for_plan(plan_id) returned 1 execution");
    } else {
        println!("  FAIL — for_plan returned {}, expected 1", execs_for_plan);
        panic!("PLAN CHECKPOINT 5 FAILED");
    }

    println!("  PASS — Plan → Execution link works (unidirectional, no duplication)");
}

// ─── Checkpoint 5b ────────────────────────────────────────────────────────
// Multi-plan, multi-exec + query + replay.
//
//   Goal G
//     ├─ Plan P1
//     │    ├─ Exec E1
//     │    └─ Exec E2
//     └─ Plan P2
//          └─ Exec E3
//
// Verify:
//   - for_plan(P1) → [E1, E2]
//   - for_plan(P2) → [E3]
//   - Replay produces identical query results

#[test]
fn plan_checkpoint_5b_multi_plan_multi_exec_query_replay() {
    println!("\n=== PLAN CHECKPOINT 5b: Multi-plan, multi-exec + query + replay ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let (_, goal_id) = create_goal(&kernel, "Goal G");
    let (_, plan_p1) = create_plan(&kernel, goal_id, "P1", vec![make_step("S1", 0)]);
    let (_, plan_p2) = create_plan(&kernel, goal_id, "P2", vec![make_step("S1", 0)]);
    println!("  Created Goal G + Plan P1 + Plan P2");

    // E1, E2 for P1; E3 for P2.
    kernel.dispatch(RawEvent::new(
        "execution.succeeded",
        json!({"operation": "E1", "plan_id": plan_p1.0.to_string(), "duration_ms": 10}),
        Actor::owner(), 0,
    )).unwrap();
    kernel.dispatch(RawEvent::new(
        "execution.succeeded",
        json!({"operation": "E2", "plan_id": plan_p1.0.to_string(), "duration_ms": 20}),
        Actor::owner(), 0,
    )).unwrap();
    kernel.dispatch(RawEvent::new(
        "execution.succeeded",
        json!({"operation": "E3", "plan_id": plan_p2.0.to_string(), "duration_ms": 30}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Created E1+E2 (plan=P1) + E3 (plan=P2)");

    // Verify live queries.
    let (p1_count, p2_count) = kernel.query(|s| {
        let es = sps_execution::reducer::ExecutionState::from_state(s).unwrap();
        (es.for_plan(plan_p1.0).len(), es.for_plan(plan_p2.0).len())
    });
    if p1_count == 2 && p2_count == 1 {
        println!("  PASS (live) — for_plan(P1)={}, for_plan(P2)={}", p1_count, p2_count);
    } else {
        println!("  FAIL (live) — for_plan(P1)={}, for_plan(P2)={}", p1_count, p2_count);
        panic!("PLAN CHECKPOINT 5b FAILED");
    }

    // Verify the operations match (E1, E2 for P1; E3 for P2).
    let p1_ops: Vec<String> = kernel.query(|s| {
        let es = sps_execution::reducer::ExecutionState::from_state(s).unwrap();
        es.for_plan(plan_p1.0).iter().map(|r| r.operation.as_str().to_string()).collect()
    });
    let p2_ops: Vec<String> = kernel.query(|s| {
        let es = sps_execution::reducer::ExecutionState::from_state(s).unwrap();
        es.for_plan(plan_p2.0).iter().map(|r| r.operation.as_str().to_string()).collect()
    });
    if p1_ops == vec!["E1".to_string(), "E2".to_string()] && p2_ops == vec!["E3".to_string()] {
        println!("  PASS (live) — P1 ops={:?}, P2 ops={:?}", p1_ops, p2_ops);
    } else {
        println!("  FAIL (live) — P1 ops={:?}, P2 ops={:?}", p1_ops, p2_ops);
        panic!("PLAN CHECKPOINT 5b FAILED");
    }

    // Replay from genesis.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_goals::reducer::GoalReducer::register(&mut reg);
        sps_planner::reducer::PlannerReducer::register(&mut reg);
        sps_execution::reducer::ExecutionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    // Verify same query results after replay.
    let (rp1_count, rp2_count) = {
        let es = sps_execution::reducer::ExecutionState::from_state(&replayed).unwrap();
        (es.for_plan(plan_p1.0).len(), es.for_plan(plan_p2.0).len())
    };
    let rp1_ops: Vec<String> = {
        let es = sps_execution::reducer::ExecutionState::from_state(&replayed).unwrap();
        es.for_plan(plan_p1.0).iter().map(|r| r.operation.as_str().to_string()).collect()
    };
    let rp2_ops: Vec<String> = {
        let es = sps_execution::reducer::ExecutionState::from_state(&replayed).unwrap();
        es.for_plan(plan_p2.0).iter().map(|r| r.operation.as_str().to_string()).collect()
    };

    if rp1_count == 2 && rp2_count == 1
        && rp1_ops == p1_ops && rp2_ops == p2_ops {
        println!("  PASS (replayed) — for_plan(P1)={} ops={:?}, for_plan(P2)={} ops={:?}",
            rp1_count, rp1_ops, rp2_count, rp2_ops);
    } else {
        println!("  FAIL (replayed) — P1: count={} ops={:?}, P2: count={} ops={:?}",
            rp1_count, rp1_ops, rp2_count, rp2_ops);
        println!("  Live was: P1 count={} ops={:?}, P2 count={} ops={:?}",
            p1_count, p1_ops, p2_count, p2_ops);
        panic!("PLAN CHECKPOINT 5b FAILED — replay mismatch");
    }

    println!("  PASS — Goal→Plan→Execution query works live AND after replay");
}

// ─── Checkpoint 6 ─────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_6_replay() {
    println!("\n=== PLAN CHECKPOINT 6: replay produces identical PlannerState ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let (_, goal_id) = create_goal(&kernel, "Test goal");
    let (_, plan_id) = create_plan(&kernel, goal_id, "test", vec![make_step("S1", 0)]);
    kernel.dispatch(RawEvent::new(
        "plan.approved",
        json!({"plan_id": plan_id.0.to_string(), "status": "approved"}),
        Actor::owner(), 0,
    )).unwrap();

    let live = kernel.query(|s| s.clone());
    let live_count = live.event_count();
    let live_hash = live.last_hash().clone();
    println!("  Live: {} events, hash={}", live_count, &live_hash.to_string()[..16]);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_planner::reducer::PlannerReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.event_count(), live_count, "FAIL: event_count mismatch");
    assert_eq!(replayed.last_hash(), live_hash, "FAIL: last_hash mismatch");

    let live_ps = sps_planner::reducer::PlannerState::from_state(&live).unwrap();
    let replayed_ps = sps_planner::reducer::PlannerState::from_state(&replayed).unwrap();
    if live_ps.plans.len() == replayed_ps.plans.len() {
        println!("  PASS — same plan count ({} == {})", live_ps.plans.len(), replayed_ps.plans.len());
    } else {
        println!("  FAIL — plan count mismatch (live={}, replayed={})",
            live_ps.plans.len(), replayed_ps.plans.len());
        panic!("PLAN CHECKPOINT 6 FAILED");
    }

    // Verify status preserved.
    let live_status = live_ps.plans.get(&plan_id.0).map(|p| p.status);
    let replayed_status = replayed_ps.plans.get(&plan_id.0).map(|p| p.status);
    assert_eq!(live_status, replayed_status, "FAIL: status mismatch after replay");
    println!("  PASS — plan status preserved through replay ({:?})", live_status);
}

// ─── Checkpoint 7 ─────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_7_sqlite() {
    println!("\n=== PLAN CHECKPOINT 7: SQLite backend ===");
    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open_in_memory().unwrap()
    );
    let kernel = boot_kernel(storage.clone());
    assert_eq!(kernel.backend_name(), "sqlite");

    let (_, goal_id) = create_goal(&kernel, "Test goal");
    let (_, _) = create_plan(&kernel, goal_id, "sqlite_test", vec![make_step("S1", 0)]);
    println!("  Created plan via SQLite backend");

    assert_eq!(plan_count(&kernel), 1);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());
    assert_eq!(report.events_verified, 2);
    println!("  PASS — SQLite hash chain verified (2 events)");

    drop(kernel);
    let kernel2 = boot_kernel(storage.clone());
    let count_after = plan_count(&kernel2);
    if count_after == 1 {
        println!("  PASS — after restart, 1 plan still present");
    } else {
        println!("  FAIL — after restart, got {}", count_after);
        panic!("PLAN CHECKPOINT 7 FAILED");
    }
}

// ─── Checkpoint 8 ─────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_8_crash_recovery() {
    println!("\n=== PLAN CHECKPOINT 8: crash recovery ===");
    let db_path = std::env::temp_dir().join(format!("sps_plan_crash_{}.db", uuid::Uuid::now_v7()));

    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel = boot_kernel(storage.clone());
        for i in 0..10 {
            let (_, goal_id) = create_goal(&kernel, &format!("Goal {}", i));
            let (_, _) = create_plan(&kernel, goal_id, "crash_test", vec![make_step("S", 0)]);
        }
        let count_before = plan_count(&kernel);
        println!("  Phase 1: created {} plans", count_before);
        println!("  CRASH — dropping kernel");
    }

    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel2 = boot_kernel(storage.clone());
        let count_after = plan_count(&kernel2);
        if count_after == 10 {
            println!("  Phase 2: PASS — reconstructed {} plans", count_after);
        } else {
            println!("  FAIL — expected 10, got {}", count_after);
            panic!("PLAN CHECKPOINT 8 FAILED");
        }
    }
    std::fs::remove_file(&db_path).ok();
}

// ─── Checkpoint 9 ─────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_9_large_corpus() {
    println!("\n=== PLAN CHECKPOINT 9: large corpus (500 plans) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    const N: usize = 500;
    let start = std::time::Instant::now();
    for i in 0..N {
        let (_, goal_id) = create_goal(&kernel, &format!("Goal {}", i));
        let (_, _) = create_plan(&kernel, goal_id, "stress", vec![make_step("S1", 0)]);
    }
    let dispatch_ms = start.elapsed().as_millis();
    println!("  Dispatched {} plans in {}ms ({:.0}/sec)",
        N, dispatch_ms, N as f64 / (dispatch_ms as f64 / 1000.0));

    assert_eq!(plan_count(&kernel), N);
    println!("  PASS — {} plans in state", N);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());
    assert_eq!(report.events_verified, (N * 2) as u64);
    println!("  PASS — hash chain verified ({} events)", report.events_verified);

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_planner::reducer::PlannerReducer::register(&mut reg);
        sps_goals::reducer::GoalReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replay_start = std::time::Instant::now();
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
    let replay_ms = replay_start.elapsed().as_millis();
    let replayed_count = sps_planner::reducer::PlannerState::from_state(&replayed)
        .map(|ps| ps.plans.len()).unwrap_or(0);
    if replayed_count == N {
        println!("  PASS — replayed {} plans in {}ms ({:.0}/sec)",
            N, replay_ms, N as f64 / (replay_ms as f64 / 1000.0));
    } else {
        println!("  FAIL — replayed {} (expected {})", replayed_count, N);
        panic!("PLAN CHECKPOINT 9 FAILED");
    }
}

// ─── Checkpoint 10 ────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_10_multi_goal_planning() {
    println!("\n=== PLAN CHECKPOINT 10: multi-goal planning (isolation) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let (_, goal_a) = create_goal(&kernel, "Goal A");
    let (_, goal_b) = create_goal(&kernel, "Goal B");
    let (_, goal_c) = create_goal(&kernel, "Goal C");

    // 2 plans for A, 3 for B, 1 for C.
    for _ in 0..2 { create_plan(&kernel, goal_a, "test", vec![make_step("S", 0)]); }
    for _ in 0..3 { create_plan(&kernel, goal_b, "test", vec![make_step("S", 0)]); }
    create_plan(&kernel, goal_c, "test", vec![make_step("S", 0)]);
    println!("  Created 6 plans across 3 goals (A=2, B=3, C=1)");

    let counts = kernel.query(|s| {
        let ps = sps_planner::reducer::PlannerState::from_state(s).unwrap();
        let a = ps.plans.values().filter(|p| p.goal_id == goal_a).count();
        let b = ps.plans.values().filter(|p| p.goal_id == goal_b).count();
        let c = ps.plans.values().filter(|p| p.goal_id == goal_c).count();
        (a, b, c)
    });

    if counts == (2, 3, 1) {
        println!("  PASS — A={}, B={}, C={} (correctly isolated)", counts.0, counts.1, counts.2);
    } else {
        println!("  FAIL — A={}, B={}, C={} (expected 2, 3, 1)", counts.0, counts.1, counts.2);
        panic!("PLAN CHECKPOINT 10 FAILED");
    }
}

// ─── Checkpoint 11 ────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_11_malformed_payload_rejected() {
    println!("\n=== PLAN CHECKPOINT 11: malformed payload rejected at dispatch ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Valid plan first.
    let (_, goal_id) = create_goal(&kernel, "Test goal");
    let (_, _) = create_plan(&kernel, goal_id, "test", vec![make_step("S", 0)]);
    println!("  Step 1: dispatched 1 valid plan");

    // Malformed plan.created (missing required fields).
    let malformed = json!({
        "template": "incomplete",
        // missing: id, goal_id, steps, status, created_at, origin_tick
    });
    let result = kernel.dispatch(RawEvent::new(
        "plan.created",
        malformed,
        Actor::owner(),
        0,
    ));

    if result.is_err() {
        println!("  Step 2: PASS — malformed plan.created rejected at dispatch");
    } else {
        println!("  FAIL — malformed payload was accepted");
        panic!("PLAN CHECKPOINT 11 FAILED");
    }

    assert_eq!(plan_count(&kernel), 1, "FAIL: malformed plan leaked into state");
    println!("  Step 3: PASS — only 1 plan in state (malformed rejected)");

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert_eq!(report.events_verified, 2, "FAIL: expected 2 events, got {}", report.events_verified);
    println!("  Step 4: PASS — hash chain has 2 events (no malformed event in chain)");

    println!("  PASS — validate-on-write covers Planner");
}

// ─── Checkpoint 12 ────────────────────────────────────────────────────────

#[test]
fn plan_checkpoint_12_deterministic_ids() {
    println!("\n=== PLAN CHECKPOINT 12: deterministic IDs (Plan IDs come from payload, not reducer) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let (_, goal_id) = create_goal(&kernel, "Test goal");
    let (_, plan_id) = create_plan(&kernel, goal_id, "test", vec![make_step("S1", 0)]);

    // Capture live plan ID + step IDs.
    let live_plan_id = plan_id.0;
    let live_step_ids: Vec<uuid::Uuid> = kernel.query(|s| {
        let ps = sps_planner::reducer::PlannerState::from_state(s).unwrap();
        let p = ps.plans.get(&plan_id.0).unwrap();
        p.steps.iter().map(|s| s.id).collect()
    });
    println!("  Live plan_id={}, step_ids={:?}",
        &live_plan_id.to_string()[..8],
        live_step_ids.iter().map(|i| i.to_string()[..8].to_string()).collect::<Vec<_>>());

    // Replay.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_planner::reducer::PlannerReducer::register(&mut reg);
        sps_goals::reducer::GoalReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    let replayed_plan_id: uuid::Uuid;
    let replayed_step_ids: Vec<uuid::Uuid>;
    {
        let ps = sps_planner::reducer::PlannerState::from_state(&replayed).unwrap();
        let p = ps.plans.values().next().unwrap();
        replayed_plan_id = p.id.0;
        replayed_step_ids = p.steps.iter().map(|s| s.id).collect();
    }

    // Plan IDs come from the event payload (Plan struct), so they're
    // already deterministic — the reducer just inserts the Plan as-is.
    if live_plan_id == replayed_plan_id {
        println!("  PASS — plan ID deterministic ({} == {})",
            &live_plan_id.to_string()[..8], &replayed_plan_id.to_string()[..8]);
    } else {
        println!("  FAIL — plan ID mismatch (live={}, replayed={})",
            &live_plan_id.to_string()[..8], &replayed_plan_id.to_string()[..8]);
        panic!("PLAN CHECKPOINT 12 FAILED");
    }

    // Step IDs ALSO come from the event payload (PlanStep struct), so they
    // should be deterministic too — UNLESS PlanTemplate::generate used
    // Uuid::now_v7() to create them (which would be non-deterministic).
    if live_step_ids == replayed_step_ids {
        println!("  PASS — step IDs deterministic");
        println!("  NOTE: step IDs come from event payload, so they're deterministic");
        println!("  NOTE: if PlanTemplate::generate() used Uuid::now_v7(), each plan");
        println!("        creation would produce different step IDs, but replay would");
        println!("        still match (since the same event is replayed).");
    } else {
        println!("  FAIL — step IDs differ");
        println!("  Live:     {:?}", live_step_ids);
        println!("  Replayed: {:?}", replayed_step_ids);
        panic!("PLAN CHECKPOINT 12 FAILED");
    }

    println!("  PASS — all IDs deterministic across replay");
}
