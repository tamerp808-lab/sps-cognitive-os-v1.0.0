//! End-to-end cognitive pipeline integration test.
//!
//! Exercises: command → goal → plan → task → effect → reflection →
//! learning → memory. Verifies that all phases compose correctly and
//! that the canonical state reflects every step.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_agents::agent::{AgentArchetype, AgentCapabilities};
use sps_agents::archetypes::Developer;
use sps_agents::runtime::AgentRuntime;
use sps_agents::reducer::AgentReducer;
use sps_bus::event_bus::EventBus;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::event_store::EventStore;
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::state::CanonicalState;
use sps_core::storage::port::StoragePort;
use sps_effects::effect::{EffectIntent, EffectType};
use sps_effects::executors::ShellExecutor;
use sps_effects::providers::adapters::StaticAdapter;
use sps_effects::providers::llm::{LlmProvider, LlmRequest, ProviderConfig};
use sps_effects::providers::registry::ProviderRegistry;
use sps_effects::registry::EffectRegistry;
use sps_effects::EffectManager;
use sps_goals::hierarchy::{Goal, GoalId, GoalStatus, Objective, Milestone, Task, TaskStatus};
use sps_goals::reducer::GoalReducer;
use sps_memory::memory::{MemoryId, MemoryKind, MemoryRecord};
use sps_memory::reducer::MemoryReducer;
use sps_planner::plan::PlanStatus;
use sps_planner::reducer::PlannerReducer;
use sps_planner::templates::builtin_templates;
use sps_reasoning::reducer::ReasoningReducer;
use sps_reflection::analyzers::{FailureAnalyzer, SuccessAnalyzer};
use sps_reflection::reducer::ReflectionReducer;
use sps_storage_memory::InMemoryStorage;
use sps_world::reducer::WorldReducer;

/// Build a full reducer pipeline with ALL phase reducers registered.
///
/// Note: KernelMetaReducer is invoked always-on by the pipeline
/// (Fix #16), so we don't register it explicitly here.
fn full_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    // Phase 2: bus (owner).
    sps_bus::state_ext::OwnerReducer::register(&mut reg);
    // Phase 3: memory.
    MemoryReducer::register(&mut reg);
    // Phase 4: world.
    WorldReducer::register(&mut reg);
    // Phase 5: reasoning.
    ReasoningReducer::register(&mut reg);
    // Phase 6: goals.
    GoalReducer::register(&mut reg);
    // Phase 7: planner.
    PlannerReducer::register(&mut reg);
    // Phase 9: reflection.
    ReflectionReducer::register(&mut reg);
    // Phase 13: agents.
    AgentReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

