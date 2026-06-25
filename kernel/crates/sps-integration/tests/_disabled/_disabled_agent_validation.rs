//! Agent Validation Suite — 14/14 PASS required.
//!
//! Same methodology as Goals/Memory/Reflection/Planner/Execution.
//! Stop at first failure, document, fix, re-run from Checkpoint 1.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_agents::agent::{Agent, AgentArchetype, AgentId};
use sps_agents::messages::{AgentMessage, MessageKind};
use sps_agents::runtime::AgentRuntime;
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

fn make_runtime(kernel: &Arc<SpsKernel>) -> Arc<AgentRuntime> {
    let rt = Arc::new(AgentRuntime::with_sink(
        sps_agents::runtime::AgentRuntimeConfig::default(),
        kernel.clone(),
    ));
    rt
}

fn make_agent(archetype: AgentArchetype, name: &str) -> Arc<Agent> {
    Arc::new(Agent::new(archetype, SmolStr::new(name), "system prompt"))
}

fn agent_count(kernel: &SpsKernel) -> usize {
    kernel.query(|s| {
        sps_agents::reducer::AgentState::from_state(s)
            .map(|as_| as_.agents.len())
            .unwrap_or(0)
    })
}

// ─── Checkpoint 1 ─────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_1_activated_materialization() {
    println!("\n=== AGENT CHECKPOINT 1: agent.activated materializes AgentState ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);

    let architect = make_agent(AgentArchetype::Architect, "Architect");
    let id = runtime.register(architect);
    println!("  Registered Architect agent");

    let count = agent_count(&kernel);
    if count == 1 {
        let as_ = kernel.query(|s| sps_agents::reducer::AgentState::from_state(s).unwrap());
        let rec = as_.agents.get(&id.0).unwrap();
        println!("  PASS — 1 agent in AgentState, archetype={:?}, status={:?}", rec.agent.archetype, rec.status);
    } else {
        println!("  FAIL — expected 1, got {}", count);
        panic!("AGENT CHECKPOINT 1 FAILED");
    }
}

// ─── Checkpoint 2 ─────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_2_status_transitions() {
    println!("\n=== AGENT CHECKPOINT 2: agent.idle / blocked / deactivated ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);

    let agent = make_agent(AgentArchetype::Developer, "Dev");
    let id = runtime.register(agent);

    // idle
    kernel.dispatch(RawEvent::new("agent.idle", json!({"id": id.0.to_string()}), Actor::owner(), 0)).unwrap();
    let status = kernel.query(|s| {
        sps_agents::reducer::AgentState::from_state(s)
            .and_then(|as_| as_.agents.get(&id.0).map(|r| r.status))
            .unwrap_or(sps_agents::reducer::AgentStatus::Active)
    });
    assert_eq!(status, sps_agents::reducer::AgentStatus::Idle, "FAIL: not Idle");

    // blocked
    kernel.dispatch(RawEvent::new("agent.blocked", json!({"id": id.0.to_string()}), Actor::owner(), 0)).unwrap();
    let status = kernel.query(|s| {
        sps_agents::reducer::AgentState::from_state(s)
            .and_then(|as_| as_.agents.get(&id.0).map(|r| r.status))
            .unwrap_or(sps_agents::reducer::AgentStatus::Active)
    });
    assert_eq!(status, sps_agents::reducer::AgentStatus::Blocked, "FAIL: not Blocked");

    // deactivated
    kernel.dispatch(RawEvent::new("agent.deactivated", json!({"id": id.0.to_string()}), Actor::owner(), 0)).unwrap();
    let status = kernel.query(|s| {
        sps_agents::reducer::AgentState::from_state(s)
            .and_then(|as_| as_.agents.get(&id.0).map(|r| r.status))
            .unwrap_or(sps_agents::reducer::AgentStatus::Active)
    });
    if status == sps_agents::reducer::AgentStatus::Deactivated {
        println!("  PASS — transitions: Idle → Blocked → Deactivated");
    } else {
        println!("  FAIL — final status = {:?}", status);
        panic!("AGENT CHECKPOINT 2 FAILED");
    }
}

