//! Memory Validation Suite — 12/12 PASS required.
//!
//! Same methodology as the Goal Validation Suite:
//! - Stop at first failure
//! - Document expected/observed/root-cause/severity
//! - Fix one issue at a time, re-run from Checkpoint 1
//!
//! Memory subsystem claims:
//! - 4 types: Episodic / Semantic / Procedural / Conceptual
//! - Created / Accessed / Promoted / Linked / Unlinked / Consolidated / Decayed / Removed
//! - Search by keyword (title/tags/content)
//! - Replay-deterministic

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::{Event, RawEvent};
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::reducer::ReducerRegistry;
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::storage::port::StoragePort;
use sps_memory::memory::{MemoryId, MemoryKind, MemoryRecord, MemoryStrength};
use sps_storage_memory::InMemoryStorage;

// ─── Helpers ──────────────────────────────────────────────────────────────

fn boot_kernel(storage: Arc<dyn StoragePort>) -> Arc<SpsKernel> {
    let kernel = SpsKernel::boot_with(storage, KernelConfig::default(), |reg| {
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

fn make_memory(kind: MemoryKind, title: &str, content: &str, tags: &[&str]) -> MemoryRecord {
    MemoryRecord {
        id: MemoryId(uuid::Uuid::now_v7()),
        kind,
        title: SmolStr::new(title),
        content: json!({"detail": content}),
        tags: tags.iter().map(|t| SmolStr::new(*t)).collect(),
        origin_tick: 0,
        created_at: 0,
    }
}

fn dispatch_memory_created(kernel: &SpsKernel, record: &MemoryRecord) -> Event {
    let payload = serde_json::to_value(record).unwrap();
    kernel.dispatch(RawEvent::new("memory.created", payload, Actor::owner(), 0))
        .expect("dispatch memory.created")
}

// ─── Checkpoint 1 ─────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_1_created() {
    println!("\n=== MEM CHECKPOINT 1: memory.created updates state ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let record = make_memory(MemoryKind::Episodic, "First memory", "hello world", &["test"]);
    let event = dispatch_memory_created(&kernel, &record);

    println!("  Dispatched memory.created at tick {}", event.tick);

    let mem_state = kernel.query(|s| sps_memory::reducer::MemoryState::from_state(s));
    match mem_state {
        Some(ms) => {
            let count = ms.graph.count();
            if count == 1 {
                let m = ms.graph.memories.values().next().unwrap();
                println!("  PASS — 1 memory in state, title='{}', kind={:?}", m.title, m.kind);
            } else {
                println!("  FAIL — expected 1 memory, found {}", count);
                panic!("MEM CHECKPOINT 1 FAILED");
            }
        }
        None => {
            println!("  FAIL — MemoryState not in canonical state");
            panic!("MEM CHECKPOINT 1 FAILED");
        }
    }
}

// ─── Checkpoint 2 ─────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_2_retrieved() {
    println!("\n=== MEM CHECKPOINT 2: memory.accessed increments access_count ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let record = make_memory(MemoryKind::Semantic, "Rust ownership", "borrow checker", &["rust"]);
    let _created = dispatch_memory_created(&kernel, &record);
    let mem_id = record.id;

    // Initial access_count should be 0.
    let initial_count = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .and_then(|ms| ms.graph.get(&mem_id).map(|m| m.access_count))
            .unwrap_or(0)
    });
    assert_eq!(initial_count, 0, "FAIL: initial access_count != 0");
    println!("  Initial access_count: 0");

    // Dispatch memory.accessed.
    kernel.dispatch(RawEvent::new(
        "memory.accessed",
        json!({"id": mem_id.0.to_string(), "at": 12345}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Dispatched memory.accessed");

    let after_count = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .and_then(|ms| ms.graph.get(&mem_id).map(|m| m.access_count))
            .unwrap_or(0)
    });
    if after_count == 1 {
        println!("  PASS — access_count incremented to 1");
    } else {
        println!("  FAIL — expected access_count=1, got {}", after_count);
        panic!("MEM CHECKPOINT 2 FAILED");
    }

    // Access again, should be 2.
    kernel.dispatch(RawEvent::new(
        "memory.accessed",
        json!({"id": mem_id.0.to_string(), "at": 12346}),
        Actor::owner(), 0,
    )).unwrap();
    let after2 = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .and_then(|ms| ms.graph.get(&mem_id).map(|m| m.access_count))
            .unwrap_or(0)
    });
    assert_eq!(after2, 2, "FAIL: access_count after 2nd access != 2");
    println!("  PASS — access_count after 2nd access: 2");
}