#[test]
fn full_cognitive_pipeline_end_to_end() {
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let store = Arc::new(EventStore::new(storage.clone()).unwrap());
    let pipeline = full_pipeline();

    // ---- Phase 0: system.booted ----
    let boot_event = store
        .append(RawEvent::new(
            "system.booted",
            json!({"schema_version": 1}),
            Actor::owner(),
            1_000,
        ))
        .unwrap();

    // ---- Phase 1: effect (shell.exec via EffectManager) ----
    let executors = Arc::new(EffectRegistry::new());
    executors.register(
        "shell.exec",
        Arc::new(ShellExecutor::new(std::path::PathBuf::from("/tmp"))),
    );
    let providers = Arc::new(ProviderRegistry::new());
    let manager = EffectManager::new(executors.clone(), providers.clone(), store.clone());
    let (intent, executed) = manager
        .dispatch(
            EffectType::ShellExec,
            json!({"command": "echo", "args": ["pipeline-test"]}),
            &Actor::owner(),
            2_000,
        )
        .unwrap();
    assert_eq!(executed.payload["output"]["success"], true);

    // ---- Phase 6: goal.created ----
    let goal = Goal {
        id: GoalId::new(),
        title: SmolStr::new("Test goal"),
        description: "integration test goal".into(),
        priority: 5,
        status: GoalStatus::Pending,
        objectives: vec![Objective {
            id: uuid::Uuid::now_v7(),
            title: SmolStr::new("obj1"),
            milestones: vec![Milestone {
                id: uuid::Uuid::now_v7(),
                title: SmolStr::new("mil1"),
                tasks: vec![Task {
                    id: uuid::Uuid::now_v7(),
                    title: SmolStr::new("task1"),
                    description: "do the thing".into(),
                    status: TaskStatus::Pending,
                    assigned_agent: None,
                    origin_tick: 0,
                }],
            }],
        }],
        dependencies: vec![],
        created_at: 3_000,
        origin_tick: 0,
    };
    let goal_id = goal.id;
    let task_id = goal.objectives[0].milestones[0].tasks[0].id;
    let goal_event = store
        .append(RawEvent::new(
            "goal.created",
            serde_json::to_value(&goal).unwrap(),
            Actor::owner(),
            3_000,
        ))
        .unwrap();

    // ---- Phase 7: plan.created (from generic workflow template) ----
    let template = builtin_templates()[0].clone();
    let plan = template.generate(goal_id, goal_event.tick, 4_000);
    let plan_event = store
        .append(RawEvent::new(
            "plan.created",
            serde_json::to_value(&plan).unwrap(),
            Actor::owner(),
            4_000,
        ))
        .unwrap();

    // ---- Phase 6: task.status_changed → completed ----
    let task_done_event = store
        .append(RawEvent::new(
            "task.status_changed",
            json!({"task_id": task_id.to_string(), "status": "completed"}),
            Actor::owner(),
            5_000,
        ))
        .unwrap();

    // ---- Phase 9: reflection.success_analyzed ----
    let analysis = SuccessAnalyzer::analyze(
        task_id,
        vec!["effect executed cleanly".into()],
        "shell output matched expectation".into(),
        true,
    );
    let reflection_event = store
        .append(RawEvent::new(
            "reflection.success_analyzed",
            serde_json::to_value(&analysis).unwrap(),
            Actor::owner(),
            6_000,
        ))
        .unwrap();

    // ---- Phase 3: memory.created (episodic memory of this run) ----
    let memory_record = MemoryRecord {
        id: MemoryId::new(),
        kind: MemoryKind::Episodic,
        title: SmolStr::new("Pipeline run"),
        content: json!({"goal_id": goal_id.to_string(), "task_id": task_id.to_string()}),
        tags: vec![SmolStr::new("integration"), SmolStr::new("success")],
        origin_tick: reflection_event.tick,
        created_at: 7_000,
    };
    let memory_event = store
        .append(RawEvent::new(
            "memory.created",
            serde_json::to_value(&memory_record).unwrap(),
            Actor::owner(),
            7_000,
        ))
        .unwrap();

    // ---- Verify the hash chain ----
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none(), "chain should be intact");
    assert_eq!(
        report.events_verified,
        8,
        "should have 8 events: boot + intent + executed + goal + plan + task_done + reflection + memory"
    );

    // ---- Replay from genesis and verify state matches ----
    let engine = ReplayEngine::new(pipeline.clone());
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    // Verify all phases left their mark on canonical state.
    assert_eq!(replayed.event_count(), 8);
    assert_eq!(replayed.last_tick(), memory_event.tick);
    assert_eq!(replayed.last_hash(), memory_event.hash);

    // Goals.
    let goal_state = sps_goals::reducer::GoalState::from_state(&replayed).unwrap();
    assert_eq!(goal_state.tree.goals.len(), 1);
    let replayed_goal = goal_state.tree.get(&goal_id).unwrap();
    let replayed_task = &replayed_goal.objectives[0].milestones[0].tasks[0];
    assert_eq!(replayed_task.status, TaskStatus::Completed);

    // Plans.
    let plan_state = sps_planner::reducer::PlannerState::from_state(&replayed).unwrap();
    assert_eq!(plan_state.plans.len(), 1);

    // Memory.
    let mem_state = sps_memory::reducer::MemoryState::from_state(&replayed).unwrap();
    assert_eq!(mem_state.graph.count(), 1);
    let mem = mem_state.graph.memories.values().next().unwrap();
    assert_eq!(mem.kind, MemoryKind::Episodic);

    // Reflection.
    let refl_state = sps_reflection::reducer::ReflectionState::from_state(&replayed).unwrap();
    assert_eq!(refl_state.reflections.len(), 1);
}