// ─── Checkpoint 3 ─────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_3_dispatched_tracking() {
    println!("\n=== AGENT CHECKPOINT 3: agent.dispatched tracks tasks_dispatched ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);

    let agent = make_agent(AgentArchetype::Tester, "Tester");
    let id = runtime.register(agent);

    runtime.dispatch(AgentArchetype::Tester, "Run tests", "Run the test suite", 0, 0).unwrap();
    runtime.dispatch(AgentArchetype::Tester, "Run lint", "Run the linter", 0, 0).unwrap();
    println!("  Dispatched 2 tasks to Tester");

    let count = kernel.query(|s| {
        sps_agents::reducer::AgentState::from_state(s)
            .and_then(|as_| as_.agents.get(&id.0).map(|r| r.tasks_dispatched.len()))
            .unwrap_or(0)
    });
    if count == 2 {
        println!("  PASS — Tester has {} dispatched tasks", count);
    } else {
        println!("  FAIL — expected 2, got {}", count);
        panic!("AGENT CHECKPOINT 3 FAILED");
    }
}

// ─── Checkpoint 4 ─────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_4_delegated_chain() {
    println!("\n=== AGENT CHECKPOINT 4: agent.delegated preserves from→to graph ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);

    let architect = make_agent(AgentArchetype::Architect, "Architect");
    let developer = make_agent(AgentArchetype::Developer, "Developer");
    let arch_id = runtime.register(architect);
    let dev_id = runtime.register(developer);

    runtime.delegate(arch_id, AgentArchetype::Developer, "Implement auth", "JWT + bcrypt", 0).unwrap();
    println!("  Architect delegated to Developer");

    let (sent, received) = kernel.query(|s| {
        let as_ = sps_agents::reducer::AgentState::from_state(s).unwrap();
        let arch = as_.agents.get(&arch_id.0).unwrap();
        let dev = as_.agents.get(&dev_id.0).unwrap();
        (arch.delegations_sent.len(), dev.delegations_received.len())
    });

    if sent == 1 && received == 1 {
        // Verify the delegation record has correct from/to.
        let as_ = kernel.query(|s| sps_agents::reducer::AgentState::from_state(s).unwrap());
        let del = as_.agents.get(&arch_id.0).unwrap().delegations_sent[0].clone();
        if del.from == arch_id.0 && del.to == dev_id.0 {
            println!("  PASS — delegation graph: from=Architect, to=Developer, title='{}'", del.title);
        } else {
            println!("  FAIL — delegation from={:?} to={:?} (expected Architect→Developer)", del.from, del.to);
            panic!("AGENT CHECKPOINT 4 FAILED");
        }
    } else {
        println!("  FAIL — sent={}, received={} (expected 1, 1)", sent, received);
        panic!("AGENT CHECKPOINT 4 FAILED");
    }
}

// ─── Checkpoint 5 ─────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_5_messages() {
    println!("\n=== AGENT CHECKPOINT 5: message_sent + message_received ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);

    let a = make_agent(AgentArchetype::Architect, "A");
    let b = make_agent(AgentArchetype::Developer, "B");
    let id_a = runtime.register(a);
    let id_b = runtime.register(b);

    let msg = AgentMessage::new(id_a, Some(id_b), MessageKind::Question, "How?", json!({}), 0);
    runtime.send(msg);
    println!("  Sent message A → B");

    let (sent, received) = kernel.query(|s| {
        let as_ = sps_agents::reducer::AgentState::from_state(s).unwrap();
        let a = as_.agents.get(&id_a.0).unwrap();
        let b = as_.agents.get(&id_b.0).unwrap();
        (a.messages_sent.len(), b.messages_received.len())
    });
    if sent == 1 && received == 1 {
        println!("  PASS — A sent {} message(s), B received {}", sent, received);
    } else {
        println!("  FAIL — sent={}, received={}", sent, received);
        panic!("AGENT CHECKPOINT 5 FAILED");
    }
}

// ─── Checkpoint 6 ─────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_6_replay_identity() {
    println!("\n=== AGENT CHECKPOINT 6: replay produces identical AgentState ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);

    let a = make_agent(AgentArchetype::Architect, "A");
    let b = make_agent(AgentArchetype::Developer, "B");
    let id_a = runtime.register(a);
    let id_b = runtime.register(b);
    runtime.dispatch(AgentArchetype::Architect, "task", "desc", 0, 0).unwrap();
    runtime.delegate(id_a, AgentArchetype::Developer, "sub", "subdesc", 0).unwrap();

    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_agents::reducer::AgentReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.last_hash(), live_hash, "FAIL: hash mismatch");

    let live_as = sps_agents::reducer::AgentState::from_state(&live).unwrap();
    let replayed_as = sps_agents::reducer::AgentState::from_state(&replayed).unwrap();
    assert_eq!(live_as.agents.len(), replayed_as.agents.len(), "FAIL: count mismatch");

    // Verify agent IDs match (determinism).
    let live_ids: std::collections::BTreeSet<_> = live_as.agents.keys().copied().collect();
    let replayed_ids: std::collections::BTreeSet<_> = replayed_as.agents.keys().copied().collect();
    if live_ids == replayed_ids {
        println!("  PASS — agent IDs deterministic, count matches ({} == {})", live_as.agents.len(), replayed_as.agents.len());
    } else {
        println!("  FAIL — agent IDs differ after replay");
        panic!("AGENT CHECKPOINT 6 FAILED");
    }
}

