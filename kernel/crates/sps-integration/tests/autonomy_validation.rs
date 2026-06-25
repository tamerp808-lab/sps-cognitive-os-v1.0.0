//! Autonomy Validation Suite — 8/8 PASS required.

use std::sync::Arc;

use serde_json::json;
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

fn dispatch_activation(kernel: &SpsKernel, goal_id: uuid::Uuid, milestones: serde_json::Value) {
    kernel.dispatch(RawEvent::new(
        "autonomous.goal_activated",
        json!({"goal_id": goal_id.to_string(), "milestones": milestones, "activated_at": 0}),
        Actor::owner(), 0,
    )).unwrap();
}

fn dispatch_review(kernel: &SpsKernel, goal_id: uuid::Uuid, review: &str) {
    kernel.dispatch(RawEvent::new(
        "autonomous.weekly_review",
        json!({"goal_id": goal_id.to_string(), "review": review, "reviewed_at": 0}),
        Actor::owner(), 0,
    )).unwrap();
}

fn counts(kernel: &SpsKernel) -> (usize, usize) {
    kernel.query(|s| {
        sps_autonomy::reducer::AutonomyState::from_state(s)
            .map(|as_| (as_.active_goals.len(), as_.reviews.len()))
            .unwrap_or((0, 0))
    })
}

// ─── Checkpoint 1 ─────────────────────────────────────────────────────────

#[test]
fn autonomy_checkpoint_1_goal_activated() {
    println!("\n=== AUTONOMY CHECKPOINT 1: autonomous.goal_activated materializes ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let gid = uuid::Uuid::now_v7();
    dispatch_activation(&kernel, gid, json!([{"title": "M1"}]));
    println!("  Dispatched autonomous.goal_activated");

    let (active, _) = counts(&kernel);
    if active == 1 {
        println!("  PASS — 1 active goal in AutonomyState");
    } else {
        println!("  FAIL — expected 1, got {}", active);
        panic!("AUTONOMY CHECKPOINT 1 FAILED");
    }
}

// ─── Checkpoint 2 ─────────────────────────────────────────────────────────

#[test]
fn autonomy_checkpoint_2_weekly_review() {
    println!("\n=== AUTONOMY CHECKPOINT 2: autonomous.weekly_review materializes ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let gid = uuid::Uuid::now_v7();
    dispatch_review(&kernel, gid, "On track. 3 tasks done.");
    println!("  Dispatched autonomous.weekly_review");

    let (_, reviews) = counts(&kernel);
    if reviews == 1 {
        println!("  PASS — 1 review in AutonomyState");
    } else {
        println!("  FAIL — expected 1, got {}", reviews);
        panic!("AUTONOMY CHECKPOINT 2 FAILED");
    }
}

// ─── Checkpoint 3 ─────────────────────────────────────────────────────────

#[test]
fn autonomy_checkpoint_3_review_attribution_by_goal() {
    println!("\n=== AUTONOMY CHECKPOINT 3: review attribution by goal_id ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let g1 = uuid::Uuid::now_v7();
    let g2 = uuid::Uuid::now_v7();
    dispatch_review(&kernel, g1, "review 1 for G1");
    dispatch_review(&kernel, g1, "review 2 for G1");
    dispatch_review(&kernel, g2, "review 1 for G2");
    println!("  Dispatched 3 reviews (2 for G1, 1 for G2)");

    let (g1_count, g2_count) = kernel.query(|s| {
        let as_ = sps_autonomy::reducer::AutonomyState::from_state(s).unwrap();
        (as_.reviews_for_goal(g1).len(), as_.reviews_for_goal(g2).len())
    });
    if g1_count == 2 && g2_count == 1 {
        println!("  PASS — G1: {} reviews, G2: {} reviews (correctly attributed)", g1_count, g2_count);
    } else {
        println!("  FAIL — G1={}, G2={} (expected 2, 1)", g1_count, g2_count);
        panic!("AUTONOMY CHECKPOINT 3 FAILED");
    }
}

// ─── Checkpoint 4 ─────────────────────────────────────────────────────────

#[test]
fn autonomy_checkpoint_4_multi_goal_isolation() {
    println!("\n=== AUTONOMY CHECKPOINT 4: multi-goal activation isolation ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let g1 = uuid::Uuid::now_v7();
    let g2 = uuid::Uuid::now_v7();
    let g3 = uuid::Uuid::now_v7();
    dispatch_activation(&kernel, g1, json!([{"title": "M1"}]));
    dispatch_activation(&kernel, g2, json!([{"title": "M2"}]));
    dispatch_activation(&kernel, g3, json!([{"title": "M3"}]));
    println!("  Activated 3 goals");

    let (active, _) = counts(&kernel);
    if active == 3 {
        println!("  PASS — 3 active goals in AutonomyState (no cross-contamination)");
    } else {
        println!("  FAIL — expected 3, got {}", active);
        panic!("AUTONOMY CHECKPOINT 4 FAILED");
    }

    // Verify each goal has its own milestones.
    let milestones_match = kernel.query(|s| {
        let as_ = sps_autonomy::reducer::AutonomyState::from_state(s).unwrap();
        let m1 = as_.active_goals.get(&g1).and_then(|a| a.milestones.get(0)).and_then(|v| v.get("title")).and_then(|v| v.as_str());
        let m2 = as_.active_goals.get(&g2).and_then(|a| a.milestones.get(0)).and_then(|v| v.get("title")).and_then(|v| v.as_str());
        let m3 = as_.active_goals.get(&g3).and_then(|a| a.milestones.get(0)).and_then(|v| v.get("title")).and_then(|v| v.as_str());
        m1 == Some("M1") && m2 == Some("M2") && m3 == Some("M3")
    });
    if milestones_match {
        println!("  PASS — each goal has its own milestones (M1, M2, M3)");
    } else {
        println!("  FAIL — milestone attribution wrong");
        panic!("AUTONOMY CHECKPOINT 4 FAILED");
    }
}

// ─── Checkpoint 5 ─────────────────────────────────────────────────────────

#[test]
fn autonomy_checkpoint_5_replay() {
    println!("\n=== AUTONOMY CHECKPOINT 5: replay produces identical AutonomyState ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let g1 = uuid::Uuid::now_v7();
    dispatch_activation(&kernel, g1, json!([{"title": "M1"}]));
    dispatch_review(&kernel, g1, "review 1");
    dispatch_review(&kernel, g1, "review 2");

    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();
    let (live_active, live_reviews) = counts(&kernel);

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_autonomy::reducer::AutonomyReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.last_hash(), live_hash, "FAIL: hash mismatch");
    let replayed_as = sps_autonomy::reducer::AutonomyState::from_state(&replayed).unwrap();
    if replayed_as.active_goals.len() == live_active && replayed_as.reviews.len() == live_reviews {
        println!("  PASS — active_goals ({}) + reviews ({}) match after replay",
            replayed_as.active_goals.len(), replayed_as.reviews.len());
    } else {
        println!("  FAIL — live: ({}, {}), replayed: ({}, {})",
            live_active, live_reviews,
            replayed_as.active_goals.len(), replayed_as.reviews.len());
        panic!("AUTONOMY CHECKPOINT 5 FAILED");
    }
}

// ─── Checkpoint 6 ─────────────────────────────────────────────────────────

#[test]
fn autonomy_checkpoint_6_sqlite() {
    println!("\n=== AUTONOMY CHECKPOINT 6: SQLite backend ===");
    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open_in_memory().unwrap()
    );
    let kernel = boot_kernel(storage.clone());
    assert_eq!(kernel.backend_name(), "sqlite");

    let gid = uuid::Uuid::now_v7();
    dispatch_activation(&kernel, gid, json!([]));
    dispatch_review(&kernel, gid, "sqlite review");
    println!("  Created activation + review via SQLite");

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());

    drop(kernel);
    let kernel2 = boot_kernel(storage.clone());
    let (active, reviews) = counts(&kernel2);
    if active == 1 && reviews == 1 {
        println!("  PASS — after restart: 1 active goal + 1 review");
    } else {
        println!("  FAIL — after restart: active={}, reviews={}", active, reviews);
        panic!("AUTONOMY CHECKPOINT 6 FAILED");
    }
}

// ─── Checkpoint 7 ─────────────────────────────────────────────────────────

#[test]
fn autonomy_checkpoint_7_crash_recovery() {
    println!("\n=== AUTONOMY CHECKPOINT 7: crash recovery ===");
    let db_path = std::env::temp_dir().join(format!("sps_auto_crash_{}.db", uuid::Uuid::now_v7()));

    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel = boot_kernel(storage.clone());
        for i in 0..5 {
            let gid = uuid::Uuid::now_v7();
            dispatch_activation(&kernel, gid, json!([{"title": format!("M{}", i)}]));
            dispatch_review(&kernel, gid, &format!("review {}", i));
        }
        let (active, reviews) = counts(&kernel);
        println!("  Phase 1: {} active goals + {} reviews", active, reviews);
        println!("  CRASH");
    }

    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel2 = boot_kernel(storage.clone());
        let (active, reviews) = counts(&kernel2);
        if active == 5 && reviews == 5 {
            println!("  Phase 2: PASS — reconstructed {} active + {} reviews", active, reviews);
        } else {
            println!("  FAIL — expected (5, 5), got ({}, {})", active, reviews);
            panic!("AUTONOMY CHECKPOINT 7 FAILED");
        }
    }
    std::fs::remove_file(&db_path).ok();
}

