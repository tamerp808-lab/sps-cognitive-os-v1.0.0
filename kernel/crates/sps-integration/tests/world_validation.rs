//! World Model Validation Suite — 12/12 PASS required.
//!
//! World Model tracks all entities (projects, files, agents, tools, external
//! systems) and their relationships. If broken, the cognitive OS has no
//! "map" of the user's world.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::{Event, RawEvent};
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::reducer::ReducerRegistry;
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::storage::port::StoragePort;
use sps_world::entities::{EntityId, ExternalSystem, FileNode, Project, ToolDescriptor, AgentDescriptor};
use sps_world::graph::{WorldLinkKind, WorldRelationship};
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

fn make_project(name: &str) -> Project {
    Project {
        id: EntityId::new(),
        name: SmolStr::new(name),
        path: SmolStr::new("/tmp/test"),
        tags: vec![],
        created_at: 0,
        origin_tick: 0,
    }
}

fn make_file(project_id: EntityId, path: &str) -> FileNode {
    FileNode {
        id: EntityId::new(),
        project_id,
        path: SmolStr::new(path),
        content_hash: None,
        size: 100,
        origin_tick: 0,
    }
}

fn dispatch_project(kernel: &SpsKernel, project: &Project) -> Event {
    let payload = serde_json::to_value(project).unwrap();
    kernel.dispatch(RawEvent::new("world.project_added", payload, Actor::owner(), 0)).unwrap()
}

fn dispatch_file(kernel: &SpsKernel, file: &FileNode) -> Event {
    let payload = serde_json::to_value(file).unwrap();
    kernel.dispatch(RawEvent::new("world.file_added", payload, Actor::owner(), 0)).unwrap()
}

fn dispatch_relationship(kernel: &SpsKernel, rel: &WorldRelationship) -> Event {
    let payload = serde_json::to_value(rel).unwrap();
    kernel.dispatch(RawEvent::new("world.relationship_added", payload, Actor::owner(), 0)).unwrap()
}

fn world_counts(kernel: &SpsKernel) -> (usize, usize, usize, usize, usize, usize) {
    kernel.query(|s| {
        let ws = sps_world::reducer::WorldState::from_state(s).unwrap_or_default();
        (
            ws.graph.projects.len(),
            ws.graph.files.len(),
            ws.graph.agents.len(),
            ws.graph.tools.len(),
            ws.graph.external_systems.len(),
            ws.graph.relationships.len(),
        )
    })
}

// ─── Checkpoint 1 ─────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_1_project_added() {
    println!("\n=== WORLD CHECKPOINT 1: world.project_added materializes WorldState ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let p = make_project("test-project");
    dispatch_project(&kernel, &p);
    println!("  Dispatched world.project_added");

    let (projects, _, _, _, _, _) = world_counts(&kernel);
    if projects == 1 {
        println!("  PASS — 1 project in WorldState");
    } else {
        println!("  FAIL — expected 1 project, got {}", projects);
        panic!("WORLD CHECKPOINT 1 FAILED");
    }
}

// ─── Checkpoint 2 ─────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_2_file_added_and_project_link() {
    println!("\n=== WORLD CHECKPOINT 2: file_added + files_in_project query ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let p = make_project("proj");
    let pid = p.id;
    dispatch_project(&kernel, &p);

    let f1 = make_file(pid, "src/main.rs");
    let f2 = make_file(pid, "src/lib.rs");
    dispatch_file(&kernel, &f1);
    dispatch_file(&kernel, &f2);
    println!("  Added 1 project + 2 files");

    let (_, files, _, _, _, _) = world_counts(&kernel);
    assert_eq!(files, 2, "FAIL: expected 2 files, got {}", files);

    let files_in_proj = kernel.query(|s| {
        sps_world::reducer::WorldState::from_state(s)
            .map(|ws| ws.graph.files_in_project(&pid).len())
            .unwrap_or(0)
    });
    if files_in_proj == 2 {
        println!("  PASS — 2 files linked to project via files_in_project()");
    } else {
        println!("  FAIL — files_in_project returned {}", files_in_proj);
        panic!("WORLD CHECKPOINT 2 FAILED");
    }
}

