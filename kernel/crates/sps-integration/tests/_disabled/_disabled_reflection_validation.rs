//! Reflection Validation Suite — 12/12 PASS required.
//!
//! Reflection is the most cross-cutting subsystem — it analyzes Goals, Tasks,
//! Memories, and produces Patterns. Bugs here propagate everywhere.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::{Event, RawEvent};
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::reducer::ReducerRegistry;
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::storage::port::StoragePort;
use sps_reflection::analyzers::{FailureAnalysis, FailureAnalyzer, Pattern, PatternExtractor, RootCause, SuccessAnalysis, SuccessAnalyzer};
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

fn dispatch_success(kernel: &SpsKernel, analysis: &SuccessAnalysis) -> Event {
    let payload = serde_json::to_value(analysis).unwrap();
    kernel.dispatch(RawEvent::new("reflection.success_analyzed", payload, Actor::owner(), 0))
        .expect("dispatch reflection.success_analyzed")
}

fn dispatch_failure(kernel: &SpsKernel, analysis: &FailureAnalysis) -> Event {
    let payload = serde_json::to_value(analysis).unwrap();
    kernel.dispatch(RawEvent::new("reflection.failure_analyzed", payload, Actor::owner(), 0))
        .expect("dispatch reflection.failure_analyzed")
}

fn dispatch_pattern(kernel: &SpsKernel, pattern: &Pattern) -> Event {
    let payload = serde_json::to_value(pattern).unwrap();
    kernel.dispatch(RawEvent::new("reflection.pattern_extracted", payload, Actor::owner(), 0))
        .expect("dispatch reflection.pattern_extracted")
}

fn reflection_count(kernel: &SpsKernel) -> usize {
    kernel.query(|s| {
        sps_reflection::reducer::ReflectionState::from_state(s)
            .map(|rs| rs.reflections.len())
            .unwrap_or(0)
    })
}

// ─── Checkpoint 1 ─────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_1_success_analyzed() {
    println!("\n=== REFL CHECKPOINT 1: reflection.success_analyzed produces Reflection ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let task_id = uuid::Uuid::now_v7();
    let analysis = SuccessAnalyzer::analyze(
        task_id,
        vec!["clean code".into(), "fast tests".into()],
        "modular design paid off".into(),
        true,
    );
    let event = dispatch_success(&kernel, &analysis);
    println!("  Dispatched reflection.success_analyzed at tick {}", event.tick);

    let count = reflection_count(&kernel);
    if count == 1 {
        let rs = kernel.query(|s| sps_reflection::reducer::ReflectionState::from_state(s).unwrap());
        let r = rs.reflections.values().next().unwrap();
        match r {
            sps_reflection::reducer::Reflection::Success(s) => {
                println!("  PASS — 1 Reflection::Success in state");
                println!("    what_worked={:?}, generalizable={}", s.what_worked, s.generalizable);
            }
            _ => {
                println!("  FAIL — wrong Reflection variant: {:?}", r);
                panic!("REFL CHECKPOINT 1 FAILED");
            }
        }
    } else {
        println!("  FAIL — expected 1 reflection, got {}", count);
        panic!("REFL CHECKPOINT 1 FAILED");
    }
}

// ─── Checkpoint 2 ─────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_2_failure_analyzed() {
    println!("\n=== REFL CHECKPOINT 2: reflection.failure_analyzed produces different variant ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let task_id = uuid::Uuid::now_v7();
    let analysis = FailureAnalyzer::analyze(task_id, "operation timeout after 30s");
    let _event = dispatch_failure(&kernel, &analysis);
    println!("  Dispatched reflection.failure_analyzed (timeout)");

    let count = reflection_count(&kernel);
    assert_eq!(count, 1, "FAIL: expected 1 reflection, got {}", count);

    let rs = kernel.query(|s| sps_reflection::reducer::ReflectionState::from_state(s).unwrap());
    let r = rs.reflections.values().next().unwrap();
    match r {
        sps_reflection::reducer::Reflection::Failure(f) => {
            if f.root_cause == RootCause::Timeout {
                println!("  PASS — 1 Reflection::Failure with root_cause=Timeout");
                println!("    suggested_fix='{}'", f.suggested_fix);
            } else {
                println!("  FAIL — wrong root_cause: {:?}", f.root_cause);
                panic!("REFL CHECKPOINT 2 FAILED");
            }
        }
        _ => {
            println!("  FAIL — wrong variant: {:?}", r);
            panic!("REFL CHECKPOINT 2 FAILED");
        }
    }
}

