//! Foundation Recovery Smoke Tests
//!
//! 4 tests that prove the full Producer → EventSink → Reducer → Replay chain:
//! 1. Agent Smoke: register → dispatch → delegate → send → replay
//! 2. Autonomy Smoke: goal_activated + weekly_review → state + replay
//! 3. Factory Smoke: run_with_sink → FactoryState + WorldState + Execution + replay
//! 4. H0 Drift: store.count == meta.event_count

use std::sync::Arc;

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
        sps_agents::reducer::AgentReducer::register(&mut reg);
        sps_autonomy::reducer::AutonomyReducer::register(&mut reg);
        sps_execution::reducer::ExecutionReducer::register(&mut reg);
        sps_factory::reducer::FactoryReducer::register(&mut reg);
        sps_world::reducer::WorldReducer::register(&mut reg);
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        reg
    })))
}

// ════════════════════════════════════════════════════════════════════════
// 1. AGENT SMOKE TEST
// ════════════════════════════════════════════════════════════════════════

#[test]
fn agent_smoke_full_chain() {
    println!("\n=== AGENT SMOKE: register → dispatch → delegate → send → replay ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = Arc::new(sps_agents::runtime::AgentRuntime::with_sink(
        sps_agents::runtime::AgentRuntimeConfig::default(),
        kernel.clone(),
    ));

    // 1. Register Architect + Developer.
    let architect = Arc::new(sps_agents::agent::Agent::new(
        sps_agents::agent::AgentArchetype::Architect,
        SmolStr::new("Architect"),
        "system prompt",
    ));
    let arch_id = runtime.register(architect);

    let developer = Arc::new(sps_agents::agent::Agent::new(
        sps_agents::agent::AgentArchetype::Developer,
        SmolStr::new("Developer"),
        "system prompt",
    ));
    let dev_id = runtime.register(developer);
    println!("  Step 1: registered Architect + Developer");

    // Verify AgentState populated.
    let agent_count = kernel.query(|s| {
        sps_agents::reducer::AgentState::from_state(s).map(|a| a.agents.len()).unwrap_or(0)
    });
    assert_eq!(agent_count, 2, "FAIL: expected 2 agents, got {}", agent_count);
    println!("  PASS — AgentState has 2 agents");

    // 2. Dispatch task to Architect.
    runtime.dispatch(sps_agents::agent::AgentArchetype::Architect, "Design auth", "Plan JWT", 0, 0).unwrap();
    let dispatched = kernel.query(|s| {
        sps_agents::reducer::AgentState::from_state(s)
            .and_then(|a| a.agents.get(&arch_id.0).map(|r| r.tasks_dispatched.len()))
            .unwrap_or(0)
    });
    assert_eq!(dispatched, 1, "FAIL: expected 1 dispatched, got {}", dispatched);
    println!("  PASS — Architect has 1 dispatched task");

    // 3. Delegate Architect → Developer.
    runtime.delegate(arch_id, sps_agents::agent::AgentArchetype::Developer, "Implement auth", "Write JWT code", 0).unwrap();
    let (sent, received) = kernel.query(|s| {
        let a = sps_agents::reducer::AgentState::from_state(s).unwrap();
        let arch = a.agents.get(&arch_id.0).unwrap();
        let dev = a.agents.get(&dev_id.0).unwrap();
        (arch.delegations_sent.len(), dev.delegations_received.len())
    });
    assert_eq!(sent, 1, "FAIL: delegations_sent={}", sent);
    assert_eq!(received, 1, "FAIL: delegations_received={}", received);
    println!("  PASS — delegation graph: sent={}, received={}", sent, received);

    // 4. Send message Architect → Developer.
    let msg = sps_agents::messages::AgentMessage::new(
        arch_id, Some(dev_id),
        sps_agents::messages::MessageKind::Question,
        "How?", json!({}), 0,
    );
    runtime.send(msg);
    let (msg_sent, msg_received) = kernel.query(|s| {
        let a = sps_agents::reducer::AgentState::from_state(s).unwrap();
        let arch = a.agents.get(&arch_id.0).unwrap();
        let dev = a.agents.get(&dev_id.0).unwrap();
        (arch.messages_sent.len(), dev.messages_received.len())
    });
    assert_eq!(msg_sent, 1, "FAIL: messages_sent={}", msg_sent);
    assert_eq!(msg_received, 1, "FAIL: messages_received={}", msg_received);
    println!("  PASS — messages: sent={}, received={}", msg_sent, msg_received);

    // 5. Replay.
    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();
    let live_count = kernel.store().count().unwrap_or(0);

    let pipeline = make_pipeline();
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.last_hash(), live_hash, "FAIL: hash mismatch");
    let replayed_agents = sps_agents::reducer::AgentState::from_state(&replayed).unwrap();
    assert_eq!(replayed_agents.agents.len(), 2, "FAIL: agent count after replay");
    let r_arch = replayed_agents.agents.get(&arch_id.0).unwrap();
    assert_eq!(r_arch.tasks_dispatched.len(), 1, "FAIL: dispatched after replay");
    assert_eq!(r_arch.delegations_sent.len(), 1, "FAIL: delegations_sent after replay");
    assert_eq!(r_arch.messages_sent.len(), 1, "FAIL: messages_sent after replay");
    let r_dev = replayed_agents.agents.get(&dev_id.0).unwrap();
    assert_eq!(r_dev.delegations_received.len(), 1, "FAIL: delegations_received after replay");
    assert_eq!(r_dev.messages_received.len(), 1, "FAIL: messages_received after replay");
    println!("  PASS — replay identical (agents={}, hash match, all links preserved)", replayed_agents.agents.len());

    println!("\n  === AGENT SMOKE PASSED ===");
}