#[test]
fn agent_dispatches_and_delegates_correctly() {
    let runtime = AgentRuntime::default();
    let ids = runtime.register_builtins();
    assert_eq!(ids.len(), 6);

    // Architect delegates to Developer.
    let architect_id = ids[0];
    let result = runtime
        .delegate(
            architect_id,
            AgentArchetype::Developer,
            "implement auth module",
            "build the user authentication module with JWT",
            1,
        )
        .expect("delegation should succeed");

    assert_eq!(result.messages[0].kind, sps_agents::messages::MessageKind::Delegation);
    assert_eq!(result.messages[0].from, architect_id);
    // Developer agent should be the recipient.
    let dev = runtime.find_by_archetype(AgentArchetype::Developer).unwrap();
    assert_eq!(result.messages[0].to, Some(dev.id));
}

#[test]
fn llm_effect_with_static_provider_works_end_to_end() {
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let store = Arc::new(EventStore::new(storage.clone()).unwrap());

    let executors = Arc::new(EffectRegistry::new());
    let providers = Arc::new(ProviderRegistry::new());

    // Register a static provider.
    let static_provider = Arc::new(StaticAdapter::new("test-llm", "LLM response for test"));
    let config = ProviderConfig {
        id: "test-llm".into(),
        name: "Test LLM".into(),
        api_url: "http://localhost".into(),
        api_key: None,
        model_name: "test-model".into(),
        metadata: Default::default(),
    };
    providers.register(config, static_provider);

    let manager = EffectManager::new(executors, providers, store.clone());

    let request = LlmRequest {
        provider_id: "test-llm".into(),
        model: None,
        system: Some("You are a test assistant.".into()),
        user: "Say hello.".into(),
        max_tokens: Some(50),
        temperature: Some(0.7),
    };

    let (intent, executed) = manager
        .dispatch(
            EffectType::LlmComplete,
            serde_json::to_value(&request).unwrap(),
            &Actor::owner(),
            1_000,
        )
        .unwrap();

    assert_eq!(intent.event_type.as_str(), "effect.intent");
    assert_eq!(executed.event_type.as_str(), "effect.executed");
    assert_eq!(
        executed.payload["output"]["text"],
        "LLM response for test"
    );

    // Verify chain.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());
    assert_eq!(report.events_verified, 2);
}

#[test]
fn memory_search_finds_relevant_memories() {
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let store = Arc::new(EventStore::new(storage.clone()).unwrap());

    let mut reg = ReducerRegistry::new();
    MemoryReducer::register(&mut reg);
    // KernelMetaReducer runs always-on in the pipeline (Fix #16).
    let pipeline = Arc::new(ReducerPipeline::new(Arc::new(reg)));

    let mut state = CanonicalState::genesis();
    let memories = vec![
        ("Rust ownership model", MemoryKind::Semantic, "rust ownership borrow checker"),
        ("Python decorators", MemoryKind::Semantic, "python decorator syntax @"),
        ("Rust async programming", MemoryKind::Semantic, "rust async await tokio"),
    ];
    for (title, kind, content) in &memories {
        let record = MemoryRecord {
            id: MemoryId::new(),
            kind: *kind,
            title: SmolStr::new(*title),
            content: json!({"detail": content}),
            tags: vec![],
            origin_tick: 0,
            created_at: 0,
        };
        // Append via the store (takes RawEvent).
        let raw = RawEvent::new(
            "memory.created",
            serde_json::to_value(&record).unwrap(),
            Actor::owner(),
            0,
        );
        let event = store.append(raw).unwrap();
        pipeline.apply(&mut state, &event).unwrap();
    }

    let mem_state = sps_memory::reducer::MemoryState::from_state(&state).unwrap();
    let results = mem_state.graph.search("rust", 10);
    assert_eq!(results.len(), 2);
}

#[test]
fn reflection_failure_analyzer_classifies_root_causes() {
    let id = uuid::Uuid::now_v7();

    let a1 = FailureAnalyzer::analyze(id, "no provider available for LLM effect");
    assert_eq!(a1.root_cause, sps_reflection::analyzers::RootCause::ProviderIssue);

    let a2 = FailureAnalyzer::analyze(id, "operation timeout after 30s");
    assert_eq!(a2.root_cause, sps_reflection::analyzers::RootCause::Timeout);

    let a3 = FailureAnalyzer::analyze(id, "goal is ambiguous");
    assert_eq!(a3.root_cause, sps_reflection::analyzers::RootCause::Ambiguity);
}