// ─── Checkpoint 7 ─────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_7_sqlite() {
    println!("\n=== AGENT CHECKPOINT 7: SQLite backend ===");
    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open_in_memory().unwrap()
    );
    let kernel = boot_kernel(storage.clone());
    assert_eq!(kernel.backend_name(), "sqlite");

    let runtime = make_runtime(&kernel);
    runtime.register_builtins();
    println!("  Registered 6 built-in agents via SQLite backend");

    assert_eq!(agent_count(&kernel), 6);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());
    assert_eq!(report.events_verified, 6);
    println!("  PASS — SQLite hash chain verified (6 events)");

    drop(kernel);
    drop(runtime);
    let kernel2 = boot_kernel(storage.clone());
    let count = agent_count(&kernel2);
    if count == 6 {
        println!("  PASS — after restart, 6 agents still present");
    } else {
        println!("  FAIL — after restart, got {}", count);
        panic!("AGENT CHECKPOINT 7 FAILED");
    }
}

// ─── Checkpoint 8 ─────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_8_crash_recovery() {
    println!("\n=== AGENT CHECKPOINT 8: crash recovery ===");
    let db_path = std::env::temp_dir().join(format!("sps_agent_crash_{}.db", uuid::Uuid::now_v7()));

    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel = boot_kernel(storage.clone());
        let runtime = make_runtime(&kernel);
        runtime.register_builtins();
        runtime.dispatch(AgentArchetype::Architect, "task1", "desc", 0, 0).unwrap();
        runtime.dispatch(AgentArchetype::Developer, "task2", "desc", 0, 0).unwrap();
        let count = agent_count(&kernel);
        println!("  Phase 1: {} agents + 2 dispatches", count);
        println!("  CRASH");
    }

    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel2 = boot_kernel(storage.clone());
        let count = agent_count(&kernel2);
        if count == 6 {
            println!("  Phase 2: PASS — reconstructed {} agents", count);
        } else {
            println!("  FAIL — expected 6, got {}", count);
            panic!("AGENT CHECKPOINT 8 FAILED");
        }

        // Verify dispatches were also recovered.
        let total_dispatched: usize = kernel2.query(|s| {
            let as_ = sps_agents::reducer::AgentState::from_state(s).unwrap();
            as_.agents.values().map(|r| r.tasks_dispatched.len()).sum()
        });
        if total_dispatched == 2 {
            println!("  PASS — {} dispatched tasks recovered", total_dispatched);
        } else {
            println!("  FAIL — expected 2 dispatched, got {}", total_dispatched);
            panic!("AGENT CHECKPOINT 8 FAILED");
        }
    }
    std::fs::remove_file(&db_path).ok();
}