// ─── Checkpoint 3 ─────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_3_promotion() {
    println!("\n=== MEM CHECKPOINT 3: memory.promoted changes kind + resets strength ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Create episodic memory with strength = 0.3 (decayed).
    let mut record = make_memory(MemoryKind::Episodic, "Yesterday's build", "build failed", &[]);
    let _created = dispatch_memory_created(&kernel, &record);
    let mem_id = record.id;

    // Decay it manually via memory.decayed.
    kernel.dispatch(RawEvent::new(
        "memory.decayed",
        json!({"factor": 0.3, "kind": "episodic"}),
        Actor::owner(), 0,
    )).unwrap();

    let strength_before = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .and_then(|ms| ms.graph.get(&mem_id).map(|m| m.strength.0))
            .unwrap_or(0.0)
    });
    println!("  After decay: strength = {:.3}", strength_before);
    assert!(strength_before < 1.0, "FAIL: decay didn't reduce strength");

    // Promote to semantic.
    kernel.dispatch(RawEvent::new(
        "memory.promoted",
        json!({"id": mem_id.0.to_string(), "new_kind": "semantic"}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Dispatched memory.promoted (episodic → semantic)");

    let (kind_after, strength_after) = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .and_then(|ms| ms.graph.get(&mem_id).map(|m| (m.kind, m.strength.0)))
            .unwrap_or((MemoryKind::Episodic, 0.0))
    });
    if kind_after == MemoryKind::Semantic && (strength_after - 1.0).abs() < 0.01 {
        println!("  PASS — kind=Semantic, strength=1.0 (reset on promotion)");
    } else {
        println!("  FAIL — kind={:?}, strength={:.3}", kind_after, strength_after);
        println!("  EXPECTED: kind=Semantic, strength=1.0 (reset on promotion)");
        panic!("MEM CHECKPOINT 3 FAILED");
    }

    // Suppress unused warning.
    let _ = record.id;
}