#[test]
fn world_model_tracks_projects_and_files() {
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let store = Arc::new(EventStore::new(storage.clone()).unwrap());

    let mut reg = ReducerRegistry::new();
    WorldReducer::register(&mut reg);
    // KernelMetaReducer runs always-on in the pipeline (Fix #16).
    let pipeline = Arc::new(ReducerPipeline::new(Arc::new(reg)));

    let mut state = CanonicalState::genesis();

    let project = sps_world::entities::Project {
        id: sps_world::entities::ProjectId::new(),
        name: SmolStr::new("test-project"),
        path: SmolStr::new("/tmp/test"),
        tags: vec![],
        created_at: 0,
        origin_tick: 0,
    };
    let pid = project.id;
    let raw1 = RawEvent::new(
        "world.project_added",
        serde_json::to_value(&project).unwrap(),
        Actor::owner(),
        0,
    );
    let e1 = store.append(raw1).unwrap();
    pipeline.apply(&mut state, &e1).unwrap();

    let file = sps_world::entities::FileNode {
        id: sps_world::entities::FileId::new(),
        project_id: pid,
        path: SmolStr::new("src/main.rs"),
        content_hash: None,
        size: 100,
        origin_tick: 0,
    };
    let raw2 = RawEvent::new(
        "world.file_added",
        serde_json::to_value(&file).unwrap(),
        Actor::owner(),
        0,
    );
    let e2 = store.append(raw2).unwrap();
    pipeline.apply(&mut state, &e2).unwrap();

    let world = sps_world::reducer::WorldState::from_state(&state).unwrap();
    assert_eq!(world.graph.projects.len(), 1);
    assert_eq!(world.graph.files.len(), 1);
    let files = world.graph.files_in_project(&pid);
    assert_eq!(files.len(), 1);
}

#[test]
fn reasoning_dependency_solver_handles_complex_graphs() {
    use sps_reasoning::analyzers::DependencySolver;
    // Complex DAG:
    //   0 → 1 → 3
    //   0 → 2 → 3
    //   3 → 4
    let deps = vec![(0, 1), (0, 2), (1, 3), (2, 3), (3, 4)];
    let order = DependencySolver::solve(5, &deps).unwrap();
    assert_eq!(order.len(), 5);
    let pos: std::collections::HashMap<u32, usize> =
        order.iter().enumerate().map(|(i, &v)| (v, i)).collect();
    assert!(pos[&0] < pos[&1]);
    assert!(pos[&0] < pos[&2]);
    assert!(pos[&1] < pos[&3]);
    assert!(pos[&2] < pos[&3]);
    assert!(pos[&3] < pos[&4]);
}

#[test]
fn autonomy_sandbox_blocks_unauthorized_paths() {
    use sps_autonomy::sandbox::{AutonomySandbox, SandboxBoundary, SandboxViolation};
    use std::path::PathBuf;
    let sandbox = AutonomySandbox::with_boundary(SandboxBoundary::new(vec![
        PathBuf::from("/workspace"),
    ]));
    // Allowed.
    assert!(sandbox.check(&PathBuf::from("/workspace/src/main.rs")).is_ok());
    // Blocked.
    let result = sandbox.check(&PathBuf::from("/etc/passwd"));
    assert!(matches!(result, Err(SandboxViolation::OutsideBoundary { .. })));
}

#[test]
fn vector_search_finds_similar_memories() {
    use sps_vectors::embedding::hash_embedding;
    use sps_vectors::index::{VectorEntry, VectorIndex};
    let emb = hash_embedding(128);
    let index = VectorIndex::new();
    for text in &[
        "rust systems programming language",
        "rust ownership and borrowing",
        "python dynamic scripting",
        "machine learning with python",
    ] {
        let vector = emb.embed(text).unwrap();
        index.add(VectorEntry {
            id: uuid::Uuid::now_v7(),
            vector,
            text: Some(text.to_string()),
            metadata: json!({}),
        }).unwrap();
    }
    let query = emb.embed("rust programming").unwrap();
    let results = index.search(&query, 4);
    assert_eq!(results.len(), 4);
    // Top 2 should be rust-related.
    let mut rust_count = 0;
    for r in results.iter().take(2) {
        if let Some(entry) = index.get(&r.id) {
            if entry.text.as_ref().map(|t| t.contains("rust")).unwrap_or(false) {
                rust_count += 1;
            }
        }
    }
    assert!(rust_count >= 1, "expected at least 1 rust result in top 2");
}