// ─── Checkpoint 3 ─────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_3_pattern_from_successes() {
    println!("\n=== REFL CHECKPOINT 3: 10 successes → pattern extraction ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Dispatch 10 success analyses.
    for i in 0..10 {
        let task_id = uuid::Uuid::now_v7();
        let analysis = SuccessAnalyzer::analyze(
            task_id,
            vec![format!("step {}", i)],
            format!("approach {} worked", i % 3),
            true,
        );
        dispatch_success(&kernel, &analysis);
    }
    println!("  Dispatched 10 success reflections");

    let success_count = kernel.query(|s| {
        let rs = sps_reflection::reducer::ReflectionState::from_state(s).unwrap();
        rs.reflections.values()
            .filter(|r| matches!(r, sps_reflection::reducer::Reflection::Success(_)))
            .count()
    });
    assert_eq!(success_count, 10, "FAIL: expected 10 successes, got {}", success_count);

    // Use PatternExtractor to derive a pattern from the analyses.
    let patterns = PatternExtractor::extract(&[(RootCause::Unknown, 10)]);
    assert_eq!(patterns.len(), 1);
    let p = &patterns[0];
    println!("  PatternExtractor produced: name='{}', count={}, confidence={:.2}",
        p.name, p.count, p.confidence);

    // Dispatch the pattern.
    dispatch_pattern(&kernel, p);
    println!("  Dispatched reflection.pattern_extracted");

    let pattern_count = kernel.query(|s| {
        let rs = sps_reflection::reducer::ReflectionState::from_state(s).unwrap();
        rs.reflections.values()
            .filter(|r| matches!(r, sps_reflection::reducer::Reflection::Pattern(_)))
            .count()
    });
    if pattern_count == 1 {
        println!("  PASS — 1 Pattern in state (after 10 successes + extraction)");
    } else {
        println!("  FAIL — expected 1 pattern, got {}", pattern_count);
        panic!("REFL CHECKPOINT 3 FAILED");
    }
}

// ─── Checkpoint 4 ─────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_4_pattern_from_failures() {
    println!("\n=== REFL CHECKPOINT 4: 10 failures → pattern extraction ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // 10 failures with different root causes.
    let failure_specs = vec![
        ("no provider available", RootCause::ProviderIssue),
        ("provider returned 500", RootCause::ProviderIssue),
        ("operation timeout after 30s", RootCause::Timeout),
        ("timeout waiting for lock", RootCause::Timeout),
        ("resource conflict on file", RootCause::ResourceConflict),
        ("resource conflict on db", RootCause::ResourceConflict),
        ("goal is ambiguous", RootCause::Ambiguity),
        ("plan was wrong", RootCause::PlanningError),
        ("effect executor failed", RootCause::EffectFailure),
        ("unknown error occurred", RootCause::Unknown),
    ];

    let mut root_cause_counts: Vec<(RootCause, u32)> = Vec::new();
    for (msg, expected_rc) in &failure_specs {
        let task_id = uuid::Uuid::now_v7();
        let analysis = FailureAnalyzer::analyze(task_id, msg);
        assert_eq!(analysis.root_cause, *expected_rc,
            "FAIL: FailureAnalyzer misclassified '{}' as {:?}, expected {:?}",
            msg, analysis.root_cause, expected_rc);
        if let Some(entry) = root_cause_counts.iter_mut().find(|(rc, _)| *rc == analysis.root_cause) {
            entry.1 += 1;
        } else {
            root_cause_counts.push((analysis.root_cause, 1));
        }
        dispatch_failure(&kernel, &analysis);
    }
    println!("  Dispatched 10 failure reflections with {} distinct root causes",
        root_cause_counts.len());

    // Extract patterns.
    let patterns = PatternExtractor::extract(&root_cause_counts);
    println!("  PatternExtractor produced {} patterns", patterns.len());

    // Dispatch all patterns.
    for p in &patterns {
        dispatch_pattern(&kernel, p);
    }

    let counts = kernel.query(|s| {
        let rs = sps_reflection::reducer::ReflectionState::from_state(s).unwrap();
        let failures = rs.reflections.values()
            .filter(|r| matches!(r, sps_reflection::reducer::Reflection::Failure(_)))
            .count();
        let patterns = rs.reflections.values()
            .filter(|r| matches!(r, sps_reflection::reducer::Reflection::Pattern(_)))
            .count();
        (failures, patterns)
    });

    if counts.0 == 10 && counts.1 == patterns.len() {
        println!("  PASS — {} failures + {} patterns in state", counts.0, counts.1);
    } else {
        println!("  FAIL — expected (10, {}), got {:?}", patterns.len(), counts);
        panic!("REFL CHECKPOINT 4 FAILED");
    }
}