// ─── Checkpoint 3 ─────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_3_relationship_added() {
    println!("\n=== WORLD CHECKPOINT 3: relationship_added creates graph edge ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let p1 = make_project("P1");
    let p2 = make_project("P2");
    let p1_id = p1.id;
    let p2_id = p2.id;
    dispatch_project(&kernel, &p1);
    dispatch_project(&kernel, &p2);

    let rel = WorldRelationship {
        id: uuid::Uuid::now_v7(),
        from: p1_id,
        to: p2_id,
        kind: WorldLinkKind::Related,
    };
    dispatch_relationship(&kernel, &rel);
    println!("  Added 2 projects + 1 relationship (P1 → P2)");

    let (_, _, _, _, _, rels) = world_counts(&kernel);
    if rels == 1 {
        println!("  PASS — 1 relationship in WorldState");
    } else {
        println!("  FAIL — expected 1 relationship, got {}", rels);
        panic!("WORLD CHECKPOINT 3 FAILED");
    }
}

// ─── Checkpoint 4 ─────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_4_entity_removed() {
    println!("\n=== WORLD CHECKPOINT 4: world.entity_removed deletes entity ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let p = make_project("to-delete");
    let pid = p.id;
    dispatch_project(&kernel, &p);

    kernel.dispatch(RawEvent::new(
        "world.entity_removed",
        json!({"id": pid.0.to_string()}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Dispatched world.entity_removed");

    let (projects, _, _, _, _, _) = world_counts(&kernel);
    if projects == 0 {
        println!("  PASS — project removed from WorldState");
    } else {
        println!("  FAIL — expected 0 projects, got {}", projects);
        panic!("WORLD CHECKPOINT 4 FAILED");
    }
}

// ─── Checkpoint 5 ─────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_5_replay() {
    println!("\n=== WORLD CHECKPOINT 5: replay produces identical WorldState ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let p = make_project("replay-test");
    let pid = p.id;
    dispatch_project(&kernel, &p);
    dispatch_file(&kernel, &make_file(pid, "main.rs"));

    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_world::reducer::WorldReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.last_hash(), live_hash, "FAIL: hash mismatch");

    let live_ws = sps_world::reducer::WorldState::from_state(&live).unwrap_or_default();
    let replayed_ws = sps_world::reducer::WorldState::from_state(&replayed).unwrap_or_default();

    if live_ws.graph.projects.len() == replayed_ws.graph.projects.len()
        && live_ws.graph.files.len() == replayed_ws.graph.files.len() {
        println!("  PASS — projects ({}) and files ({}) match after replay",
            live_ws.graph.projects.len(), live_ws.graph.files.len());
    } else {
        println!("  FAIL — mismatch: live ({}, {}), replayed ({}, {})",
            live_ws.graph.projects.len(), live_ws.graph.files.len(),
            replayed_ws.graph.projects.len(), replayed_ws.graph.files.len());
        panic!("WORLD CHECKPOINT 5 FAILED");
    }
}

// ─── Checkpoint 6 ─────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_6_sqlite() {
    println!("\n=== WORLD CHECKPOINT 6: SQLite backend ===");
    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open_in_memory().unwrap()
    );
    let kernel = boot_kernel(storage.clone());
    assert_eq!(kernel.backend_name(), "sqlite");

    dispatch_project(&kernel, &make_project("sqlite-proj"));
    println!("  Created project via SQLite");

    let (projects, _, _, _, _, _) = world_counts(&kernel);
    assert_eq!(projects, 1);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());

    drop(kernel);
    let kernel2 = boot_kernel(storage.clone());
    let (projects_after, _, _, _, _, _) = world_counts(&kernel2);
    if projects_after == 1 {
        println!("  PASS — after restart, 1 project still present");
    } else {
        println!("  FAIL — after restart, got {}", projects_after);
        panic!("WORLD CHECKPOINT 6 FAILED");
    }
}

// ─── Checkpoint 7 ─────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_7_crash_recovery() {
    println!("\n=== WORLD CHECKPOINT 7: crash recovery ===");
    let db_path = std::env::temp_dir().join(format!("sps_world_crash_{}.db", uuid::Uuid::now_v7()));

    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel = boot_kernel(storage.clone());
        for i in 0..10 {
            dispatch_project(&kernel, &make_project(&format!("P{}", i)));
        }
        println!("  Phase 1: created 10 projects");
        println!("  CRASH");
    }

    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel2 = boot_kernel(storage.clone());
        let (projects, _, _, _, _, _) = world_counts(&kernel2);
        if projects == 10 {
            println!("  Phase 2: PASS — reconstructed {} projects", projects);
        } else {
            println!("  FAIL — expected 10, got {}", projects);
            panic!("WORLD CHECKPOINT 7 FAILED");
        }
    }
    std::fs::remove_file(&db_path).ok();
}