// ─── Checkpoint 4 ─────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_4_decay() {
    println!("\n=== MEM CHECKPOINT 4: memory.decayed reduces strength ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Create 3 memories: 2 episodic + 1 semantic.
    let e1 = make_memory(MemoryKind::Episodic, "Episodic 1", "event 1", &[]);
    let e2 = make_memory(MemoryKind::Episodic, "Episodic 2", "event 2", &[]);
    let s1 = make_memory(MemoryKind::Semantic, "Semantic 1", "fact 1", &[]);
    dispatch_memory_created(&kernel, &e1);
    dispatch_memory_created(&kernel, &e2);
    dispatch_memory_created(&kernel, &s1);
    println!("  Created 2 episodic + 1 semantic (all strength=1.0)");

    // Apply decay only to episodic memories.
    kernel.dispatch(RawEvent::new(
        "memory.decayed",
        json!({"factor": 0.5, "kind": "episodic"}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Dispatched memory.decayed (factor=0.5, kind=episodic)");

    // Verify episodic memories decayed, semantic unchanged.
    let (e1_str, e2_str, s1_str) = kernel.query(|s| {
        let ms = sps_memory::reducer::MemoryState::from_state(s).unwrap();
        let e1 = ms.graph.get(&e1.id).map(|m| m.strength.0).unwrap_or(0.0);
        let e2 = ms.graph.get(&e2.id).map(|m| m.strength.0).unwrap_or(0.0);
        let s1 = ms.graph.get(&s1.id).map(|m| m.strength.0).unwrap_or(0.0);
        (e1, e2, s1)
    });

    if (e1_str - 0.5).abs() < 0.01 && (e2_str - 0.5).abs() < 0.01 && (s1_str - 1.0).abs() < 0.01 {
        println!("  PASS — episodic: {:.2}, {:.2}; semantic: {:.2} (untouched)", e1_str, e2_str, s1_str);
    } else {
        println!("  FAIL — e1={:.3}, e2={:.3}, s1={:.3}", e1_str, e2_str, s1_str);
        println!("  EXPECTED: e1=0.5, e2=0.5, s1=1.0 (kind filter works)");
        panic!("MEM CHECKPOINT 4 FAILED");
    }
}

// ─── Checkpoint 5 ─────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_5_search() {
    println!("\n=== MEM CHECKPOINT 5: search by keyword finds memories ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Create varied memories.
    let memories = vec![
        make_memory(MemoryKind::Semantic, "Rust ownership model", "borrow checker rules", &["rust", "memory"]),
        make_memory(MemoryKind::Semantic, "Python decorators", "@decorator syntax", &["python"]),
        make_memory(MemoryKind::Episodic, "Rust async debugging", "tokio runtime issue", &["rust", "async"]),
        make_memory(MemoryKind::Procedural, "How to bake bread", "knead for 10 minutes", &["cooking"]),
    ];
    for m in &memories {
        dispatch_memory_created(&kernel, m);
    }
    println!("  Created 4 memories (2 rust, 1 python, 1 cooking)");

    // Search for "rust" — should find 2.
    let rust_results = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| ms.graph.search("rust", 10).len())
            .unwrap_or(0)
    });
    if rust_results == 2 {
        println!("  PASS — search('rust') returned {} results", rust_results);
    } else {
        println!("  FAIL — search('rust') returned {}, expected 2", rust_results);
        panic!("MEM CHECKPOINT 5 FAILED");
    }

    // Search by content "tokio" — should find 1.
    let tokio_results = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| ms.graph.search("tokio", 10).len())
            .unwrap_or(0)
    });
    assert_eq!(tokio_results, 1, "FAIL: search('tokio') != 1");
    println!("  PASS — search('tokio') returned 1 result (content match)");

    // Search for "nonexistent" — should find 0.
    let none_results = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| ms.graph.search("nonexistent", 10).len())
            .unwrap_or(0)
    });
    assert_eq!(none_results, 0, "FAIL: search('nonexistent') != 0");
    println!("  PASS — search('nonexistent') returned 0 results");
}