// ─── Checkpoint 5 ─────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_5_mixed_history() {
    println!("\n=== REFL CHECKPOINT 5: mixed success + failure history, distinguishable ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // 5 successes + 5 failures + 2 patterns.
    for i in 0..5 {
        let a = SuccessAnalyzer::analyze(uuid::Uuid::now_v7(), vec![format!("s{}", i)], "worked".into(), i % 2 == 0);
        dispatch_success(&kernel, &a);
    }
    for i in 0..5 {
        let a = FailureAnalyzer::analyze(uuid::Uuid::now_v7(), &format!("failure {}", i));
        dispatch_failure(&kernel, &a);
    }
    for i in 0..2 {
        let p = Pattern {
            name: SmolStr::new(format!("pattern_{}", i)),
            description: format!("desc {}", i),
            count: 3,
            confidence: 0.5,
        };
        dispatch_pattern(&kernel, &p);
    }
    println!("  Dispatched 5 success + 5 failure + 2 pattern reflections");

    let counts = kernel.query(|s| {
        let rs = sps_reflection::reducer::ReflectionState::from_state(s).unwrap();
        let s = rs.reflections.values()
            .filter(|r| matches!(r, sps_reflection::reducer::Reflection::Success(_)))
            .count();
        let f = rs.reflections.values()
            .filter(|r| matches!(r, sps_reflection::reducer::Reflection::Failure(_)))
            .count();
        let p = rs.reflections.values()
            .filter(|r| matches!(r, sps_reflection::reducer::Reflection::Pattern(_)))
            .count();
        (s, f, p)
    });

    if counts == (5, 5, 2) {
        println!("  PASS — Success={}, Failure={}, Pattern={} (correctly distinguished)", counts.0, counts.1, counts.2);
    } else {
        println!("  FAIL — expected (5, 5, 2), got {:?}", counts);
        panic!("REFL CHECKPOINT 5 FAILED");
    }
}

// ─── Checkpoint 6 ─────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_6_replay() {
    println!("\n=== REFL CHECKPOINT 6: replay produces identical ReflectionState ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let s1 = SuccessAnalyzer::analyze(uuid::Uuid::now_v7(), vec!["a".into()], "why".into(), true);
    let f1 = FailureAnalyzer::analyze(uuid::Uuid::now_v7(), "timeout error");
    let p1 = Pattern { name: SmolStr::new("p1"), description: "d".into(), count: 5, confidence: 0.8 };
    dispatch_success(&kernel, &s1);
    dispatch_failure(&kernel, &f1);
    dispatch_pattern(&kernel, &p1);

    let live = kernel.query(|s| s.clone());
    let live_count = live.event_count();
    let live_hash = live.last_hash().clone();
    println!("  Live: {} events, hash={}", live_count, &live_hash.to_string()[..16]);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.event_count(), live_count, "FAIL: event_count mismatch");
    assert_eq!(replayed.last_hash(), live_hash, "FAIL: hash mismatch");

    let live_rs = sps_reflection::reducer::ReflectionState::from_state(&live).unwrap();
    let replayed_rs = sps_reflection::reducer::ReflectionState::from_state(&replayed).unwrap();

    // NOTE: Reflection ids are auto-generated by the reducer (Uuid::now_v7()),
    // so they will DIFFER between live and replayed. We verify count only.
    if live_rs.reflections.len() == replayed_rs.reflections.len() {
        println!("  PASS — same reflection count ({} == {})", live_rs.reflections.len(), replayed_rs.reflections.len());
        println!("  NOTE: reflection ids may differ (auto-generated in reducer)");
        println!("  NOTE: this is a known determinism issue — see Checkpoint 12");
    } else {
        println!("  FAIL — reflection count mismatch (live={}, replayed={})",
            live_rs.reflections.len(), replayed_rs.reflections.len());
        panic!("REFL CHECKPOINT 6 FAILED");
    }
}

