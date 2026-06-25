//! Phase 3 — Memory subsystem tests.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::reducer::{Reducer, ReducerPipeline, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::event::EventHash;
use sps_memory::graph::{MemoryGraph, MemoryLink, MemoryLinkKind};
use sps_memory::reducer::{MemoryReducer, MemoryState};
use sps_memory::stats::MemoryStats;
use sps_memory::memory::{Memory, MemoryId, MemoryKind, MemoryRecord, MemoryStrength};

fn fresh_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    MemoryReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

fn make_memory(kind: MemoryKind, title: &str) -> Memory {
    Memory {
        id: MemoryId::new(),
        kind,
        title: SmolStr::new(title),
        content: json!({}),
        strength: MemoryStrength::default(),
        tags: vec![],
        created_at: 0,
        last_accessed_at: 0,
        access_count: 0,
        origin_tick: 0,
    }
}

#[test]
fn memory_graph_add_get_remove() {
    let mut g = MemoryGraph::new();
    let m = make_memory(MemoryKind::Semantic, "rust is fast");
    let id = m.id;
    g.add_memory(m);
    assert_eq!(g.count(), 1);
    assert!(g.get(&id).is_some());

    let removed = g.remove_memory(&id);
    assert!(removed.is_some());
    assert_eq!(g.count(), 0);
}

#[test]
fn memory_graph_search_by_title() {
    let mut g = MemoryGraph::new();
    g.add_memory(make_memory(MemoryKind::Semantic, "Rust is fast"));
    g.add_memory(make_memory(MemoryKind::Semantic, "Python is dynamic"));
    g.add_memory(make_memory(MemoryKind::Semantic, "rust has ownership"));

    let results = g.search("rust", 10);
    assert_eq!(results.len(), 2);
}

#[test]
fn memory_graph_search_by_content() {
    let mut g = MemoryGraph::new();
    let mut m = make_memory(MemoryKind::Semantic, "fact");
    m.content = json!({"detail": "the earth orbits the sun"});
    g.add_memory(m);

    let results = g.search("earth", 10);
    assert_eq!(results.len(), 1);
}

#[test]
fn memory_graph_search_by_tag() {
    let mut g = MemoryGraph::new();
    let mut m = make_memory(MemoryKind::Procedural, "workflow");
    m.tags = vec![SmolStr::new("automation"), SmolStr::new("rust")];
    g.add_memory(m);

    let results = g.search("automation", 10);
    assert_eq!(results.len(), 1);
}

#[test]
fn memory_graph_links_round_trip() {
    let mut g = MemoryGraph::new();
    let m1 = make_memory(MemoryKind::Episodic, "ran project gen");
    let m2 = make_memory(MemoryKind::Semantic, "project gen uses cargo");
    let id1 = m1.id;
    let id2 = m2.id;
    g.add_memory(m1);
    g.add_memory(m2);

    let link = MemoryLink {
        from: id1,
        to: id2,
        kind: MemoryLinkKind::Caused,
        weight: Some(0.8),
    };
    let link_id = g.add_link(link);

    assert_eq!(g.links_from(&id1).len(), 1);
    assert_eq!(g.links_to(&id2).len(), 1);

    g.remove_link(link_id);
    assert_eq!(g.links_from(&id1).len(), 0);
}

#[test]
fn memory_graph_decay_removes_weak_memories() {
    let mut g = MemoryGraph::new();
    let mut m = make_memory(MemoryKind::Episodic, "ephemeral");
    m.strength = MemoryStrength::new(0.05);
    let _id = m.id;
    g.add_memory(m);

    // Decay by 0.1 — 0.05 * 0.1 = 0.005, below death threshold (0.01).
    let dead = g.apply_decay(0.1, None);
    assert_eq!(dead.len(), 1);
    assert_eq!(g.count(), 0);
}

#[test]
fn memory_graph_promote_changes_kind() {
    let mut g = MemoryGraph::new();
    let m = make_memory(MemoryKind::Episodic, "event");
    let id = m.id;
    g.add_memory(m);

    let link = g.promote(&id, MemoryKind::Semantic);
    assert!(link.is_some());
    assert_eq!(g.get(&id).unwrap().kind, MemoryKind::Semantic);
}

#[test]
fn memory_graph_boost_on_access() {
    let mut g = MemoryGraph::new();
    let mut m = make_memory(MemoryKind::Semantic, "fact");
    m.strength = MemoryStrength::new(0.5);
    let id = m.id;
    g.add_memory(m);

    g.boost(&id, 0.3);
    let after = g.get(&id).unwrap().strength.0;
    assert!((after - 0.8).abs() < 0.001);
}

// --- Reducer tests ---

#[test]
fn memory_created_event_adds_memory_to_state() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();

    let record = MemoryRecord {
        id: MemoryId::new(),
        kind: MemoryKind::Semantic,
        title: SmolStr::new("test fact"),
        content: json!({"detail": "value"}),
        tags: vec![SmolStr::new("test")],
        origin_tick: 0,
        created_at: 1234,
    };
    let payload = serde_json::to_value(&record).unwrap();
    let event = RawEvent::new("memory.created", payload, Actor::owner(), 0)
        .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();

    let mem_state = MemoryState::from_state(&state).unwrap();
    assert_eq!(mem_state.graph.count(), 1);
}

