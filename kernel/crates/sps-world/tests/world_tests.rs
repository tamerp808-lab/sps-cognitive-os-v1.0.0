//! Phase 4 — World Model tests.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_world::entities::{
    AgentDescriptor, EntityId, ExternalSystem, FileNode, Project, ToolDescriptor,
};
use sps_world::graph::{WorldGraph, WorldLinkKind, WorldRelationship};
use sps_world::reducer::{WorldReducer, WorldState};

fn fresh_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    WorldReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
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

#[test]
fn world_graph_add_and_query_projects() {
    let mut g = WorldGraph::new();
    let p = make_project("test-project");
    let pid = p.id;
    g.add_project(p);
    assert_eq!(g.projects.len(), 1);
    assert!(g.projects.contains_key(&pid.0));
}

#[test]
fn world_graph_files_in_project() {
    let mut g = WorldGraph::new();
    let p = make_project("test");
    let pid = p.id;
    g.add_project(p);

    let f1 = FileNode {
        id: EntityId::new(),
        project_id: pid,
        path: SmolStr::new("src/main.rs"),
        content_hash: None,
        size: 100,
        origin_tick: 0,
    };
    let f2 = FileNode {
        id: EntityId::new(),
        project_id: pid,
        path: SmolStr::new("src/lib.rs"),
        content_hash: None,
        size: 200,
        origin_tick: 0,
    };
    g.add_file(f1);
    g.add_file(f2);

    let files = g.files_in_project(&pid);
    assert_eq!(files.len(), 2);
}

#[test]
fn world_graph_relationships() {
    let mut g = WorldGraph::new();
    let p = make_project("proj");
    let pid = p.id;
    g.add_project(p);

    let agent = AgentDescriptor {
        id: EntityId::new(),
        archetype: SmolStr::new("developer"),
        name: SmolStr::new("Dev Agent"),
        origin_tick: 0,
    };
    let aid = agent.id;
    g.add_agent(agent);

    let rel_id = g.add_relationship(WorldRelationship {
        from: pid,
        to: aid,
        kind: WorldLinkKind::Uses,
    });
    assert_eq!(g.relationships.len(), 1);
    assert!(g.relationships.contains_key(&rel_id));
}

#[test]
fn world_graph_remove_entity_cleans_relationships() {
    let mut g = WorldGraph::new();
    let p = make_project("p");
    let pid = p.id;
    g.add_project(p);
    let agent = AgentDescriptor {
        id: EntityId::new(),
        archetype: SmolStr::new("architect"),
        name: SmolStr::new("Arch"),
        origin_tick: 0,
    };
    let aid = agent.id;
    g.add_agent(agent);
    g.add_relationship(WorldRelationship {
        from: pid,
        to: aid,
        kind: WorldLinkKind::Uses,
    });
    assert_eq!(g.relationships.len(), 1);

    g.remove_entity(&pid);
    assert_eq!(g.relationships.len(), 0);
    assert_eq!(g.projects.len(), 0);
}

#[test]
fn world_graph_external_systems_and_tools() {
    let mut g = WorldGraph::new();
    g.add_tool(ToolDescriptor {
        id: EntityId::new(),
        name: SmolStr::new("cargo"),
        version: Some(SmolStr::new("1.96")),
        origin_tick: 0,
    });
    g.add_external_system(ExternalSystem {
        id: EntityId::new(),
        name: SmolStr::new("github"),
        kind: SmolStr::new("git_remote"),
        endpoint: "https://github.com".into(),
        origin_tick: 0,
    });
    assert_eq!(g.tools.len(), 1);
    assert_eq!(g.external_systems.len(), 1);
    assert_eq!(g.entity_count(), 2);
}

#[test]
fn world_project_added_event_updates_state() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let p = make_project("via-event");
    let event = RawEvent::new(
        "world.project_added",
        serde_json::to_value(&p).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let world = WorldState::from_state(&state).unwrap();
    assert_eq!(world.graph.projects.len(), 1);
}

#[test]
fn world_file_added_and_updated_events() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();

    let p = make_project("p");
    let pid = p.id;
    let e1 = RawEvent::new(
        "world.project_added",
        serde_json::to_value(&p).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();

    let mut f = FileNode {
        id: EntityId::new(),
        project_id: pid,
        path: SmolStr::new("src/main.rs"),
        content_hash: None,
        size: 0,
        origin_tick: 0,
    };
    let e2 = RawEvent::new(
        "world.file_added",
        serde_json::to_value(&f).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();

    let world = WorldState::from_state(&state).unwrap();
    assert_eq!(world.graph.files.len(), 1);

    // Update the file.
    f.size = 500;
    f.content_hash = Some("abc123".into());
    let e3 = RawEvent::new(
        "world.file_updated",
        serde_json::to_value(&f).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(3, e2.hash);
    pipeline.apply(&mut state, &e3).unwrap();

    let world = WorldState::from_state(&state).unwrap();
    let file = world.graph.files.values().next().unwrap();
    assert_eq!(file.size, 500);
    assert_eq!(file.content_hash, Some("abc123".to_string()));
}

#[test]
fn world_entity_removed_event_cleans_state() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();

    let p = make_project("doomed");
    let pid = p.id;
    let e1 = RawEvent::new(
        "world.project_added",
        serde_json::to_value(&p).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();
    assert_eq!(
        WorldState::from_state(&state).unwrap().graph.projects.len(),
        1
    );

    let e2 = RawEvent::new(
        "world.entity_removed",
        json!({"id": pid.to_string()}),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();
    assert_eq!(
        WorldState::from_state(&state).unwrap().graph.projects.len(),
        0
    );
}

#[test]
fn world_relationship_added_event() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let from = EntityId::new();
    let to = EntityId::new();
    let rel = WorldRelationship {
        from,
        to,
        kind: WorldLinkKind::DependsOn,
    };
    let event = RawEvent::new(
        "world.relationship_added",
        serde_json::to_value(&rel).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let world = WorldState::from_state(&state).unwrap();
    assert_eq!(world.graph.relationships.len(), 1);
}

#[test]
fn world_state_round_trips() {
    let mut state = CanonicalState::genesis();
    let mut ws = WorldState::default();
    ws.graph.add_project(make_project("round-trip"));
    ws.save_to(&mut state).unwrap();
    let loaded = WorldState::from_state(&state).unwrap();
    assert_eq!(loaded.graph.projects.len(), 1);
}
