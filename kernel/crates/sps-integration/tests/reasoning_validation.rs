//! Reasoning Validation Suite — 8/8 PASS required.
//!
//! Reasoning is the analytical layer above Goals/Plans/Execution. If broken,
//! the system can't explain WHY it made decisions.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::{Event, RawEvent};
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::reducer::ReducerRegistry;
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::storage::port::StoragePort;
use sps_reasoning::reducer::{Alternative, Conflict, Degradation, ReasoningStep, Risk};
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

fn make_step(goal_id: uuid::Uuid, analyzer: &str, input: &str, tick: u64) -> ReasoningStep {
    ReasoningStep {
        id: uuid::Uuid::now_v7(),
        goal_id,
        analyzer: SmolStr::new(analyzer),
        input: input.to_string(),
        output: json!({"result": "ok"}),
        tick,
    }
}

fn dispatch_step(kernel: &SpsKernel, step: &ReasoningStep) -> Event {
    let payload = serde_json::to_value(step).unwrap();
    kernel.dispatch(RawEvent::new("reasoning.step", payload, Actor::owner(), 0)).unwrap()
}

// ─── Checkpoint 1 ─────────────────────────────────────────────────────────

#[test]
fn reasoning_checkpoint_1_step_materialization() {
    println!("\n=== REASONING CHECKPOINT 1: reasoning.step materializes state ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let goal_id = uuid::Uuid::now_v7();
    let step = make_step(goal_id, "goal_analyzer", "test goal", 1);
    dispatch_step(&kernel, &step);
    println!("  Dispatched reasoning.step");

    let (steps, traces) = kernel.query(|s| {
        let rs = sps_reasoning::reducer::ReasoningState::from_state(s).unwrap_or_default();
        (rs.steps.len(), rs.traces.len())
    });
    if steps == 1 && traces == 1 {
        println!("  PASS — 1 step + 1 trace in state");
    } else {
        println!("  FAIL — steps={}, traces={}", steps, traces);
        panic!("REASONING CHECKPOINT 1 FAILED");
    }
}

// ─── Checkpoint 2 ─────────────────────────────────────────────────────────

#[test]
fn reasoning_checkpoint_2_multiple_steps_same_goal_one_trace() {
    println!("\n=== REASONING CHECKPOINT 2: multiple steps same goal → one trace (Fix #10a) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let goal_id = uuid::Uuid::now_v7();
    let s1 = make_step(goal_id, "goal_analyzer", "analyze", 1);
    let s2 = make_step(goal_id, "task_decomposer", "decompose", 2);
    let s3 = make_step(goal_id, "risk_analyzer", "assess", 3);
    dispatch_step(&kernel, &s1);
    dispatch_step(&kernel, &s2);
    dispatch_step(&kernel, &s3);
    println!("  Dispatched 3 steps for the same goal");

    let (steps_count, traces_count, steps_in_trace) = kernel.query(|s| {
        let rs = sps_reasoning::reducer::ReasoningState::from_state(s).unwrap();
        let steps_count = rs.steps.len();
        let traces_count = rs.traces.len();
        let steps_in_trace = rs.traces.get(&goal_id).map(|t| t.steps.len()).unwrap_or(0);
        (steps_count, traces_count, steps_in_trace)
    });

    if steps_count == 3 && traces_count == 1 && steps_in_trace == 3 {
        println!("  PASS — 3 steps, 1 trace (3 steps in that trace)");
        println!("  Fix #10a verified: goal_id is the trace key, not step.id");
    } else {
        println!("  FAIL — steps={}, traces={}, steps_in_trace={}", steps_count, traces_count, steps_in_trace);
        println!("  EXPECTED: 3 steps, 1 trace, 3 steps in trace");
        panic!("REASONING CHECKPOINT 2 FAILED");
    }

    // Verify step order in trace matches dispatch order.
    let trace_steps: Vec<u64> = kernel.query(|s| {
        let rs = sps_reasoning::reducer::ReasoningState::from_state(s).unwrap();
        rs.traces.get(&goal_id).unwrap().steps.iter().map(|s| s.tick).collect()
    });
    if trace_steps == vec![1, 2, 3] {
        println!("  PASS — steps ordered by tick: {:?}", trace_steps);
    } else {
        println!("  FAIL — step order wrong: {:?}", trace_steps);
        panic!("REASONING CHECKPOINT 2 FAILED");
    }
}

// ─── Checkpoint 3 ─────────────────────────────────────────────────────────

#[test]
fn reasoning_checkpoint_3_alternative_generated() {
    println!("\n=== REASONING CHECKPOINT 3: reasoning.alternative_generated (Fix #10b) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let goal_id = uuid::Uuid::now_v7();
    let alt = Alternative {
        goal_id,
        description: "Use microservices instead of monolith".to_string(),
        confidence: 0.7,
        origin_tick: 1,
    };
    let payload = serde_json::to_value(&alt).unwrap();
    kernel.dispatch(RawEvent::new("reasoning.alternative_generated", payload, Actor::owner(), 0)).unwrap();
    println!("  Dispatched reasoning.alternative_generated");

    let count = kernel.query(|s| {
        sps_reasoning::reducer::ReasoningState::from_state(s)
            .map(|rs| rs.alternatives.len())
            .unwrap_or(0)
    });
    if count == 1 {
        println!("  PASS — 1 alternative in state (previously silently ignored)");
    } else {
        println!("  FAIL — expected 1, got {}", count);
        panic!("REASONING CHECKPOINT 3 FAILED");
    }
}

// ─── Checkpoint 4 ─────────────────────────────────────────────────────────

#[test]
fn reasoning_checkpoint_4_conflict_detected() {
    println!("\n=== REASONING CHECKPOINT 4: reasoning.conflict_detected (Fix #10b) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let conflict = Conflict {
        entities: vec![uuid::Uuid::now_v7(), uuid::Uuid::now_v7()],
        description: "Two agents claim the same file".to_string(),
        severity: 0.8,
        origin_tick: 1,
    };
    let payload = serde_json::to_value(&conflict).unwrap();
    kernel.dispatch(RawEvent::new("reasoning.conflict_detected", payload, Actor::owner(), 0)).unwrap();
    println!("  Dispatched reasoning.conflict_detected");

    let count = kernel.query(|s| {
        sps_reasoning::reducer::ReasoningState::from_state(s)
            .map(|rs| rs.conflicts.len())
            .unwrap_or(0)
    });
    if count == 1 {
        println!("  PASS — 1 conflict in state (previously silently ignored)");
    } else {
        println!("  FAIL — expected 1, got {}", count);
        panic!("REASONING CHECKPOINT 4 FAILED");
    }
}

// ─── Checkpoint 5 ─────────────────────────────────────────────────────────

#[test]
fn reasoning_checkpoint_5_risk_assessed() {
    println!("\n=== REASONING CHECKPOINT 5: reasoning.risk_assessed (Fix #10b) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let risk = Risk {
        target_id: uuid::Uuid::now_v7(),
        risk_score: 0.65,
        factors: vec!["tight deadline".into(), "new technology".into()],
        origin_tick: 1,
    };
    let payload = serde_json::to_value(&risk).unwrap();
    kernel.dispatch(RawEvent::new("reasoning.risk_assessed", payload, Actor::owner(), 0)).unwrap();
    println!("  Dispatched reasoning.risk_assessed");

    let count = kernel.query(|s| {
        sps_reasoning::reducer::ReasoningState::from_state(s)
            .map(|rs| rs.risks.len())
            .unwrap_or(0)
    });
    if count == 1 {
        println!("  PASS — 1 risk in state (previously silently ignored)");
    } else {
        println!("  FAIL — expected 1, got {}", count);
        panic!("REASONING CHECKPOINT 5 FAILED");
    }
}

// ─── Checkpoint 6 ─────────────────────────────────────────────────────────

#[test]
fn reasoning_checkpoint_6_replay() {
    println!("\n=== REASONING CHECKPOINT 6: replay produces identical ReasoningState ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let goal_id = uuid::Uuid::now_v7();
    dispatch_step(&kernel, &make_step(goal_id, "a1", "i1", 1));
    dispatch_step(&kernel, &make_step(goal_id, "a2", "i2", 2));
    kernel.dispatch(RawEvent::new(
        "reasoning.alternative_generated",
        serde_json::to_value(&Alternative {
            goal_id,
            description: "alt".into(),
            confidence: 0.5,
            origin_tick: 3,
        }).unwrap(),
        Actor::owner(), 0,
    )).unwrap();

    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();
    let live_steps = sps_reasoning::reducer::ReasoningState::from_state(&live).unwrap().steps.len();
    let live_alts = sps_reasoning::reducer::ReasoningState::from_state(&live).unwrap().alternatives.len();

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_reasoning::reducer::ReasoningReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.last_hash(), live_hash, "FAIL: hash mismatch");
    let replayed_rs = sps_reasoning::reducer::ReasoningState::from_state(&replayed).unwrap();
    if replayed_rs.steps.len() == live_steps && replayed_rs.alternatives.len() == live_alts {
        println!("  PASS — replayed: {} steps + {} alternatives (matches live)",
            replayed_rs.steps.len(), replayed_rs.alternatives.len());
    } else {
        println!("  FAIL — live: {} steps + {} alts, replayed: {} steps + {} alts",
            live_steps, live_alts, replayed_rs.steps.len(), replayed_rs.alternatives.len());
        panic!("REASONING CHECKPOINT 6 FAILED");
    }
}

// ─── Checkpoint 7 ─────────────────────────────────────────────────────────

#[test]
fn reasoning_checkpoint_7_sqlite() {
    println!("\n=== REASONING CHECKPOINT 7: SQLite backend ===");
    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open_in_memory().unwrap()
    );
    let kernel = boot_kernel(storage.clone());
    assert_eq!(kernel.backend_name(), "sqlite");

    let goal_id = uuid::Uuid::now_v7();
    dispatch_step(&kernel, &make_step(goal_id, "analyzer", "input", 1));
    println!("  Created reasoning step via SQLite");

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());

    drop(kernel);
    let kernel2 = boot_kernel(storage.clone());
    let steps = kernel2.query(|s| {
        sps_reasoning::reducer::ReasoningState::from_state(s)
            .map(|rs| rs.steps.len())
            .unwrap_or(0)
    });
    if steps == 1 {
        println!("  PASS — after restart, 1 step still present");
    } else {
        println!("  FAIL — after restart, got {}", steps);
        panic!("REASONING CHECKPOINT 7 FAILED");
    }
}

// ─── Checkpoint 8 ─────────────────────────────────────────────────────────

#[test]
fn reasoning_checkpoint_8_deterministic_ids() {
    println!("\n=== REASONING CHECKPOINT 8: deterministic step IDs across replay ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let goal_id = uuid::Uuid::now_v7();
    let s1 = make_step(goal_id, "a1", "i1", 1);
    let s2 = make_step(goal_id, "a2", "i2", 2);
    let id1 = s1.id;
    let id2 = s2.id;
    dispatch_step(&kernel, &s1);
    dispatch_step(&kernel, &s2);

    let live_ids: std::collections::BTreeSet<uuid::Uuid> = kernel.query(|s| {
        sps_reasoning::reducer::ReasoningState::from_state(s)
            .map(|rs| rs.steps.keys().copied().collect())
            .unwrap_or_default()
    });
    assert!(live_ids.contains(&id1) && live_ids.contains(&id2));

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_reasoning::reducer::ReasoningReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    let replayed_ids: std::collections::BTreeSet<uuid::Uuid> =
        sps_reasoning::reducer::ReasoningState::from_state(&replayed)
            .map(|rs| rs.steps.keys().copied().collect())
            .unwrap_or_default();

    if live_ids == replayed_ids {
        println!("  PASS — step IDs deterministic across replay");
    } else {
        println!("  FAIL — step IDs differ after replay");
        panic!("REASONING CHECKPOINT 8 FAILED");
    }

    // Verify trace goal_id is preserved.
    let replayed_rs = sps_reasoning::reducer::ReasoningState::from_state(&replayed).unwrap();
    let trace = replayed_rs.traces.get(&goal_id);
    if let Some(t) = trace {
        if t.steps.len() == 2 && t.goal_id == goal_id {
            println!("  PASS — trace for goal {} has 2 steps (goal_id preserved)", &goal_id.to_string()[..8]);
        } else {
            println!("  FAIL — trace wrong: goal_id={:?}, steps={}", t.goal_id, t.steps.len());
            panic!("REASONING CHECKPOINT 8 FAILED");
        }
    } else {
        println!("  FAIL — trace not found after replay");
        panic!("REASONING CHECKPOINT 8 FAILED");
    }
}