// ─── Checkpoint 7 ─────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_7_sqlite() {
    println!("\n=== REFL CHECKPOINT 7: SQLite backend ===");
    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open_in_memory().unwrap()
    );
    let kernel = boot_kernel(storage.clone());
    assert_eq!(kernel.backend_name(), "sqlite");

    let a = SuccessAnalyzer::analyze(uuid::Uuid::now_v7(), vec!["x".into()], "y".into(), true);
    dispatch_success(&kernel, &a);
    println!("  Created reflection via SQLite backend");

    let count = reflection_count(&kernel);
    assert_eq!(count, 1, "FAIL: expected 1, got {}", count);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());
    assert_eq!(report.events_verified, 1);
    println!("  PASS — SQLite hash chain verified");

    drop(kernel);
    let kernel2 = boot_kernel(storage.clone());
    let count_after = reflection_count(&kernel2);
    if count_after == 1 {
        println!("  PASS — after restart, 1 reflection still present");
    } else {
        println!("  FAIL — after restart, got {}", count_after);
        panic!("REFL CHECKPOINT 7 FAILED");
    }
}

// ─── Checkpoint 8 ─────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_8_crash_recovery() {
    println!("\n=== REFL CHECKPOINT 8: crash recovery ===");
    let db_path = std::env::temp_dir().join(format!("sps_refl_crash_{}.db", uuid::Uuid::now_v7()));

    // Phase 1: create reflections, then crash.
    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel = boot_kernel(storage.clone());

        for i in 0..10 {
            let a = SuccessAnalyzer::analyze(
                uuid::Uuid::now_v7(),
                vec![format!("step {}", i)],
                format!("reason {}", i),
                i % 2 == 0,
            );
            dispatch_success(&kernel, &a);
        }
        let count_before = reflection_count(&kernel);
        println!("  Phase 1: created {} reflections", count_before);
        println!("  CRASH — dropping kernel");
    }

    // Phase 2: restart.
    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel2 = boot_kernel(storage.clone());

        let count_after = reflection_count(&kernel2);
        if count_after == 10 {
            println!("  Phase 2: PASS — reconstructed {} reflections", count_after);
        } else {
            println!("  FAIL — expected 10, got {}", count_after);
            panic!("REFL CHECKPOINT 8 FAILED");
        }
    }

    std::fs::remove_file(&db_path).ok();
}