// ─── Checkpoint 6 ─────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_6_replay() {
    println!("\n=== MEM CHECKPOINT 6: replay produces identical memory state ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let m1 = make_memory(MemoryKind::Episodic, "Event A", "first", &[]);
    let m2 = make_memory(MemoryKind::Semantic, "Fact B", "second", &[]);
    dispatch_memory_created(&kernel, &m1);
    dispatch_memory_created(&kernel, &m2);
    kernel.dispatch(RawEvent::new(
        "memory.accessed",
        json!({"id": m1.id.0.to_string(), "at": 100}),
        Actor::owner(), 0,
    )).unwrap();
    kernel.dispatch(RawEvent::new(
        "memory.decayed",
        json!({"factor": 0.7}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Dispatched 4 events (2 created + 1 accessed + 1 decayed)");

    // Capture live state.
    let live = kernel.query(|s| s.clone());
    let live_count = live.event_count();
    let live_hash = live.last_hash().clone();
    println!("  Live: {} events, hash={}", live_count, &live_hash.to_string()[..16]);

    // Verify chain.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken");
    println!("  PASS — hash chain verified ({} events)", report.events_verified);

    // Replay from genesis.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.event_count(), live_count, "FAIL: event_count mismatch");
    assert_eq!(replayed.last_hash(), live_hash, "FAIL: last_hash mismatch");
    println!("  PASS — replayed event_count + last_hash match live");

    // Compare memory slice.
    let live_mem = sps_memory::reducer::MemoryState::from_state(&live).unwrap();
    let replayed_mem = sps_memory::reducer::MemoryState::from_state(&replayed).unwrap();
    assert_eq!(live_mem.graph.count(), replayed_mem.graph.count(),
        "FAIL: memory count mismatch (live={}, replayed={})",
        live_mem.graph.count(), replayed_mem.graph.count());

    // Verify access_count + strength match per memory.
    for live_m in live_mem.graph.memories.values() {
        let replayed_m = replayed_mem.graph.get(&live_m.id).unwrap();
        assert_eq!(live_m.access_count, replayed_m.access_count,
            "FAIL: access_count mismatch for '{}'", live_m.title);
        assert!((live_m.strength.0 - replayed_m.strength.0).abs() < 0.001,
            "FAIL: strength mismatch for '{}' (live={:.3}, replayed={:.3})",
            live_m.title, live_m.strength.0, replayed_m.strength.0);
    }
    println!("  PASS — {} memories with matching access_count + strength", live_mem.graph.count());

    println!("  PASS — deterministic replay confirmed");
}

// ─── Checkpoint 7 ─────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_7_sqlite() {
    println!("\n=== MEM CHECKPOINT 7: SQLite backend works end-to-end ===");
    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open_in_memory()
            .expect("failed to open SQLite")
    );
    let kernel = boot_kernel(storage.clone());
    assert_eq!(kernel.backend_name(), "sqlite");

    let m1 = make_memory(MemoryKind::Semantic, "SQLite memory", "persisted via sqlite", &["sqlite"]);
    let _event = dispatch_memory_created(&kernel, &m1);
    println!("  Created memory via SQLite backend");

    let count = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| ms.graph.count()).unwrap_or(0)
    });
    if count == 1 {
        println!("  PASS — 1 memory in SQLite-backed state");
    } else {
        println!("  FAIL — expected 1, got {}", count);
        panic!("MEM CHECKPOINT 7 FAILED");
    }

    // Verify hash chain.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());
    assert_eq!(report.events_verified, 1);
    println!("  PASS — SQLite hash chain verified");

    // Drop + re-boot — verify state reconstructed.
    drop(kernel);
    let kernel2 = boot_kernel(storage.clone());
    let count_after = kernel2.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| ms.graph.count()).unwrap_or(0)
    });
    if count_after == 1 {
        let title = kernel2.query(|s| {
            sps_memory::reducer::MemoryState::from_state(s)
                .and_then(|ms| ms.graph.memories.values().next().map(|m| m.title.as_str().to_string()))
                .unwrap_or_default()
        });
        println!("  PASS — after restart, memory '{}' still present", title);
    } else {
        println!("  FAIL — after restart, expected 1, got {}", count_after);
        panic!("MEM CHECKPOINT 7 FAILED");
    }
}

// ─── Checkpoint 8 ─────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_8_crash_recovery() {
    println!("\n=== MEM CHECKPOINT 8: crash recovery ===");
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("sps_mem_crash_{}.db", uuid::Uuid::now_v7()));

    // Phase 1: write memories, then "crash" (drop kernel without snapshot).
    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel = boot_kernel(storage.clone());

        for i in 0..10 {
            let m = make_memory(
                MemoryKind::Episodic,
                &format!("Crash test memory {}", i),
                &format!("content {}", i),
                &[],
            );
            dispatch_memory_created(&kernel, &m);
        }
        // Access a few.
        for _ in 0..3 {
            let first_id = kernel.query(|s| {
                sps_memory::reducer::MemoryState::from_state(s)
                    .and_then(|ms| ms.graph.memories.values().next().map(|m| m.id))
                    .unwrap()
            });
            kernel.dispatch(RawEvent::new(
                "memory.accessed",
                json!({"id": first_id.0.to_string(), "at": 100}),
                Actor::owner(), 0,
            )).unwrap();
        }
        let count_before = kernel.query(|s| {
            sps_memory::reducer::MemoryState::from_state(s)
                .map(|ms| ms.graph.count()).unwrap_or(0)
        });
        println!("  Phase 1: created {} memories, accessed 1 three times", count_before);
        println!("  CRASH — dropping kernel without snapshot");
    }

    // Phase 2: re-boot, verify state.
    {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_sqlite::SqliteStorage::open(&db_path).unwrap()
        );
        let kernel2 = boot_kernel(storage.clone());

        let count_after = kernel2.query(|s| {
            sps_memory::reducer::MemoryState::from_state(s)
                .map(|ms| ms.graph.count()).unwrap_or(0)
        });
        if count_after == 10 {
            println!("  Phase 2: PASS — reconstructed {} memories", count_after);
        } else {
            println!("  FAIL — expected 10 memories, got {}", count_after);
            panic!("MEM CHECKPOINT 8 FAILED");
        }

        // Verify the accessed memory has access_count = 3.
        let accessed_count = kernel2.query(|s| {
            let ms = sps_memory::reducer::MemoryState::from_state(s).unwrap();
            ms.graph.memories.values()
                .map(|m| m.access_count)
                .max()
                .unwrap_or(0)
        });
        if accessed_count == 3 {
            println!("  Phase 2: PASS — access_count=3 preserved after restart");
        } else {
            println!("  FAIL — expected max access_count=3, got {}", accessed_count);
            panic!("MEM CHECKPOINT 8 FAILED");
        }
    }

    drop(db_path);
    std::fs::remove_file(format!("/tmp/sps_mem_crash_{}.db",
        // Extract UUID from path (best-effort cleanup)
        "")).ok();
}