// ─── Checkpoint 8 ─────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_8_large_graph() {
    println!("\n=== WORLD CHECKPOINT 8: large graph (100 projects + 1000 files) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    const N_PROJ: usize = 100;
    const N_FILES_PER: usize = 10;
    let start = std::time::Instant::now();
    for i in 0..N_PROJ {
        let p = make_project(&format!("P{}", i));
        let pid = p.id;
        dispatch_project(&kernel, &p);
        for j in 0..N_FILES_PER {
            dispatch_file(&kernel, &make_file(pid, &format!("file_{}.rs", j)));
        }
    }
    let dispatch_ms = start.elapsed().as_millis();
    println!("  Created {} projects + {} files in {}ms",
        N_PROJ, N_PROJ * N_FILES_PER, dispatch_ms);

    let (projects, files, _, _, _, _) = world_counts(&kernel);
    assert_eq!(projects, N_PROJ);
    assert_eq!(files, N_PROJ * N_FILES_PER);
    println!("  PASS — {} projects + {} files in WorldState", projects, files);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());
    let expected_events = (N_PROJ + N_PROJ * N_FILES_PER) as u64;
    assert_eq!(report.events_verified, expected_events);
    println!("  PASS — hash chain verified ({} events)", report.events_verified);

    // Replay.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_world::reducer::WorldReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replay_start = std::time::Instant::now();
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
    let replay_ms = replay_start.elapsed().as_millis();
    let replayed_ws = sps_world::reducer::WorldState::from_state(&replayed).unwrap_or_default();
    if replayed_ws.graph.projects.len() == N_PROJ && replayed_ws.graph.files.len() == N_PROJ * N_FILES_PER {
        println!("  PASS — replayed in {}ms ({} projects + {} files)",
            replay_ms, replayed_ws.graph.projects.len(), replayed_ws.graph.files.len());
    } else {
        println!("  FAIL — replay mismatch");
        panic!("WORLD CHECKPOINT 8 FAILED");
    }
}

// ─── Checkpoint 9 ─────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_9_all_entity_types() {
    println!("\n=== WORLD CHECKPOINT 9: all entity types (project, file, agent, tool, external) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let p = make_project("proj");
    dispatch_project(&kernel, &p);
    dispatch_file(&kernel, &make_file(p.id, "main.rs"));

    let agent = AgentDescriptor {
        id: EntityId::new(),
        archetype: SmolStr::new("developer"),
        name: SmolStr::new("DevAgent"),
        origin_tick: 0,
    };
    let agent_payload = serde_json::to_value(&agent).unwrap();
    kernel.dispatch(RawEvent::new("world.agent_added", agent_payload, Actor::owner(), 0)).unwrap();

    let tool = ToolDescriptor {
        id: EntityId::new(),
        name: SmolStr::new("cargo"),
        version: Some(SmolStr::new("1.0")),
        origin_tick: 0,
    };
    let tool_payload = serde_json::to_value(&tool).unwrap();
    kernel.dispatch(RawEvent::new("world.tool_added", tool_payload, Actor::owner(), 0)).unwrap();

    let ext = ExternalSystem {
        id: EntityId::new(),
        name: SmolStr::new("GitHub"),
        kind: SmolStr::new("git_remote"),
        endpoint: "https://github.com".to_string(),
        origin_tick: 0,
    };
    let ext_payload = serde_json::to_value(&ext).unwrap();
    kernel.dispatch(RawEvent::new("world.external_system_added", ext_payload, Actor::owner(), 0)).unwrap();
    println!("  Added 1 of each entity type");

    let (projects, files, agents, tools, external, _) = world_counts(&kernel);
    if projects == 1 && files == 1 && agents == 1 && tools == 1 && external == 1 {
        println!("  PASS — all entity types present: projects={}, files={}, agents={}, tools={}, external={}",
            projects, files, agents, tools, external);
    } else {
        println!("  FAIL — counts: ({}, {}, {}, {}, {})", projects, files, agents, tools, external);
        panic!("WORLD CHECKPOINT 9 FAILED");
    }
}

