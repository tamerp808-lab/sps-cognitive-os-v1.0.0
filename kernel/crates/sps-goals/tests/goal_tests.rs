//! Phase 6 — Goal System tests.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_goals::hierarchy::{
    Goal, GoalId, GoalStatus, GoalTree, Milestone, Objective, Task, TaskStatus,
};
use sps_goals::reducer::{GoalReducer, GoalState};

fn fresh_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    GoalReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

fn make_goal(title: &str, n_tasks: usize) -> Goal {
    let mut tasks = Vec::new();
    for i in 0..n_tasks {
        tasks.push(Task {
            id: uuid::Uuid::now_v7(),
            title: SmolStr::new(format!("task-{}", i)),
            description: String::new(),
            status: TaskStatus::Pending,
            assigned_agent: None,
            origin_tick: 0,
        });
    }
    Goal {
        id: GoalId::new(),
        title: SmolStr::new(title),
        description: "test goal".into(),
        priority: 5,
        status: GoalStatus::Pending,
        objectives: vec![Objective {
            id: uuid::Uuid::now_v7(),
            title: SmolStr::new("obj1"),
            milestones: vec![Milestone {
                id: uuid::Uuid::now_v7(),
                title: SmolStr::new("mil1"),
                tasks,
            }],
        }],
        dependencies: vec![],
        created_at: 0,
        origin_tick: 0,
    }
}

#[test]
fn goal_tree_add_get_remove() {
    let mut t = GoalTree::new();
    let g = make_goal("g1", 2);
    let id = g.id;
    t.add_goal(g);
    assert!(t.get(&id).is_some());
    assert!(t.remove(&id).is_some());
    assert!(t.get(&id).is_none());
}

#[test]
fn goal_tree_total_tasks() {
    let mut t = GoalTree::new();
    t.add_goal(make_goal("g1", 3));
    t.add_goal(make_goal("g2", 2));
    assert_eq!(t.total_tasks(), 5);
}

#[test]
fn goal_tree_verify_complete() {
    let mut t = GoalTree::new();
    let g = make_goal("g1", 2);
    let id = g.id;
    t.add_goal(g);
    // Mark both tasks completed.
    let goal = t.get_mut(&id).unwrap();
    for o in &mut goal.objectives {
        for m in &mut o.milestones {
            for task in &mut m.tasks {
                task.status = TaskStatus::Completed;
            }
        }
    }
    let v = t.verify(&id);
    assert!(v.verified);
    assert_eq!(v.tasks_total, 2);
    assert_eq!(v.tasks_completed, 2);
}

#[test]
fn goal_tree_verify_incomplete() {
    let mut t = GoalTree::new();
    let g = make_goal("g1", 3);
    let id = g.id;
    t.add_goal(g);
    let v = t.verify(&id);
    assert!(!v.verified);
    assert_eq!(v.tasks_completed, 0);
}

#[test]
fn goal_tree_by_priority() {
    let mut t = GoalTree::new();
    let mut g1 = make_goal("low", 1);
    g1.priority = 1;
    let mut g2 = make_goal("high", 1);
    g2.priority = 10;
    t.add_goal(g1);
    t.add_goal(g2);
    let sorted = t.by_priority();
    assert_eq!(sorted[0].title, "high");
    assert_eq!(sorted[1].title, "low");
}

#[test]
fn goal_created_event_updates_state() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let g = make_goal("event-goal", 2);
    let event = RawEvent::new(
        "goal.created",
        serde_json::to_value(&g).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let gs = GoalState::from_state(&state).unwrap();
    assert_eq!(gs.tree.goals.len(), 1);
}

#[test]
fn goal_status_changed_event() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let g = make_goal("g", 1);
    let id = g.id;
    let e1 = RawEvent::new(
        "goal.created",
        serde_json::to_value(&g).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();

    let e2 = RawEvent::new(
        "goal.status_changed",
        json!({"goal_id": id, "status": "active"}),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();

    let gs = GoalState::from_state(&state).unwrap();
    assert_eq!(gs.tree.get(&id).unwrap().status, GoalStatus::Active);
}

#[test]
fn task_status_changed_event() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let g = make_goal("g", 1);
    let id = g.id;
    let task_id = g.objectives[0].milestones[0].tasks[0].id;
    let e1 = RawEvent::new(
        "goal.created",
        serde_json::to_value(&g).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();

    let e2 = RawEvent::new(
        "task.status_changed",
        json!({"task_id": task_id.to_string(), "status": "completed"}),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();

    let gs = GoalState::from_state(&state).unwrap();
    let goal = gs.tree.get(&id).unwrap();
    let task = &goal.objectives[0].milestones[0].tasks[0];
    assert_eq!(task.status, TaskStatus::Completed);
}

#[test]
fn goal_state_round_trips() {
    let mut state = CanonicalState::genesis();
    let mut gs = GoalState::default();
    gs.tree.add_goal(make_goal("rt", 1));
    gs.save_to(&mut state).unwrap();
    let loaded = GoalState::from_state(&state).unwrap();
    assert_eq!(loaded.tree.goals.len(), 1);
}