// ─── Checkpoint 9 ─────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_9_large_corpus() {
    println!("\n=== REFL CHECKPOINT 9: large corpus (1000 reflections) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    const N: usize = 1000;
    let start = std::time::Instant::now();
    for i in 0..N {
        let a = if i % 2 == 0 {
            let s = SuccessAnalyzer::analyze(uuid::Uuid::now_v7(), vec![format!("s{}", i)], format!("r{}", i), true);
            let p = serde_json::to_value(&s).unwrap();
            kernel.dispatch(RawEvent::new("reflection.success_analyzed", p, Actor::owner(), 0)).unwrap();
        } else {
            let f = FailureAnalyzer::analyze(uuid::Uuid::now_v7(), &format!("error {}", i));
            let p = serde_json::to_value(&f).unwrap();
            kernel.dispatch(RawEvent::new("reflection.failure_analyzed", p, Actor::owner(), 0)).unwrap();
        };
        let _ = a;
    }
    let dispatch_ms = start.elapsed().as_millis();
    println!("  Dispatched {} reflections in {}ms ({:.0}/sec)",
        N, dispatch_ms, N as f64 / (dispatch_ms as f64 / 1000.0));

    let count = reflection_count(&kernel);
    if count == N {
        println!("  PASS — {} reflections in state", count);
    } else {
        println!("  FAIL — expected {}, got {}", N, count);
        panic!("REFL CHECKPOINT 9 FAILED");
    }

    // Verify hash chain.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken");
    assert_eq!(report.events_verified, N as u64);
    println!("  PASS — hash chain verified ({} events)", report.events_verified);

    // Replay.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replay_start = std::time::Instant::now();
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
    let replay_ms = replay_start.elapsed().as_millis();
    let replayed_count = sps_reflection::reducer::ReflectionState::from_state(&replayed)
        .map(|rs| rs.reflections.len()).unwrap_or(0);
    if replayed_count == N {
        println!("  PASS — replayed {} reflections in {}ms ({:.0}/sec)",
            N, replay_ms, N as f64 / (replay_ms as f64 / 1000.0));
    } else {
        println!("  FAIL — replayed {} (expected {})", replayed_count, N);
        panic!("REFL CHECKPOINT 9 FAILED");
    }
}

// ─── Checkpoint 10 ────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_10_cross_goal_analysis() {
    println!("\n=== REFL CHECKPOINT 10: cross-goal analysis (no isolation needed — reflections share graph) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Reflections for "Goal A" (task ids derived from goal A).
    let goal_a_task = uuid::Uuid::now_v7();
    let goal_b_task = uuid::Uuid::now_v7();

    // 3 successes for Goal A.
    for _ in 0..3 {
        let a = SuccessAnalyzer::analyze(goal_a_task, vec!["a".into()], "a reason".into(), true);
        dispatch_success(&kernel, &a);
    }
    // 2 failures for Goal B.
    for _ in 0..2 {
        let a = FailureAnalyzer::analyze(goal_b_task, "timeout");
        dispatch_failure(&kernel, &a);
    }
    println!("  Created 3 success (task=Goal A) + 2 failure (task=Goal B) reflections");

    // Reflections are stored in a single BTreeMap — they're NOT isolated by goal.
    // The `id` field on SuccessAnalysis/FailureAnalysis references the TASK, not the goal.
    // This is by design: reflections are global insights, not per-goal.
    let total = reflection_count(&kernel);
    assert_eq!(total, 5, "FAIL: expected 5 total reflections, got {}", total);

    // Verify we can filter by task id (simulating cross-goal analysis).
    let (a_count, b_count) = kernel.query(|s| {
        let rs = sps_reflection::reducer::ReflectionState::from_state(s).unwrap();
        let a_count = rs.reflections.values()
            .filter(|r| match r {
                sps_reflection::reducer::Reflection::Success(s) => s.id == goal_a_task,
                _ => false,
            })
            .count();
        let b_count = rs.reflections.values()
            .filter(|r| match r {
                sps_reflection::reducer::Reflection::Failure(f) => f.id == goal_b_task,
                _ => false,
            })
            .count();
        (a_count, b_count)
    });

    if a_count == 3 && b_count == 2 {
        println!("  PASS — Goal A task: {} reflections, Goal B task: {} reflections (correctly attributed)", a_count, b_count);
    } else {
        println!("  FAIL — A={}, B={} (expected 3, 2)", a_count, b_count);
        panic!("REFL CHECKPOINT 10 FAILED");
    }

    // Verify they coexist in the same state (no isolation needed).
    println!("  PASS — reflections from multiple goals coexist in single ReflectionState");
}

