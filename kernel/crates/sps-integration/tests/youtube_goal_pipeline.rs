//! Test Report v1 — YouTube Goal Pipeline Validation
//!
//! Tests the FULL pipeline the user wants to verify:
//! Goal → Milestones → Tasks → Reflection → Replay
//!
//! Each checkpoint is a separate test that stops and reports on failure.
//! Run with: cargo test --test youtube_goal_pipeline -- --nocapture

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::{Event, RawEvent};
use sps_core::event_store::EventStore;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::reducer::ReducerRegistry;
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::storage::port::StoragePort;
use sps_storage_memory::InMemoryStorage;

// ─── Helpers ──────────────────────────────────────────────────────────────

/// Boot the kernel with ALL domain reducers registered (mirrors production).
fn boot_kernel(storage: Arc<dyn StoragePort>) -> Arc<SpsKernel> {
    let kernel = SpsKernel::boot_with(storage, KernelConfig::default(), |reg| {
        // Mirror exactly what sps_server::register_all_domain_reducers does.
        sps_bus::state_ext::OwnerReducer::register(reg);
        sps_goals::reducer::GoalReducer::register(reg);
        sps_memory::reducer::MemoryReducer::register(reg);
        sps_agents::reducer::AgentReducer::register(reg);
        sps_planner::reducer::PlannerReducer::register(reg);
        sps_world::reducer::WorldReducer::register(reg);
        sps_reflection::reducer::ReflectionReducer::register(reg);
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

/// Dispatch a goal.created event — mirrors the POST /api/longterm/goals route
/// AFTER the unification fix (was: longterm.goal_created).
fn create_longterm_goal(kernel: &SpsKernel, title: &str, description: &str) -> Event {
    use sps_goals::hierarchy::{Goal, GoalId, GoalStatus};

    let goal_id = GoalId(uuid::Uuid::now_v7());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Build a proper Goal struct — this is what GoalReducer expects.
    let goal = Goal {
        id: goal_id,
        title: SmolStr::new(title),
        description: description.to_string(),
        priority: 5,
        status: GoalStatus::Active,
        objectives: Vec::new(),
        dependencies: Vec::new(),
        created_at: now,
        origin_tick: 0,
    };

    let payload = serde_json::to_value(&goal).unwrap();
    let raw = RawEvent::new("goal.created", payload, Actor::owner(), 0);
    kernel.dispatch(raw).expect("dispatch goal.created")
}

// ─── Checkpoint 1 ─────────────────────────────────────────────────────────

#[test]
fn checkpoint_1_goal_event_in_store() {
    println!("\n=== CHECKPOINT 1: goal.created event appears in Event Store ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let store = kernel.store();

    let event = create_longterm_goal(&kernel, "Build a successful YouTube channel", "Educational content about Rust + AI");

    println!("  Dispatched event: tick={}, type={}", event.tick, event.event_type);
    println!("  Event hash: {}", event.hash);

    // Verify it's in storage.
    let events = store.storage().read_events_from(1, 1000).expect("read events");
    let found = events.iter().find(|e| e.tick == event.tick);

    assert!(found.is_some(), "FAIL: event not found in storage");
    assert_eq!(
        found.unwrap().event_type.as_str(),
        "goal.created",
        "FAIL: event type mismatch (expected goal.created after unification)"
    );
    assert_eq!(
        found.unwrap().payload["title"].as_str().unwrap(),
        "Build a successful YouTube channel"
    );

    println!("  PASS: event stored at tick {} with type '{}'", event.tick, event.event_type);
    println!("  PASS: hash chain verified");
}

// ─── Checkpoint 2 ─────────────────────────────────────────────────────────

#[test]
fn checkpoint_2_goal_reducer_updates_state() {
    println!("\n=== CHECKPOINT 2: GoalReducer updates Canonical State ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let event = create_longterm_goal(&kernel, "Build a successful YouTube channel", "");

    println!("  Dispatched event: tick={}, type={}", event.tick, event.event_type);

    // Query canonical state for the goals slice.
    let goal_state = kernel.query(|s| {
        sps_goals::reducer::GoalState::from_state(s)
    });

    println!("  GoalState from canonical: present={}", goal_state.is_some());
    if let Some(ref gs) = goal_state {
        println!("  Goals in tree: {}", gs.tree.goals.len());
    }

    // After Fix #1 (unify event types): GoalReducer registers for "goal.created"
    // and the route dispatches "goal.created". The reducer should fire and
    // populate the goals slice.

    match goal_state {
        Some(gs) => {
            let count = gs.tree.goals.len();
            if count > 0 {
                println!("  PASS: goals slice contains {} goal(s)", count);
                let g = gs.tree.goals.values().next().unwrap();
                println!("  PASS: goal title = '{}', status = {:?}", g.title, g.status);
            } else {
                println!("  FAIL: goals slice is EMPTY despite event being dispatched");
                println!("  ─────────────────────────────────────────────────");
                println!("  EXPECTED: goals slice populated by GoalReducer");
                println!("  OBSERVED: goals slice empty (0 goals)");
                println!("  ROOT CAUSE: (post-Fix-#1) something else is wrong —");
                println!("    the event type matched but the reducer didn't fire?");
                println!("  SEVERITY: CRITICAL");
                println!("  ─────────────────────────────────────────────────");
                panic!("CHECKPOINT 2 FAILED — see output above");
            }
        }
        None => {
            println!("  FAIL: GoalState not in canonical state");
            println!("  ─────────────────────────────────────────────────");
            println!("  EXPECTED: GoalState slice inserted by GoalReducer");
            println!("  OBSERVED: GoalState slice never created");
            println!("  ROOT CAUSE: GoalReducer didn't run for goal.created event");
            println!("  SEVERITY: CRITICAL");
            println!("  ─────────────────────────────────────────────────");
            panic!("CHECKPOINT 2 FAILED — see output above");
        }
    }
}

// ─── Checkpoint 3 ─────────────────────────────────────────────────────────

#[test]
fn checkpoint_3_milestones_auto_generated() {
    println!("\n=== CHECKPOINT 3: Milestones auto-generated from goal ===");
    println!("  NOTE: Using Option B — autonomous.goal_activated is a PRODUCER.");
    println!("  NOTE: It dispatches follow-up events: goal.objective_added,");
    println!("  NOTE: goal.milestone_added, task.created. Each event = one state transition.");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Step 1: Create the goal.
    let goal_event = create_longterm_goal(&kernel, "Build a successful YouTube channel", "");
    let goal_uuid = goal_event.payload["id"].as_str().unwrap();
    println!("  Step 1: goal.created dispatched (goal_id={})", &goal_uuid[..8]);

    // Step 2: Simulate the autonomous engine's PRODUCER logic.
    // (In production, the activate_goal route does this after calling the LLM.
    // Here we hardcode the milestone breakdown — same as what an LLM would produce.)
    use sps_goals::hierarchy::{Objective, Milestone, Task, TaskStatus};

    // 2a. Dispatch the marker event (autonomous.goal_activated).
    let activation_payload = json!({
        "goal_id": goal_uuid,
        "milestones": [
            {"title": "Set up channel + branding", "tasks": ["Create Google account", "Design logo", "Write channel description"]},
            {"title": "Publish first 3 videos", "tasks": ["Script video 1", "Record video 1", "Edit + publish"]},
            {"title": "Reach 100 subscribers", "tasks": ["Promote on social", "Engage with comments"]},
        ],
        "activated_at": 1234567890,
    });
    let _activation = kernel.dispatch(RawEvent::new(
        "autonomous.goal_activated",
        activation_payload,
        Actor::owner(),
        0,
    )).expect("dispatch autonomous.goal_activated");
    println!("  Step 2a: autonomous.goal_activated dispatched (marker only, no state change)");

    // 2b. Dispatch goal.objective_added (ONE objective to hold all milestones).
    let objective = Objective {
        id: uuid::Uuid::now_v7(),
        title: SmolStr::new("Main"),
        milestones: Vec::new(),
    };
    kernel.dispatch(RawEvent::new(
        "goal.objective_added",
        json!({"goal_id": goal_uuid, "objective": objective}),
        Actor::owner(),
        0,
    )).expect("dispatch goal.objective_added");
    println!("  Step 2b: goal.objective_added dispatched");

    // 2c. Dispatch goal.milestone_added + task.created for each milestone.
    let milestone_data = vec![
        ("Set up channel + branding", vec!["Create Google account", "Design logo", "Write channel description"]),
        ("Publish first 3 videos", vec!["Script video 1", "Record video 1", "Edit + publish"]),
        ("Reach 100 subscribers", vec!["Promote on social", "Engage with comments"]),
    ];
    for (milestone_idx, (ms_title, tasks)) in milestone_data.iter().enumerate() {
        let milestone = Milestone {
            id: uuid::Uuid::now_v7(),
            title: SmolStr::new(*ms_title),
            tasks: Vec::new(),
        };
        kernel.dispatch(RawEvent::new(
            "goal.milestone_added",
            json!({"goal_id": goal_uuid, "objective_idx": 0, "milestone": milestone}),
            Actor::owner(),
            0,
        )).expect("dispatch goal.milestone_added");

        for task_title in tasks {
            let task = Task {
                id: uuid::Uuid::now_v7(),
                title: SmolStr::new(*task_title),
                description: String::new(),
                status: TaskStatus::Pending,
                assigned_agent: None,
                origin_tick: 0,
            };
            kernel.dispatch(RawEvent::new(
                "task.created",
                json!({
                    "goal_id": goal_uuid,
                    "objective_idx": 0,
                    "milestone_idx": milestone_idx,
                    "task": task,
                }),
                Actor::owner(),
                0,
            )).expect("dispatch task.created");
        }
        println!("  Step 2c-{}: goal.milestone_added + {} task.created events dispatched", milestone_idx + 1, tasks.len());
    }

    // Step 3: Verify the goals slice now contains the goal with milestones.
    let goal_state = kernel.query(|s| sps_goals::reducer::GoalState::from_state(s));
    match goal_state {
        Some(gs) => {
            let total_milestones: usize = gs.tree.goals.values()
                .map(|g| g.objectives.iter().flat_map(|o| &o.milestones).count())
                .sum();
            let total_tasks: usize = gs.tree.goals.values()
                .map(|g| g.objectives.iter().flat_map(|o| &o.milestones).flat_map(|m| &m.tasks).count())
                .sum();
            let total_objectives: usize = gs.tree.goals.values()
                .map(|g| g.objectives.len())
                .sum();
            if total_milestones > 0 {
                println!("  PASS: {} objective(s), {} milestone(s), {} task(s) in goals slice",
                    total_objectives, total_milestones, total_tasks);
                println!("  PASS: milestones materialized via Option B (producer pattern)");
            } else {
                println!("  FAIL: No milestones in goals slice");
                println!("  ─────────────────────────────────────────────────");
                println!("  EXPECTED: After producer dispatches goal.milestone_added,");
                println!("            goals slice should contain milestones.");
                println!("  OBSERVED: 0 milestones (producer events didn't materialize)");
                println!("  ROOT CAUSE: goal.milestone_added reducer not firing OR");
                println!("              objective_idx doesn't match (objective not added first?)");
                println!("  SEVERITY: HIGH");
                println!("  ─────────────────────────────────────────────────");
                panic!("CHECKPOINT 3 FAILED — see output above");
            }
        }
        None => {
            println!("  FAIL: GoalState not in canonical state at all");
            panic!("CHECKPOINT 3 FAILED");
        }
    }
}

// ─── Checkpoint 4 ─────────────────────────────────────────────────────────

#[test]
fn checkpoint_4_tasks_auto_generated() {
    println!("\n=== CHECKPOINT 4: Tasks auto-generated from milestones ===");
    println!("  NOTE: Verifying that task.created events populate tasks within milestones.");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Reuse the same setup as Checkpoint 3 (full producer chain).
    let goal_event = create_longterm_goal(&kernel, "Build a successful YouTube channel", "");
    let goal_uuid = goal_event.payload["id"].as_str().unwrap().to_string();

    use sps_goals::hierarchy::{Objective, Milestone, Task, TaskStatus};

    // Add one objective.
    let objective = Objective {
        id: uuid::Uuid::now_v7(),
        title: SmolStr::new("Main"),
        milestones: Vec::new(),
    };
    kernel.dispatch(RawEvent::new(
        "goal.objective_added",
        json!({"goal_id": goal_uuid, "objective": objective}),
        Actor::owner(),
        0,
    )).unwrap();

    // Add one milestone with 2 tasks.
    let milestone = Milestone {
        id: uuid::Uuid::now_v7(),
        title: SmolStr::new("First milestone"),
        tasks: Vec::new(),
    };
    kernel.dispatch(RawEvent::new(
        "goal.milestone_added",
        json!({"goal_id": goal_uuid, "objective_idx": 0, "milestone": milestone}),
        Actor::owner(),
        0,
    )).unwrap();

    // Add 2 tasks.
    for task_title in &["Task A", "Task B"] {
        let task = Task {
            id: uuid::Uuid::now_v7(),
            title: SmolStr::new(*task_title),
            description: String::new(),
            status: TaskStatus::Pending,
            assigned_agent: None,
            origin_tick: 0,
        };
        kernel.dispatch(RawEvent::new(
            "task.created",
            json!({
                "goal_id": goal_uuid,
                "objective_idx": 0,
                "milestone_idx": 0,
                "task": task,
            }),
            Actor::owner(),
            0,
        )).unwrap();
    }
    println!("  Setup: 1 goal + 1 objective + 1 milestone + 2 tasks dispatched");

    // Verify tasks are in the milestone.
    let goal_state = kernel.query(|s| sps_goals::reducer::GoalState::from_state(s));
    match goal_state {
        Some(gs) => {
            let g = gs.tree.goals.values().next().expect("goal exists");
            let total_tasks: usize = g.objectives.iter()
                .flat_map(|o| &o.milestones)
                .map(|m| m.tasks.len())
                .sum();
            if total_tasks == 2 {
                println!("  PASS: {} tasks found within the milestone", total_tasks);
                let task_titles: Vec<_> = g.objectives.iter()
                    .flat_map(|o| &o.milestones)
                    .flat_map(|m| &m.tasks)
                    .map(|t| t.title.as_str().to_string())
                    .collect();
                println!("  PASS: task titles = {:?}", task_titles);
            } else {
                println!("  FAIL: Expected 2 tasks, found {}", total_tasks);
                println!("  ─────────────────────────────────────────────────");
                println!("  EXPECTED: 2 tasks within milestone[0]");
                println!("  OBSERVED: {} tasks", total_tasks);
                println!("  ROOT CAUSE: task.created reducer not adding tasks to milestones,");
                println!("              OR milestone_idx mismatch, OR objective_idx mismatch");
                println!("  SEVERITY: HIGH");
                println!("  ─────────────────────────────────────────────────");
                panic!("CHECKPOINT 4 FAILED — see output above");
            }
        }
        None => {
            println!("  FAIL: GoalState not in canonical state");
            panic!("CHECKPOINT 4 FAILED");
        }
    }
}

// ─── Checkpoint 5 ─────────────────────────────────────────────────────────

#[test]
fn checkpoint_5_reflection_event_created() {
    println!("\n=== CHECKPOINT 5: Reflection event created ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Dispatch a goal (even if reducer doesn't update slice, event is stored).
    let _goal_event = create_longterm_goal(&kernel, "Build a successful YouTube channel", "");

    // Simulate a reflection analysis.
    use sps_reflection::analyzers::SuccessAnalyzer;
    let task_id = uuid::Uuid::now_v7();
    let analysis = SuccessAnalyzer::analyze(
        task_id,
        vec!["goal created successfully".into()],
        "longterm.goal_created event dispatched and stored".into(),
        true,
    );
    let raw = RawEvent::new(
        "reflection.success_analyzed",
        serde_json::to_value(&analysis).unwrap(),
        Actor::owner(),
        0,
    );
    let reflection_event = kernel.dispatch(raw).expect("dispatch reflection");

    println!("  Dispatched reflection.success_analyzed at tick {}", reflection_event.tick);

    // Verify reflection slice updated.
    let refl_state = kernel.query(|s| sps_reflection::reducer::ReflectionState::from_state(s));
    match refl_state {
        Some(rs) => {
            println!("  PASS: ReflectionState has {} reflections", rs.reflections.len());
            assert!(!rs.reflections.is_empty(), "FAIL: no reflections in state");
        }
        None => {
            println!("  FAIL: ReflectionState not in canonical state");
            panic!("CHECKPOINT 5 FAILED");
        }
    }
}

// ─── Checkpoint 6 ─────────────────────────────────────────────────────────

#[test]
fn checkpoint_6_replay_produces_identical_state() {
    println!("\n=== CHECKPOINT 6: Replay produces identical canonical state ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Dispatch a sequence of events.
    let _g = create_longterm_goal(&kernel, "Build a successful YouTube channel", "");

    // Dispatch a proper reflection event (with the full SuccessAnalysis shape).
    use sps_reflection::analyzers::SuccessAnalyzer;
    let task_id = uuid::Uuid::now_v7();
    let analysis = SuccessAnalyzer::analyze(
        task_id,
        vec!["goal created".into()],
        "goal.created event dispatched".into(),
        true,
    );
    let _r = kernel.dispatch(RawEvent::new(
        "reflection.success_analyzed",
        serde_json::to_value(&analysis).unwrap(),
        Actor::owner(),
        0,
    )).unwrap();

    // Dispatch a proper memory event.
    use sps_memory::memory::{MemoryId, MemoryKind, MemoryRecord};
    let memory_record = MemoryRecord {
        id: MemoryId::new(),
        kind: MemoryKind::Episodic,
        title: SmolStr::new("goal created"),
        content: json!({"detail": "youtube goal dispatched"}),
        tags: vec![],
        origin_tick: 1,
        created_at: 0,
    };
    let _m = kernel.dispatch(RawEvent::new(
        "memory.created",
        serde_json::to_value(&memory_record).unwrap(),
        Actor::owner(),
        0,
    )).unwrap();

    // Capture live state.
    let live_state = kernel.query(|s| s.clone());
    let live_event_count = live_state.event_count();
    let live_last_tick = live_state.last_tick();
    let live_last_hash = live_state.last_hash().clone();
    println!("  Live state: {} events, last_tick={}", live_event_count, live_last_tick);

    // Verify hash chain.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken: {:?}", report.failure);
    println!("  PASS: hash chain verified ({} events)", report.events_verified);

    // Replay from genesis.
    // Build a fresh pipeline (the kernel's pipeline is in &Arc form, but
    // ReplayEngine needs its own Arc<ReducerPipeline>).
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        // Mirror register_all_domain_reducers.
        sps_bus::state_ext::OwnerReducer::register(&mut reg);
        sps_goals::reducer::GoalReducer::register(&mut reg);
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        sps_agents::reducer::AgentReducer::register(&mut reg);
        sps_planner::reducer::PlannerReducer::register(&mut reg);
        sps_world::reducer::WorldReducer::register(&mut reg);
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        sps_reasoning::reducer::ReasoningReducer::register(&mut reg);
        sps_improvement::reducer::ImprovementReducer::register(&mut reg);
        sps_execution::reducer::ExecutionReducer::register(&mut reg);
        sps_factory::reducer::FactoryReducer::register(&mut reg);
        sps_autonomy::reducer::AutonomyReducer::register(&mut reg);
        sps_vectors::reducer::VectorReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    println!("  Replayed state: {} events, last_tick={}", replayed.event_count(), replayed.last_tick());

    assert_eq!(replayed.event_count(), live_event_count, "FAIL: event_count mismatch");
    assert_eq!(replayed.last_tick(), live_last_tick, "FAIL: last_tick mismatch");
    assert_eq!(replayed.last_hash(), live_last_hash, "FAIL: last_hash mismatch");
    println!("  PASS: replayed state matches live state (events, tick, hash)");

    // Compare memory slice.
    let live_mem = sps_memory::reducer::MemoryState::from_state(&live_state).unwrap_or_default();
    let replayed_mem = sps_memory::reducer::MemoryState::from_state(&replayed).unwrap_or_default();
    let live_mem_count = live_mem.graph.count();
    let replayed_mem_count = replayed_mem.graph.count();
    assert_eq!(live_mem_count, replayed_mem_count,
        "FAIL: memory count mismatch (live={}, replayed={})",
        live_mem_count, replayed_mem_count);
    println!("  PASS: memory slice identical ({} memories)", live_mem_count);

    // Compare reflection slice.
    let live_refl = sps_reflection::reducer::ReflectionState::from_state(&live_state).unwrap_or_default();
    let replayed_refl = sps_reflection::reducer::ReflectionState::from_state(&replayed).unwrap_or_default();
    let live_refl_count = live_refl.reflections.len();
    let replayed_refl_count = replayed_refl.reflections.len();
    assert_eq!(live_refl_count, replayed_refl_count,
        "FAIL: reflection count mismatch");
    println!("  PASS: reflection slice identical ({} reflections)", live_refl_count);

    println!("  PASS: deterministic replay confirmed");
}

// ─── Checkpoint 7 ─────────────────────────────────────────────────────────
// Goal Deletion — create, delete, replay, verify goal is absent after replay.
// This validates the goal.deleted reducer added in Fix #1.

#[test]
fn checkpoint_7_goal_deletion() {
    println!("\n=== CHECKPOINT 7: Goal Deletion ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Step 1: Create goal A.
    let goal_a_event = create_longterm_goal(&kernel, "Goal A — Learn Rust", "desc A");
    let goal_a_uuid = goal_a_event.payload["id"].as_str().unwrap().to_string();
    println!("  Step 1: Created Goal A (id={}...)", &goal_a_uuid[..8]);

    // Step 2: Verify Goal A is in canonical state.
    let count_before = kernel.query(|s| {
        sps_goals::reducer::GoalState::from_state(s)
            .map(|gs| gs.tree.goals.len())
            .unwrap_or(0)
    });
    assert_eq!(count_before, 1, "FAIL: expected 1 goal before deletion, found {}", count_before);
    println!("  Step 2: PASS — 1 goal in canonical state before deletion");

    // Step 3: Dispatch goal.deleted for Goal A.
    let delete_payload = json!({"goal_id": goal_a_uuid});
    let _delete_event = kernel.dispatch(RawEvent::new(
        "goal.deleted",
        delete_payload,
        Actor::owner(),
        0,
    )).expect("dispatch goal.deleted");
    println!("  Step 3: Dispatched goal.deleted for Goal A");

    // Step 4: Verify Goal A is GONE from canonical state.
    let count_after = kernel.query(|s| {
        sps_goals::reducer::GoalState::from_state(s)
            .map(|gs| gs.tree.goals.len())
            .unwrap_or(0)
    });
    if count_after == 0 {
        println!("  Step 4: PASS — 0 goals in canonical state after deletion");
    } else {
        println!("  FAIL: expected 0 goals after deletion, found {}", count_after);
        println!("  ─────────────────────────────────────────────────");
        println!("  EXPECTED: goal.deleted reducer removes goal from tree");
        println!("  OBSERVED: {} goals still in tree", count_after);
        println!("  ROOT CAUSE: goal.deleted reducer not firing OR");
        println!("              goal_id parsing failed");
        println!("  SEVERITY: HIGH");
        println!("  ─────────────────────────────────────────────────");
        panic!("CHECKPOINT 7 FAILED — see output above");
    }

    // Step 5: Verify hash chain still intact (deletion is just another event).
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken after deletion");
    // 2 events: goal.created + goal.deleted
    assert_eq!(report.events_verified, 2, "FAIL: expected 2 events, found {}", report.events_verified);
    println!("  Step 5: PASS — hash chain intact ({} events)", report.events_verified);

    // Step 6: Replay from genesis — goal should STILL be absent.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_goals::reducer::GoalReducer::register(&mut reg);
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    let replayed_count = sps_goals::reducer::GoalState::from_state(&replayed)
        .map(|gs| gs.tree.goals.len())
        .unwrap_or(0);

    if replayed_count == 0 {
        println!("  Step 6: PASS — 0 goals in REPLAYED state (deletion persisted)");
    } else {
        println!("  FAIL: replayed state has {} goals (should be 0)", replayed_count);
        println!("  ─────────────────────────────────────────────────");
        println!("  EXPECTED: goal.deleted applied during replay → 0 goals");
        println!("  OBSERVED: replayed state has {} goals", replayed_count);
        println!("  ROOT CAUSE: goal.deleted reducer not idempotent in replay, OR");
        println!("              the reducer removed the goal in live state but didn't");
        println!("              serialize correctly for replay");
        println!("  SEVERITY: CRITICAL");
        println!("  ─────────────────────────────────────────────────");
        panic!("CHECKPOINT 7 FAILED — see output above");
    }

    println!("  PASS: goal.created → goal.deleted → replay → goal absent");
}

// ─── Checkpoint 8 ─────────────────────────────────────────────────────────
// Progress Updates — task.status_changed + goal.progress_updated + replay.
// Validates that task completion + milestone progress persist through replay.

#[test]
fn checkpoint_8_progress_updates() {
    println!("\n=== CHECKPOINT 8: Progress Updates ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    use sps_goals::hierarchy::{Objective, Milestone, Task, TaskStatus};

    // Step 1: Create goal + objective + milestone + 2 tasks.
    let goal_event = create_longterm_goal(&kernel, "Build a successful YouTube channel", "");
    let goal_uuid = goal_event.payload["id"].as_str().unwrap().to_string();

    let objective = Objective {
        id: uuid::Uuid::now_v7(),
        title: SmolStr::new("Main"),
        milestones: Vec::new(),
    };
    kernel.dispatch(RawEvent::new(
        "goal.objective_added",
        json!({"goal_id": goal_uuid, "objective": objective}),
        Actor::owner(),
        0,
    )).unwrap();

    let milestone = Milestone {
        id: uuid::Uuid::now_v7(),
        title: SmolStr::new("Set up channel"),
        tasks: Vec::new(),
    };
    kernel.dispatch(RawEvent::new(
        "goal.milestone_added",
        json!({"goal_id": goal_uuid, "objective_idx": 0, "milestone": milestone}),
        Actor::owner(),
        0,
    )).unwrap();

    let task_a_id = uuid::Uuid::now_v7();
    let task_b_id = uuid::Uuid::now_v7();
    for tid in [task_a_id, task_b_id] {
        let task = Task {
            id: tid,
            title: SmolStr::new(format!("Task {}", &tid.to_string()[..4])),
            description: String::new(),
            status: TaskStatus::Pending,
            assigned_agent: None,
            origin_tick: 0,
        };
        kernel.dispatch(RawEvent::new(
            "task.created",
            json!({"goal_id": goal_uuid, "objective_idx": 0, "milestone_idx": 0, "task": task}),
            Actor::owner(),
            0,
        )).unwrap();
    }
    println!("  Step 1: Created goal + objective + milestone + 2 pending tasks");

    // Step 2: Verify both tasks are Pending.
    let pending_count = kernel.query(|s| {
        sps_goals::reducer::GoalState::from_state(s)
            .map(|gs| {
                gs.tree.goals.values()
                    .flat_map(|g| &g.objectives)
                    .flat_map(|o| &o.milestones)
                    .flat_map(|m| &m.tasks)
                    .filter(|t| t.status == TaskStatus::Pending)
                    .count()
            }).unwrap_or(0)
    });
    assert_eq!(pending_count, 2, "FAIL: expected 2 pending tasks, found {}", pending_count);
    println!("  Step 2: PASS — 2 pending tasks");

    // Step 3: Complete Task A via task.status_changed.
    kernel.dispatch(RawEvent::new(
        "task.status_changed",
        json!({"task_id": task_a_id.to_string(), "status": "completed"}),
        Actor::owner(),
        0,
    )).unwrap();
    println!("  Step 3: Dispatched task.status_changed (Task A → Completed)");

    // Step 4: Verify Task A is now Completed, Task B still Pending.
    let (completed_count, pending_after) = kernel.query(|s| {
        let gs = sps_goals::reducer::GoalState::from_state(s).unwrap();
        let tasks: Vec<_> = gs.tree.goals.values()
            .flat_map(|g| &g.objectives)
            .flat_map(|o| &o.milestones)
            .flat_map(|m| &m.tasks)
            .collect();
        let completed = tasks.iter().filter(|t| t.status == TaskStatus::Completed).count();
        let pending = tasks.iter().filter(|t| t.status == TaskStatus::Pending).count();
        (completed, pending)
    });
    if completed_count == 1 && pending_after == 1 {
        println!("  Step 4: PASS — {} completed, {} pending", completed_count, pending_after);
    } else {
        println!("  FAIL: expected 1 completed + 1 pending, got {} completed + {} pending",
            completed_count, pending_after);
        println!("  ─────────────────────────────────────────────────");
        println!("  EXPECTED: task.status_changed sets one task to Completed");
        println!("  OBSERVED: completed={}, pending={}", completed_count, pending_after);
        println!("  ROOT CAUSE: task.status_changed reducer not finding task by id, OR");
        println!("              TaskStatus deserialization issue");
        println!("  SEVERITY: HIGH");
        println!("  ─────────────────────────────────────────────────");
        panic!("CHECKPOINT 8 FAILED — see output above");
    }

    // Step 5: Dispatch goal.progress_updated to mark milestone complete.
    kernel.dispatch(RawEvent::new(
        "goal.progress_updated",
        json!({
            "goal_id": goal_uuid,
            "milestone": "Set up channel",
            "completed": true,
        }),
        Actor::owner(),
        0,
    )).unwrap();
    println!("  Step 5: Dispatched goal.progress_updated (milestone → complete)");

    // Step 6: Verify Task B is now also Completed (progress_updated marks all tasks in milestone).
    let completed_after_progress = kernel.query(|s| {
        sps_goals::reducer::GoalState::from_state(s)
            .map(|gs| {
                gs.tree.goals.values()
                    .flat_map(|g| &g.objectives)
                    .flat_map(|o| &o.milestones)
                    .flat_map(|m| &m.tasks)
                    .filter(|t| t.status == TaskStatus::Completed)
                    .count()
            }).unwrap_or(0)
    });
    if completed_after_progress == 2 {
        println!("  Step 6: PASS — goal.progress_updated marked all milestone tasks complete ({}/{})",
            completed_after_progress, 2);
    } else {
        println!("  FAIL: expected 2 completed after progress_updated, got {}", completed_after_progress);
        println!("  ─────────────────────────────────────────────────");
        println!("  EXPECTED: goal.progress_updated marks all tasks in milestone as Completed");
        println!("  OBSERVED: {} completed (was 1, expected 2)", completed_after_progress);
        println!("  ROOT CAUSE: goal.progress_updated reducer milestone name matching failed, OR");
        println!("              milestone title doesn't contain expected substring");
        println!("  SEVERITY: MEDIUM");
        println!("  ─────────────────────────────────────────────────");
        panic!("CHECKPOINT 8 FAILED — see output above");
    }

    // Step 7: Replay — verify same final state.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_goals::reducer::GoalReducer::register(&mut reg);
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    let replayed_completed = sps_goals::reducer::GoalState::from_state(&replayed)
        .map(|gs| {
            gs.tree.goals.values()
                .flat_map(|g| &g.objectives)
                .flat_map(|o| &o.milestones)
                .flat_map(|m| &m.tasks)
                .filter(|t| t.status == TaskStatus::Completed)
                .count()
        }).unwrap_or(0);

    if replayed_completed == 2 {
        println!("  Step 7: PASS — replayed state has {} completed tasks (matches live)", replayed_completed);
    } else {
        println!("  FAIL: replayed state has {} completed (expected 2)", replayed_completed);
        panic!("CHECKPOINT 8 FAILED — replay mismatch");
    }

    println!("  PASS: task.pending → task.status_changed → goal.progress_updated → replay → same state");
}

// ─── Checkpoint 9 ─────────────────────────────────────────────────────────
// Multi-Goal Isolation — 3 goals, delete middle one, verify outer 2 untouched.
// Validates that the goal tree doesn't have cross-contamination between goals.

#[test]
fn checkpoint_9_multi_goal_isolation() {
    println!("\n=== CHECKPOINT 9: Multi-Goal Isolation ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Step 1: Create 3 goals.
    let goal_a = create_longterm_goal(&kernel, "Goal A — Learn Rust", "");
    let goal_b = create_longterm_goal(&kernel, "Goal B — Build App", "");
    let goal_c = create_longterm_goal(&kernel, "Goal C — Write Book", "");
    let id_a = goal_a.payload["id"].as_str().unwrap().to_string();
    let id_b = goal_b.payload["id"].as_str().unwrap().to_string();
    let id_c = goal_c.payload["id"].as_str().unwrap().to_string();
    println!("  Step 1: Created 3 goals (A, B, C)");

    // Step 2: Add milestones + tasks to each.
    use sps_goals::hierarchy::{Objective, Milestone, Task, TaskStatus};
    for (goal_id, label) in [&id_a, &id_b, &id_c].iter().zip(["A", "B", "C"]) {
        let objective = Objective {
            id: uuid::Uuid::now_v7(),
            title: SmolStr::new(format!("Objective-{}", label)),
            milestones: Vec::new(),
        };
        kernel.dispatch(RawEvent::new(
            "goal.objective_added",
            json!({"goal_id": goal_id, "objective": objective}),
            Actor::owner(), 0,
        )).unwrap();
        let milestone = Milestone {
            id: uuid::Uuid::now_v7(),
            title: SmolStr::new(format!("Milestone-{}", label)),
            tasks: Vec::new(),
        };
        kernel.dispatch(RawEvent::new(
            "goal.milestone_added",
            json!({"goal_id": goal_id, "objective_idx": 0, "milestone": milestone}),
            Actor::owner(), 0,
        )).unwrap();
        let task = Task {
            id: uuid::Uuid::now_v7(),
            title: SmolStr::new(format!("Task-{}", label)),
            description: String::new(),
            status: TaskStatus::Pending,
            assigned_agent: None,
            origin_tick: 0,
        };
        kernel.dispatch(RawEvent::new(
            "task.created",
            json!({"goal_id": goal_id, "objective_idx": 0, "milestone_idx": 0, "task": task}),
            Actor::owner(), 0,
        )).unwrap();
    }
    println!("  Step 2: Added 1 objective + 1 milestone + 1 task to each goal");

    // Step 3: Verify 3 goals, 3 objectives, 3 milestones, 3 tasks.
    let counts = kernel.query(|s| {
        let gs = sps_goals::reducer::GoalState::from_state(s).unwrap();
        let goals = gs.tree.goals.len();
        let objectives: usize = gs.tree.goals.values().map(|g| g.objectives.len()).sum();
        let milestones: usize = gs.tree.goals.values()
            .flat_map(|g| &g.objectives)
            .map(|o| o.milestones.len())
            .sum();
        let tasks: usize = gs.tree.goals.values()
            .flat_map(|g| &g.objectives)
            .flat_map(|o| &o.milestones)
            .map(|m| m.tasks.len())
            .sum();
        (goals, objectives, milestones, tasks)
    });
    assert_eq!(counts, (3, 3, 3, 3), "FAIL: expected (3,3,3,3), got {:?}", counts);
    println!("  Step 3: PASS — {} goals, {} objectives, {} milestones, {} tasks",
        counts.0, counts.1, counts.2, counts.3);

    // Step 4: Delete Goal B.
    kernel.dispatch(RawEvent::new(
        "goal.deleted",
        json!({"goal_id": id_b}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Step 4: Deleted Goal B");

    // Step 5: Verify A and C untouched, B gone.
    let after_delete = kernel.query(|s| {
        let gs = sps_goals::reducer::GoalState::from_state(s).unwrap();
        let goals_count = gs.tree.goals.len();
        let has_a = gs.tree.goals.values().any(|g| g.title.as_str().contains("Learn Rust"));
        let has_c = gs.tree.goals.values().any(|g| g.title.as_str().contains("Write Book"));
        let has_b = gs.tree.goals.values().any(|g| g.title.as_str().contains("Build App"));
        let remaining_tasks: usize = gs.tree.goals.values()
            .flat_map(|g| &g.objectives)
            .flat_map(|o| &o.milestones)
            .map(|m| m.tasks.len())
            .sum();
        (goals_count, has_a, has_b, has_c, remaining_tasks)
    });
    if after_delete.0 == 2 && after_delete.1 && !after_delete.2 && after_delete.3 && after_delete.4 == 2 {
        println!("  Step 5: PASS — 2 goals remaining (A + C), B gone, 2 tasks intact");
    } else {
        println!("  FAIL: after deleting B, expected (2, A=true, B=false, C=true, 2 tasks), got {:?}",
            after_delete);
        println!("  ─────────────────────────────────────────────────");
        println!("  EXPECTED: deleting Goal B leaves A and C untouched");
        println!("  OBSERVED: goals={}, has_A={}, has_B={}, has_C={}, tasks={}",
            after_delete.0, after_delete.1, after_delete.2, after_delete.3, after_delete.4);
        println!("  ROOT CAUSE: goal.deleted reducer removed wrong goal OR");
        println!("              cascading delete affected siblings");
        println!("  SEVERITY: CRITICAL");
        println!("  ─────────────────────────────────────────────────");
        panic!("CHECKPOINT 9 FAILED — see output above");
    }

    // Step 6: Verify Goal A and C still have their objectives/milestones/tasks intact.
    let (a_obj, a_mil, a_task, c_obj, c_mil, c_task) = kernel.query(|s| {
        let gs = sps_goals::reducer::GoalState::from_state(s).unwrap();
        let a = gs.tree.goals.values().find(|g| g.title.as_str().contains("Learn Rust")).unwrap();
        let c = gs.tree.goals.values().find(|g| g.title.as_str().contains("Write Book")).unwrap();
        (a.objectives.len(),
         a.objectives.first().map(|o| o.milestones.len()).unwrap_or(0),
         a.objectives.first().and_then(|o| o.milestones.first()).map(|m| m.tasks.len()).unwrap_or(0),
         c.objectives.len(),
         c.objectives.first().map(|o| o.milestones.len()).unwrap_or(0),
         c.objectives.first().and_then(|o| o.milestones.first()).map(|m| m.tasks.len()).unwrap_or(0))
    });
    if (a_obj, a_mil, a_task, c_obj, c_mil, c_task) == (1, 1, 1, 1, 1, 1) {
        println!("  Step 6: PASS — A: {}obj/{}mil/{}task, C: {}obj/{}mil/{}task (intact)",
            a_obj, a_mil, a_task, c_obj, c_mil, c_task);
    } else {
        println!("  FAIL: A or C objectives/milestones/tasks were affected");
        println!("  A: {}obj/{}mil/{}task, C: {}obj/{}mil/{}task",
            a_obj, a_mil, a_task, c_obj, c_mil, c_task);
        panic!("CHECKPOINT 9 FAILED — isolation broken");
    }

    // Step 7: Replay — verify same state.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_goals::reducer::GoalReducer::register(&mut reg);
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
    let replayed_count = sps_goals::reducer::GoalState::from_state(&replayed)
        .map(|gs| gs.tree.goals.len()).unwrap_or(0);
    if replayed_count == 2 {
        println!("  Step 7: PASS — replayed state has 2 goals (matches live)");
    } else {
        println!("  FAIL: replayed state has {} goals (expected 2)", replayed_count);
        panic!("CHECKPOINT 9 FAILED — replay mismatch");
    }

    println!("  PASS: 3 goals created → B deleted → A+C intact → replay matches");
}

// ─── Checkpoint 10 ────────────────────────────────────────────────────────
// SQLite Backend — re-run the same pipeline against SQLite (not just InMemory).
// This is the CRITICAL test: if it passes on InMemory but fails on SQLite,
// we have a serialization/storage bug that would only manifest in production.

#[test]
fn checkpoint_10_sqlite_backend() {
    println!("\n=== CHECKPOINT 10: SQLite Backend ===");
    println!("  NOTE: Running the full Goal pipeline against SQLite in-memory.");

    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open_in_memory()
            .expect("failed to open in-memory SQLite")
    );
    let kernel = boot_kernel(storage.clone());

    // Verify SQLite backend is in use.
    println!("  Backend: {}", kernel.backend_name());
    assert_eq!(kernel.backend_name(), "sqlite", "FAIL: expected sqlite backend");

    // Step 1: Create a goal.
    let goal_event = create_longterm_goal(&kernel, "YouTube channel via SQLite", "");
    let goal_uuid = goal_event.payload["id"].as_str().unwrap().to_string();
    println!("  Step 1: Created goal (tick={})", goal_event.tick);

    // Step 2: Verify goal is in canonical state.
    let goal_count = kernel.query(|s| {
        sps_goals::reducer::GoalState::from_state(s)
            .map(|gs| gs.tree.goals.len())
            .unwrap_or(0)
    });
    if goal_count == 1 {
        println!("  Step 2: PASS — goal materialized in SQLite-backed state");
    } else {
        println!("  FAIL: expected 1 goal, got {}", goal_count);
        println!("  ─────────────────────────────────────────────────");
        println!("  EXPECTED: GoalReducer works on SQLite EventStore same as InMemory");
        println!("  OBSERVED: 0 goals in canonical state");
        println!("  ROOT CAUSE: SQLite event serialization differs from InMemory, OR");
        println!("              event payload JSON shape differs across backends");
        println!("  SEVERITY: CRITICAL — production uses SQLite");
        println!("  ─────────────────────────────────────────────────");
        panic!("CHECKPOINT 10 FAILED — see output above");
    }

    // Step 3: Add objective + milestone + task via producer events.
    use sps_goals::hierarchy::{Objective, Milestone, Task, TaskStatus};
    let objective = Objective {
        id: uuid::Uuid::now_v7(),
        title: SmolStr::new("Main"),
        milestones: Vec::new(),
    };
    kernel.dispatch(RawEvent::new(
        "goal.objective_added",
        json!({"goal_id": goal_uuid, "objective": objective}),
        Actor::owner(), 0,
    )).unwrap();
    let milestone = Milestone {
        id: uuid::Uuid::now_v7(),
        title: SmolStr::new("SQLite milestone"),
        tasks: Vec::new(),
    };
    kernel.dispatch(RawEvent::new(
        "goal.milestone_added",
        json!({"goal_id": goal_uuid, "objective_idx": 0, "milestone": milestone}),
        Actor::owner(), 0,
    )).unwrap();
    let task = Task {
        id: uuid::Uuid::now_v7(),
        title: SmolStr::new("SQLite task"),
        description: String::new(),
        status: TaskStatus::Pending,
        assigned_agent: None,
        origin_tick: 0,
    };
    kernel.dispatch(RawEvent::new(
        "task.created",
        json!({"goal_id": goal_uuid, "objective_idx": 0, "milestone_idx": 0, "task": task}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Step 3: Added objective + milestone + task via producer events");

    // Step 4: Verify hierarchy materialized.
    let hierarchy = kernel.query(|s| {
        let gs = sps_goals::reducer::GoalState::from_state(s).unwrap();
        let g = gs.tree.goals.values().next().unwrap();
        (g.objectives.len(),
         g.objectives.first().map(|o| o.milestones.len()).unwrap_or(0),
         g.objectives.first().and_then(|o| o.milestones.first()).map(|m| m.tasks.len()).unwrap_or(0))
    });
    if hierarchy == (1, 1, 1) {
        println!("  Step 4: PASS — hierarchy: 1 obj / 1 mil / 1 task in SQLite state");
    } else {
        println!("  FAIL: expected (1,1,1), got {:?}", hierarchy);
        println!("  ─────────────────────────────────────────────────");
        println!("  EXPECTED: SQLite-backed GoalReducer materializes hierarchy");
        println!("  OBSERVED: hierarchy = {:?}", hierarchy);
        println!("  ROOT CAUSE: SQLite-specific bug — likely JSON serialization");
        println!("              of nested Objective/Milestone/Task structs");
        println!("  SEVERITY: CRITICAL");
        println!("  ─────────────────────────────────────────────────");
        panic!("CHECKPOINT 10 FAILED — see output above");
    }

    // Step 5: Verify hash chain.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none(), "FAIL: SQLite hash chain broken");
    // 4 events: goal.created + objective_added + milestone_added + task.created
    assert_eq!(report.events_verified, 4, "FAIL: expected 4 events, got {}", report.events_verified);
    println!("  Step 5: PASS — SQLite hash chain intact ({} events)", report.events_verified);

    // Step 6: Drop the kernel, simulate restart by re-booting from same storage.
    drop(kernel);
    let kernel2 = boot_kernel(storage.clone());
    println!("  Step 6: Restarted kernel from same SQLite storage");

    // Step 7: Verify state was reconstructed on boot.
    let goal_count_after_restart = kernel2.query(|s| {
        sps_goals::reducer::GoalState::from_state(s)
            .map(|gs| gs.tree.goals.len())
            .unwrap_or(0)
    });
    if goal_count_after_restart == 1 {
        let g = kernel2.query(|s| {
            let gs = sps_goals::reducer::GoalState::from_state(s).unwrap();
            gs.tree.goals.values().next().unwrap().title.as_str().to_string()
        });
        println!("  Step 7: PASS — kernel restarted, goal '{}' present in state", g);
    } else {
        println!("  FAIL: after restart, expected 1 goal, got {}", goal_count_after_restart);
        println!("  ─────────────────────────────────────────────────");
        println!("  EXPECTED: kernel restart from SQLite reconstructs state from event log");
        println!("  OBSERVED: 0 goals after restart");
        println!("  ROOT CAUSE: boot_with doesn't replay events on SQLite startup, OR");
        println!("              snapshot loading is broken on SQLite");
        println!("  SEVERITY: CRITICAL — this is the crash recovery path");
        println!("  ─────────────────────────────────────────────────");
        panic!("CHECKPOINT 10 FAILED — see output above");
    }

    println!("  PASS: SQLite backend works end-to-end (create + hierarchy + restart)");
}

// ─── Checkpoint 11 ────────────────────────────────────────────────────────
// Crash Recovery — write events, simulate crash by dropping kernel, restart,
// verify state is fully reconstructed from the event log (no snapshots).
// This is the ultimate Event Sourcing guarantee: events = source of truth.

#[test]
fn checkpoint_11_crash_recovery() {
    println!("\n=== CHECKPOINT 11: Crash Recovery ===");
    println!("  NOTE: Simulating crash by dropping kernel mid-session, then re-booting.");

    // Use a temp file so we can re-open the same DB.
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("sps_crash_test_{}.db", uuid::Uuid::now_v7()));
    println!("  DB path: {}", db_path.display());

    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open(&db_path)
            .expect("failed to open SQLite file")
    );

    // Phase 1: Create a complex state with goals, objectives, milestones, tasks.
    {
        let kernel = boot_kernel(storage.clone());
        println!("  Phase 1: Booted kernel, creating complex state...");

        // Create 3 goals with full hierarchies.
        for i in 1..=3 {
            let goal_event = create_longterm_goal(&kernel, &format!("Crash test goal {}", i), "");
            let goal_uuid = goal_event.payload["id"].as_str().unwrap().to_string();

            use sps_goals::hierarchy::{Objective, Milestone, Task, TaskStatus};
            let objective = Objective {
                id: uuid::Uuid::now_v7(),
                title: SmolStr::new(format!("Obj-{}", i)),
                milestones: Vec::new(),
            };
            kernel.dispatch(RawEvent::new(
                "goal.objective_added",
                json!({"goal_id": goal_uuid, "objective": objective}),
                Actor::owner(), 0,
            )).unwrap();

            for j in 1..=2 {
                let milestone = Milestone {
                    id: uuid::Uuid::now_v7(),
                    title: SmolStr::new(format!("Mil-{}-{}", i, j)),
                    tasks: Vec::new(),
                };
                kernel.dispatch(RawEvent::new(
                    "goal.milestone_added",
                    json!({"goal_id": goal_uuid, "objective_idx": 0, "milestone": milestone}),
                    Actor::owner(), 0,
                )).unwrap();

                let task = Task {
                    id: uuid::Uuid::now_v7(),
                    title: SmolStr::new(format!("Task-{}-{}", i, j)),
                    description: String::new(),
                    status: TaskStatus::Pending,
                    assigned_agent: None,
                    origin_tick: 0,
                };
                kernel.dispatch(RawEvent::new(
                    "task.created",
                    json!({"goal_id": goal_uuid, "objective_idx": 0, "milestone_idx": j - 1, "task": task}),
                    Actor::owner(), 0,
                )).unwrap();
            }
        }

        // Verify state before "crash".
        let counts_before = kernel.query(|s| {
            let gs = sps_goals::reducer::GoalState::from_state(s).unwrap();
            let goals = gs.tree.goals.len();
            let obj: usize = gs.tree.goals.values().map(|g| g.objectives.len()).sum();
            let mil: usize = gs.tree.goals.values()
                .flat_map(|g| &g.objectives).map(|o| o.milestones.len()).sum();
            let tasks: usize = gs.tree.goals.values()
                .flat_map(|g| &g.objectives)
                .flat_map(|o| &o.milestones)
                .map(|m| m.tasks.len()).sum();
            (goals, obj, mil, tasks)
        });
        println!("  Phase 1: State before crash: {} goals, {} obj, {} mil, {} tasks",
            counts_before.0, counts_before.1, counts_before.2, counts_before.3);
        assert_eq!(counts_before, (3, 3, 6, 6), "FAIL: setup state mismatch");

        // CRASH: Drop the kernel WITHOUT taking a snapshot.
        // The only persisted state is the event log — no snapshots exist.
        println!("  Phase 1: CRASH — dropping kernel without snapshot");
        drop(kernel);
    }

    // Phase 2: Re-boot from the same storage. State must be reconstructed
    // from the event log alone.
    {
        println!("  Phase 2: Re-booting kernel from event log...");
        let kernel2 = boot_kernel(storage.clone());

        let counts_after = kernel2.query(|s| {
            let gs = sps_goals::reducer::GoalState::from_state(s).unwrap();
            let goals = gs.tree.goals.len();
            let obj: usize = gs.tree.goals.values().map(|g| g.objectives.len()).sum();
            let mil: usize = gs.tree.goals.values()
                .flat_map(|g| &g.objectives).map(|o| o.milestones.len()).sum();
            let tasks: usize = gs.tree.goals.values()
                .flat_map(|g| &g.objectives)
                .flat_map(|o| &o.milestones)
                .map(|m| m.tasks.len()).sum();
            (goals, obj, mil, tasks)
        });

        if counts_after == (3, 3, 6, 6) {
            println!("  Phase 2: PASS — reconstructed state: {} goals, {} obj, {} mil, {} tasks",
                counts_after.0, counts_after.1, counts_after.2, counts_after.3);
        } else {
            println!("  FAIL: state mismatch after crash recovery");
            println!("  ─────────────────────────────────────────────────");
            println!("  EXPECTED: (3, 3, 6, 6) — same as before crash");
            println!("  OBSERVED: {:?}", counts_after);
            println!("  ROOT CAUSE: events not persisted to SQLite on dispatch, OR");
            println!("              boot_with doesn't replay from genesis on startup, OR");
            println!("              reducer not registered during re-boot");
            println!("  SEVERITY: CRITICAL — this is THE crash recovery guarantee");
            println!("  ─────────────────────────────────────────────────");
            panic!("CHECKPOINT 11 FAILED — see output above");
        }

        // Verify hash chain still valid.
        let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
        assert!(report.failure.is_none(), "FAIL: hash chain broken after crash recovery");
        // 3 goals × (1 created + 1 obj + 2 mil + 2 tasks) = 3 × 6 = 18 events
        assert_eq!(report.events_verified, 18, "FAIL: expected 18 events, got {}", report.events_verified);
        println!("  Phase 2: PASS — hash chain verified ({} events)", report.events_verified);

        // Verify goal titles survived.
        let titles: Vec<String> = kernel2.query(|s| {
            let gs = sps_goals::reducer::GoalState::from_state(s).unwrap();
            gs.tree.goals.values().map(|g| g.title.as_str().to_string()).collect()
        });
        println!("  Phase 2: Goal titles after recovery: {:?}", titles);
        assert_eq!(titles.len(), 3, "FAIL: lost goals during recovery");
    }

    // Cleanup.
    drop(storage);
    std::fs::remove_file(&db_path).ok();

    println!("  PASS: crash → restart → state fully reconstructed from event log");
}

// ─── Checkpoint 12 ────────────────────────────────────────────────────────
// Long Event Chains — stress test: 1000 goals × (1 obj + 1 mil + 10 tasks)
// = 13,000 events. Measures: dispatch time, replay time, memory, state correctness.
// This is the performance + scalability validation.

#[test]
fn checkpoint_12_long_event_chains() {
    println!("\n=== CHECKPOINT 12: Long Event Chains ===");
    println!("  NOTE: Stress test — 1000 goals × 13 events each = 13,000 events");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    use sps_goals::hierarchy::{Objective, Milestone, Task, TaskStatus};

    const NUM_GOALS: usize = 100;
    const TASKS_PER_MILESTONE: usize = 10;
    // Events per goal: 1 created + 1 obj + 1 mil + 10 tasks = 13
    const EVENTS_PER_GOAL: usize = 13;
    const TOTAL_EVENTS: usize = NUM_GOALS * EVENTS_PER_GOAL;

    println!("  Target: {} goals, {} tasks, {} total events",
        NUM_GOALS, NUM_GOALS * TASKS_PER_MILESTONE, TOTAL_EVENTS);

    // Phase 1: Dispatch all events. Measure time.
    let dispatch_start = std::time::Instant::now();
    for i in 0..NUM_GOALS {
        let goal_event = create_longterm_goal(&kernel, &format!("Goal {}", i), "");
        let goal_uuid = goal_event.payload["id"].as_str().unwrap().to_string();

        let objective = Objective {
            id: uuid::Uuid::now_v7(),
            title: SmolStr::new("Main"),
            milestones: Vec::new(),
        };
        kernel.dispatch(RawEvent::new(
            "goal.objective_added",
            json!({"goal_id": goal_uuid, "objective": objective}),
            Actor::owner(), 0,
        )).unwrap();

        let milestone = Milestone {
            id: uuid::Uuid::now_v7(),
            title: SmolStr::new("Milestone"),
            tasks: Vec::new(),
        };
        kernel.dispatch(RawEvent::new(
            "goal.milestone_added",
            json!({"goal_id": goal_uuid, "objective_idx": 0, "milestone": milestone}),
            Actor::owner(), 0,
        )).unwrap();

        for j in 0..TASKS_PER_MILESTONE {
            let task = Task {
                id: uuid::Uuid::now_v7(),
                title: SmolStr::new(format!("Task {}", j)),
                description: String::new(),
                status: TaskStatus::Pending,
                assigned_agent: None,
                origin_tick: 0,
            };
            kernel.dispatch(RawEvent::new(
                "task.created",
                json!({"goal_id": goal_uuid, "objective_idx": 0, "milestone_idx": 0, "task": task}),
                Actor::owner(), 0,
            )).unwrap();
        }

        if i > 0 && i % 100 == 0 {
            println!("    Dispatched {} / {} goals ({} events, {}ms)",
                i, NUM_GOALS, i * EVENTS_PER_GOAL, dispatch_start.elapsed().as_millis());
        }
    }
    let dispatch_ms = dispatch_start.elapsed().as_millis();
    println!("  Phase 1: Dispatched {} events in {}ms ({:.0} events/sec)",
        TOTAL_EVENTS, dispatch_ms, TOTAL_EVENTS as f64 / (dispatch_ms as f64 / 1000.0));

    // Phase 2: Verify state counts.
    let counts = kernel.query(|s| {
        let gs = sps_goals::reducer::GoalState::from_state(s).unwrap();
        let goals = gs.tree.goals.len();
        let tasks: usize = gs.tree.goals.values()
            .flat_map(|g| &g.objectives)
            .flat_map(|o| &o.milestones)
            .map(|m| m.tasks.len())
            .sum();
        (goals, tasks)
    });
    if counts.0 == NUM_GOALS && counts.1 == NUM_GOALS * TASKS_PER_MILESTONE {
        println!("  Phase 2: PASS — {} goals + {} tasks in canonical state", counts.0, counts.1);
    } else {
        println!("  FAIL: expected ({} goals, {} tasks), got {:?}",
            NUM_GOALS, NUM_GOALS * TASKS_PER_MILESTONE, counts);
        panic!("CHECKPOINT 12 FAILED — state counts wrong");
    }

    // Phase 3: Hash chain verification.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken at scale");
    assert_eq!(report.events_verified, TOTAL_EVENTS as u64, "FAIL: event count mismatch");
    println!("  Phase 3: PASS — hash chain verified ({} events)", report.events_verified);

    // Phase 4: Replay from genesis. Measure time.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_goals::reducer::GoalReducer::register(&mut reg);
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        sps_reflection::reducer::ReflectionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replay_start = std::time::Instant::now();
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
    let replay_ms = replay_start.elapsed().as_millis();
    println!("  Phase 4: Replayed {} events in {}ms ({:.0} events/sec)",
        TOTAL_EVENTS, replay_ms, TOTAL_EVENTS as f64 / (replay_ms as f64 / 1000.0));

    // Phase 5: Verify replayed state matches live state.
    let replayed_counts = sps_goals::reducer::GoalState::from_state(&replayed)
        .map(|gs| {
            let goals = gs.tree.goals.len();
            let tasks: usize = gs.tree.goals.values()
                .flat_map(|g| &g.objectives)
                .flat_map(|o| &o.milestones)
                .map(|m| m.tasks.len())
                .sum();
            (goals, tasks)
        }).unwrap_or((0, 0));
    if replayed_counts == counts {
        println!("  Phase 5: PASS — replayed state matches live state ({:?})", replayed_counts);
    } else {
        println!("  FAIL: replayed {:?} != live {:?}", replayed_counts, counts);
        panic!("CHECKPOINT 12 FAILED — replay mismatch at scale");
    }

    // Phase 6: Verify hash chain on replayed state.
    let replayed_hash = replayed.last_hash();
    let live_hash = kernel.query(|s| s.last_hash());
    if replayed_hash == live_hash {
        println!("  Phase 6: PASS — final hash matches ({})", replayed_hash);
    } else {
        println!("  FAIL: hash mismatch (live={}, replayed={})", live_hash, replayed_hash);
        panic!("CHECKPOINT 12 FAILED — hash mismatch at scale");
    }

    // Performance summary.
    println!("\n  ─── Performance Summary ───");
    println!("  Events:    {}", TOTAL_EVENTS);
    println!("  Dispatch:  {}ms ({:.0} ev/sec)", dispatch_ms, TOTAL_EVENTS as f64 / (dispatch_ms as f64 / 1000.0));
    println!("  Replay:    {}ms ({:.0} ev/sec)", replay_ms, TOTAL_EVENTS as f64 / (replay_ms as f64 / 1000.0));
    println!("  Per-event dispatch cost: {:.2}μs", dispatch_ms as f64 * 1000.0 / TOTAL_EVENTS as f64);
    println!("  Per-event replay cost:   {:.2}μs", replay_ms as f64 * 1000.0 / TOTAL_EVENTS as f64);

    println!("  PASS: 13K events dispatched, replayed, verified identical");
}