// ─── Checkpoint 9 ─────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_9_large_corpus() {
    println!("\n=== MEM CHECKPOINT 9: large memory corpus (500 memories) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    const N: usize = 500;
    let start = std::time::Instant::now();
    for i in 0..N {
        let kind = match i % 4 {
            0 => MemoryKind::Episodic,
            1 => MemoryKind::Semantic,
            2 => MemoryKind::Procedural,
            _ => MemoryKind::Conceptual,
        };
        let m = make_memory(kind, &format!("Memory {}", i), &format!("content {}", i), &[]);
        dispatch_memory_created(&kernel, &m);
    }
    let dispatch_ms = start.elapsed().as_millis();

    let count = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| ms.graph.count()).unwrap_or(0)
    });
    if count == N {
        println!("  PASS — {} memories created in {}ms ({:.0} mem/sec)",
            N, dispatch_ms, N as f64 / (dispatch_ms as f64 / 1000.0));
    } else {
        println!("  FAIL — expected {} memories, got {}", N, count);
        panic!("MEM CHECKPOINT 9 FAILED");
    }

    // Verify per-kind counts.
    let (ep, sm, pr, co) = kernel.query(|s| {
        let ms = sps_memory::reducer::MemoryState::from_state(s).unwrap();
        (
            ms.graph.by_kind(MemoryKind::Episodic).len(),
            ms.graph.by_kind(MemoryKind::Semantic).len(),
            ms.graph.by_kind(MemoryKind::Procedural).len(),
            ms.graph.by_kind(MemoryKind::Conceptual).len(),
        )
    });
    let expected = N / 4;
    if ep == expected && sm == expected && pr == expected && co == expected {
        println!("  PASS — kind distribution: E={}, S={}, P={}, C={}", ep, sm, pr, co);
    } else {
        println!("  FAIL — kind distribution: E={}, S={}, P={}, C={} (expected {} each)",
            ep, sm, pr, co, expected);
        panic!("MEM CHECKPOINT 9 FAILED");
    }

    // Replay at scale.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replay_start = std::time::Instant::now();
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
    let replay_ms = replay_start.elapsed().as_millis();

    let replayed_count = sps_memory::reducer::MemoryState::from_state(&replayed)
        .map(|ms| ms.graph.count()).unwrap_or(0);
    if replayed_count == N {
        println!("  PASS — replayed {} memories in {}ms ({:.0} mem/sec)",
            N, replay_ms, N as f64 / (replay_ms as f64 / 1000.0));
    } else {
        println!("  FAIL — replayed {} memories (expected {})", replayed_count, N);
        panic!("MEM CHECKPOINT 9 FAILED");
    }
}