// ─── Checkpoint 11 ────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_11_malformed_payload_rejected() {
    println!("\n=== REFL CHECKPOINT 11: malformed payload rejected at dispatch (validate on write) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Dispatch a valid reflection first.
    let valid = SuccessAnalyzer::analyze(uuid::Uuid::now_v7(), vec!["ok".into()], "reason".into(), true);
    dispatch_success(&kernel, &valid);
    println!("  Step 1: dispatched 1 valid reflection");

    // Dispatch a MALFORMED reflection (missing required fields).
    // Per "validate on write" principle, this MUST fail at dispatch time,
    // NOT crash the kernel during replay.
    let malformed_payload = json!({
        "what_worked": ["incomplete"],
        // missing: id, why, generalizable
    });
    let result = kernel.dispatch(RawEvent::new(
        "reflection.success_analyzed",
        malformed_payload,
        Actor::owner(),
        0,
    ));

    if result.is_err() {
        println!("  Step 2: PASS — malformed payload rejected at dispatch time");
        let err = result.unwrap_err();
        println!("    error: {}", err);
    } else {
        println!("  FAIL — malformed payload was ACCEPTED (should have been rejected)");
        println!("  ─────────────────────────────────────────────────");
        println!("  EXPECTED: dispatch() returns Err for malformed payload");
        println!("  OBSERVED: dispatch() accepted the event");
        println!("  ROOT CAUSE: reducer doesn't validate payload shape, OR");
        println!("              dispatch() swallows reducer errors");
        println!("  SEVERITY: CRITICAL — corrupted event would be in the chain");
        panic!("REFL CHECKPOINT 11 FAILED");
    }

    // Verify the valid reflection is still there, and the malformed one is NOT.
    let count = reflection_count(&kernel);
    if count == 1 {
        println!("  Step 3: PASS — only 1 reflection in state (malformed one rejected)");
    } else {
        println!("  FAIL — expected 1 reflection, got {}", count);
        panic!("REFL CHECKPOINT 11 FAILED");
    }

    // Verify the hash chain does NOT include the rejected event.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());
    assert_eq!(report.events_verified, 1, "FAIL: expected 1 event, got {}", report.events_verified);
    println!("  Step 4: PASS — hash chain has {} event (malformed event not in chain)", report.events_verified);

    // Verify the kernel can still replay without crashing.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
    let replayed_count = sps_reflection::reducer::ReflectionState::from_state(&replayed)
        .map(|rs| rs.reflections.len()).unwrap_or(0);
    assert_eq!(replayed_count, 1, "FAIL: replayed count != 1");
    println!("  Step 5: PASS — replay produces 1 reflection (no crash, no corruption)");

    println!("  PASS — validate-on-write principle enforced");
}

// ─── Checkpoint 12 ────────────────────────────────────────────────────────