// ─── Checkpoint 9 ─────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_9_large_corpus() {
    println!("\n=== AGENT CHECKPOINT 9: large corpus (100 agents + 1000 messages) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);

    const N_AGENTS: usize = 100;
    const N_MSGS: usize = 1000;

    let start = std::time::Instant::now();
    let mut ids = Vec::new();
    for i in 0..N_AGENTS {
        let archetype = AgentArchetype::all()[i % 6];
        let agent = make_agent(archetype, &format!("Agent-{}", i));
        ids.push(runtime.register(agent));
    }
    let reg_ms = start.elapsed().as_millis();
    println!("  Registered {} agents in {}ms", N_AGENTS, reg_ms);

    assert_eq!(agent_count(&kernel), N_AGENTS);

    // Send N_MSGS messages between random agents.
    let msg_start = std::time::Instant::now();
    for i in 0..N_MSGS {
        let from = ids[i % N_AGENTS];
        let to = ids[(i + 1) % N_AGENTS];
        let msg = AgentMessage::new(from, Some(to), MessageKind::StatusUpdate, &format!("msg-{}", i), json!({}), 0);
        runtime.send(msg);
    }
    let msg_ms = msg_start.elapsed().as_millis();
    println!("  Sent {} messages in {}ms", N_MSGS, msg_ms);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());
    // N_AGENTS activations + N_MSGS sent + N_MSGS received
    let expected = (N_AGENTS + N_MSGS * 2) as u64;
    assert_eq!(report.events_verified, expected);
    println!("  PASS — hash chain verified ({} events)", report.events_verified);

    // Replay.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_agents::reducer::AgentReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replay_start = std::time::Instant::now();
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
    let replay_ms = replay_start.elapsed().as_millis();
    let replayed_count = sps_agents::reducer::AgentState::from_state(&replayed)
        .map(|as_| as_.agents.len()).unwrap_or(0);
    if replayed_count == N_AGENTS {
        println!("  PASS — replayed {} agents in {}ms", N_AGENTS, replay_ms);
    } else {
        println!("  FAIL — replayed {} (expected {})", replayed_count, N_AGENTS);
        panic!("AGENT CHECKPOINT 9 FAILED");
    }
}

// ─── Checkpoint 10 ────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_10_multi_agent_isolation() {
    println!("\n=== AGENT CHECKPOINT 10: multi-agent isolation (no cross-contamination) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);

    let a = make_agent(AgentArchetype::Architect, "A");
    let b = make_agent(AgentArchetype::Developer, "B");
    let c = make_agent(AgentArchetype::Tester, "C");
    let id_a = runtime.register(a);
    let id_b = runtime.register(b);
    let id_c = runtime.register(c);
    println!("  Registered 3 agents: A (Architect), B (Developer), C (Tester)");

    // A dispatches 3 tasks.
    for _ in 0..3 {
        runtime.dispatch(AgentArchetype::Architect, "task", "desc", 0, 0).unwrap();
    }
    // B dispatches 2 tasks.
    for _ in 0..2 {
        runtime.dispatch(AgentArchetype::Developer, "task", "desc", 0, 0).unwrap();
    }
    // C dispatches 1 task.
    runtime.dispatch(AgentArchetype::Tester, "task", "desc", 0, 0).unwrap();

    // A sends 3 messages to B.
    for _ in 0..3 {
        let msg = AgentMessage::new(id_a, Some(id_b), MessageKind::Question, "q", json!({}), 0);
        runtime.send(msg);
    }
    // C sends 1 message to A.
    let msg = AgentMessage::new(id_c, Some(id_a), MessageKind::Answer, "ans", json!({}), 0);
    runtime.send(msg);
    println!("  A: 3 dispatches + 3 sent to B + 1 received from C");
    println!("  B: 2 dispatches + 3 received from A");
    println!("  C: 1 dispatch + 1 sent to A");

    // Verify isolation.
    let (a_dispatched, a_sent, a_received,
         b_dispatched, b_sent, b_received,
         c_dispatched, c_sent, c_received) = kernel.query(|s| {
        let as_ = sps_agents::reducer::AgentState::from_state(s).unwrap();
        let a = as_.agents.get(&id_a.0).unwrap();
        let b = as_.agents.get(&id_b.0).unwrap();
        let c = as_.agents.get(&id_c.0).unwrap();
        (
            a.tasks_dispatched.len(), a.messages_sent.len(), a.messages_received.len(),
            b.tasks_dispatched.len(), b.messages_sent.len(), b.messages_received.len(),
            c.tasks_dispatched.len(), c.messages_sent.len(), c.messages_received.len(),
        )
    });

    let expected = (3, 3, 1, 2, 0, 3, 1, 1, 0);
    let actual = (a_dispatched, a_sent, a_received, b_dispatched, b_sent, b_received, c_dispatched, c_sent, c_received);
    if actual == expected {
        println!("  PASS — isolation verified:");
        println!("    A: dispatched={}, sent={}, received={}", a_dispatched, a_sent, a_received);
        println!("    B: dispatched={}, sent={}, received={}", b_dispatched, b_sent, b_received);
        println!("    C: dispatched={}, sent={}, received={}", c_dispatched, c_sent, c_received);
    } else {
        println!("  FAIL — cross-contamination detected");
        println!("  Expected: {:?}", expected);
        println!("  Actual:   {:?}", actual);
        panic!("AGENT CHECKPOINT 10 FAILED — isolation broken");
    }
}