// ─── Checkpoint 10 ────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_10_all_four_kinds() {
    println!("\n=== MEM CHECKPOINT 10: all 4 memory kinds work ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let memories = vec![
        make_memory(MemoryKind::Episodic, "Yesterday's debugging session", "fixed bug in parser", &[]),
        make_memory(MemoryKind::Semantic, "Rust ownership model", "borrow checker enforces ownership", &[]),
        make_memory(MemoryKind::Procedural, "How to deploy to production", "1. build, 2. test, 3. ship", &[]),
        make_memory(MemoryKind::Conceptual, "Event Sourcing pattern", "events are source of truth", &[]),
    ];
    for m in &memories {
        dispatch_memory_created(&kernel, m);
    }
    println!("  Created 1 of each kind");

    let counts = kernel.query(|s| {
        let ms = sps_memory::reducer::MemoryState::from_state(s).unwrap();
        (
            ms.graph.by_kind(MemoryKind::Episodic).len(),
            ms.graph.by_kind(MemoryKind::Semantic).len(),
            ms.graph.by_kind(MemoryKind::Procedural).len(),
            ms.graph.by_kind(MemoryKind::Conceptual).len(),
        )
    });
    if counts == (1, 1, 1, 1) {
        println!("  PASS — kind counts: E={}, S={}, P={}, C={}", counts.0, counts.1, counts.2, counts.3);
    } else {
        println!("  FAIL — kind counts: {:?} (expected (1,1,1,1))", counts);
        panic!("MEM CHECKPOINT 10 FAILED");
    }

    // Promote each to a different kind.
    for (m, target) in &[
        (memories[0].id, MemoryKind::Semantic),
        (memories[1].id, MemoryKind::Conceptual),
        (memories[2].id, MemoryKind::Semantic),
        (memories[3].id, MemoryKind::Procedural),
    ] {
        kernel.dispatch(RawEvent::new(
            "memory.promoted",
            json!({"id": m.0.to_string(), "new_kind": format!("{:?}", target).to_lowercase()}),
            Actor::owner(), 0,
        )).unwrap();
    }
    println!("  Promoted all 4 memories to different kinds");

    let after = kernel.query(|s| {
        let ms = sps_memory::reducer::MemoryState::from_state(s).unwrap();
        (
            ms.graph.by_kind(MemoryKind::Episodic).len(),
            ms.graph.by_kind(MemoryKind::Semantic).len(),
            ms.graph.by_kind(MemoryKind::Procedural).len(),
            ms.graph.by_kind(MemoryKind::Conceptual).len(),
        )
    });
    // E: 0, S: 2 (orig + promoted from E), P: 1 (promoted from C), C: 1 (promoted from S)
    if after == (0, 2, 1, 1) {
        println!("  PASS — after promotions: E={}, S={}, P={}, C={}", after.0, after.1, after.2, after.3);
    } else {
        println!("  FAIL — after promotions: {:?} (expected (0, 2, 1, 1))", after);
        panic!("MEM CHECKPOINT 10 FAILED");
    }
}

// ─── Checkpoint 11 ────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_11_removed_and_consolidated() {
    println!("\n=== MEM CHECKPOINT 11: memory.removed + memory.consolidated ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let m1 = make_memory(MemoryKind::Episodic, "A", "content A", &[]);
    let m2 = make_memory(MemoryKind::Episodic, "B", "content B", &[]);
    let m3 = make_memory(MemoryKind::Episodic, "C", "content C", &[]);
    dispatch_memory_created(&kernel, &m1);
    dispatch_memory_created(&kernel, &m2);
    dispatch_memory_created(&kernel, &m3);
    println!("  Created 3 memories (A, B, C)");

    // Remove A.
    kernel.dispatch(RawEvent::new(
        "memory.removed",
        json!({"id": m1.id.0.to_string()}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Removed memory A");

    let count_after_remove = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| ms.graph.count()).unwrap_or(0)
    });
    if count_after_remove == 2 {
        println!("  PASS — 2 memories remain after removal");
    } else {
        println!("  FAIL — expected 2 after removal, got {}", count_after_remove);
        panic!("MEM CHECKPOINT 11 FAILED");
    }

    // Consolidate B into C (B is the loser, gets removed).
    kernel.dispatch(RawEvent::new(
        "memory.consolidated",
        json!({"loser_id": m2.id.0.to_string()}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Consolidated (B removed, C survives)");

    let count_after_consolidate = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| ms.graph.count()).unwrap_or(0)
    });
    if count_after_consolidate == 1 {
        println!("  PASS — 1 memory remains after consolidation");
    } else {
        println!("  FAIL — expected 1 after consolidation, got {}", count_after_consolidate);
        panic!("MEM CHECKPOINT 11 FAILED");
    }

    // Verify the survivor is C.
    let survivor_title = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .and_then(|ms| ms.graph.memories.values().next().map(|m| m.title.as_str().to_string()))
            .unwrap_or_default()
    });
    if survivor_title == "C" {
        println!("  PASS — survivor is '{}'", survivor_title);
    } else {
        println!("  FAIL — survivor is '{}', expected 'C'", survivor_title);
        panic!("MEM CHECKPOINT 11 FAILED");
    }
}