#[test]
fn refl_checkpoint_12_deterministic_replay_hash() {
    println!("\n=== REFL CHECKPOINT 12: deterministic replay hash (KernelMetaReducer invariant) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Dispatch a mix of reflection events.
    let s1 = SuccessAnalyzer::analyze(uuid::Uuid::now_v7(), vec!["a".into()], "r1".into(), true);
    let f1 = FailureAnalyzer::analyze(uuid::Uuid::now_v7(), "timeout");
    let p1 = Pattern { name: SmolStr::new("p"), description: "d".into(), count: 5, confidence: 0.9 };
    dispatch_success(&kernel, &s1);
    dispatch_failure(&kernel, &f1);
    dispatch_pattern(&kernel, &p1);

    let live = kernel.query(|s| s.clone());
    let live_event_count = live.event_count();
    let live_last_tick = live.last_tick();
    let live_last_hash = live.last_hash().clone();
    println!("  Live: {} events, last_tick={}, hash={}",
        live_event_count, live_last_tick, &live_last_hash.to_string()[..16]);

    // Verify KernelMetaReducer tracked all events (the Fix #3 from Goal validation).
    if live_event_count == 3 && live_last_tick == 3 {
        println!("  Step 1: PASS — kernel meta tracked 3 events, last_tick=3");
    } else {
        println!("  FAIL — kernel meta not tracking reflection events");
        println!("  EXPECTED: event_count=3, last_tick=3");
        println!("  OBSERVED: event_count={}, last_tick={}", live_event_count, live_last_tick);
        println!("  ROOT CAUSE: KernelMetaReducer not firing for reflection.* events");
        println!("  SEVERITY: CRITICAL — same bug class as Goal Checkpoint 12");
        panic!("REFL CHECKPOINT 12 FAILED");
    }

    // Hash chain.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());
    assert_eq!(report.events_verified, 3);
    println!("  Step 2: PASS — hash chain verified ({} events)", report.events_verified);

    // Replay.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.event_count(), live_event_count, "FAIL: event_count mismatch");
    assert_eq!(replayed.last_tick(), live_last_tick, "FAIL: last_tick mismatch");
    assert_eq!(replayed.last_hash(), live_last_hash, "FAIL: last_hash mismatch — NON-DETERMINISTIC REPLAY");
    println!("  Step 3: PASS — replayed event_count + last_tick + last_hash all match live");

    // Reflection count match.
    let live_rs = sps_reflection::reducer::ReflectionState::from_state(&live).unwrap();
    let replayed_rs = sps_reflection::reducer::ReflectionState::from_state(&replayed).unwrap();
    assert_eq!(live_rs.reflections.len(), replayed_rs.reflections.len(),
        "FAIL: reflection count mismatch");
    println!("  Step 4: PASS — reflection count matches ({} == {})",
        live_rs.reflections.len(), replayed_rs.reflections.len());

    // NOTE: The reducer auto-generates reflection ids via Uuid::now_v7(),
    // so the ids in live vs replayed will DIFFER. This is a known determinism
    // issue — the ReflectionState's BTreeMap is keyed by these auto-generated
    // ids, so the same events produce different internal state.
    //
    // For SPS's hash chain guarantee, what matters is:
    //   - event_count: matches (KernelMetaReducer tracks this)
    //   - last_hash: matches (event payloads are deterministic)
    //   - reflection COUNT: matches
    //
    // The internal ids differing doesn't break the hash chain, but it WOULD
    // break any code that tries to reference reflections by id after replay.
    // This is documented as a known issue for future hardening.

    println!("  PASS — deterministic replay hash confirmed");
    println!("  NOTE: reflection internal ids are auto-generated (Uuid::now_v7) —");
    println!("        count matches but ids may differ. Acceptable for hash chain;");
    println!("        would break id-based lookups after replay. Documented.");
}

// ─── Checkpoint 13 ────────────────────────────────────────────────────────
// Reflection IDs Replay Test — verify that reflection IDs are deterministic
// across replay. If the reducer auto-generates IDs via Uuid::now_v7(), the
// replayed state will have DIFFERENT IDs than the live state, breaking any
// code that references reflections by ID after a restart.

#[test]
fn refl_checkpoint_13_reflection_ids_replay() {
    println!("\n=== REFL CHECKPOINT 13: Reflection IDs deterministic across replay ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Create 3 reflections and capture their IDs.
    let s1 = SuccessAnalyzer::analyze(uuid::Uuid::now_v7(), vec!["a".into()], "r1".into(), true);
    let f1 = FailureAnalyzer::analyze(uuid::Uuid::now_v7(), "timeout");
    let p1 = Pattern { name: SmolStr::new("p"), description: "d".into(), count: 5, confidence: 0.9 };
    dispatch_success(&kernel, &s1);
    dispatch_failure(&kernel, &f1);
    dispatch_pattern(&kernel, &p1);
    println!("  Step 1: dispatched 3 reflections (1 success, 1 failure, 1 pattern)");

    // Capture the LIVE reflection IDs.
    let live_ids: std::collections::BTreeSet<uuid::Uuid> = kernel.query(|s| {
        sps_reflection::reducer::ReflectionState::from_state(s)
            .map(|rs| rs.reflections.keys().copied().collect())
            .unwrap_or_default()
    });
    println!("  Step 2: captured {} live reflection IDs", live_ids.len());
    for id in &live_ids {
        println!("    {}", id);
    }

    // Replay from genesis.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    // Capture the REPLAYED reflection IDs.
    let replayed_ids: std::collections::BTreeSet<uuid::Uuid> =
        sps_reflection::reducer::ReflectionState::from_state(&replayed)
            .map(|rs| rs.reflections.keys().copied().collect())
            .unwrap_or_default();
    println!("  Step 3: captured {} replayed reflection IDs", replayed_ids.len());

    // Compare.
    if live_ids == replayed_ids {
        println!("  PASS — reflection IDs are deterministic across replay");
        println!("  Live and replayed have identical ID sets");
    } else {
        println!("  FAIL — reflection IDs DIFFER between live and replayed");
        println!("  ─────────────────────────────────────────────────");
        println!("  EXPECTED: live IDs == replayed IDs (deterministic)");
        println!("  OBSERVED:");
        println!("    Live IDs:     {:?}", live_ids.iter().map(|i| i.to_string()).collect::<Vec<_>>());
        println!("    Replayed IDs: {:?}", replayed_ids.iter().map(|i| i.to_string()).collect::<Vec<_>>());
        println!("  ROOT CAUSE: ReflectionReducer::reduce() uses `let id = Uuid::now_v7();`");
        println!("    to generate reflection IDs. This is NON-DETERMINISTIC — each call");
        println!("    produces a different UUID, so replayed state has different IDs.");
        println!("  IMPACT: Any code that references reflections by ID after a restart");
        println!("    will fail. Cross-references between reflections and other entities");
        println!("    (e.g., 'this pattern was derived from reflection X') will break.");
        println!("  SEVERITY: HIGH — violates Event Sourcing determinism contract");
        println!("  ─────────────────────────────────────────────────");
        panic!("REFL CHECKPOINT 13 FAILED — reflection IDs are non-deterministic");
    }
}

// ─── Checkpoint 14 ────────────────────────────────────────────────────────
// Reducer Double-Execution Test — verify that dispatch()'s validate-then-commit
// pattern doesn't cause the reducer to mutate state twice per event.

#[test]
fn refl_checkpoint_14_no_double_execution() {
    println!("\n=== REFL CHECKPOINT 14: reducer runs exactly once per dispatch ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    use sps_memory::memory::{MemoryId, MemoryKind, MemoryRecord};
    let record = MemoryRecord {
        id: MemoryId(uuid::Uuid::now_v7()),
        kind: MemoryKind::Semantic,
        title: smol_str::SmolStr::new("Test"),
        content: json!({}),
        tags: vec![],
        origin_tick: 0,
        created_at: 0,
    };
    let mem_id = record.id;
    let payload = serde_json::to_value(&record).unwrap();
    kernel.dispatch(RawEvent::new("memory.created", payload, Actor::owner(), 0)).unwrap();
    println!("  Step 1: created memory");

    let initial_count = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .and_then(|ms| ms.graph.get(&mem_id).map(|m| m.access_count))
            .unwrap_or(0)
    });
    assert_eq!(initial_count, 0);

    // Dispatch memory.accessed ONCE.
    kernel.dispatch(RawEvent::new(
        "memory.accessed",
        json!({"id": mem_id.0.to_string(), "at": 100}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Step 3: dispatched memory.accessed ONCE");

    // If double-execution occurred, access_count would be 2.
    let after_count = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .and_then(|ms| ms.graph.get(&mem_id).map(|m| m.access_count))
            .unwrap_or(0)
    });
    if after_count == 1 {
        println!("  Step 4: PASS — access_count = 1 (reducer ran exactly once)");
    } else {
        println!("  FAIL — access_count = {} (expected 1)", after_count);
        println!("  ROOT CAUSE: validate-on-write is double-applying the reducer");
        println!("  SEVERITY: CRITICAL");
        panic!("REFL CHECKPOINT 14 FAILED — reducer double-execution detected");
    }

    // Verify event_count is 2 (created + accessed), not 3.
    let event_count = kernel.query(|s| s.event_count());
    if event_count == 2 {
        println!("  Step 5: PASS — event_count = 2 (no trial event leaked)");
    } else {
        println!("  FAIL — event_count = {} (expected 2)", event_count);
        panic!("REFL CHECKPOINT 14 FAILED — trial event leaked into store");
    }
}