// ─── Checkpoint 11 ────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_11_execution_attribution() {
    println!("\n=== AGENT CHECKPOINT 11: execution attribution (agent_id ↔ execution) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);

    let dev = make_agent(AgentArchetype::Developer, "Dev");
    let dev_id = runtime.register(dev);

    // Dispatch 2 executions by the Developer agent.
    kernel.dispatch(RawEvent::new(
        "execution.succeeded",
        json!({"operation": "op1", "agent_id": dev_id.0.to_string(), "duration_ms": 100}),
        Actor::owner(), 0,
    )).unwrap();
    kernel.dispatch(RawEvent::new(
        "execution.failed",
        json!({"operation": "op2", "agent_id": dev_id.0.to_string(), "error": "err", "duration_ms": 50}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Dispatched 2 executions attributed to Developer");

    let dev_execs = kernel.query(|s| {
        let es = sps_execution::reducer::ExecutionState::from_state(s).unwrap();
        es.for_agent(dev_id.0).len()
    });
    if dev_execs == 2 {
        println!("  PASS — for_agent(Developer) returned {} executions", dev_execs);
    } else {
        println!("  FAIL — expected 2, got {}", dev_execs);
        panic!("AGENT CHECKPOINT 11 FAILED");
    }

    // Verify no other agent has executions.
    let other = make_agent(AgentArchetype::Architect, "Arch");
    let other_id = runtime.register(other);
    let other_execs = kernel.query(|s| {
        let es = sps_execution::reducer::ExecutionState::from_state(s).unwrap();
        es.for_agent(other_id.0).len()
    });
    if other_execs == 0 {
        println!("  PASS — Architect has 0 executions (attribution isolated)");
    } else {
        println!("  FAIL — Architect has {} executions (expected 0)", other_execs);
        panic!("AGENT CHECKPOINT 11 FAILED");
    }
}

// ─── Checkpoint 12 ────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_12_malformed_events() {
    println!("\n=== AGENT CHECKPOINT 12: malformed events rejected (validate-on-write) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);
    runtime.register(make_agent(AgentArchetype::Architect, "A"));
    println!("  Step 1: registered 1 valid agent");

    // Malformed agent.activated (missing "agent" field).
    let result = kernel.dispatch(RawEvent::new(
        "agent.activated",
        json!({"not_agent": "wrong"}),
        Actor::owner(), 0,
    ));
    if result.is_err() {
        println!("  Step 2: PASS — malformed agent.activated rejected");
    } else {
        println!("  FAIL — malformed event accepted");
        panic!("AGENT CHECKPOINT 12 FAILED");
    }

    // Malformed agent.idle (missing "id" field).
    let result2 = kernel.dispatch(RawEvent::new(
        "agent.idle",
        json!({"not_id": "wrong"}),
        Actor::owner(), 0,
    ));
    if result2.is_err() {
        println!("  Step 3: PASS — malformed agent.idle rejected");
    } else {
        println!("  FAIL — malformed agent.idle accepted");
        panic!("AGENT CHECKPOINT 12 FAILED");
    }

    assert_eq!(agent_count(&kernel), 1);
    println!("  Step 4: PASS — only 1 agent in state (malformed rejected)");

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert_eq!(report.events_verified, 1, "FAIL: expected 1 event, got {}", report.events_verified);
    println!("  Step 5: PASS — hash chain has 1 event (no malformed events)");
}

// ─── Checkpoint 13 ────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_13_deterministic_ids() {
    println!("\n=== AGENT CHECKPOINT 13: deterministic IDs across replay ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);

    let a = make_agent(AgentArchetype::Architect, "A");
    let b = make_agent(AgentArchetype::Developer, "B");
    let id_a = runtime.register(a);
    let id_b = runtime.register(b);

    // Agent IDs come from the event payload (Agent struct), so they're
    // deterministic — the reducer inserts the Agent as-is.
    let live_ids: std::collections::BTreeSet<uuid::Uuid> = kernel.query(|s| {
        sps_agents::reducer::AgentState::from_state(s)
            .map(|as_| as_.agents.keys().copied().collect())
            .unwrap_or_default()
    });

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_agents::reducer::AgentReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    let replayed_ids: std::collections::BTreeSet<uuid::Uuid> =
        sps_agents::reducer::AgentState::from_state(&replayed)
            .map(|as_| as_.agents.keys().copied().collect())
            .unwrap_or_default();

    if live_ids == replayed_ids {
        println!("  PASS — agent IDs deterministic across replay");
        println!("  Live:     {:?}", live_ids.iter().map(|i| i.to_string()[..8].to_string()).collect::<Vec<_>>());
        println!("  Replayed: {:?}", replayed_ids.iter().map(|i| i.to_string()[..8].to_string()).collect::<Vec<_>>());
    } else {
        println!("  FAIL — agent IDs differ");
        panic!("AGENT CHECKPOINT 13 FAILED");
    }
}

// ─── Checkpoint 14 ────────────────────────────────────────────────────────

#[test]
fn agent_checkpoint_14_delegation_graph_replay() {
    println!("\n=== AGENT CHECKPOINT 14: full delegation graph survives replay ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let runtime = make_runtime(&kernel);

    // Build a 3-agent delegation chain: Architect → Developer → Tester.
    let architect = make_agent(AgentArchetype::Architect, "Architect");
    let developer = make_agent(AgentArchetype::Developer, "Developer");
    let tester = make_agent(AgentArchetype::Tester, "Tester");
    let arch_id = runtime.register(architect);
    let dev_id = runtime.register(developer);
    let test_id = runtime.register(tester);

    runtime.delegate(arch_id, AgentArchetype::Developer, "Implement feature", "Build the auth module", 0).unwrap();
    runtime.delegate(dev_id, AgentArchetype::Tester, "Test feature", "Write tests for auth", 0).unwrap();
    println!("  Built delegation chain: Architect → Developer → Tester");

    // Capture live delegation graph.
    let live_graph = kernel.query(|s| {
        let as_ = sps_agents::reducer::AgentState::from_state(s).unwrap();
        let arch_sent: Vec<_> = as_.agents.get(&arch_id.0).unwrap().delegations_sent.iter()
            .map(|d| (d.from, d.to, d.title.as_str().to_string()))
            .collect();
        let dev_sent: Vec<_> = as_.agents.get(&dev_id.0).unwrap().delegations_sent.iter()
            .map(|d| (d.from, d.to, d.title.as_str().to_string()))
            .collect();
        let dev_received: Vec<_> = as_.agents.get(&dev_id.0).unwrap().delegations_received.iter()
            .map(|d| (d.from, d.to, d.title.as_str().to_string()))
            .collect();
        let test_received: Vec<_> = as_.agents.get(&test_id.0).unwrap().delegations_received.iter()
            .map(|d| (d.from, d.to, d.title.as_str().to_string()))
            .collect();
        (arch_sent, dev_sent, dev_received, test_received)
    });

    println!("  Live graph:");
    println!("    Architect sent:     {:?}", live_graph.0);
    println!("    Developer sent:     {:?}", live_graph.1);
    println!("    Developer received: {:?}", live_graph.2);
    println!("    Tester received:    {:?}", live_graph.3);

    // Replay.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_agents::reducer::AgentReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    let replayed_graph = sps_agents::reducer::AgentState::from_state(&replayed).map(|as_| {
        let arch_sent: Vec<_> = as_.agents.get(&arch_id.0).unwrap().delegations_sent.iter()
            .map(|d| (d.from, d.to, d.title.as_str().to_string()))
            .collect();
        let dev_sent: Vec<_> = as_.agents.get(&dev_id.0).unwrap().delegations_sent.iter()
            .map(|d| (d.from, d.to, d.title.as_str().to_string()))
            .collect();
        let dev_received: Vec<_> = as_.agents.get(&dev_id.0).unwrap().delegations_received.iter()
            .map(|d| (d.from, d.to, d.title.as_str().to_string()))
            .collect();
        let test_received: Vec<_> = as_.agents.get(&test_id.0).unwrap().delegations_received.iter()
            .map(|d| (d.from, d.to, d.title.as_str().to_string()))
            .collect();
        (arch_sent, dev_sent, dev_received, test_received)
    }).unwrap();

    if live_graph == replayed_graph {
        println!("  PASS — full delegation graph survived replay");
        println!("  Architect → Developer → Tester chain intact");
    } else {
        println!("  FAIL — delegation graph mismatch after replay");
        println!("  Replayed:");
        println!("    Architect sent:     {:?}", replayed_graph.0);
        println!("    Developer sent:     {:?}", replayed_graph.1);
        println!("    Developer received: {:?}", replayed_graph.2);
        println!("    Tester received:    {:?}", replayed_graph.3);
        panic!("AGENT CHECKPOINT 14 FAILED");
    }
}