// ─── Checkpoint 8 ─────────────────────────────────────────────────────────

#[test]
fn autonomy_checkpoint_8_deterministic_state() {
    println!("\n=== AUTONOMY CHECKPOINT 8: deterministic state across replay ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let g1 = uuid::Uuid::now_v7();
    dispatch_activation(&kernel, g1, json!([{"title": "M1"}, {"title": "M2"}]));
    dispatch_review(&kernel, g1, "review text");

    // Capture live goal_id + review text.
    let (live_goal_id, live_review_text, live_milestone_count) = kernel.query(|s| {
        let as_ = sps_autonomy::reducer::AutonomyState::from_state(s).unwrap();
        let activation = as_.active_goals.get(&g1).unwrap();
        let review = as_.reviews.first().unwrap();
        (activation.goal_id, review.review.clone(), activation.milestones.as_array().map(|a| a.len()).unwrap_or(0))
    });

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_autonomy::reducer::AutonomyReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    let replayed_as = sps_autonomy::reducer::AutonomyState::from_state(&replayed).unwrap();
    let replayed_activation = replayed_as.active_goals.get(&g1).unwrap();
    let replayed_review = replayed_as.reviews.first().unwrap();
    let replayed_milestone_count = replayed_activation.milestones.as_array().map(|a| a.len()).unwrap_or(0);

    if replayed_activation.goal_id == live_goal_id
        && replayed_review.review == live_review_text
        && replayed_milestone_count == live_milestone_count {
        println!("  PASS — goal_id, review text, milestone count all match after replay");
        println!("    goal_id: {}", &live_goal_id.to_string()[..8]);
        println!("    review:  '{}'", live_review_text);
        println!("    milestones: {}", live_milestone_count);
    } else {
        println!("  FAIL — mismatch after replay");
        panic!("AUTONOMY CHECKPOINT 8 FAILED");
    }
}