// ════════════════════════════════════════════════════════════════════════
// 2. AUTONOMY SMOKE TEST
// ════════════════════════════════════════════════════════════════════════

#[test]
fn autonomy_smoke_full_chain() {
    println!("\n=== AUTONOMY SMOKE: goal_activated + weekly_review → state + replay ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let goal_id = uuid::Uuid::now_v7();
    kernel.dispatch(RawEvent::new(
        "autonomous.goal_activated",
        json!({"goal_id": goal_id.to_string(), "milestones": [{"title": "M1"}], "activated_at": 1000}),
        Actor::owner(), 0,
    )).unwrap();
    kernel.dispatch(RawEvent::new(
        "autonomous.weekly_review",
        json!({"goal_id": goal_id.to_string(), "review": "On track", "reviewed_at": 2000}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Dispatched autonomous.goal_activated + autonomous.weekly_review");

    let (active, reviews) = kernel.query(|s| {
        sps_autonomy::reducer::AutonomyState::from_state(s)
            .map(|a| (a.active_goals.len(), a.reviews.len()))
            .unwrap_or((0, 0))
    });
    assert_eq!(active, 1, "FAIL: active_goals={}", active);
    assert_eq!(reviews, 1, "FAIL: reviews={}", reviews);
    println!("  PASS — active_goals={}, reviews={}", active, reviews);

    // Replay.
    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();
    let pipeline = make_pipeline();
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
    assert_eq!(replayed.last_hash(), live_hash, "FAIL: hash mismatch");

    let r_state = sps_autonomy::reducer::AutonomyState::from_state(&replayed).unwrap();
    assert_eq!(r_state.active_goals.len(), 1, "FAIL: active_goals after replay");
    assert_eq!(r_state.reviews.len(), 1, "FAIL: reviews after replay");
    println!("  PASS — replay identical (active={}, reviews={}, hash match)", r_state.active_goals.len(), r_state.reviews.len());

    println!("\n  === AUTONOMY SMOKE PASSED ===");
}

// ════════════════════════════════════════════════════════════════════════
// 3. FACTORY SMOKE TEST
// ════════════════════════════════════════════════════════════════════════

#[test]
fn factory_smoke_full_chain() {
    println!("\n=== FACTORY SMOKE: run_with_sink → FactoryState + WorldState + Execution + replay ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let request = sps_factory::workflow::ProjectRequest {
        description: "A simple CLI tool".to_string(),
        preferred_name: Some(SmolStr::new("test-cli")),
        output_dir: Some("/tmp/test".to_string()),
    };
    let result = sps_factory::workflow::FactoryWorkflow::run_with_sink(
        request, "/tmp/test", kernel.as_ref() as &dyn EventSink, None,
    ).unwrap();
    println!("  Factory run completed (run_id={}..., {} files)", &result.run_id.to_string()[..8], result.files.len());

    // Verify FactoryState.
    let factory_runs = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s).map(|f| f.runs.len()).unwrap_or(0)
    });
    assert_eq!(factory_runs, 1, "FAIL: factory_runs={}", factory_runs);
    println!("  PASS — FactoryState.runs == 1");

    // Verify WorldState.
    let (projects, files) = kernel.query(|s| {
        let ws = sps_world::reducer::WorldState::from_state(s).unwrap_or_default();
        (ws.graph.projects.len(), ws.graph.files.len())
    });
    assert_eq!(projects, 1, "FAIL: world projects={}", projects);
    assert!(files > 0, "FAIL: world files={}", files);
    println!("  PASS — WorldState: {} project(s), {} file(s)", projects, files);

    // Verify ExecutionState link.
    let execs = kernel.query(|s| {
        sps_execution::reducer::ExecutionState::from_state(s)
            .map(|es| es.for_factory_run(result.run_id).len())
            .unwrap_or(0)
    });
    assert_eq!(execs, 1, "FAIL: for_factory_run={}", execs);
    println!("  PASS — ExecutionState.for_factory_run == 1");

    // Replay.
    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();
    let pipeline = make_pipeline();
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
    assert_eq!(replayed.last_hash(), live_hash, "FAIL: hash mismatch");

    let r_factory = sps_factory::reducer::FactoryState::from_state(&replayed).unwrap();
    assert_eq!(r_factory.runs.len(), 1, "FAIL: factory runs after replay");
    let r_world = sps_world::reducer::WorldState::from_state(&replayed).unwrap();
    assert_eq!(r_world.graph.projects.len(), 1, "FAIL: world projects after replay");
    assert_eq!(r_world.graph.files.len(), files, "FAIL: world files after replay");
    let r_exec = sps_execution::reducer::ExecutionState::from_state(&replayed).unwrap();
    assert_eq!(r_exec.for_factory_run(result.run_id).len(), 1, "FAIL: exec link after replay");
    println!("  PASS — replay identical (factory={}, world={}+{}, exec={}, hash match)",
        r_factory.runs.len(), r_world.graph.projects.len(), r_world.graph.files.len(),
        r_exec.for_factory_run(result.run_id).len());

    println!("\n  === FACTORY SMOKE PASSED ===");
}

// ════════════════════════════════════════════════════════════════════════
// 4. H0 DRIFT TEST
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h0_drift_no_drift() {
    println!("\n=== H0 DRIFT: store.count == meta.event_count after all fixes ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Dispatch various event types (including autonomous.* which was the original drift source).
    for i in 0..10 {
        let record = sps_memory::memory::MemoryRecord {
            id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
            kind: sps_memory::memory::MemoryKind::Episodic,
            title: SmolStr::new(format!("mem-{}", i)),
            content: json!({}),
            tags: vec![],
            origin_tick: 0,
            created_at: 0,
        };
        kernel.dispatch(RawEvent::new("memory.created", serde_json::to_value(&record).unwrap(), Actor::owner(), 0)).unwrap();
    }
    for i in 0..5 {
        kernel.dispatch(RawEvent::new(
            "autonomous.goal_activated",
            json!({"goal_id": uuid::Uuid::now_v7().to_string(), "milestones": [], "activated_at": i}),
            Actor::owner(), 0,
        )).unwrap();
    }
    kernel.dispatch(RawEvent::new(
        "execution.succeeded",
        json!({"operation": "test", "plan_id": uuid::Uuid::now_v7().to_string()}),
        Actor::owner(), 0,
    )).unwrap();

    let store_count = kernel.store().count().unwrap_or(0);
    let meta_count = kernel.query(|s| s.event_count());

    println!("  Dispatched 16 events (10 memory + 5 autonomous + 1 execution)");
    println!("  store.count = {}", store_count);
    println!("  meta.event_count = {}", meta_count);

    assert_eq!(store_count, meta_count, "FAIL: drift! store={} meta={}", store_count, meta_count);
    assert_eq!(store_count, 16, "FAIL: expected 16, got {}", store_count);
    println!("  PASS — store.count == meta.event_count == 16 (no drift)");

    println!("\n  === H0 DRIFT PASSED ===");
}