#[test]
fn memory_accessed_event_boosts_strength() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();

    // Create
    let id = MemoryId::new();
    let record = MemoryRecord {
        id,
        kind: MemoryKind::Semantic,
        title: SmolStr::new("test"),
        content: json!({}),
        tags: vec![],
        origin_tick: 0,
        created_at: 0,
    };
    let event1 = RawEvent::new(
        "memory.created",
        serde_json::to_value(&record).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event1).unwrap();

    // Decay the memory so its strength is below 1.0 — then boost should
    // make it stronger than the decayed value.
    let decay_event = RawEvent::new(
        "memory.decayed",
        json!({"factor": 0.5, "kind": "semantic"}),
        Actor::owner(),
        0,
    )
    .finalize(2, event1.hash);
    pipeline.apply(&mut state, &decay_event).unwrap();

    let before = MemoryState::from_state(&state)
        .unwrap()
        .graph
        .get(&id)
        .unwrap()
        .strength
        .0;
    assert!(before < 1.0, "expected decayed strength < 1.0, got {}", before);

    // Access — should boost strength by 0.05.
    let event2 = RawEvent::new(
        "memory.accessed",
        json!({"id": id, "at": 9999u64}),
        Actor::owner(),
        0,
    )
    .finalize(3, decay_event.hash);
    pipeline.apply(&mut state, &event2).unwrap();

    let after = MemoryState::from_state(&state)
        .unwrap()
        .graph
        .get(&id)
        .unwrap()
        .strength
        .0;
    assert!(after > before, "expected after ({}) > before ({})", after, before);
    let mem_state = MemoryState::from_state(&state).unwrap();
    let mem = mem_state.graph.get(&id).unwrap();
    assert_eq!(mem.access_count, 1);
    assert_eq!(mem.last_accessed_at, 9999);
}

#[test]
fn memory_promoted_event_changes_kind() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();

    let id = MemoryId::new();
    let record = MemoryRecord {
        id,
        kind: MemoryKind::Episodic,
        title: SmolStr::new("event"),
        content: json!({}),
        tags: vec![],
        origin_tick: 0,
        created_at: 0,
    };
    let e1 = RawEvent::new(
        "memory.created",
        serde_json::to_value(&record).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();

    let e2 = RawEvent::new(
        "memory.promoted",
        json!({"id": id, "new_kind": "semantic"}),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();

    let mem_state = MemoryState::from_state(&state).unwrap();
    let mem = mem_state.graph.get(&id).unwrap();
    assert_eq!(mem.kind, MemoryKind::Semantic);
}

#[test]
fn memory_linked_event_adds_link() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();

    let id1 = MemoryId::new();
    let id2 = MemoryId::new();
    let link = MemoryLink {
        from: id1,
        to: id2,
        kind: MemoryLinkKind::Caused,
        weight: Some(1.0),
    };
    let event = RawEvent::new(
        "memory.linked",
        serde_json::to_value(&link).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();

    let mem_state = MemoryState::from_state(&state).unwrap();
    assert_eq!(mem_state.graph.links.len(), 1);
}

#[test]
fn memory_decayed_event_applies_decay() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();

    let id = MemoryId::new();
    let record = MemoryRecord {
        id,
        kind: MemoryKind::Episodic,
        title: SmolStr::new("ephemeral"),
        content: json!({}),
        tags: vec![],
        origin_tick: 0,
        created_at: 0,
    };
    let e1 = RawEvent::new(
        "memory.created",
        serde_json::to_value(&record).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();

    let before_strength = MemoryState::from_state(&state)
        .unwrap()
        .graph
        .get(&id)
        .unwrap()
        .strength
        .0;

    let e2 = RawEvent::new(
        "memory.decayed",
        json!({"factor": 0.5, "kind": "episodic"}),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();

    let after = MemoryState::from_state(&state)
        .unwrap()
        .graph
        .get(&id)
        .unwrap()
        .strength
        .0;
    assert!(after < before_strength);
}

#[test]
fn memory_removed_event_deletes_memory() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();

    let id = MemoryId::new();
    let record = MemoryRecord {
        id,
        kind: MemoryKind::Semantic,
        title: SmolStr::new("doomed"),
        content: json!({}),
        tags: vec![],
        origin_tick: 0,
        created_at: 0,
    };
    let e1 = RawEvent::new(
        "memory.created",
        serde_json::to_value(&record).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();
    assert_eq!(MemoryState::from_state(&state).unwrap().graph.count(), 1);

    let e2 = RawEvent::new(
        "memory.removed",
        json!({"id": id}),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();
    assert_eq!(MemoryState::from_state(&state).unwrap().graph.count(), 0);
}

#[test]
fn memory_stats_aggregate_correctly() {
    let mut g = MemoryGraph::new();
    g.add_memory(make_memory(MemoryKind::Episodic, "e1"));
    g.add_memory(make_memory(MemoryKind::Episodic, "e2"));
    g.add_memory(make_memory(MemoryKind::Semantic, "s1"));
    g.add_memory(make_memory(MemoryKind::Procedural, "p1"));

    let stats = MemoryStats::from_graph(&g);
    assert_eq!(stats.total, 4);
    assert_eq!(stats.by_kind.get("episodic"), Some(&2));
    assert_eq!(stats.by_kind.get("semantic"), Some(&1));
    assert_eq!(stats.by_kind.get("procedural"), Some(&1));
    assert_eq!(stats.by_kind.get("conceptual"), Some(&0));
}

#[test]
fn memory_state_round_trips_through_canonical_state() {
    let mut state = CanonicalState::genesis();
    let mut mem_state = MemoryState::default();
    mem_state.graph.add_memory(make_memory(MemoryKind::Semantic, "fact"));
    mem_state.save_to(&mut state).unwrap();

    let loaded = MemoryState::from_state(&state).unwrap();
    assert_eq!(loaded.graph.count(), 1);
}