// ─── Checkpoint 12 ────────────────────────────────────────────────────────

#[test]
fn mem_checkpoint_12_linked_unlinked() {
    println!("\n=== MEM CHECKPOINT 12: memory.linked + memory.unlinked ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let m1 = make_memory(MemoryKind::Semantic, "Rust", "systems language", &[]);
    let m2 = make_memory(MemoryKind::Semantic, "Tokio", "async runtime", &[]);
    let m3 = make_memory(MemoryKind::Semantic, "Cargo", "package manager", &[]);
    dispatch_memory_created(&kernel, &m1);
    dispatch_memory_created(&kernel, &m2);
    dispatch_memory_created(&kernel, &m3);
    println!("  Created 3 memories");

    // Link m1 → m2 (Rust related to Tokio).
    // Note: MemoryLinkKind only has: caused, generalizes, related, part_of, promoted_from.
    // Note: MemoryLink.id is REQUIRED (deterministic) — comes from event payload
    // so that replay produces the same id and unlink can find the link.
    let link_id = uuid::Uuid::now_v7();
    kernel.dispatch(RawEvent::new(
        "memory.linked",
        json!({
            "id": link_id.to_string(),
            "from": m1.id.0.to_string(),
            "to": m2.id.0.to_string(),
            "kind": "related",
        }),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Linked Rust → Tokio (related) [link_id={}]", &link_id.to_string()[..8]);

    let link_count = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| ms.graph.links.len()).unwrap_or(0)
    });
    if link_count == 1 {
        println!("  PASS — 1 link in graph");
    } else {
        println!("  FAIL — expected 1 link, got {}", link_count);
        panic!("MEM CHECKPOINT 12 FAILED");
    }

    // Unlink using the same id we used to create.
    kernel.dispatch(RawEvent::new(
        "memory.unlinked",
        json!({"link_id": link_id.to_string()}),
        Actor::owner(), 0,
    )).unwrap();
    println!("  Unlinked");

    let link_count_after = kernel.query(|s| {
        sps_memory::reducer::MemoryState::from_state(s)
            .map(|ms| ms.graph.links.len()).unwrap_or(0)
    });
    if link_count_after == 0 {
        println!("  PASS — 0 links after unlink");
    } else {
        println!("  FAIL — expected 0 links after unlink, got {}", link_count_after);
        panic!("MEM CHECKPOINT 12 FAILED");
    }

    // Replay — verify link state.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_memory::reducer::MemoryReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();
    let replayed_links = sps_memory::reducer::MemoryState::from_state(&replayed)
        .map(|ms| ms.graph.links.len()).unwrap_or(0);
    if replayed_links == 0 {
        println!("  PASS — replayed state has 0 links (matches live)");
    } else {
        println!("  FAIL — replayed state has {} links (expected 0)", replayed_links);
        panic!("MEM CHECKPOINT 12 FAILED");
    }

    println!("  PASS — link → unlink → replay all consistent");
}

// Suppress unused warning.
#[allow(dead_code)]
fn _suppress(_s: MemoryStrength) {}