// ─── Checkpoint 10 ────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_10_file_updated() {
    println!("\n=== WORLD CHECKPOINT 10: file_updated overwrites existing file ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let p = make_project("proj");
    let pid = p.id;
    dispatch_project(&kernel, &p);

    let mut f = make_file(pid, "main.rs");
    f.size = 100;
    let fid = f.id;
    dispatch_file(&kernel, &f);

    // Update the file with new size.
    let f2 = FileNode {
        id: fid,
        project_id: pid,
        path: SmolStr::new("main.rs"),
        content_hash: Some("abc123".to_string()),
        size: 200,
        origin_tick: 0,
    };
    let f2_payload = serde_json::to_value(&f2).unwrap();
    kernel.dispatch(RawEvent::new("world.file_updated", f2_payload, Actor::owner(), 0)).unwrap();
    println!("  Added file (size=100), then updated (size=200, hash=abc123)");

    let (size, hash, file_count) = kernel.query(|s| {
        let ws = sps_world::reducer::WorldState::from_state(s).unwrap();
        let f = ws.graph.files.get(&fid.0).unwrap();
        (f.size, f.content_hash.clone(), ws.graph.files.len())
    });

    if size == 200 && hash.as_deref() == Some("abc123") && file_count == 1 {
        println!("  PASS — file updated: size={}, hash={:?}, count={} (no duplicate)",
            size, hash, file_count);
    } else {
        println!("  FAIL — size={}, hash={:?}, count={} (expected 200, Some(abc123), 1)",
            size, hash, file_count);
        panic!("WORLD CHECKPOINT 10 FAILED");
    }
}

// ─── Checkpoint 11 ────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_11_malformed_payload_rejected() {
    println!("\n=== WORLD CHECKPOINT 11: malformed payload rejected ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    dispatch_project(&kernel, &make_project("valid"));
    println!("  Step 1: dispatched 1 valid project");

    let result = kernel.dispatch(RawEvent::new(
        "world.project_added",
        json!({"not_a_project": true}),
        Actor::owner(), 0,
    ));
    if result.is_err() {
        println!("  Step 2: PASS — malformed project rejected");
    } else {
        println!("  FAIL — malformed accepted");
        panic!("WORLD CHECKPOINT 11 FAILED");
    }

    let (projects, _, _, _, _, _) = world_counts(&kernel);
    assert_eq!(projects, 1, "FAIL: malformed leaked into state");

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert_eq!(report.events_verified, 1, "FAIL: malformed in chain");
    println!("  PASS — only 1 event in chain (validate-on-write works)");
}

// ─── Checkpoint 12 ────────────────────────────────────────────────────────

#[test]
fn world_checkpoint_12_deterministic_relationship_ids() {
    println!("\n=== WORLD CHECKPOINT 12: deterministic relationship IDs across replay ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let p1 = make_project("P1");
    let p2 = make_project("P2");
    dispatch_project(&kernel, &p1);
    dispatch_project(&kernel, &p2);
    let rel = WorldRelationship {
        id: uuid::Uuid::now_v7(),
        from: p1.id,
        to: p2.id,
        kind: WorldLinkKind::Related,
    };
    dispatch_relationship(&kernel, &rel);
    println!("  Created 2 projects + 1 relationship");

    let live_ids: std::collections::BTreeSet<uuid::Uuid> = kernel.query(|s| {
        sps_world::reducer::WorldState::from_state(s)
            .map(|ws| ws.graph.relationships.keys().copied().collect())
            .unwrap_or_default()
    });

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_world::reducer::WorldReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    let replayed_ids: std::collections::BTreeSet<uuid::Uuid> =
        sps_world::reducer::WorldState::from_state(&replayed)
            .map(|ws| ws.graph.relationships.keys().copied().collect())
            .unwrap_or_default();

    if live_ids == replayed_ids {
        println!("  PASS — relationship IDs deterministic across replay");
    } else {
        println!("  FAIL — relationship IDs differ after replay");
        println!("  Live:     {:?}", live_ids.iter().map(|i| i.to_string()[..8].to_string()).collect::<Vec<_>>());
        println!("  Replayed: {:?}", replayed_ids.iter().map(|i| i.to_string()[..8].to_string()).collect::<Vec<_>>());
        println!("  ─────────────────────────────────────────────────");
        println!("  ROOT CAUSE: add_relationship uses Uuid::now_v7() — same bug class");
        println!("  as MemoryLink (Fix #3) and reflection IDs (Fix #5).");
        println!("  SEVERITY: HIGH — non-deterministic relationship IDs break");
        println!("             any code that references relationships by ID after replay.");
        println!("  ─────────────────────────────────────────────────");
        panic!("WORLD CHECKPOINT 12 FAILED — relationship IDs non-deterministic");
    }
}
